# Requirements Document

## Introduction

This document specifies requirements for improving the developer experience of the Auwgent code generator by automatically embedding IR imports in generated TypeScript files. Currently, users must manually import both the IR JSON file and the types file to create an agent. This feature will eliminate the manual IR import step by embedding the import directly in the generated types file.

## Glossary

- **Auwgent**: A domain-specific language (DSL) for building AI agents
- **IR**: Intermediate Representation - the compiled JSON output from `.agent` files
- **Types_Generator**: The component responsible for generating TypeScript type definitions and factory functions from agent definitions
- **Agent_Factory**: The generated `createAgent` function that instantiates an agent with configuration
- **Config_Interface**: The TypeScript interface defining required parameters for the Agent_Factory
- **IR_JSON_File**: The `.agent.json` file containing the compiled intermediate representation
- **Types_File**: The `.agent.types.ts` file containing generated TypeScript interfaces and factory function

## Requirements

### Requirement 1: Automatic IR Import Generation

**User Story:** As a developer using Auwgent, I want the generated types file to automatically import the IR JSON file, so that I don't have to manually import it in my code.

#### Acceptance Criteria

1. WHEN the Types_Generator creates a Types_File, THE Types_Generator SHALL include an import statement for the sibling IR_JSON_File at the top of the file
2. WHEN generating the import statement, THE Types_Generator SHALL use a relative path that references the IR_JSON_File as a sibling file
3. WHEN importing the IR_JSON_File, THE Types_Generator SHALL cast the imported JSON to the AgentIR type
4. THE Types_Generator SHALL generate valid TypeScript import syntax compatible with the `resolveJsonModule` compiler option

### Requirement 2: Factory Configuration Simplification

**User Story:** As a developer using Auwgent, I want to create agents without passing the IR parameter, so that my code is simpler and less error-prone.

#### Acceptance Criteria

1. WHEN the Types_Generator creates a Config_Interface, THE Types_Generator SHALL exclude the `ir` property from the interface definition
2. WHEN the Agent_Factory is invoked, THE Agent_Factory SHALL use the automatically imported IR internally without requiring it as a parameter
3. WHEN the Agent_Factory validates configuration, THE Agent_Factory SHALL maintain all existing validation logic for non-IR properties
4. THE Agent_Factory SHALL function correctly for agents of any size without requiring changes to the IR_JSON_File format

### Requirement 3: Path Resolution Correctness

**User Story:** As a developer using Auwgent, I want the generated import paths to work correctly, so that my TypeScript compiler can resolve the IR file without errors.

#### Acceptance Criteria

1. WHEN the IR_JSON_File and Types_File are in the same directory, THE Types_Generator SHALL generate an import path using the format `./filename.agent.json`
2. WHEN TypeScript compiles the Types_File, THE TypeScript compiler SHALL successfully resolve the IR_JSON_File import without errors
3. THE Types_Generator SHALL generate import paths that work correctly regardless of the directory depth where the files are located

### Requirement 4: Backward Compatibility

**User Story:** As a developer maintaining Auwgent, I want the changes to maintain compatibility with existing code, so that the generator continues to work reliably.

#### Acceptance Criteria

1. WHEN the Types_Generator is modified, THE Types_Generator SHALL continue to generate valid TypeScript code for all existing agent definitions
2. WHEN the Agent_Factory is modified, THE Agent_Factory SHALL preserve all existing validation and initialization logic
3. WHEN processing IR files of varying sizes, THE Types_Generator SHALL handle both small and large IR_JSON_Files without performance degradation
