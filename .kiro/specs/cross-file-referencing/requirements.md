# Requirements Document: Cross-File Referencing in Auwgent DSL

## Introduction

This document specifies the requirements for implementing cross-file referencing capabilities in the Auwgent DSL. The feature enables modular agent development by allowing helpers, types, and prompts to be defined in separate files and imported where needed. This follows Langium best practices for language workbench frameworks and enables better code organization, reusability, and maintainability for complex agent systems.

## Glossary

- **Auwgent_DSL**: The domain-specific language for building AI agents with declarative syntax
- **Langium**: The language workbench framework used to implement the Auwgent DSL
- **Parser**: The component that reads Auwgent source files and builds an abstract syntax tree
- **ScopeProvider**: The component responsible for resolving references between AST nodes
- **IndexManager**: The Langium service that tracks symbols across multiple documents
- **Exportable_Element**: A DSL element that can be exported from a file (Helper, TypeDeclaration, NamedPrompt)
- **Import_Statement**: A DSL statement that declares dependencies on elements from other files
- **Export_Statement**: A DSL statement that makes elements available for import by other files
- **URI**: Uniform Resource Identifier used to uniquely identify files in the workspace
- **Qualified_Reference**: A reference that includes the source file or namespace information
- **Symbol**: A named entity in the DSL that can be referenced (helper, type, prompt, etc.)
- **Workspace**: The collection of all Auwgent files in a project
- **Language_Server**: The VS Code extension component that provides IDE features

## Requirements

### Requirement 1: Import Statement Syntax

**User Story:** As a developer, I want to import helpers, types, and prompts from other files, so that I can reuse components across my agent system.

#### Acceptance Criteria

1. WHEN a file contains an import statement with valid syntax, THE Parser SHALL parse it into an Import AST node
2. THE Auwgent_DSL SHALL support named imports using the syntax `import { Name1, Name2 } from "path"`
3. THE Auwgent_DSL SHALL support wildcard imports using the syntax `import * as Namespace from "path"`
4. WHEN multiple imports reference the same file, THE Parser SHALL allow multiple import statements for the same source file
5. THE Auwgent_DSL SHALL require import statements to appear before any agent, helper, type, or prompt definitions in a file

### Requirement 2: Export Statement Syntax

**User Story:** As a developer, I want to explicitly mark which elements can be imported by other files, so that I have control over my module's public API.

#### Acceptance Criteria

1. WHEN a helper, type, or prompt is prefixed with the `export` keyword, THE Parser SHALL mark it as an Exportable_Element
2. THE Auwgent_DSL SHALL support the syntax `export helper HelperName { ... }`
3. THE Auwgent_DSL SHALL support the syntax `export type TypeName { ... }`
4. THE Auwgent_DSL SHALL support the syntax `export prompt PromptName { ... }`
5. WHEN an element is not marked with `export`, THE ScopeProvider SHALL prevent it from being imported by other files

### Requirement 3: File Path Resolution

**User Story:** As a developer, I want to use relative file paths in import statements, so that my project structure remains flexible and portable.

#### Acceptance Criteria

1. WHEN an import path starts with "./", THE URI resolver SHALL resolve it relative to the importing file's directory
2. WHEN an import path starts with "../", THE URI resolver SHALL resolve it relative to the parent directory of the importing file
3. WHEN an import path does not include a file extension, THE URI resolver SHALL automatically append ".agent" extension
4. WHEN an import path includes the ".agent" extension explicitly, THE URI resolver SHALL use it as provided
5. WHEN an import path cannot be resolved to an existing file, THE Parser SHALL report an error with the unresolved path

### Requirement 4: Symbol Resolution and Scoping

**User Story:** As a developer, I want imported symbols to be available for use in my file, so that I can reference helpers, types, and prompts from other modules.

#### Acceptance Criteria

1. WHEN a symbol is imported via named import, THE ScopeProvider SHALL make it available for reference using its imported name
2. WHEN symbols are imported via wildcard import, THE ScopeProvider SHALL make them available for reference using qualified names (Namespace.SymbolName)
3. WHEN a reference cannot be resolved, THE ScopeProvider SHALL check imported symbols before reporting an error
4. WHEN multiple files export symbols with the same name, THE ScopeProvider SHALL allow importing both if they use different import statements
5. WHEN a local definition has the same name as an imported symbol, THE ScopeProvider SHALL prioritize the local definition

### Requirement 5: Cross-File Type Safety

**User Story:** As a developer, I want type definitions to work correctly across file boundaries, so that my agent system maintains type safety.

#### Acceptance Criteria

1. WHEN a type is imported and used in a tool signature, THE Parser SHALL validate that the type structure matches the exported definition
2. WHEN a type is imported and used in an input/output declaration, THE Parser SHALL validate field types against the imported type definition
3. WHEN an imported type references other types, THE Parser SHALL resolve those nested type references correctly
4. WHEN a type definition changes in the source file, THE Language_Server SHALL update validation in all importing files
5. WHEN a type is used without being imported, THE Parser SHALL report a clear error indicating the missing import

