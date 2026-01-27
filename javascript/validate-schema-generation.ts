/**
 * VALIDATION SCRIPT: Enhanced Type System Schema Generation
 * 
 * This script validates that:
 * 1. The IR is correctly loaded from the generated JSON
 * 2. Type references are properly resolved to full JSON schemas
 * 3. Descriptions are preserved at all levels
 * 4. Nested types (arrays, objects, type refs) are correctly expanded
 * 5. The final schema matches what the LLM expects for structured output
 */

import { Synthesizer } from './loader/Synthesizer';
import type { AgentIR } from './loader/types/ir';
import * as fs from 'fs';

const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const RESET = '\x1b[0m';
const BOLD = '\x1b[1m';

function pass(msg: string) {
    console.log(`${GREEN}✓${RESET} ${msg}`);
}

function fail(msg: string) {
    console.log(`${RED}✗${RESET} ${msg}`);
}

function section(title: string) {
    console.log(`\n${BOLD}${YELLOW}=== ${title} ===${RESET}\n`);
}

// Load the generated IR
const irPath = '../type-system-test.agent.json';
const ir: AgentIR = JSON.parse(fs.readFileSync(irPath, 'utf-8'));

section('STEP 1: Verify IR Structure');

// Check IR has types
if (ir.types && Object.keys(ir.types).length > 0) {
    pass(`IR contains ${Object.keys(ir.types).length} type definitions`);
    console.log(`   Types: ${Object.keys(ir.types).join(', ')}`);
} else {
    fail('IR missing type definitions');
}

// Check output structure
if (ir.output && Object.keys(ir.output).length > 0) {
    pass(`IR output has ${Object.keys(ir.output).length} fields`);
    for (const [key, typeInfo] of Object.entries(ir.output)) {
        console.log(`   - ${key}: ${JSON.stringify(typeInfo.type)} ${typeInfo.description ? `"${typeInfo.description}"` : ''}`);
    }
} else {
    fail('IR output is empty');
}

section('STEP 2: Build Output Schema');

const synthesizer = new Synthesizer(ir);
const outputSchema = (synthesizer as any).buildOutputSchema();

if (outputSchema) {
    pass('Output schema generated successfully');
} else {
    fail('Failed to generate output schema');
    process.exit(1);
}

section('STEP 3: Validate Type Reference Resolution');

// Test 1: AnalysisResult type reference
const analysisField = outputSchema.properties?.analysis;
if (analysisField?.type === 'object' && analysisField.properties) {
    pass('AnalysisResult type reference resolved to object schema');
    
    // Check properties exist
    const expectedProps = ['summary', 'confidence', 'keyFindings'];
    const actualProps = Object.keys(analysisField.properties);
    const hasAllProps = expectedProps.every(p => actualProps.includes(p));
    
    if (hasAllProps) {
        pass(`AnalysisResult has all expected properties: ${expectedProps.join(', ')}`);
    } else {
        fail(`AnalysisResult missing properties. Expected: ${expectedProps.join(', ')}, Got: ${actualProps.join(', ')}`);
    }
    
    // Check keyFindings is an array
    const keyFindings = analysisField.properties.keyFindings;
    if (keyFindings?.type === 'array' && keyFindings.items) {
        pass('keyFindings correctly resolved as array type');
        console.log(`   Items type: ${JSON.stringify(keyFindings.items)}`);
    } else {
        fail('keyFindings not correctly resolved as array');
    }
} else {
    fail('AnalysisResult type reference not resolved');
}

// Test 2: SearchResult type reference with nested inline object array
const searchResultsField = outputSchema.properties?.searchResults;
if (searchResultsField?.type === 'object' && searchResultsField.properties) {
    pass('SearchResult type reference resolved to object schema');
    
    // Check results array with inline object
    const resultsArray = searchResultsField.properties.results;
    if (resultsArray?.type === 'array' && resultsArray.items?.type === 'object') {
        pass('results field correctly resolved as array of inline objects');
        
        const inlineObjProps = resultsArray.items.properties;
        if (inlineObjProps && inlineObjProps.title && inlineObjProps.url && inlineObjProps.snippet) {
            pass('Inline object properties (title, url, snippet) present');
        } else {
            fail('Inline object missing expected properties');
        }
    } else {
        fail('results field not correctly resolved as array of objects');
    }
} else {
    fail('SearchResult type reference not resolved');
}

section('STEP 4: Validate Description Preservation');

// Check top-level descriptions
if (analysisField?.description === 'The complete analysis') {
    pass('Top-level description preserved for analysis field');
} else {
    fail(`analysis description incorrect: "${analysisField?.description}"`);
}

if (searchResultsField?.description === 'Related search results') {
    pass('Top-level description preserved for searchResults field');
} else {
    fail(`searchResults description incorrect: "${searchResultsField?.description}"`);
}

// Check nested property descriptions
const summaryDesc = analysisField?.properties?.summary?.description;
if (summaryDesc === 'High-level summary of findings') {
    pass('Nested property description preserved (summary)');
} else {
    fail(`summary description incorrect: "${summaryDesc}"`);
}

const resultsDesc = searchResultsField?.properties?.results?.description;
if (resultsDesc === 'Array of search results') {
    pass('Array field description preserved (results)');
} else {
    fail(`results description incorrect: "${resultsDesc}"`);
}

section('STEP 5: Validate Required Fields');

const requiredFields = outputSchema.required || [];
if (requiredFields.includes('analysis') && requiredFields.includes('searchResults')) {
    pass(`Required fields correctly set: ${requiredFields.join(', ')}`);
} else {
    fail(`Required fields incorrect: ${requiredFields.join(', ')}`);
}

// Check nested required fields
const analysisRequired = analysisField?.required || [];
if (analysisRequired.length === 3) {
    pass(`AnalysisResult required fields: ${analysisRequired.join(', ')}`);
} else {
    fail(`AnalysisResult required fields incorrect: ${analysisRequired.join(', ')}`);
}

section('STEP 6: Display Final Schema for LLM');

console.log('This is the exact JSON Schema sent to the LLM for structured output:\n');
console.log(JSON.stringify(outputSchema, null, 2));

section('VALIDATION COMPLETE');

console.log('\nThe schema transformation is working correctly if all checks passed ✓');
