# Enhanced Type System - Requirements

## Introduction

This specification defines enhancements to the Auwgent DSL type system to support reusable type definitions, nested type references, and rich field descriptions for LLM guidance. The enhancement enables developers to define types once and reuse them across agents, tools, and workflows, while providing detailed descriptions that help LLMs understand the expected structure of outputs.

## Glossary

- **Type Declaration**: A named, reusable type definition (e.g., `type Point { x: number, y: number }`)
- **Output Type**: A special type declaration with field descriptions for LLM guidance (e.g., `output type Result { ... }`)
- **Type Reference**: Using a defined type by name (e.g., `point: Point`)
- **Nested Type Reference**: A type that references another type (e.g., `type User { address: Address }`)
- **Field Description**: A `@desc` annotation that provides guidance to the LLM about a field's purpose
- **Inline Object Type**: An anonymous object type defined directly in place (e.g., `{ x: number, y: number }`)

## Requirements

### Requirement 1: Top-Level Type Declarations

**User Story:** As a developer, I want to define reusable types at the agent level, so that I don't have to repeat the same type definitions across multiple tools and fields.

#### Acceptance Criteria

1.1. WHEN a type is declared at the top level using `type Name { ... }`, THEN it SHALL be available for use throughout the agent definition

1.2. WHEN a type is declared, THEN it SHALL support all existing type primitives (string, number, boolean, arrays, unions, objects)

1.3. WHEN a type is declared, THEN it SHALL support optional fields using the `?` syntax

1.4. WHEN a type is declared, THEN it SHALL be collected during compilation and included in the IR

1.5. WHEN a type name conflicts with another type in the same file, THEN the compiler SHALL report an error

### Requirement 2: Output Types with Field Descriptions

**User Story:** As a developer, I want to define output types with field-level descriptions, so that the LLM understands the purpose and format of each field in the structured output.

#### Acceptance Criteria

2.1. WHEN an output type is declared using `output type Name { ... }`, THEN it SHALL support field descriptions using `@desc`

2.2. WHEN an output type field has a description, THEN the description SHALL be included in the generated JSON Schema

2.3. WHEN an output type contains nested inline objects, THEN those nested fields SHALL also support `@desc` annotations

2.4. WHEN an output type is used in an agent's output block, THEN all field descriptions SHALL be preserved in the final JSON Schema sent to the LLM

2.5. WHEN a regular `type` (not `output type`) is used, THEN field descriptions SHALL NOT be required (but MAY be supported for consistency)

### Requirement 3: Type References

**User Story:** As a developer, I want to reference defined types by name, so that I can reuse type definitions across input, output, context, and tool definitions.

#### Acceptance Criteria

3.1. WHEN a type is referenced by name in an input block, THEN the compiler SHALL resolve the reference to the type definition

3.2. WHEN a type is referenced by name in an output block, THEN the compiler SHALL resolve the reference to the type definition

3.3. WHEN a type is referenced by name in a tool parameter, THEN the compiler SHALL resolve the reference to the type definition

3.4. WHEN a type is referenced by name in a tool return type, THEN the compiler SHALL resolve the reference to the type definition

3.5. WHEN a type is referenced by name in a context block, THEN the compiler SHALL resolve the reference to the type definition

3.6. WHEN a type reference cannot be resolved, THEN the compiler SHALL report an error with the undefined type name

### Requirement 4: Nested Type References

**User Story:** As a developer, I want to define types that reference other types, so that I can build complex data structures from simpler components.

#### Acceptance Criteria

4.1. WHEN a type definition contains a field that references another type, THEN the compiler SHALL resolve the nested reference

4.2. WHEN nested type references are resolved, THEN the resolution SHALL be recursive (types can reference types that reference other types)

4.3. WHEN a circular type reference is detected (e.g., `type A { b: B }` and `type B { a: A }`), THEN the compiler SHALL report an error

4.4. WHEN nested types are used in JSON Schema generation, THEN the schema SHALL be properly expanded with all nested type definitions

