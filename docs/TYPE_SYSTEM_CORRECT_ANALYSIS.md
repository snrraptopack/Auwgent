# Type System - CORRECT Analysis

## Current State (What Actually Exists)

### 1. **Grammar Analysis**

#### A. Output Block
```langium
OutputConfig:
    "output" "{" outProperties+=(Output)* "}";

Output:
    td=TypeConfigDeclaration "@desc" description=STRING;

TypeConfigDeclaration:
    name=ID (isOptional?='?')? ":" t=Types;
```

**What this means:**
```typescript
// ✅ WORKS: Simple types with descriptions
output {
    reply: string @desc "Your response"
    count: number @desc "The count"
}

// ✅ WORKS: Optional fields
output {
    reply?: string @desc "Optional response"
}

// ❓ QUESTION: Can we do inline objects with field descriptions?
output {
    analysis: {
        summary: string @desc "Summary",  // Does @desc work here?
        confidence: number @desc "Score"
    } @desc "Analysis result"
}
```

#### B. Tool Definitions
```langium
ToolFunction:
    name=ID 
    "(" (params+=TypeConfigDeclaration ("," params+=TypeConfigDeclaration)*)? ")" 
    ":" returns=Types 
    ("{" "description" ":" desc+=STRING "}")?
    ("@desc" desc+=STRING)?;
```

**What this means:**
```typescript
// ✅ WORKS: Both syntaxes for tool description
tool getThing(): string { description: "Gets a thing" }
tool getThing(): string @desc "Gets a thing"

// ✅ WORKS: Inline return types
tool getUserInfo(userId: string): {
    name: string,
    email: string,
    tier: "free" | "pro" | "enterprise"
} @desc "Gets user info"

// ❓ QUESTION: Can we add descriptions to the inline object fields?
tool getUserInfo(userId: string): {
    name: string @desc "User's name",  // Does this work?
    email: string @desc "User's email"
}
```

#### C. Type Declarations (ALREADY EXISTS!)
```langium
Statement:
    VariableDeclartion | ReturnStatement | IfStatement | TypeDeclaration | TransferStatement;

TypeDeclaration:
    "type" name=ID "{" types+=TypeConfigDeclaration* "}";
```

**What this means:**
```typescript
// ✅ WORKS: Type declarations inside workflows!
workflow processData(input: string): Result {
    description: "Processes data"
    
    type Result {
        value: string
        count: number
    }
    
    return { value: input, count: 1 }
}

// ❌ DOESN'T WORK: Type declarations at top level (not in Element)
type Point {  // This is NOT allowed at agent level
    x: number
    y: number
}
```

#### D. Object Types
```langium
ObjectType:
    '{' (properties+=PropertyType (',' properties+=PropertyType)*)? '}';

PropertyType:
    name=ID (isOptional?='?')? ":" type=Types;
```

**What this means:**
```typescript
// ✅ WORKS: Inline object types
input {
    point: { x: number, y: number }
}

// ❌ DOESN'T WORK: Descriptions on inline object properties
input {
    point: {
        x: number @desc "X coordinate",  // @desc not in PropertyType!
        y: number @desc "Y coordinate"
    }
}
```

---

## 2. **What's Actually Missing**

### A. Top-Level Type Declarations
```typescript
// ❌ CURRENT: Can't do this
type Point {
    x: number
    y: number
}

agent MapAnalyzer {
    input { start: Point, end: Point }
}

// ✅ CURRENT: Must do this
agent MapAnalyzer {
    input {
        start: { x: number, y: number }
        end: { x: number, y: number }
    }
}
```

### B. Descriptions on Inline Object Properties
```typescript
// ❌ CURRENT: Can't do this
output {
    analysis: {
        summary: string @desc "High-level summary",
        confidence: number @desc "Score 0-1"
    } @desc "Analysis result"
}

// ✅ CURRENT: Can only describe the whole field
output {
    analysis: {
        summary: string,
        confidence: number
    } @desc "Analysis result"
}
```

### C. Type References
```typescript
// ❌ CURRENT: Can't reference types
type UserInfo {
    name: string
    email: string
}

tool getUser(id: string): UserInfo  // Can't reference UserInfo
```

---

## 3. **What You Want to Add**

Based on your proposal, you want:

