# Implementation Plan: Cross-File Referencing

## Overview

This implementation plan breaks down the cross-file referencing feature into discrete coding tasks. The approach follows Langium best practices for implementing cross-document references, including grammar extensions, scope computation, scope provider customization, and validation.

The implementation is organized into phases:
1. Grammar extensions for import/export syntax
2. URI resolution for import paths
3. Scope computation for collecting exports
4. Scope provider for resolving imported symbols
5. Validation for imports, exports, and circular dependencies
6. IDE features integration

## Tasks

- [x] 1. Extend grammar for import/export statements
  - [x] 1.1 Add Import statement grammar rules
    - Define `Import` rule with named and wildcard import support
    - Define `ImportSpecifier` rule for named imports with optional aliases
    - Define `WildcardImport` rule for namespace imports
    - Add import path as STRING terminal
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 16.1_
  
  - [x] 1.2 Add export keyword to existing element definitions
    - Modify `Helper` rule to support optional `export` keyword
    - Modify `TypeDeclaration` rule to support optional `export` keyword  
    - Modify `NamedPrompt` rule to support optional `export` keyword
    - Create `Exportable` union type for all exportable elements
    - _Requirements: 2.1, 2.2, 2.3, 2.4_
  
  - [x] 1.3 Update file structure grammar
    - Modify `AuwgentFile` rule to include imports array
    - Ensure imports appear before other elements
    - Update element ordering constraints
    - _Requirements: 1.5_
  
  - [x] 1.4 Regenerate AST from grammar
    - Run Langium generator to create updated AST types
    - Verify generated TypeScript interfaces match design
    - Update type guards for new AST nodes
    - _Requirements: All grammar-related requirements_

- [x] 2. Implement URI resolution
  - [x] 2.1 Create UriResolver service
    - Implement `resolveImportUri()` method
    - Handle relative paths starting with "./" and "../"
    - Automatically append ".agent" extension when missing
    - Handle explicit ".agent" extensions
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  
  - [x] 2.2 Add URI resolution caching
    - Create `ImportCacheManager` class
    - Implement cache for resolved URIs
    - Implement cache invalidation on file changes
    - _Requirements: 20.4, 20.5_
  
  - [x] 2.3 Add error handling for unresolved paths
    - Return undefined for unresolved paths
    - Log resolution attempts for debugging
    - _Requirements: 3.5_

- [x] 3. Implement scope computation
  - [x] 3.1 Create AuwgentScopeComputation service
    - Extend `DefaultScopeComputation` from Langium
    - Override `computeExports()` method
    - Collect all exportable elements from document
    - Filter for elements with `exported: true`
    - _Requirements: 2.5, 4.1_
  
  - [x] 3.2 Create AST node descriptions for exports
    - Use `descriptions.createDescription()` for each export
    - Include element name, type, and document reference
    - Return array of descriptions for IndexManager
    - _Requirements: 15.1, 15.2_
  
  - [x] 3.3 Register scope computation service
    - Add to Langium module configuration
    - Ensure it's called during document processing
    - _Requirements: 15.1_

- [x] 4. Implement scope provider
  - [x] 4.1 Create AuwgentScopeProvider service
    - Extend `DefaultScopeProvider` from Langium
    - Override `getScope()` method
    - Identify references to importable types
    - _Requirements: 4.1, 4.2, 4.3_
  
  - [x] 4.2 Implement local scope resolution
    - Collect symbols defined in current file
    - Create scope from local definitions
    - _Requirements: 4.5, 10.1, 10.2_
  
  - [x] 4.3 Implement imported scope resolution
    - Iterate through import statements in current file
    - Resolve import paths using UriResolver
    - Query IndexManager for exported symbols from target files
    - Handle named imports with original and aliased names
    - Handle wildcard imports with qualified names
    - _Requirements: 4.1, 4.2, 16.1, 16.2_
  
  - [x] 4.4 Combine local and imported scopes
    - Create combined scope with local precedence
    - Use Langium's scope composition utilities
    - _Requirements: 4.5_
  
  - [x] 4.5 Register scope provider service
    - Add to Langium module configuration
    - Ensure it's used for all reference resolution
    - _Requirements: All scope-related requirements_