### Requirement 6: Helper Cross-File References

**User Story:** As a developer, I want to import helpers from other files and use them in my agents, so that I can build modular agent architectures.

#### Acceptance Criteria

1. WHEN a helper is imported, THE ScopeProvider SHALL make it available for reference in the `helpers { }` block of agents
2. WHEN an imported helper is referenced, THE Parser SHALL validate that the helper exists in the source file
3. WHEN a helper's interface changes in the source file, THE Language_Server SHALL update validation in all importing files
4. WHEN a helper is used without being imported, THE Parser SHALL report an error indicating the missing import
5. WHEN a helper is imported but not used, THE Language_Server SHALL optionally warn about the unused import

### Requirement 7: Named Prompt Cross-File References

**User Story:** As a developer, I want to import named prompts from other files and use them in agent configurations, so that I can share common prompts across agents.

#### Acceptance Criteria

1. WHEN a named prompt is imported, THE ScopeProvider SHALL make it available for reference in config blocks
2. WHEN an imported prompt is referenced, THE Parser SHALL validate that the prompt exists in the source file
3. WHEN a prompt definition changes in the source file, THE Language_Server SHALL update validation in all importing files
4. WHEN a prompt is used without being imported, THE Parser SHALL report an error indicating the missing import
5. WHEN a prompt is imported but not used, THE Language_Server SHALL optionally warn about the unused import

### Requirement 8: Circular Dependency Detection

**User Story:** As a developer, I want to be notified when circular dependencies exist, so that I can restructure my code to avoid runtime issues.

#### Acceptance Criteria

1. WHEN file A imports from file B and file B imports from file A, THE Parser SHALL detect the circular dependency
2. WHEN a circular dependency is detected, THE Parser SHALL report an error listing all files in the dependency cycle
3. WHEN a circular dependency involves more than two files, THE Parser SHALL detect and report the complete cycle
4. WHEN a file imports itself directly, THE Parser SHALL report a self-reference error
5. THE Parser SHALL perform circular dependency detection during the validation phase after all files are parsed

### Requirement 9: Validation and Error Reporting

**User Story:** As a developer, I want clear error messages when imports fail, so that I can quickly identify and fix issues.

#### Acceptance Criteria

1. WHEN an import path cannot be resolved, THE Parser SHALL report an error with the file path and the importing file location
2. WHEN an imported symbol does not exist in the target file, THE Parser SHALL report an error listing available exported symbols
3. WHEN an imported symbol is not exported, THE Parser SHALL report an error indicating the symbol exists but is not exported
4. WHEN import syntax is malformed, THE Parser SHALL report a syntax error with the expected format
5. WHEN a file has multiple errors, THE Parser SHALL report all import-related errors in a single validation pass

### Requirement 10: Backward Compatibility

**User Story:** As a developer with existing single-file agents, I want them to continue working without modification, so that I can adopt cross-file referencing incrementally.

#### Acceptance Criteria

1. WHEN a file contains no import statements, THE Parser SHALL process it using the existing single-file semantics
2. WHEN a file defines helpers, types, or prompts without export keywords, THE ScopeProvider SHALL make them available within the same file
3. WHEN a workspace contains both single-file and multi-file agents, THE Parser SHALL handle both correctly
4. WHEN migrating from single-file to multi-file, THE Parser SHALL allow gradual addition of export keywords without breaking existing code
5. THE Parser SHALL maintain all existing validation rules for single-file agents

### Requirement 11: IDE Autocomplete Support

**User Story:** As a developer, I want autocomplete suggestions for imported symbols, so that I can write code faster with fewer errors.

#### Acceptance Criteria

1. WHEN typing in an import statement, THE Language_Server SHALL suggest available file paths based on the workspace structure
2. WHEN typing inside import braces, THE Language_Server SHALL suggest exported symbols from the target file
3. WHEN referencing a symbol in code, THE Language_Server SHALL include imported symbols in autocomplete suggestions
4. WHEN using a wildcard import, THE Language_Server SHALL suggest qualified names (Namespace.Symbol) in autocomplete
5. WHEN an import is incomplete, THE Language_Server SHALL provide autocomplete for partially typed symbol names

### Requirement 12: IDE Go-to-Definition Support

**User Story:** As a developer, I want to navigate to the definition of imported symbols, so that I can understand their implementation quickly.

#### Acceptance Criteria

1. WHEN clicking on an imported symbol name in an import statement, THE Language_Server SHALL navigate to the symbol's definition in the source file
2. WHEN clicking on a reference to an imported symbol, THE Language_Server SHALL navigate to the symbol's definition in the source file
3. WHEN using keyboard shortcuts for go-to-definition, THE Language_Server SHALL navigate to the correct file and location
4. WHEN a symbol is defined locally and imported, THE Language_Server SHALL navigate to the local definition when referenced
5. WHEN a file path in an import statement is clicked, THE Language_Server SHALL open the target file

### Requirement 13: IDE Find References Support

**User Story:** As a developer, I want to find all usages of an exported symbol across my workspace, so that I can understand its impact when making changes.

#### Acceptance Criteria

