# Auwgent DSL Syntax Highlighting Guide

**Date**: 2026-01-27  
**Status**: Enhanced ✅

---

## Overview

The Auwgent DSL now has comprehensive syntax highlighting with proper TextMate scopes that work with all VSCode themes.

---

## Syntax Elements

### 1. Comments

**Line Comments**:
```auwgent
// This is a line comment
```
- Scope: `comment.line.double-slash.auwgent`
- Color: Typically gray/muted

**Block Comments**:
```auwgent
/*
 * This is a block comment
 * Multiple lines
 */
```
- Scope: `comment.block.auwgent`
- Color: Typically gray/muted

---

### 2. Type Declarations

**Regular Types**:
```auwgent
type Point {
    x: number
    y: number
}
```
- `type` keyword: `storage.type.auwgent` (purple/blue)
- `Point` (type name): `entity.name.type.auwgent` (yellow/gold)

**Output Types**:
```auwgent
output type AnalysisResult {
    summary: string
}
```
- `output` modifier: `storage.modifier.auwgent` (purple)
- `type` keyword: `storage.type.auwgent` (purple/blue)
- `AnalysisResult` (type name): `entity.name.type.auwgent` (yellow/gold)

---

### 3. Agent Declarations

```auwgent
agent TypeSystemTest {
    // ...
}
```
- `agent` keyword: `storage.type.class.auwgent` (purple/blue)
- `TypeSystemTest` (agent name): `entity.name.class.auwgent` (yellow/gold)

---

### 4. Helper Declarations

```auwgent
helper analyzeData {
    // ...
}
```
- `helper` keyword: `storage.type.function.auwgent` (purple/blue)
- `analyzeData` (helper name): `entity.name.function.auwgent` (yellow/gold)

---

### 5. Workflow Declarations

```auwgent
workflow processData {
    // ...
}
```
- `workflow` keyword: `storage.type.function.auwgent` (purple/blue)
- `processData` (workflow name): `entity.name.function.auwgent` (yellow/gold)

---

### 6. Decorators

```auwgent
summary: string @desc "High-level summary"
```
- `@desc`: `entity.name.tag.auwgent` (orange/yellow)
- Description string: `string.quoted.double.auwgent` (green/orange)

---

### 7. Keywords

**Control Flow**:
```auwgent
if (condition) {
    return value
} else {
    continue
}
```
- `if`, `else`, `return`, `continue`: `keyword.control.auwgent` (pink/magenta)

**Other Keywords**:
```auwgent
input {
    message: string
}

output {
    result: string
}

context {
    sessionId: string
}

default config {
    model: gemini("gemini-2.0-flash")
}
```
- `input`, `output`, `context`, `config`, `default`, `model`, `prompt`: `keyword.other.auwgent` (pink/magenta)

---

### 8. Type Annotations

**Primitive Types**:
```auwgent
name: string
age: number
active: boolean
```
- `string`, `number`, `boolean`: `support.type.primitive.auwgent` (cyan/blue)
- `:` (colon): `keyword.operator.type.auwgent` (white)

**Array Types**:
```auwgent
tags: string[]
points: Point[]
```
- `string`, `Point`: Type name (cyan/yellow)
- `[]`: `meta.type.array.auwgent` (white)

**Type References**:
```auwgent
address: Address
user: User
```
- `Address`, `User`: `entity.name.type.auwgent` (yellow/gold)

---

### 9. Property Names

```auwgent
type User {
    id: string
    name: string
    email: string
}
```
- `id`, `name`, `email`: `variable.other.property.auwgent` (light blue/white)

---

### 10. Strings

**Double Quotes**:
```auwgent
prompt: "You are a helpful assistant"
```
- Scope: `string.quoted.double.auwgent` (green/orange)

**Single Quotes**:
```auwgent
name: 'John Doe'
```
- Scope: `string.quoted.single.auwgent` (green/orange)

**Escape Sequences**:
```auwgent
text: "Line 1\nLine 2\tTabbed"
```
- `\n`, `\t`: `constant.character.escape.auwgent` (orange/red)

---

### 11. Numbers

```auwgent
confidence: 0.85
maxTokens: 10000
temperature: 1.5e-2
```
- Scope: `constant.numeric.decimal.auwgent` (orange/green)

---

### 12. Booleans

```auwgent
enabled: true
disabled: false
```
- `true`, `false`: `constant.language.boolean.auwgent` (orange/blue)

---

### 13. Operators

**Assignment**:
```auwgent
let result = value
```
- `=`: `keyword.operator.assignment.auwgent` (white)

**Comparison**:
```auwgent
if (x == 5) { }
if (y != 10) { }
if (z >= 100) { }
```
- `==`, `!=`, `<=`, `>=`, `<`, `>`: `keyword.operator.comparison.auwgent` (pink)

**Logical**:
```auwgent
if (a && b) { }
if (x || y) { }
if (!flag) { }
```
- `&&`, `||`, `!`: `keyword.operator.logical.auwgent` (pink)

**Arithmetic**:
```auwgent
let sum = a + b
let diff = x - y
```
- `+`, `-`, `*`, `/`, `%`: `keyword.operator.arithmetic.auwgent` (white)

**Optional**:
```auwgent
zipCode?: string
```
- `?`: `keyword.operator.optional.auwgent` (pink)

**Union**:
```auwgent
status: "pending" | "complete"
```
- `|`: `keyword.operator.union.auwgent` (white)

---

### 14. Function Calls

**Provider Functions**:
```auwgent
model: gemini("gemini-2.0-flash")
model: openai("gpt-4")
```
- `gemini`, `openai`: `entity.name.function.provider.auwgent` (yellow/gold)

