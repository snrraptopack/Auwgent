# Implementation Plan: IR Auto-Import

## Overview

This implementation plan breaks down the IR auto-import feature into discrete coding tasks. The approach is to modify the types generator to automatically embed IR imports in generated TypeScript files, eliminating the need for users to manually import IR JSON files.

The implementation focuses on two key changes:
1. Adding an import statement for the IR JSON file in `generateTypesFile()`
2. Removing the `ir` property from the config interface in `generateAgentFactory()`

## Tasks

- [-] 1. Modify `generateTypesFile()` to add IR import statement
  - [x] 1.1 Add IR import statement generation after runtime imports
    - Generate import statement using format: `import agentIR from './${agent.name}.agent.json' assert { type: 'json' };`
    - Insert the import statement in the sections array after the runtime imports and before custom types
    - Ensure proper spacing with blank lines
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 3.1_
  
  - [ ] 1.2 Write property test for import statement generation
    - **Property 1: Import Statement Generation**
    - **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 3.1**
    - Generate random AgentIR objects with varying names
    - Verify output contains import statement with correct format
    - Verify import path uses agent name correctly
    - Run 100 iterations minimum

- [-] 2. Modify `generateAgentFactory()` to remove IR from config and use imported IR
  - [x] 2.1 Remove `ir` property from config interface generation
    - Remove the line that adds `ir: AgentIR` to configProps array
    - Ensure other properties (apiKeys, context, tools, lifecycle) are still added correctly
    - _Requirements: 2.1_
  
  - [x] 2.2 Update factory to use imported `agentIR` instead of `config.ir`
    - Change `agent.load(config.ir)` to `agent.load(agentIR)`
    - Update all validation checks to reference `agentIR` instead of `config.ir`
    - Update tool validation to use `agentIR.tools` and `agentIR.workflows`
    - Update lifecycle validation to use `agentIR.lifecycle`
    - _Requirements: 2.2, 2.3_
  
  - [ ] 2.3 Write property test for config interface changes
    - **Property 2: Config Interface Excludes IR Property**
    - **Validates: Requirements 2.1, 2.2**
    - Generate random AgentIR objects
    - Verify config interface does NOT contain `ir` property
    - Verify factory references `agentIR` not `config.ir`
    - Run 100 iterations minimum
  
  - [ ] 2.4 Write property test for validation logic preservation
    - **Property 3: Validation Logic Preservation**
    - **Validates: Requirements 2.3, 4.2**
    - Generate random AgentIR objects with tools and/or lifecycle
    - Verify validation code references `agentIR.tools`, `agentIR.workflows`, `agentIR.lifecycle`
    - Verify validation code does NOT reference `config.ir`
    - Run 100 iterations minimum

- [ ] 3. Update JSDoc examples in factory function
  - [x] 3.1 Remove `ir` parameter from JSDoc examples
    - Update the `@example` block in the factory function JSDoc
    - Remove `ir: agentIR` line from example code
    - Ensure example still shows correct usage with apiKeys, context, tools, lifecycle
    - _Requirements: 2.1_

- [x] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Add comprehensive property-based tests
  - [ ] 5.1 Write property test for backward compatibility
    - **Property 4: Backward Compatibility Across Agent Configurations**
    - **Validates: Requirements 2.4, 4.1**
    - Generate random AgentIR objects with various feature combinations
    - Verify generated code is syntactically valid TypeScript
    - Verify all expected config properties are present
    - Verify factory loads IR using `agent.load(agentIR)`
    - Run 100 iterations minimum
  
  - [ ] 5.2 Write integration test for TypeScript compilation
    - **Property 5: Generated Code Compilation**
    - **Validates: Requirements 3.2**
    - Generate random AgentIR objects
    - Write generated TypeScript and JSON files to temporary directory
    - Run TypeScript compiler with `resolveJsonModule: true`
    - Verify compilation succeeds without errors
    - Run 100 iterations minimum

- [ ] 6. Add unit tests for edge cases
  - [ ] 6.1 Write unit tests for agent name variations
    - Test agent names with hyphens (e.g., "my-agent")
    - Test agent names with underscores (e.g., "my_agent")
    - Test agent names with numbers (e.g., "agent123")
    - _Requirements: 1.2, 3.1_
  
  - [ ] 6.2 Write unit tests for feature combinations
    - Test minimal agent (no tools, no context, no lifecycle)
    - Test maximal agent (all features enabled)
    - Test agent with only tools
    - Test agent with only context
    - Test agent with only lifecycle
    - _Requirements: 4.1_
  
  - [ ] 6.3 Write unit tests for validation logic
    - Test that missing tools trigger validation errors
    - Test that missing lifecycle hooks trigger validation errors
    - Verify error messages are preserved
    - _Requirements: 2.3, 4.2_

- [ ] 7. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- All tasks are required for comprehensive implementation
- The core implementation is in tasks 1-3, which modify the generator code
- Property tests validate universal correctness properties across randomized inputs
- Unit tests validate specific examples and edge cases
- Each task references specific requirements for traceability
- The implementation maintains backward compatibility with existing validation logic