### A. Top-Level Type Declarations
```typescript
// NEW: Define types at agent level
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
// NEW: Special output types with descriptions
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

### C. Descriptions on Inline Object Properties
```typescript
// NEW: Add @desc to PropertyType
tool getUserInfo(userId: string): {
    name: string @desc "User's full name",
    email: string @desc "User's email address",
    tier: "free" | "pro" | "enterprise" @desc "Subscription tier"
} @desc "Gets user information"
```

---

## 4. **Grammar Changes Needed**

### Change 1: Add TypeDeclaration to Element
```diff
Element:
-    Agent | NamedPrompt | Helper;
+    Agent | NamedPrompt | Helper | TypeDeclaration;
```

**Effect**: Allows type declarations at top level, not just in workflows

### Change 2: Add output keyword to TypeDeclaration
```diff
TypeDeclaration:
-    "type" name=ID "{" types+=TypeConfigDeclaration* "}";
+    (isOutput?="output")? "type" name=ID "{" types+=TypeConfigDeclaration* "}";
```

**Effect**: Distinguishes `type Point` from `output type Result`

### Change 3: Add descriptions to TypeConfigDeclaration
```diff
TypeConfigDeclaration:
-    name=ID (isOptional?='?')? ":" t=Types;
+    name=ID (isOptional?='?')? ":" t=Types ("@desc" description=STRING)?;
```

**Effect**: Allows `x: number @desc "X coordinate"` in type definitions

### Change 4: Add descriptions to PropertyType
```diff
PropertyType:
-    name=ID (isOptional?='?')? ":" type=Types;
+    name=ID (isOptional?='?')? ":" type=Types ("@desc" description=STRING)?;
```

**Effect**: Allows descriptions in inline object types

### Change 5: Add type references to Types
```diff
Types:
-    types=(ArrayType|ObjectType|UnionType|BooleanType|StringType|NumberType|INT);
+    types=(ArrayType|ObjectType|UnionType|BooleanType|StringType|NumberType|INT)
+    | typeRef=[TypeDeclaration:ID];
```

**Effect**: Allows `Point` to reference a type declaration

---

## 5. **Tool Description Syntax**

### Current State
```langium
ToolFunction:
    name=ID 
    "(" params... ")" 
    ":" returns=Types 
    ("{" "description" ":" desc+=STRING "}")?  // Block syntax
    ("@desc" desc+=STRING)?;                    // Shorthand syntax
```

**Both work:**
```typescript
tool getThing(): string { description: "Gets a thing" }
tool getThing(): string @desc "Gets a thing"
```

### Your Concern
You said users might prefer one over the other. Let's analyze:

**Option 1: Block syntax**
```typescript
tool getUser(id: string): UserInfo {
    description: "Gets user information"
}
```
- ✅ Consistent with other blocks (`config { }`, `workflow { }`)
- ✅ Extensible (could add more properties later)
- ❌ More verbose

**Option 2: Shorthand syntax**
```typescript
tool getUser(id: string): UserInfo @desc "Gets user information"
```
- ✅ Concise
- ✅ Consistent with output field descriptions
- ❌ Less extensible

### Recommendation
**Keep both!** They serve different use cases:
- Use `@desc` for simple one-liners
- Use `{ description: "..." }` when you might add more metadata later

**Future extensibility:**
```typescript
tool getUser(id: string): UserInfo {
    description: "Gets user information"
    rateLimit: 100  // Future: rate limiting
    cache: true     // Future: caching
}
```

---

## 6. **Proposed Final Syntax**

### Example 1: Simple Agent with Reusable Types
```typescript
// Top-level type definitions
type Point {
    x: number
    y: number
}

type Distance {
    value: number
    unit: "meters" | "kilometers"
}

agent MapAnalyzer {
    input {
        start: Point
        end: Point
    }
    
    output {
        distance: Distance @desc "Calculated distance"
    }
    
    tool calculateDistance(p1: Point, p2: Point): Distance
        @desc "Calculates distance between two points"
}
```

### Example 2: Output Types with Rich Descriptions
```typescript
// Output type with field descriptions for LLM guidance
output type AnalysisResult {
    summary: string @desc "High-level summary of the analysis"
    confidence: number @desc "Confidence score between 0 and 1"
    keyFindings: string[] @desc "List of key findings"
    recommendations: string[] @desc "Actionable recommendations"
}

