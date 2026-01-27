# Type System Enhancement - Final Proposal

## What We're Actually Building

Based on clarifications, here's what makes sense:

---

## 1. **Core Features**

### A. Top-Level Type Declarations
```typescript
// Define reusable types at agent level
type Point {
    x: number
    y: number
}

type UserInfo {
    name: string
    email: string
    tier: "free" | "pro" | "enterprise"
}

agent MyAgent {
    input { point: Point }
    tool getUser(id: string): UserInfo
}
```

### B. Output Types with Field Descriptions
```typescript
// Special output types with descriptions for LLM
output type AnalysisResult {
    summary: string @desc "High-level summary"
    confidence: number @desc "Confidence score 0-1"
    details: string[] @desc "Detailed observations"
}

agent Analyzer {
    output {
        result: AnalysisResult @desc "The analysis"
    }
}
```

### C. Nested Type References
```typescript
type Address {
    street: string
    city: string
    zipCode?: string
}

type User {
    name: string
    address: Address  // ✅ Nested type reference
}
```

### D. Array and Union Type Descriptions
```typescript
agent DataProcessor {
    input {
        points: Point[] @desc "List of coordinate points"
        status: "active" | "inactive" @desc "Current status"
    }
}
```

---

## 2. **What We're NOT Building**

### ❌ Inline Object Field Descriptions
```typescript
// ❌ DOESN'T MAKE SENSE: Can't describe inline object fields
tool getUserInfo(userId: string): {
    name: string @desc "User's full name",  // NO
    email: string @desc "User's email"
} @desc "Gets user info"

// ✅ INSTEAD: Define a type
type UserInfo {
    name: string @desc "User's full name"
    email: string @desc "User's email"
}

tool getUserInfo(userId: string): UserInfo @desc "Gets user info"
```

**Reason**: If you need descriptions, define a proper type. Inline objects are for simple cases.

### ❌ Type Declarations Inside Workflows
```typescript
// ❌ DOESN'T MAKE SENSE: Can't use type as return type
workflow processData(input: string): Result {
    description: "Processes data"
    
    type Result {  // This Result can't be used above
        value: string
        count: number
    }
    
    return { value: input, count: 1 }
}

// ✅ INSTEAD: Define type at top level
type Result {
    value: string
    count: number
}

workflow processData(input: string): Result {
    description: "Processes data"
    return { value: input, count: 1 }
}
```

**Reason**: Type declarations should be at top level for reusability.

### ❌ Tool Metadata Block
```typescript
// ❌ NOT NEEDED: Tool metadata in block
tool getUser(id: string): UserInfo {
    description: "Gets user information"
    rateLimit: 100
    cache: true
}

// ✅ INSTEAD: Keep it simple
tool getUser(id: string): UserInfo @desc "Gets user information"
```

**Reason**: Rate limiting, caching, etc. should be handled at runtime/infrastructure level, not in DSL.

### ❌ Generic Types
```typescript
// ❌ NOT NEEDED: Generics
type Result<T> {
    data: T
    error?: string
}
```

**Reason**: Adds complexity without clear use case. Can add later if needed.

### ❌ Type Exports/Imports
```typescript
// ❌ NOT NEEDED: Cross-file types
import { Point, Address } from "./common.types.agent"
```

**Reason**: Single-file agents are simpler. Can add later if needed.

---

## 3. **Grammar Changes**

### Change 1: Add TypeDeclaration to Element
```diff
Element:
-    Agent | NamedPrompt | Helper;
+    Agent | NamedPrompt | Helper | TypeDeclaration;
```

### Change 2: Update TypeDeclaration
```diff
+// Top-level type declarations
+TypeDeclaration:
+    (isOutput?="output")? "type" name=ID "{"
+        properties+=TypeConfigDeclaration*
+    "}";

-// Remove from Statement (no longer needed in workflows)
Statement:
-    VariableDeclartion | ReturnStatement | IfStatement | TypeDeclaration | TransferStatement
+    VariableDeclartion | ReturnStatement | IfStatement | TransferStatement
```

### Change 3: Add Descriptions to TypeConfigDeclaration
```diff
TypeConfigDeclaration:
-    name=ID (isOptional?='?')? ":" t=Types;
+    name=ID (isOptional?='?')? ":" t=Types ("@desc" description=STRING)?;
```