1. WHEN requesting "find references" on an exported symbol, THE Language_Server SHALL list all import statements that import it
2. WHEN requesting "find references" on an exported symbol, THE Language_Server SHALL list all locations where it is used in importing files
3. WHEN requesting "find references" on an import statement, THE Language_Server SHALL list all usages of that imported symbol in the current file
4. THE Language_Server SHALL display find references results grouped by file
5. THE Language_Server SHALL update find references results when files are modified

### Requirement 14: IDE Rename Refactoring Support

**User Story:** As a developer, I want to rename exported symbols and have all imports updated automatically, so that I can refactor code safely.

#### Acceptance Criteria

1. WHEN renaming an exported symbol, THE Language_Server SHALL update all import statements that reference it
2. WHEN renaming an exported symbol, THE Language_Server SHALL update all usages in importing files
3. WHEN renaming an imported symbol in an import statement, THE Language_Server SHALL update all usages in the current file only
4. WHEN a rename would create a naming conflict, THE Language_Server SHALL prevent the rename and show an error
5. THE Language_Server SHALL provide a preview of all changes before applying a rename refactoring

### Requirement 15: Workspace Indexing

**User Story:** As a developer, I want the language server to track all exported symbols across my workspace, so that IDE features work efficiently even in large projects.

#### Acceptance Criteria

1. WHEN a workspace is opened, THE IndexManager SHALL scan all .agent files and index exported symbols
2. WHEN a file is modified, THE IndexManager SHALL update the index for that file and dependent files
3. WHEN a file is added to the workspace, THE IndexManager SHALL index its exported symbols
4. WHEN a file is deleted from the workspace, THE IndexManager SHALL remove its symbols from the index and report errors in dependent files
5. THE IndexManager SHALL complete initial indexing within 5 seconds for workspaces with up to 1000 files

### Requirement 16: Import Alias Support

**User Story:** As a developer, I want to import symbols with different names to avoid conflicts, so that I can use multiple symbols with the same name from different files.

#### Acceptance Criteria

1. THE Auwgent_DSL SHALL support import aliases using the syntax `import { OriginalName as AliasName } from "path"`
2. WHEN a symbol is imported with an alias, THE ScopeProvider SHALL make it available using the alias name only
3. WHEN multiple symbols are imported with aliases, THE Parser SHALL allow each to have a unique alias
4. WHEN an alias conflicts with a local definition, THE Parser SHALL report an error
5. THE Language_Server SHALL show the original name in hover tooltips for aliased imports

### Requirement 17: Export Validation

**User Story:** As a developer, I want to be notified if I export elements that have dependencies on non-exported elements, so that I can ensure my module's API is complete.

#### Acceptance Criteria

1. WHEN an exported helper references a non-exported type in its interface, THE Parser SHALL report a warning
2. WHEN an exported type references a non-exported type in its definition, THE Parser SHALL report a warning
3. WHEN an exported prompt references a non-exported helper, THE Parser SHALL report a warning
4. THE Parser SHALL allow exporting elements that reference built-in types without warnings
5. THE Parser SHALL provide an option to suppress export validation warnings for specific cases

### Requirement 18: Import Organization

**User Story:** As a developer, I want my imports to be organized consistently, so that my code is readable and maintainable.

#### Acceptance Criteria

1. THE Language_Server SHALL provide a "organize imports" command that sorts imports alphabetically by file path
2. WHEN organizing imports, THE Language_Server SHALL remove unused imports
3. WHEN organizing imports, THE Language_Server SHALL group imports by directory structure
4. WHEN organizing imports, THE Language_Server SHALL maintain the relative order of imports from the same file
5. THE Language_Server SHALL provide an option to automatically organize imports on file save

### Requirement 19: Documentation and Hover Support

**User Story:** As a developer, I want to see documentation for imported symbols when I hover over them, so that I can understand their purpose without navigating to the definition.

#### Acceptance Criteria

1. WHEN hovering over an imported symbol in an import statement, THE Language_Server SHALL display the symbol's documentation from the source file
2. WHEN hovering over a reference to an imported symbol, THE Language_Server SHALL display the symbol's documentation and type information
3. WHEN hovering over a file path in an import statement, THE Language_Server SHALL display the file's path and available exports
4. THE Language_Server SHALL format documentation using markdown for rich display
5. WHEN a symbol has no documentation, THE Language_Server SHALL display its signature or type information

### Requirement 20: Performance and Scalability

**User Story:** As a developer working on large projects, I want cross-file referencing to perform efficiently, so that my IDE remains responsive.

#### Acceptance Criteria

1. WHEN resolving a reference, THE ScopeProvider SHALL complete the lookup within 100ms for workspaces with up to 1000 files
2. WHEN validating a file with imports, THE Parser SHALL complete validation within 500ms for files with up to 50 imports
3. WHEN indexing the workspace, THE IndexManager SHALL process at least 100 files per second
4. THE IndexManager SHALL use incremental indexing to avoid re-indexing unchanged files
5. THE Language_Server SHALL cache resolved imports to avoid redundant file system operations