agent DataAnalyzer {
    input {
        data: string
    }
    
    output {
        analysis: AnalysisResult @desc "The complete analysis"
    }
}
```

### Example 3: Inline Types with Descriptions
```typescript
agent UserManager {
    // Inline object with field descriptions
    tool getUserInfo(userId: string): {
        name: string @desc "User's full name",
        email: string @desc "User's email address",
        tier: "free" | "pro" | "enterprise" @desc "Subscription tier",
        joinedAt: string @desc "ISO 8601 date when user joined"
    } @desc "Gets comprehensive user information"
    
    // Or use block syntax for extensibility
    tool createUser(data: UserData): User {
        description: "Creates a new user account"
    }
}
```

---

## 7. **Implementation Strategy**

### Phase 1: Core Type System (Week 1-2)
1. Add `TypeDeclaration` to `Element`
2. Add `typeRef` to `Types`
3. Update generator to collect and resolve type references
4. Update runtime to handle type references in JSON Schema

**Result**: Can define and use types, but no descriptions yet

### Phase 2: Description Support (Week 3)
1. Add `@desc` to `TypeConfigDeclaration`
2. Add `@desc` to `PropertyType`
3. Add `isOutput` flag to `TypeDeclaration`
4. Update generator to preserve descriptions
5. Update runtime to include descriptions in JSON Schema

**Result**: Full description support everywhere

### Phase 3: Testing & Polish (Week 4)
1. Write comprehensive tests
2. Update documentation
3. Create migration guide
4. Add examples

---

## 8. **Key Design Decisions**

### Decision 1: `type` vs `output type`
**Question**: Should we distinguish data types from output types?

**Recommendation**: YES
- `type Point` = reusable data structure
- `output type Result` = LLM-facing structure with rich descriptions

**Rationale**: Makes intent clear, allows different handling

### Decision 2: Description Syntax
**Question**: `@desc` vs `{ description: "..." }`?

**Recommendation**: Support BOTH
- `@desc` for conciseness
- `{ description: "..." }` for extensibility

**Rationale**: Different use cases, both valid

### Decision 3: Type Scope
**Question**: Should types be scoped to agent or global in file?

**Recommendation**: Global in file (like NamedPrompt)
- Simpler mental model
- Easier to share types across agents in same file
- Can add imports later if needed

### Decision 4: Circular References
**Question**: Allow `type Node { next?: Node }`?

**Recommendation**: Not in v1
- Adds complexity
- Rare use case
- Can add later if needed

---

## 9. **Migration Path**

### Backward Compatibility
```typescript
// ✅ OLD: Still works
agent OldStyle {
    output {
        reply: string @desc "Response"
    }
    
    tool getThing(): string @desc "Gets thing"
}

// ✅ NEW: Also works
type Response {
    reply: string
}

agent NewStyle {
    output {
        response: Response @desc "The response"
    }
}
```

### Migration Tool (Future)
```bash
# Analyze agent file and suggest type extractions
auwgent analyze my-agent.agent

# Output:
# Found 3 duplicate inline types:
#   - { x: number, y: number } used 5 times
#   - { name: string, email: string } used 3 times
# 
# Suggested types:
#   type Point { x: number, y: number }
#   type UserInfo { name: string, email: string }
```

---

## 10. **Open Questions**

1. **Array type descriptions**: Should we allow `Point[] @desc "List of points"`?
2. **Union type descriptions**: Should we allow `"a" | "b" @desc "Status"`?
3. **Nested type refs**: Should we allow `type Outer { inner: Inner }`?
4. **Type exports**: Future feature for cross-file types?
5. **Generic types**: Future feature like `type Result<T> { data: T }`?

---

## Conclusion

Your current type system is **more capable than I initially thought**:
- ✅ Already has `TypeDeclaration` (in workflows)
- ✅ Already supports both `@desc` and `{ description }` for tools
- ✅ Already has inline object types

What's **actually missing**:
- ❌ Top-level type declarations
- ❌ Type references
- ❌ Descriptions on inline object properties
- ❌ `output type` distinction

The enhancement is **straightforward** and **well-scoped**. The grammar changes are minimal, and the implementation follows your existing patterns.

**Recommendation: Proceed with the enhancement as proposed.**