- [x] 5. Implement validation
  - [x] 5.1 Create import statement validator
    - Validate import path resolution
    - Report errors for unresolved paths
    - Validate imported symbols exist in target file
    - Validate imported symbols are exported
    - Report available exports when symbol not found
    - _Requirements: 3.5, 9.1, 9.2, 9.3, 9.4, 9.5_
  
  - [x] 5.2 Create circular dependency detector
    - Implement depth-first search for cycle detection
    - Track visited files and recursion stack
    - Build dependency graph from imports
    - Report complete cycle path when detected
    - Handle self-imports as special case
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_
  
  - [x] 5.3 Create export dependency validator
    - Check if exported elements reference non-exported elements
    - Report warnings for incomplete public APIs
    - Allow suppression for specific cases
    - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5_
  
  - [x] 5.4 Create import ordering validator
    - Check that imports appear before other elements
    - Report errors for misplaced imports
    - _Requirements: 1.5_
  
  - [x] 5.5 Register all validators
    - Add validation checks to AuwgentValidator
    - Ensure all checks run during validation phase
    - _Requirements: All validation-related requirements_

- [x] 6. Implement IDE features
  - [x] 6.1 Add autocomplete for import paths
    - Provide file path suggestions based on workspace
    - Filter suggestions by .agent extension
    - Show relative paths from current file
    - _Requirements: 11.1_
  
  - [x] 6.2 Add autocomplete for imported symbols
    - Query exported symbols from target file
    - Show symbol names with type information
    - Support partial name matching
    - _Requirements: 11.2, 11.3, 11.4, 11.5_
  
  - [x] 6.3 Add go-to-definition support
    - Navigate to symbol definition in source file
    - Handle import statement clicks
    - Handle reference clicks
    - Open target file at correct location
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_
  
  - [x] 6.4 Add find references support
    - Find all import statements for a symbol
    - Find all usages in importing files
    - Group results by file
    - Update results on file changes
    - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5_
  
  - [x] 6.5 Add rename refactoring support
    - Update all imports when renaming exported symbol
    - Update all usages in importing files
    - Detect and prevent naming conflicts
    - Provide preview before applying changes
    - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5_
  
  - [x] 6.6 Add hover documentation support
    - Show symbol documentation on hover
    - Show file path and exports on import path hover
    - Format documentation with markdown
    - Show type information for symbols
    - _Requirements: 19.1, 19.2, 19.3, 19.4, 19.5_

- [x] 7. Implement workspace indexing
  - [x] 7.1 Configure IndexManager
    - Ensure IndexManager is enabled in Langium module
    - Configure index update triggers
    - _Requirements: 15.1_
  
  - [x] 7.2 Implement incremental indexing
    - Update index only for changed files
    - Invalidate dependent file caches
    - Track file dependencies for efficient updates
    - _Requirements: 15.2, 15.3, 15.4, 20.4_
  
  - [x] 7.3 Optimize index performance
    - Add caching for frequently accessed symbols
    - Batch index updates for multiple file changes
    - Implement lazy loading for large workspaces
    - _Requirements: 15.5, 20.1, 20.2, 20.3_

