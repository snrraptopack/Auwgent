# Enhanced Type System - Implementation Tasks

## 1. Grammar Changes

### 1.1 Add TypeDeclaration to Element
- [ ] Update `Element` rule to include `TypeDeclaration`
- [ ] Verify TypeDeclaration can be parsed at top level

### 1.2 Update TypeDeclaration Rule
- [ ] Add `isOutput` optional flag
- [ ] Ensure properties use `TypeConfigDeclaration`
- [ ] Remove TypeDeclaration from `Statement` rule

### 1.3 Add Descriptions to TypeConfigDeclaration
- [ ] Add optional `@desc` with description STRING
- [ ] Test description parsing

### 1.4 Add Descriptions to PropertyType
- [ ] Add optional `@desc` with description STRING
- [ ] Test inline object property descriptions

### 1.5 Add Type References to Types
- [ ] Add `typeRef=[TypeDeclaration:ID]` alternative
- [ ] Test type reference parsing

### 1.6 Regenerate Langium Artifacts
- [ ] Run Langium generator
- [ ] Verify generated AST types
- [ ] Fix any TypeScript compilation errors

## 2. Generator Changes

### 2.1 Implement Type Collection
- [ ] Create `collectTypes()` function
- [ ] Iterate through model elements
- [ ] Build Map of type name to TypeDeclaration
- [ ] Detect and error on duplicate type names

### 2.2 Update extractType Function
- [ ] Add `typeMap` parameter
- [ ] Handle `typeRef` case
- [ ] Preserve descriptions in inline objects
- [ ] Handle nested type references
- [ ] Add error for undefined type references

### 2.3 Create extractTypeDefinitions Function
- [ ] Extract all type definitions from typeMap
- [ ] Preserve `isOutput` flag
- [ ] Extract properties with descriptions
- [ ] Handle nested type references

### 2.4 Create extractPropertiesWithDesc Function
- [ ] Extract property name, type, optional flag
- [ ] Preserve description if present
- [ ] Recursively resolve type references

### 2.5 Update generateOutput Function
- [ ] Call `collectTypes()` first
- [ ] Pass typeMap to `handleAgentConfig()`
- [ ] Add `types` field to AgentIR
- [ ] Call `extractTypeDefinitions()` and add to IR

### 2.6 Update handleAgentConfig Function
- [ ] Add `typeMap` parameter
- [ ] Pass typeMap to all type extraction calls
- [ ] Update function signature

### 2.7 Update IR Type Definitions
- [ ] Add `types?: Record<string, TypeDefinition>` to AgentIr
- [ ] Define `TypeDefinition` type
- [ ] Define `PropertyInfo` type

## 3. Runtime Changes

### 3.1 Update buildOutputSchema Method
- [ ] Check for type references in output fields
- [ ] Call `typeDefToSchema()` for type references
- [ ] Preserve field-level descriptions
- [ ] Handle missing type definitions with error

### 3.2 Create typeDefToSchema Method
- [ ] Convert TypeDefinition to JsonSchema
- [ ] Recursively resolve nested type references
- [ ] Handle inline objects
- [ ] Preserve descriptions from type definition
- [ ] Build required fields array

### 3.3 Update objectTypeToSchema Method
- [ ] Handle type references in properties
- [ ] Preserve descriptions from properties
- [ ] Support nested objects with descriptions
- [ ] Call `typeDefToSchema()` for type refs

### 3.4 Update convertTypeToSchema Method
- [ ] Handle array types with type references
- [ ] Preserve descriptions on arrays
- [ ] No changes needed for primitives

## 4. Testing

### 4.1 Grammar Tests
- [ ] Test top-level type declaration parsing
- [ ] Test `output type` parsing
- [ ] Test type reference parsing
- [ ] Test descriptions on type fields
- [ ] Test descriptions on inline object properties
- [ ] Test optional fields in types

### 4.2 Generator Unit Tests
- [ ] Test type collection
- [ ] Test type reference resolution
- [ ] Test nested type references
- [ ] Test description preservation
- [ ] Test undefined type error
- [ ] Test duplicate type error
- [ ] Test circular reference detection (future)

### 4.3 Runtime Unit Tests
- [ ] Test JSON Schema generation with type refs
- [ ] Test nested type resolution
- [ ] Test description inclusion
- [ ] Test missing type error
- [ ] Test array types with refs
- [ ] Test union types

### 4.4 Integration Tests
- [ ] Create example agent with types
- [ ] Compile to IR
- [ ] Load in runtime
- [ ] Generate JSON Schema
- [ ] Verify schema correctness
- [ ] Test with actual LLM call

### 4.5 End-to-End Tests
- [ ] Test simple type usage
- [ ] Test output type with descriptions
- [ ] Test nested type references
- [ ] Test array of custom types
- [ ] Test union types
- [ ] Test optional fields

## 5. Documentation

### 5.1 Update DSL Documentation
- [ ] Document type declaration syntax
- [ ] Document `output type` syntax
- [ ] Document type reference syntax
- [ ] Document description syntax
- [ ] Provide examples

### 5.2 Create Migration Guide
- [ ] Show before/after examples
- [ ] Explain benefits
- [ ] Provide conversion steps
- [ ] List breaking changes (none expected)

### 5.3 Update API Documentation
- [ ] Document new IR format
- [ ] Document TypeDefinition structure
- [ ] Update examples

### 5.4 Create Examples
- [ ] Simple type usage example
- [ ] Output type with descriptions example
- [ ] Nested types example
- [ ] Real-world agent example

## 6. Polish

### 6.1 Error Messages
- [ ] Improve undefined type error message
- [ ] Improve duplicate type error message
- [ ] Add suggestions for common mistakes
- [ ] Test error messages with users

### 6.2 Type Generation
- [ ] Update TypeScript type generator
- [ ] Generate types for custom type definitions
- [ ] Ensure generated types are correct
- [ ] Test with TypeScript compiler

### 6.3 Performance
- [ ] Profile type resolution
- [ ] Optimize if needed
- [ ] Add benchmarks

### 6.4 Code Quality
- [ ] Add JSDoc comments
- [ ] Ensure consistent naming
- [ ] Remove dead code
- [ ] Run linter

## Task Dependencies

```
1.1-1.6 (Grammar) → 2.1-2.7 (Generator) → 3.1-3.4 (Runtime) → 4.1-4.5 (Testing) → 5.1-5.4 (Docs) → 6.1-6.4 (Polish)
```

## Estimated Timeline

- **Week 1**: Grammar changes (1.1-1.6) + Basic generator (2.1-2.3)
- **Week 2**: Complete generator (2.4-2.7) + Runtime (3.1-3.4)
- **Week 3**: Testing (4.1-4.5)
- **Week 4**: Documentation (5.1-5.4) + Polish (6.1-6.4)

**Total: 4 weeks**
