# Enhanced Type System - Design Document

## Overview

This design implements a reusable type system for the Auwgent DSL that enables developers to define types once and use them throughout their agent definitions. The system supports nested type references, field descriptions for LLM guidance, and maintains backward compatibility with existing syntax.

## Architecture

The type system follows the existing three-layer architecture:

```
Grammar Layer (Langium)
    ↓ Parse DSL
Generator Layer (CLI)
    ↓ Compile to IR
Runtime Layer (Loader)
    ↓ Execute
```

### Key Components

1. **Grammar Layer**: Defines syntax for type declarations and references
2. **Generator Layer**: Collects types, resolves references, generates IR
3. **Runtime Layer**: Resolves type references in JSON Schema generation

## Grammar Changes

### 1. Add TypeDeclaration to Element

**Current:**
```langium
Element:
    Agent | NamedPrompt | Helper;
```

**New:**
```langium
Element:
    Agent | NamedPrompt | Helper | TypeDeclaration;
```

**Rationale**: Allows type declarations at the top level alongside agents and helpers.

### 2. Define TypeDeclaration

**New:**
```langium
TypeDeclaration:
    (isOutput?="output")? "type" name=ID "{"
        properties+=TypeConfigDeclaration*
    "}";
```

**Rationale**: 
- `isOutput` flag distinguishes output types (with descriptions) from data types
- Reuses `TypeConfigDeclaration` for consistency with existing syntax

### 3. Add Descriptions to TypeConfigDeclaration

**Current:**
```langium
TypeConfigDeclaration:
    name=ID (isOptional?='?')? ":" t=Types;
```

**New:**
```langium
TypeConfigDeclaration:
    name=ID (isOptional?='?')? ":" t=Types ("@desc" description=STRING)?;
```

**Rationale**: Enables field-level descriptions in type definitions.

### 4. Add Type References to Types

**Current:**
```langium
Types:
    types=(ArrayType|ObjectType|UnionType|BooleanType|StringType|NumberType|INT);
```

**New:**
```langium
Types:
    types=(ArrayType|ObjectType|UnionType|BooleanType|StringType|NumberType|INT)
    | typeRef=[TypeDeclaration:ID];
```

**Rationale**: Allows referencing defined types by name.

### 5. Support Descriptions on Inline Object Properties

**Current:**
```langium
PropertyType:
    name=ID (isOptional?='?')? ":" type=Types;
```

**New:**
```langium
PropertyType:
    name=ID (isOptional?='?')? ":" type=Types ("@desc" description=STRING)?;
```

**Rationale**: Enables descriptions on nested inline object fields in output types.

### 6. Remove TypeDeclaration from Statement

**Current:**
```langium
Statement:
    VariableDeclartion | ReturnStatement | IfStatement | TypeDeclaration | TransferStatement;
```

**New:**
```langium
Statement:
    VariableDeclartion | ReturnStatement | IfStatement | TransferStatement;
```

**Rationale**: Type declarations should only be at top level, not inside workflows.

## Generator Changes

### 1. Type Collection Phase

**New Function:**
```typescript
function collectTypes(model: Model): Map<string, TypeDeclaration> {
    const typeMap = new Map<string, TypeDeclaration>();
    
    for (const element of model.elements) {
        if (element.$type === "TypeDeclaration") {
            // Check for duplicates
            if (typeMap.has(element.name)) {
                throw new Error(`Type '${element.name}' is already defined`);
            }
            typeMap.set(element.name, element);
        }
    }
    
    return typeMap;
}
```

**Integration:**
```typescript
export function generateOutput(model: Model, source: string, destination: string) {
    // Collect types first
    const typeMap = collectTypes(model);
    
    // Process agents with type context
    for (const element of model.elements) {
        if (element.$type === "Agent") {
            const agentIR = handleAgentConfig(element, typeMap);
            agentIR.types = extractTypeDefinitions(typeMap);
            // ... rest
        }
    }
}
```

### 2. Type Reference Resolution

**Updated Function:**
```typescript
export function extractType(types: Types, typeMap: Map<string, TypeDeclaration>): any {
    // Handle type references
    if (types.typeRef) {
        const typeName = types.typeRef.ref?.name;
        if (!typeName) {
            throw new Error('Type reference has no name');
        }
        if (!typeMap.has(typeName)) {
            throw new Error(`Type '${typeName}' is not defined`);
        }
        return { typeRef: typeName };
    }
    
    // Handle inline object types with descriptions
    if (isObjectType(types.types)) {
        const props = {};
        types.types.properties.forEach(prop => {
            props[prop.name] = {
                type: extractType(prop.type, typeMap),
                optional: prop.isOptional,
                description: prop.description // NEW: preserve description
            };
        });
        return { type: "object", properties: props };
    }
    
    // Handle arrays
    if (isArrayType(types.types)) {
        const innerType = extractType({ types: types.types.type }, typeMap);
        return `${innerType}[]`;
   

    }
    
    // Handle unions
    if (isUnionType(types.types)) {
        return { type: "union", options: types.types.options };
    }
    
    // Handle primitives
    if (isBooleanType(types.types) || isNumberType(types.types) || isStringType(types.types)) {
        return types.types.type;
    }
    
    return 'unknown';
}
```