- [ ] 8. Add unit tests
  - [ ] 8.1 Test import statement parsing
    - Test named imports with single symbol
    - Test named imports with multiple symbols
    - Test named imports with aliases
    - Test wildcard imports
    - Test import path variations
    - _Requirements: 1.1, 1.2, 1.3, 16.1_
  
  - [ ] 8.2 Test export keyword parsing
    - Test exported helpers
    - Test exported types
    - Test exported prompts
    - Test non-exported elements
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_
  
  - [ ] 8.3 Test URI resolution
    - Test relative paths with "./"
    - Test relative paths with "../"
    - Test automatic extension appending
    - Test explicit extensions
    - Test unresolved paths
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_
  
  - [ ] 8.4 Test scope resolution
    - Test named import resolution
    - Test wildcard import resolution
    - Test import aliases
    - Test local definition shadowing
    - Test missing imports
    - _Requirements: 4.1, 4.2, 4.5, 16.1, 16.2_
  
  - [ ] 8.5 Test validation
    - Test unresolved import paths
    - Test non-existent symbols
    - Test non-exported symbols
    - Test circular dependencies
    - Test export dependencies
    - _Requirements: 8.1, 8.2, 8.3, 9.1, 9.2, 9.3, 17.1_
  
  - [ ] 8.6 Test backward compatibility
    - Test single-file agents without imports
    - Test non-exported elements in single files
    - Test mixed single-file and multi-file workspaces
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [ ] 9. Add property-based tests
  - [ ] 9.1 Property test: Import statement parsing
    - **Property 1: Import Statement Parsing**
    - Generate random valid import statements
    - Verify AST structure is correct
    - Run 100 iterations minimum
    - _Requirements: 1.1, 1.2, 1.3_
  
  - [ ] 9.2 Property test: Export keyword recognition
    - **Property 3: Export Keyword Recognition**
    - Generate random exportable elements with/without export
    - Verify exported property is set correctly
    - Run 100 iterations minimum
    - _Requirements: 2.1, 2.2, 2.3, 2.4_
  
  - [ ] 9.3 Property test: Path resolution
    - **Property 5: Relative Path Resolution**
    - Generate random relative paths
    - Verify resolution produces valid URIs
    - Run 100 iterations minimum
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  
  - [ ] 9.4 Property test: Symbol resolution
    - **Property 8: Named Import Symbol Resolution**
    - Generate random import scenarios
    - Verify symbols are resolvable
    - Run 100 iterations minimum
    - _Requirements: 4.1, 16.1, 16.2_
  
  - [ ] 9.5 Property test: Circular dependency detection
    - **Property 16: Circular Dependency Detection**
    - Generate random file dependency graphs
    - Verify cycles are detected correctly
    - Run 100 iterations minimum
    - _Requirements: 8.1, 8.2, 8.3_
  
  - [ ] 9.6 Property test: Backward compatibility
    - **Property 20: Backward Compatibility for Single-File Agents**
    - Generate random single-file agents
    - Verify they parse and validate correctly
    - Run 100 iterations minimum
    - _Requirements: 10.1, 10.2, 10.3_

- [ ] 10. Add integration tests
  - [ ] 10.1 Test multi-file agent scenarios
    - Create test workspace with multiple files
    - Test imports between files
    - Test nested imports (A imports B imports C)
    - Verify end-to-end functionality
    - _Requirements: All requirements_
  
  - [ ] 10.2 Test IDE features integration
    - Test autocomplete in VS Code extension
    - Test go-to-definition navigation
    - Test find references results
    - Test rename refactoring
    - _Requirements: 11, 12, 13, 14_
  
  - [ ] 10.3 Test performance with large workspaces
    - Create workspace with 1000+ files
    - Measure indexing time
    - Measure reference resolution time
    - Measure validation time
    - Verify performance requirements are met
    - _Requirements: 15.5, 20.1, 20.2, 20.3_

- [ ] 11. Update documentation
  - [ ] 11.1 Update language reference
    - Document import statement syntax
    - Document export keyword usage
    - Provide examples of cross-file references
    - _Requirements: All requirements_
  
  - [ ] 11.2 Create migration guide
    - Explain how to split single-file agents
    - Provide best practices for module organization
    - Show common patterns and anti-patterns
    - _Requirements: 10.4_
  
  - [ ] 11.3 Update README with examples
    - Add cross-file referencing examples
    - Show modular agent architecture
    - Demonstrate reusable components
    - _Requirements: All requirements_

## Notes

- All tasks are required for comprehensive implementation
- The implementation follows Langium best practices
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- Integration tests verify end-to-end functionality
- The implementation maintains backward compatibility with existing single-file agents

