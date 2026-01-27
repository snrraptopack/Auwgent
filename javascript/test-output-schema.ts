/**
 * Test script to validate JSON Schema generation for structured output
 * This shows exactly what schema is sent to the LLM
 */

import { Synthesizer } from './loader/Synthesizer';
import type { AgentIR } from './loader/types/ir';
import * as fs from 'fs';

// Load the generated IR from the DSL
const irPath = '../test-autoreg.agent.json';
console.log(`Loading IR from: ${irPath}\n`);

const ir: AgentIR = JSON.parse(fs.readFileSync(irPath, 'utf-8'));

// Create synthesizer
const synthesizer = new Synthesizer(ir);

console.log("=== IR OUTPUT STRUCTURE ===");
console.log(JSON.stringify(ir.output, null, 2));

console.log("\n=== IR TYPES DEFINITIONS ===");
console.log(JSON.stringify(ir.types, null, 2));

// Build the output schema (this is what gets sent to the LLM)
const outputSchema = (synthesizer as any).buildOutputSchema();

console.log("\n=== GENERATED JSON SCHEMA FOR LLM (Structured Output) ===");
console.log(JSON.stringify(outputSchema, null, 2));

console.log("\n=== VALIDATION CHECKS ===");

// Check 1: Type references should be resolved
const analysisSchema = outputSchema?.properties?.analysis;
console.log("\n1. Analysis field schema (should be fully resolved AnalysisResult):");
console.log(JSON.stringify(analysisSchema, null, 2));

// Check 2: Nested type references should be resolved
const searchResultsSchema = outputSchema?.properties?.searchResults;
console.log("\n2. SearchResults field schema (should be fully resolved SearchResult):");
console.log(JSON.stringify(searchResultsSchema, null, 2));

// Check 3: Descriptions should be preserved
console.log("\n3. Description preservation:");
console.log(`   - analysis description: "${analysisSchema?.description}"`);
console.log(`   - searchResults description: "${searchResultsSchema?.description}"`);

// Check 4: Required fields
console.log("\n4. Required fields:");
console.log(`   - ${JSON.stringify(outputSchema?.required)}`);

// Check 5: Nested arrays with inline objects
if (searchResultsSchema?.properties?.results) {
    console.log("\n5. Nested array with inline object (results field):");
    console.log(JSON.stringify(searchResultsSchema.properties.results, null, 2));
}

console.log("\n=== TEST COMPLETE ===");