### Change 4: Add Type References
```diff
Types:
-    types=(ArrayType|ObjectType|UnionType|BooleanType|StringType|NumberType|INT);
+    types=(ArrayType|ObjectType|UnionType|BooleanType|StringType|NumberType|INT)
+    | typeRef=[TypeDeclaration:ID];
```

### Change 5: Support Descriptions on Array/Union Types
```diff
+// Allow descriptions on any type usage
+TypeUsage:
+    type=Types ("@desc" description=STRING)?;

+// Update TypeConfigDeclaration to use TypeUsage
TypeConfigDeclaration:
-    name=ID (isOptional?='?')? ":" t=Types ("@desc" description=STRING)?;
+    name=ID (isOptional?='?')? ":" t=TypeUsage;
```

---

## 4. **Complete Example**

```typescript
// ============================================
// TYPE DEFINITIONS
// ============================================

// Regular data types
type Point {
    x: number
    y: number
}

type Address {
    street: string
    city: string
    zipCode?: string
}

type User {
    id: string
    name: string
    address: Address
}

// Output types with rich descriptions for LLM
output type SearchResult {
    query: string @desc "The original search query"
    results: {
        title: string
        url: string
        snippet: string
    }[] @desc "Array of search results"
    totalCount: number @desc "Total number of results found"
}

output type AnalysisResult {
    summary: string @desc "High-level summary of findings"
    confidence: number @desc "Confidence score between 0 and 1"
    keyFindings: string[] @desc "List of key findings"
    recommendations: string[] @desc "Actionable recommendations"
}

// ============================================
// AGENT DEFINITION
// ============================================

agent DataAnalyzer {
    input {
        data: Point[] @desc "List of data points to analyze"
        user: User
        mode: "fast" | "thorough" @desc "Analysis mode"
    }
    
    output {
        analysis: AnalysisResult @desc "The complete analysis"
        searchResults: SearchResult @desc "Related search results"
    }
    
    context {
        sessionId: string
    }
    
    tools {
        calculateDistance(p1: Point, p2: Point): number
            @desc "Calculates Euclidean distance between two points"
        
        getUserInfo(userId: string): User
            @desc "Fetches user information from database"
        
        searchWeb(query: string): SearchResult
            @desc "Searches the web and returns structured results"
    }
    
    default config {
        model: gemini("gemini-2.0-flash")
        prompt: "You are a data analysis assistant."
    }
}
```

---

## 5. **Implementation Plan**

### Phase 1: Grammar (Week 1)
- [ ] Move `TypeDeclaration` from `Statement` to `Element`
- [ ] Add `isOutput` flag to `TypeDeclaration`
- [ ] Add `@desc` to `TypeConfigDeclaration`
- [ ] Add `typeRef` to `Types`
- [ ] Regenerate Langium artifacts
- [ ] Test grammar with examples

### Phase 2: Generator (Week 2)
- [ ] Collect type declarations in a map
- [ ] Implement type reference resolution
- [ ] Preserve descriptions in IR
- [ ] Update IR format to include types
- [ ] Test type resolution with nested types

### Phase 3: Runtime (Week 3)
- [ ] Update Synthesizer to resolve type references
- [ ] Include descriptions in JSON Schema
- [ ] Handle `output type` vs regular `type`
- [ ] Test with actual LLM calls

### Phase 4: Testing & Docs (Week 4)
- [ ] Write comprehensive tests
- [ ] Update documentation
- [ ] Create migration examples
- [ ] Add example agents

---

## 6. **IR Format**

### Current IR
```json
{
  "name": "MyAgent",
  "input": {
    "message": { "type": "string" }
  },
  "output": {
    "reply": { "type": "string", "description": "Your response" }
  }
}
```

### New IR with Types
```json
{
  "name": "MyAgent",
  "types": {
    "Point": {
      "isOutput": false,
      "properties": {
        "x": { "type": "number" },
        "y": { "type": "number" }
      }
    },
    "AnalysisResult": {
      "isOutput": true,
      "properties": {
        "summary": {
          "type": "string",
          "description": "High-level summary"
        },
        "confidence": {
          "type": "number",
          "description": "Confidence score 0-1"
        }
      }
    }
  },
  "input": {
    "points": {
      "type": { "typeRef": "Point", "array": true },
      "description": "List of points"
    }
  },
  "output": {
    "analysis": {
      "type": { "typeRef": "AnalysisResult" },
      "description": "The analysis"
    }
  }
}
```

