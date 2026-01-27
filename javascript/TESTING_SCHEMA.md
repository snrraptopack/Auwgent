# Testing Enhanced Type System Schema Generation

## Overview

This guide explains how to validate that the enhanced type system correctly transforms DSL types into JSON schemas for LLM structured output.

## What We're Testing

The type system should:
1. ✅ Load type definitions from generated IR JSON
2. ✅ Resolve type references (e.g., `AnalysisResult`) to full object schemas
3. ✅ Preserve descriptions at all levels (output fields, type properties, nested fields)
4. ✅ Handle nested types (arrays of objects, type references within types)
5. ✅ Generate proper `required` arrays based on optional flags
6. ✅ Produce valid JSON Schema for LLM structured output

## Test Files

### 1. `validate-schema-generation.ts` (Recommended)
Comprehensive validation with pass/fail checks and colored output.

**Run:**
```bash
cd javascript
bun run validate-schema
```

**What it checks:**
- IR structure is correct
- Type references are resolved
- Descriptions are preserved
- Nested arrays and objects work
- Required fields are correct
- Final schema matches expectations

### 2. `test-output-schema.ts`
Detailed inspection of the schema generation process.

**Run:**
```bash
cd javascript
bun run test-schema
```

**What it shows:**
- Raw IR output structure
- Raw IR type definitions
- Generated JSON schema
- Individual field schemas
- Description preservation

## Expected Results

### Input DSL (from `manual-testing/type-system-test.agent`)

```
output type AnalysisResult {
    summary: string @desc "High-level summary of findings"
    confidence: number @desc "Confidence score between 0 and 1"
    keyFindings: string[] @desc "List of key findings"
}

agent TypeSystemTest {
    output {
        analysis: AnalysisResult @desc "The complete analysis"
        searchResults: SearchResult @desc "Related search results"
    }
}
```

### Expected JSON Schema Output

```json
{
  "type": "object",
  "properties": {
    "analysis": {
      "type": "object",
      "description": "The complete analysis",
      "properties": {
        "summary": {
          "type": "string",
          "description": "High-level summary of findings"
        },
        "confidence": {
          "type": "number",
          "description": "Confidence score between 0 and 1"
        },
        "keyFindings": {
          "type": "array",
          "items": { "type": "string" },
          "description": "List of key findings"
        }
      },
      "required": ["summary", "confidence", "keyFindings"]
    },
    "searchResults": {
      "type": "object",
      "description": "Related search results",
      "properties": {
        "query": { "type": "string", "description": "..." },
        "results": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "title": { "type": "string" },
              "url": { "type": "string" },
              "snippet": { "type": "string" }
            },
            "required": ["title", "url", "snippet"]
          },
          "description": "Array of search results"
        },
        "totalCount": { "type": "number", "description": "..." }
      },
      "required": ["query", "results", "totalCount"]
    }
  },
  "required": ["analysis", "searchResults"]
}
```

## Key Features Validated

### 1. Type Reference Resolution
`AnalysisResult` type reference → Full object schema with all properties

### 2. Nested Type References
`User` type contains `Address` type reference → Both fully resolved

### 3. Array Types
- Simple arrays: `string[]` → `{ type: "array", items: { type: "string" } }`
- Complex arrays: `Point[]` → `{ type: "array", items: { type: "object", properties: {...} } }`

### 4. Inline Object Arrays
```
results: {
    title: string
    url: string
}[] @desc "Array of search results"
```
→ Correctly generates array of inline object schema

### 5. Description Preservation
- Top-level: `@desc "The complete analysis"` on output field
- Nested: `@desc "High-level summary"` on type property
- Arrays: `@desc "Array of search results"` on array field

### 6. Optional Fields
- Optional properties excluded from `required` array
- Non-optional properties included in `required` array

## Troubleshooting

### Schema not resolving type references
- Check that `ir.types` contains the type definition
- Verify `typeDefToSchema()` is being called
- Ensure recursive resolution is working

### Descriptions missing
- Check IR JSON has descriptions at the right level
- Verify `extractInOutConfig()` in generator preserves descriptions
- Ensure `convertTypeToSchema()` passes descriptions through

### Arrays not working
- Check IR uses `{ type: "array", items: ... }` format (not `"string[]"`)
- Verify `convertTypeToSchema()` handles array objects
- Ensure nested items are recursively converted

## Next Steps

After validation passes:
1. Test with actual LLM calls (Gemini/OpenAI)
2. Verify LLM returns data matching the schema
3. Test edge cases (deeply nested types, circular references, etc.)