### Requirement 5: Array and Union Type Descriptions

**User Story:** As a developer, I want to add descriptions to array and union types, so that the LLM understands the purpose of collections and enumerated values.

#### Acceptance Criteria

5.1. WHEN an array type is used with a description (e.g., `points: Point[] @desc "List of coordinates"`), THEN the description SHALL be included in the JSON Schema

5.2. WHEN a union type is used with a description (e.g., `status: "active" | "inactive" @desc "Current status"`), THEN the description SHALL be included in the JSON Schema

5.3. WHEN array or union types are used in output types, THEN their descriptions SHALL be preserved in the generated JSON Schema

### Requirement 6: Input Block Simplification

**User Story:** As a developer, I want input blocks to remain simple without descriptions, since inputs are user-facing and not LLM-facing.

#### Acceptance Criteria

6.1. WHEN a field is defined in an input block, THEN descriptions SHALL NOT be required

6.2. WHEN a field in an input block has a `@desc` annotation, THEN the compiler SHALL accept it but MAY ignore it in code generation

6.3. WHEN input types are processed, THEN they SHALL focus on type structure rather than LLM guidance

### Requirement 7: Backward Compatibility

**User Story:** As a developer with existing agent definitions, I want the type system enhancements to be backward compatible, so that my existing agents continue to work without modification.

#### Acceptance Criteria

7.1. WHEN an agent uses inline object types without type declarations, THEN it SHALL continue to work as before

7.2. WHEN an agent uses the existing `@desc` syntax on output fields, THEN it SHALL continue to work as before

7.3. WHEN an agent uses the existing tool definition syntax, THEN it SHALL continue to work as before

7.4. WHEN an agent mixes old and new syntax, THEN both SHALL work together seamlessly

### Requirement 8: IR Format Extension

**User Story:** As a runtime developer, I want the IR format to include type definitions, so that the loader can resolve type references and generate correct JSON Schemas.

#### Acceptance Criteria

8.1. WHEN types are defined in an agent, THEN the IR SHALL include a `types` object containing all type definitions

8.2. WHEN a type is an output type, THEN the IR SHALL include an `isOutput: true` flag

8.3. WHEN a field references a type, THEN the IR SHALL include a `typeRef` property with the type name

8.4. WHEN nested types are defined, THEN the IR SHALL preserve the nesting structure

8.5. WHEN field descriptions are present, THEN the IR SHALL include them in the type definition

### Requirement 9: JSON Schema Generation

**User Story:** As a runtime developer, I want the Synthesizer to generate correct JSON Schemas from type references, so that structured outputs work correctly with LLMs.

#### Acceptance Criteria

9.1. WHEN an output field references a type, THEN the Synthesizer SHALL resolve the type and generate the corresponding JSON Schema

9.2. WHEN a type contains nested type references, THEN the Synthesizer SHALL recursively resolve all references

9.3. WHEN field descriptions are present in output types, THEN they SHALL be included in the JSON Schema `description` property

9.4. WHEN array types have descriptions, THEN the description SHALL be applied to the array field in the JSON Schema

9.5. WHEN union types have descriptions, THEN the description SHALL be applied to the enum field in the JSON Schema

### Requirement 10: Error Handling

**User Story:** As a developer, I want clear error messages when I make mistakes with types, so that I can quickly fix issues.

#### Acceptance Criteria

10.1. WHEN a type reference cannot be resolved, THEN the compiler SHALL report "Type 'TypeName' is not defined"

10.2. WHEN a circular type reference is detected, THEN the compiler SHALL report "Circular type reference detected: A -> B -> A"

10.3. WHEN a type name conflicts, THEN the compiler SHALL report "Type 'TypeName' is already defined"

10.4. WHEN a type is used incorrectly, THEN the compiler SHALL provide a helpful error message with the location

## Non-Requirements

- Generic types (e.g., `type Result<T>`) are NOT required
- Type imports/exports across files are NOT required
- Type inheritance or extension is NOT required
- Runtime type validation is NOT required (compile-time only)