---

## 7. **Generator Changes**

### Collect Types
```typescript
export function generateOutput(model: Model, source: string, destination: string) {
    // Collect type declarations
    const typeMap = new Map<string, TypeDeclaration>();
    for (const element of model.elements) {
        if (element.$type === "TypeDeclaration") {
            typeMap.set(element.name, element);
        }
    }
    
    // Process agents with type context
    for (const element of model.elements) {
        if (element.$type === "Agent") {
            const agentIR = handleAgentConfig(element, typeMap);
            // ... rest
        }
    }
}
```

### Resolve Type References
```typescript
export function extractType(types: Types, typeMap: Map<string, TypeDeclaration>): any {
    // Handle type references
    if (types.typeRef) {
        const typeName = types.typeRef.ref?.name;
        return { typeRef: typeName };
    }
    
    // Handle inline types
    if (isObjectType(types.types)) {
        const props = {};
        types.types.properties.forEach(prop => {
            props[prop.name] = {
                type: extractType(prop.type, typeMap),
                optional: prop.isOptional,
                description: prop.description
            };
        });
        return { type: "object", properties: props };
    }
    
    // ... rest
}
```

### Extract Type Definitions
```typescript
function extractTypeDefinitions(typeMap: Map<string, TypeDeclaration>): Record<string, any> {
    const types = {};
    
    for (const [name, typeDef] of typeMap.entries()) {
        types[name] = {
            isOutput: typeDef.isOutput || false,
            properties: extractPropertiesWithDesc(typeDef.properties, typeMap)
        };
    }
    
    return types;
}
```

---

## 8. **Runtime Changes**

### Synthesizer: Resolve Type References
```typescript
private buildOutputSchema(): JsonSchema | undefined {
    const properties: Record<string, JsonSchema> = {};
    const requiredFields: string[] = [];

    for (const [key, val] of Object.entries(this.ir.output)) {
        const typeInfo = typeof val === 'string' ? { type: val } : val;
        
        // Resolve type references
        if (typeInfo.type?.typeRef) {
            const typeDef = this.ir.types?.[typeInfo.type.typeRef];
            if (typeDef) {
                properties[key] = this.typeDefToSchema(typeDef);
            }
        } else {
            properties[key] = this.convertTypeToSchema(typeInfo.type);
        }
        
        // Add field-level description
        if (typeInfo.description) {
            properties[key].description = typeInfo.description;
        }
        
        if (!typeInfo.optional) {
            requiredFields.push(key);
        }
    }

    return { type: "object", properties, required: requiredFields };
}

private typeDefToSchema(typeDef: TypeDefinition): JsonSchema {
    const properties: Record<string, JsonSchema> = {};
    const required: string[] = [];

    for (const [propName, propInfo] of Object.entries(typeDef.properties)) {
        // Recursively resolve nested type references
        if (propInfo.type?.typeRef) {
            const nestedType = this.ir.types?.[propInfo.type.typeRef];
            if (nestedType) {
                properties[propName] = this.typeDefToSchema(nestedType);
            }
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

---

## 9. **Success Criteria**

### Must Have
- ✅ Define types at top level
- ✅ Reference types in input/output/tools
- ✅ Nested type references work
- ✅ `output type` with field descriptions
- ✅ Array/union type descriptions
- ✅ Generated TypeScript types are correct
- ✅ JSON Schema includes all descriptions
- ✅ All existing tests pass

### Nice to Have
- ✅ Good error messages for undefined types
- ✅ Circular reference detection
- ✅ Type validation at compile time

---

## 10. **Testing Strategy**

### Unit Tests
```typescript
describe('Type System', () => {
    it('should collect top-level type declarations', () => {
        const dsl = `
            type Point { x: number, y: number }
            agent Test { input { point: Point } }
        `;
        const ir = compile(dsl);
        expect(ir.types.Point).toBeDefined();
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
});
```

---

## Conclusion

This is a **clean, focused enhancement** that:
- ✅ Solves the type duplication problem
- ✅ Adds rich descriptions for LLM guidance
- ✅ Maintains simplicity (no generics, no imports)
- ✅ Fits naturally into existing architecture
- ✅ Provides clear migration path

**Estimated effort: 3-4 weeks**
**Risk: Low** (well-scoped, clear requirements)
**Value: High** (major DX improvement)

Ready to implement!