### 3. Type Definition Extraction

**New Function:**
```typescript
function extractTypeDefinitions(typeMap: Map<string, TypeDeclaration>): Record<string, TypeDefinition> {
    const types: Record<string, TypeDefinition> = {};
    
    for (const [name, typeDef] of typeMap.entries()) {
        types[name] = {
            isOutput: typeDef.isOutput || false,
            properties: extractPropertiesWithDesc(typeDef.properties, typeMap)
        };
    }
    
    return types;
}

function extractPropertiesWithDesc(
    properties: TypeConfigDeclaration[],
    typeMap: Map<string, TypeDeclaration>
): Record<string, PropertyInfo> {
    const props: Record<string, PropertyInfo> = {};
    
    for (const prop of properties) {
        props[prop.name] = {
            type: extractType(prop.t, typeMap),
            optional: prop.isOptional,
            description: prop.description // Preserve description
        };
    }
    
    return props;
}
```

### 4. Updated IR Format

**Type Definitions:**
```typescript
type AgentIr = {
    name: string,
    types?: Record<string, TypeDefinition>, // NEW
    modelConfig: any[],
    input: any,
    output: any,
    context: any,
    tools: any[],
    workflows: any[],
    helpers: HelperType[],
    helperToolGrants?: Record<string, string[] | "all">,
    lifecycle?: {
        enabled: true,
        maxTokens?: number,
        maxMessages?: number
    }
}

type TypeDefinition = {
    isOutput: boolean,
    properties: Record<string, PropertyInfo>
}

type PropertyInfo = {
    type: any,
    optional?: boolean,
    description?: string
}
```

**Example IR:**
```json
{
  "name": "DataAnalyzer",
  "types": {
    "Point": {
      "isOutput": false,
      "properties": {
        "x": { "type": "number" },
        "y": { "type": "number" }
      }
    },
    "SearchResult": {
      "isOutput": true,
      "properties": {
        "query": {
          "type": "string",
          "description": "The original search query"
        },
        "results": {
          "type": {
            "type": "object",
            "properties": {
              "title": { "type": "string", "description": "Result title" },
              "url": { "type": "string", "description": "Result URL" },
              "snippet": { "type": "string", "description": "Brief excerpt" }
            }
          },
          "array": true,
          "description": "Array of search results"
        }
      }
    }
  },
  "input": {
    "data": { "type": { "typeRef": "Point", "array": true } },
    "user": { "type": { "typeRef": "User" } }
  },
  "output": {
    "analysis": {
      "type": { "typeRef": "AnalysisResult" },
      "description": "The complete analysis"
    }
  }
}
```

## Runtime Changes

### 1. Synthesizer Updates

**Update buildOutputSchema:**
```typescript
private buildOutputSchema(): JsonSchema | undefined {
    if (!this.ir.output || Object.keys(this.ir.output).length === 0) {
        return undefined;
    }

    const properties: Record<string, JsonSchema> = {};
    const requiredFields: string[] = [];

    for (const [key, val] of Object.entries(this.ir.output)) {
        const typeInfo = typeof val === 'string' ? { type: val } : val;
        
        // Resolve type references
        if (typeInfo.type?.typeRef) {
            const typeDef = this.ir.types?.[typeInfo.type.typeRef];
            if (typeDef) {
                properties[key] = this.typeDefToSchema(typeDef);
            } else {
                throw new Error(`Type '${typeInfo.type.typeRef}' not found`);
            }
        } else {
            properties[key] = this.convertTypeToSchema(typeInfo.type);
        }
        
        // Add field-level description (overrides type description)
        if (typeInfo.description) {
            properties[key].description = typeInfo.description;
        }
        
        if (!typeInfo.optional) {
            requiredFields.push(key);
        }
    }

    return { type: "object", properties, required: requiredFields };
}
```

