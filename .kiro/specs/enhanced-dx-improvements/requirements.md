# Requirements Document

## Introduction

This document specifies two developer experience improvements to the Auwgent DSL: direct output type usage and multi-line string templates with interpolation. These features aim to reduce verbosity and improve code readability while maintaining backward compatibility.

## Glossary

- **Auwgent_DSL**: The domain-specific language for defining agents and their configurations
- **Output_Type**: A named type definition that describes the structure of agent output
- **Direct_Type_Usage**: Using a type reference directly without wrapping in an output block
- **Parser**: The component that reads and validates Auwgent DSL syntax
- **Code_Generator**: The component that transforms parsed DSL into executable code
- **String_Template**: A multi-line string literal with expression interpolation support
- **Interpolation_Expression**: An expression embedded within a string template using {{}} syntax
- **JSON_Schema**: The output format generated from Auwgent type definitions

## Requirements

### Requirement 1: Direct Output Type Usage

**User Story:** As a developer, I want to use output types directly without nesting, so that my agent definitions are more concise and readable.

#### Acceptance Criteria

1. WHEN an agent definition uses direct type syntax `output: TypeName`, THE Parser SHALL accept it as valid syntax
2. WHEN an agent definition uses traditional nested syntax `output { field: Type }`, THE Parser SHALL continue to accept it as valid syntax (backward compatibility)
3. WHEN the Code_Generator processes direct type usage, THE Code_Generator SHALL flatten the structure in the generated JSON_Schema
4. WHEN a direct type usage includes an @desc annotation, THE Parser SHALL accept and preserve the annotation
5. WHERE direct type usage is specified, THE Code_Generator SHALL generate the same JSON_Schema as if the type fields were defined inline without nesting
6. WHEN an inline type is used directly `output: { name: string }`, THE Parser SHALL accept it as valid syntax
7. WHEN a type reference is used directly `output: User`, THE Parser SHALL resolve the type reference correctly

### Requirement 2: Multi-line String Templates

**User Story:** As a developer, I want to write multi-line strings with expression interpolation, so that I can create readable prompts and text without verbose concatenation.

#### Acceptance Criteria

1. WHEN a string is delimited by triple quotes `"""`, THE Parser SHALL recognize it as a multi-line string template
2. WHEN a multi-line string template contains `{{expression}}`, THE Parser SHALL extract and validate the expression
3. WHEN an interpolation expression is evaluated, THE Code_Generator SHALL substitute the expression result into the string
4. WHEN a multi-line string template contains newlines and whitespace, THE Parser SHALL preserve them in the output
5. WHEN a multi-line string template is used in a prompt block, THE Code_Generator SHALL process it correctly
6. WHEN a multi-line string template is used in any string context, THE Parser SHALL accept it as a valid string literal
7. WHEN an interpolation expression contains nested property access `{{user.email}}`, THE Parser SHALL parse and validate the full expression
8. WHEN an interpolation expression contains function calls or operators, THE Parser SHALL validate them according to DSL expression rules
9. IF a multi-line string template contains invalid interpolation syntax, THEN THE Parser SHALL return a descriptive error message

### Requirement 3: Backward Compatibility

**User Story:** As a developer with existing Auwgent code, I want new features to work alongside existing syntax, so that I don't need to rewrite my codebase.

#### Acceptance Criteria

1. WHEN existing code uses nested output blocks, THE Parser SHALL continue to parse them correctly
2. WHEN existing code uses string concatenation with +, THE Parser SHALL continue to support it
3. WHEN a codebase mixes old and new syntax, THE Parser SHALL handle both correctly in the same file
4. WHEN the Code_Generator processes mixed syntax, THE Code_Generator SHALL produce correct output for both styles

### Requirement 4: Error Handling

**User Story:** As a developer, I want clear error messages when I make syntax mistakes, so that I can quickly fix issues.

#### Acceptance Criteria

1. IF a direct type reference cannot be resolved, THEN THE Parser SHALL return an error indicating the type name and location
2. IF an interpolation expression is malformed, THEN THE Parser SHALL return an error indicating the expression and location
3. IF an interpolation expression references an undefined variable, THEN THE Parser SHALL return an error during validation
4. IF triple quotes are not properly closed, THEN THE Parser SHALL return an error indicating the unclosed string
5. WHEN multiple syntax errors exist, THE Parser SHALL report all errors in a single pass where possible