**Regular Functions**:
```auwgent
let result = processData(input)
```
- `processData`: `entity.name.function.auwgent` (yellow/gold)

---

## Color Mapping by Theme

### Dark+ (Default Dark)
- **Keywords**: Pink/Magenta (`#C586C0`)
- **Types**: Cyan (`#4EC9B0`)
- **Type Names**: Yellow (`#DCDCAA`)
- **Strings**: Orange (`#CE9178`)
- **Numbers**: Light Green (`#B5CEA8`)
- **Comments**: Green (`#6A9955`)
- **Functions**: Yellow (`#DCDCAA`)
- **Properties**: Light Blue (`#9CDCFE`)

### Light+ (Default Light)
- **Keywords**: Blue (`#0000FF`)
- **Types**: Teal (`#267F99`)
- **Type Names**: Dark Yellow (`#795E26`)
- **Strings**: Red (`#A31515`)
- **Numbers**: Green (`#098658`)
- **Comments**: Green (`#008000`)
- **Functions**: Dark Yellow (`#795E26`)
- **Properties**: Black (`#001080`)

### Monokai
- **Keywords**: Pink (`#F92672`)
- **Types**: Blue (`#66D9EF`)
- **Type Names**: Green (`#A6E22E`)
- **Strings**: Yellow (`#E6DB74`)
- **Numbers**: Purple (`#AE81FF`)
- **Comments**: Gray (`#75715E`)
- **Functions**: Green (`#A6E22E`)
- **Properties**: White (`#F8F8F2`)

---

## Example with Full Highlighting

```auwgent
// Type declaration with descriptions
output type AnalysisResult {
    summary: string @desc "High-level summary of findings"
    confidence: number @desc "Confidence score between 0 and 1"
    keyFindings: string[] @desc "List of key findings"
}

// Agent declaration
agent DataAnalyzer {
    input {
        data: string
        options?: {
            detailed: boolean
            maxResults: number
        }
    }
    
    output {
        analysis: AnalysisResult @desc "The complete analysis"
    }
    
    context {
        sessionId: string
        userId: string
    }
    
    default config {
        model: gemini("gemini-2.0-flash")
        prompt: "You are a data analysis expert."
        temperature: 0.7
    }
}

// Helper declaration
helper formatResults {
    input {
        raw: AnalysisResult
    }
    
    output {
        formatted: string
    }
    
    config {
        model: openai("gpt-4")
    }
}

// Workflow with control flow
workflow analyzeAndFormat {
    let analysis = analyze(input.data)
    
    if (analysis.confidence > 0.8) {
        return formatResults(analysis)
    } else {
        return "Low confidence result"
    }
}
```

---

## Improvements Over Previous Version

### Before
- All keywords in one color
- No distinction between types and values
- No highlighting for decorators
- No highlighting for property names
- No highlighting for function calls
- Basic string and comment support only

### After
✅ **Type declarations** highlighted distinctly  
✅ **Agent/Helper/Workflow** declarations stand out  
✅ **Decorators** (`@desc`) clearly visible  
✅ **Property names** distinguished from types  
✅ **Function calls** highlighted (especially providers)  
✅ **Type annotations** properly colored  
✅ **Operators** categorized (assignment, comparison, logical)  
✅ **Numbers and booleans** as constants  
✅ **Array types** (`[]`) highlighted  
✅ **Union types** (`|`) highlighted  
✅ **Optional markers** (`?`) highlighted  

---

## Testing

### Manual Testing

1. Open any `.agent` file in VSCode
2. Verify syntax highlighting for:
   - Type declarations (should be distinct)
   - Agent declarations (should stand out)
   - Property names (should be visible)
   - Decorators (should be highlighted)
   - Strings (should be colored)
   - Numbers (should be colored)
   - Comments (should be muted)

### Test File

Use `manual-testing/type-system-test.agent` as a comprehensive test file.

---

## Language Configuration Improvements

### Auto-Closing Pairs
- `{` → `}` (with smart context)
- `[` → `]`
- `(` → `)`
- `"` → `"` (not inside strings)
- `'` → `'` (not inside strings)

### Folding
- Region markers: `// #region` and `// #endregion`
- Block folding for `{}`, `[]`, `()`

### Indentation
- Auto-indent after `{`, `[`, `(`
- Auto-outdent after `}`, `]`, `)`

### On Enter Rules
- Smart comment continuation for `/** */` blocks
- Auto-indent for block structures

---

## Future Enhancements

### Semantic Highlighting
- Type checking integration
- Error highlighting
- Unused variable detection
- Type reference validation

### IntelliSense
- Auto-completion for keywords
- Type suggestions
- Property suggestions
- Snippet support

### Hover Information
- Type information on hover
- Documentation on hover
- Quick info for functions

---

## Files Modified

1. **`packages/extension/syntaxes/auwgent.tmLanguage.json`**
   - Complete rewrite with proper scopes
   - 14 syntax element categories
   - Proper TextMate grammar structure

2. **`packages/extension/language-configuration.json`**
   - Enhanced auto-closing pairs
   - Added folding markers
   - Added indentation rules
   - Added on-enter rules

3. **`docs/SYNTAX_HIGHLIGHTING_GUIDE.md`** (NEW)
   - Complete syntax highlighting guide
   - Color mapping by theme
   - Examples and testing guide

---

## Summary

The Auwgent DSL now has professional-grade syntax highlighting that:

✅ Works with all VSCode themes  
✅ Distinguishes all syntax elements  
✅ Provides clear visual hierarchy  
✅ Enhances code readability  
✅ Improves developer experience  

The highlighting is no longer "too basic" - it's now on par with major programming languages!