**New Method: typeDefToSchema**
```typescript
private typeDefToSchema(typeDef: TypeDefinition): JsonSchema {
    const properties: Record<string, JsonSchema> = {};
    const required: string[] = [];

    for (const [propName, propInfo] of Object.entries(typeDef.properties)) {
        // Recursively resolve nested type references
        if (propInfo.type?.typeRef) {
            const nestedType = this.ir.types?.[propInfo.type.typeRef];
            if (nestedType) {
                properties[propName] = this.typeDefToSchema(nestedType);
            } else {
                throw new Error(`Type '${propInfo.type.typeRef}' not found`);
            }
        } else if (propInfo.type?.type === 'object') {
            // Handle inline objects with descriptions
            properties[propName] = this.objectTypeToSchema(propInfo.type.properties);
        } else {
            properties[propName] = this.convertTypeToSchema(propInfo.type);
        }
        
        // Add description from type definition
        if (propInfo.description) {
            properties[propName].description = propInfo.description;
        }
        
        if (!propInfo.optional) {
            required.push(propName);
        }
    }

    return { type: "object", properties, required };
}
```

**Update objectTypeToSchema:**
```typescript
private objectTypeToSchema(properties: Record<string, any>): JsonSchema {
    const schemaProps: Record<string, JsonSchema> = {};
    const required: string[] = [];

    for (const [key, typeVal] of Object.entries(properties)) {
        const actualType = this.unwrapType(typeVal.type || typeVal);
        const isOptional = typeVal.optional === true;

        // Recursive: Handle nested objects
        if (typeof actualType === 'object' && actualType.type === 'object' && actualType.properties) {
            schemaProps[key] = this.objectTypeToSchema(actualType.properties);
        }
        // Handle type references
        else if (typeof actualType === 'object' && actualType.typeRef) {
            const typeDef = this.ir.types?.[actualType.typeRef];
            if (typeDef) {
                schemaProps[key] = this.typeDefToSchema(typeDef);
            }
        }
        // Handle unions
        else if (typeof actualType === 'object' && actualType.type === 'union') {
            schemaProps[key] = {
                type: 'string',
                enum: actualType.options.map((o: string) => o.replace(/^["']|["']$/g, ''))
            };
        }
        // Handle primitives and arrays
        else {
            schemaProps[key] = this.convertTypeToSchema(typeof actualType === 'string' ? actualType : 'string');
        }
        
        // Add description if present
        if (typeVal.description) {
            schemaProps[key].description = typeVal.description;
        }
        
        if (!isOptional) {
            required.push(key);
        }
    }

    return {
        type: "object",
        properties: schemaProps,
        required
    };
}
```

## Key Design Decisions

### 1. Input vs Output Descriptions

**Decision**: Input fields do NOT need descriptions.

**Rationale**: 
- Input is for the user/developer, not the LLM
- The LLM never sees the input schema
- Input will support other file formats in the future
- Descriptions would add noise without value

**Example:**
```typescript
// ✅ CORRECT: No descriptions on input
input {
    data: Point[]
    user: User
    mode: "fast" | "thorough"
}

// ✅ CORRECT: Descriptions on output (LLM sees this)
output {
    analysis: AnalysisResult @desc "The complete analysis"
}
```

### 2. Inline Object Descriptions

**Decision**: Support descriptions on inline object properties in output types.

**Rationale**:
- Output types need rich descriptions for LLM guidance
- Inline objects in output types should support field descriptions
- This is specifically for `output type` definitions

**Example:**
```typescript
output type SearchResult {
    query: string @desc "The original search query"
    results: {
        title: string @desc "Result title"
        url: string @desc "Result URL"
        snippet: string @desc "Brief excerpt"
    }[] @desc "Array of search results"
}
```

### 3. Type Declaration Scope

**Decision**: Types are scoped to the file, not to individual agents.

**Rationale**:
- Simpler mental model
- Consistent with NamedPrompt and Helper
- Allows sharing types across multiple agents in same file
- Can add imports later if cross-file sharing is needed

### 4. Circular References

**Decision**: Not supported in v1.

**Rationale**:
- Adds complexity to resolution algorithm
- Rare use case
- Can be added later if needed

**Future Enhancement:**
```typescript
// Future: Circular references
type Node {
    value: string
    next?: Node
}
```

## Data Flow

### Compile Time
```
DSL File
  ↓ Parse (Langium)
AST with TypeDeclarations
  ↓ Collect Types (Generator)
Type Map
  ↓ Resolve References (Generator)
IR with Type Definitions
  ↓ Write to JSON
.agent.json file
```

### Runtime
```
Load IR
  ↓ Synthesizer
Resolve Type References
  ↓ Build JSON Schema
Schema with Descriptions
  ↓ Driver
LLM Request
```

## Error Handling

### Compile-Time Errors

1. **Undefined Type Reference**
```typescript
agent Test {
    input { point: Point }  // Error: Type 'Point' is not defined
}
```

2. **Duplicate Type Definition**
```typescript
type Point { x: number, y: number }
type Point { a: string, b: string }  // Error: Type 'Point' is already defined
```

3. **Circular Type Reference** (Future)
```typescript
type A { b: B }
type B { a: A }  // Error: Circular type reference detected
```

### Runtime Errors

1. **Missing Type in IR**
```typescript
// IR references type that doesn't exist in types map
// This should never happen if generator is correct
throw new Error(`Type '${typeName}' not found in IR`);
```

## Testing Strategy

### Unit Tests (Generator)

```typescript
describe('Type System - Generator', () => {
    it('should collect type declarations', () => {
        const dsl = `
            type Point { x: number, y: number }
            agent Test { }
        `;
        const typeMap = collectTypes(parse(dsl));
        expect(typeMap.has('Point')).toBe(true);
    });
    
    it('should resolve type references', () => {
        const dsl = `
            type Point { x: number, y: number }
            agent Test { input { point: Point } }
        `;
        const ir = compile(dsl);
        expect(ir.input.point.type.typeRef).toBe('Point');
    });
    
    it('should handle nested type references', () => {
        const dsl = `
            type Address { street: string, city: string }
            type User { name: string, address: Address }
            agent Test { input { user: User } }
        `;
        const ir = compile(dsl);
        expect(ir.types.User.properties.address.type.typeRef).toBe('Address');
    });
    
    it('should preserve field descriptions', () => {
        const dsl = `
            output type Result {
                value: string @desc "The result value"
            }
        `;
        const ir = compile(dsl);
        expect(ir.types.Result.properties.value.description).toBe("The result value");
    });
    
    it('should throw on undefined type reference', () => {
        const dsl = `
            agent Test { input { point: Point } }
        `;
        expect(() => compile(dsl)).toThrow("Type 'Point' is not defined");
    });
});
```

### Integration Tests (Runtime)

```typescript
describe('Type System - Runtime', () => {
    it('should generate correct JSON Schema for type references', async () => {
        const ir = {
            types: {
                Point: {
                    isOutput: false,
                    properties: {
                        x: { type: 'number' },
                        y: { type: 'number' }
                    }
                }
            },
            output: {
                point: { type: { typeRef: 'Point' } }
            }
        };
        
        const synthesizer = new Synthesizer(ir);
        const schema = synthesizer.buildOutputSchema();
        
        expect(schema.properties.point).toEqual({
            type: 'object',
            properties: {
                x: { type: 'number' },
                y: { type: 'number' }
            },
            required: ['x', 'y']
        });
    });
    
    it('should include descriptions in JSON Schema', async () => {
        const ir = {
            types: {
                Result: {
                    isOutput: true,
                    properties: {
                        value: {
                            type: 'string',
                            description: 'The result value'
                        }
                    }
                }
            },
            output: {
                result: {
                    type: { typeRef: 'Result' },
                    description: 'The result'
                }
            }
        };
        
        const synthesizer = new Synthesizer(ir);
        const schema = synthesizer.buildOutputSchema();
        
        expect(schema.properties.result.description).toBe('The result');
        expect(schema.properties.result.properties.value.description).toBe('The result value');
    });
});
```

## Migration Guide

### Before (Current Syntax)
```typescript
agent DataAnalyzer {
    input {
        start: { x: number, y: number }
        end: { x: number, y: number }
    }
    
    output {
        distance: number @desc "Distance in meters"
    }
    
    tool calculateDistance(
        p1: { x: number, y: number },
        p2: { x: number, y: number }
    ): number @desc "Calculates distance"
}
```

### After (With Type System)
```typescript
type Point {
    x: number
    y: number
}

output type DistanceResult {
    distance: number @desc "Distance in meters"
    unit: string @desc "Unit of measurement"
}

agent DataAnalyzer {
    input {
        start: Point
        end: Point
    }
    
    output {
        result: DistanceResult @desc "The calculated distance"
    }
    
    tool calculateDistance(p1: Point, p2: Point): number
        @desc "Calculates distance between two points"
}
```

## Performance Considerations

### Compile Time
- Type collection: O(n) where n = number of elements
- Type resolution: O(m * d) where m = number of type usages, d = max nesting depth
- Expected impact: Negligible (< 10ms for typical agents)

### Runtime
- Type resolution in Synthesizer: O(d) where d = max nesting depth
- Expected impact: Negligible (< 1ms per request)

## Future Enhancements

### Phase 2 (Future)
1. **Circular Reference Detection**
2. **Type Imports** (cross-file types)
3. **Type Aliases** (`type UserId = string`)
4. **Intersection Types** (`type Admin = User & Permissions`)

### Phase 3 (Future)
1. **Generic Types** (`type Result<T> { data: T }`)
2. **Conditional Types**
3. **Mapped Types**

## Conclusion

This design provides a clean, focused type system that:
- ✅ Eliminates type duplication
- ✅ Adds rich descriptions for LLM guidance
- ✅ Maintains backward compatibility
- ✅ Fits naturally into existing architecture
- ✅ Provides clear error messages
- ✅ Supports nested type references

The implementation is straightforward and well-scoped, with clear separation between compile-time and runtime concerns.
