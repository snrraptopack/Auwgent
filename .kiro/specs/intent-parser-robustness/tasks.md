# Implementation Plan

## Phase 1: Exploration Tests (Write BEFORE Fix)

- [x] 1. Write bug condition exploration tests
  - **Property 1: Bug Condition** - Intent Parser Robustness Bugs
  - **CRITICAL**: These tests MUST FAIL on unfixed code - failure confirms the bugs exist
  - **DO NOT attempt to fix the tests or the code when they fail**
  - **NOTE**: These tests encode the expected behavior - they will validate the fix when they pass after implementation
  - **GOAL**: Surface counterexamples that demonstrate the bugs exist
  - **Scoped PBT Approach**: For deterministic bugs, scope properties to concrete failing cases to ensure reproducibility

  - [x] 1.1 Streaming boundary bug test
    - Test that pipe block `"response_schema:\n  response: |\n    Ghana gained independence..."` split into 5-byte chunks parses completely without data loss
    - Run test on UNFIXED code
    - **EXPECTED OUTCOME**: Test FAILS (parser loses content or terminates early)
    - Document counterexamples found (e.g., "content after pipe block is lost when chunk boundary splits the header")
    - _Requirements: 1.1, 1.2, 2.1, 2.2_

  - [x] 1.2 Error reporting bug test
    - Test that parsing `"key: value\n  invalid\nkey2: value2"` produces errors with line, column, and context
    - Run test on UNFIXED code
    - **EXPECTED OUTCOME**: Test FAILS (errors empty or lack context)
    - Document counterexamples found (e.g., "ParseResult.errors is empty despite invalid syntax")
    - _Requirements: 1.3, 1.4, 2.3, 2.4_

  - [x] 1.3 Memory efficiency bug test
    - Test that parsing 10,000 tokens in streaming mode keeps token buffer bounded (<= 100 tokens)
    - Run test on UNFIXED code
    - **EXPECTED OUTCOME**: Test FAILS (token buffer grows to 10,000+ entries)
    - Document counterexamples found (e.g., "token buffer size = 10,000 after processing 10,000 tokens")
    - _Requirements: 1.5, 1.6, 1.7, 2.5, 2.6, 2.7_

  - [x] 1.4 YAML features bug test
    - Test that folded scalar `"description: >\n  This is\n  folded."` produces "This is folded."
    - Test that chomping indicator `"text: |-\n  No trailing newline"` strips trailing newline
    - Run tests on UNFIXED code
    - **EXPECTED OUTCOME**: Tests FAIL (folded scalar produces literal ">", chomping ignored)
    - Document counterexamples found (e.g., "folded scalar returns '>' instead of folded text")
    - _Requirements: 1.8, 1.9, 2.8, 2.9_

  - [x] 1.5 Type coercion bug test
    - Test that `"value: NaN"` coerces to f64::NAN
    - Test that `"value: Infinity"` coerces to f64::INFINITY
    - Test that `"enabled: yes"` coerces to true
    - Test that `"value: 1e"` falls back to string gracefully
    - Test that `"data: {a: {b: {c: {d: {e: \"value\"}}}}}"` parses correctly
    - Test that `"data: {key: \"escaped\\\"quote\"}"` parses correctly
    - Run tests on UNFIXED code
    - **EXPECTED OUTCOME**: Tests FAIL (special floats fall back to string, YAML 1.1 booleans not recognized, incomplete scientific notation panics, nested flow breaks, escaped quotes break)
    - Document counterexamples found (e.g., "NaN coerces to string instead of f64::NAN")
    - _Requirements: 1.10, 1.11, 1.12, 2.10, 2.11, 2.12, 2.13_

  - [x] 1.6 Glue heuristic bug test
    - Test that parsing `"intent1:\n  text: Hello\nmetadata: value\nintent2:\n  text: World"` with both intents registered preserves metadata as separate key
    - Run test on UNFIXED code
    - **EXPECTED OUTCOME**: Test FAILS (metadata merged into intent1)
    - Document counterexamples found (e.g., "metadata key incorrectly merged into intent1.text")
    - _Requirements: 1.13, 1.14, 2.14_

## Phase 2: Preservation Tests (Write BEFORE Fix)

- [x] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** - Existing Parser Behavior
  - **IMPORTANT**: Follow observation-first methodology
  - Observe behavior on UNFIXED code for non-buggy inputs
  - Write property-based tests capturing observed behavior patterns from Preservation Requirements
  - Property-based testing generates many test cases for stronger guarantees
  - Run tests on UNFIXED code
  - **EXPECTED OUTCOME**: Tests PASS (this confirms baseline behavior to preserve)
  - Mark task complete when tests are written, run, and passing on unfixed code

  - [x] 2.1 Normal pipe block preservation test
    - Observe: `"text: |\n  Line 1\n  Line 2"` with 50-byte chunks parses correctly on unfixed code
    - Write property-based test: for all pipe blocks with normal chunk sizes (>50 bytes), parsing produces correct multiline string
    - Verify test passes on UNFIXED code
    - _Requirements: 3.1_

  - [x] 2.2 Simple key-value preservation test
    - Observe: `"key: value\nkey2: value2"` produces correct AST on unfixed code
    - Write property-based test: for all simple key-value pairs, parsing produces correct AST structure
    - Verify test passes on UNFIXED code
    - _Requirements: 3.2_

  - [x] 2.3 Nested structure preservation test
    - Observe: `"parent:\n  child:\n    - item1\n    - item2"` produces correct nested AST on unfixed code
    - Write property-based test: for all nested mappings and sequences, parsing produces correct AST structure
    - Verify test passes on UNFIXED code
    - _Requirements: 3.3_

  - [x] 2.4 Quoted string preservation test
    - Observe: `"text: \"escaped\\nstring\""` produces correct escaped string on unfixed code
    - Write property-based test: for all quoted strings with escape sequences, parsing handles escapes correctly
    - Verify test passes on UNFIXED code
    - _Requirements: 3.4_

  - [x] 2.5 Standard type coercion preservation test
    - Observe: `"enabled: true"`, `"int: 42"`, `"float: 3.14"`, `"sci: 1.5e10"` coerce correctly on unfixed code
    - Write property-based test: for all standard boolean and number formats, type coercion produces correct types
    - Verify test passes on UNFIXED code
    - _Requirements: 3.6, 3.7_

  - [x] 2.6 Intent detection preservation test
    - Observe: `"intent1:\n  text: Hello"` with intent1 registered emits intent_ready event on unfixed code
    - Write property-based test: for all registered intent keys, orchestrator emits intent_ready events correctly
    - Verify test passes on UNFIXED code
    - _Requirements: 3.8_

  - [x] 2.7 Streaming parsing preservation test
    - Observe: Streaming input with normal chunk sizes (>50 bytes) provides partial results on unfixed code
    - Write property-based test: for all streaming input with normal chunk sizes, parser provides partial results and handles incremental parsing
    - Verify test passes on UNFIXED code
    - _Requirements: 3.9_

  - [x] 2.8 Indentation tracking preservation test
    - Observe: Indent/dedent tokens track indentation levels correctly on unfixed code
    - Write property-based test: for all block structures, parser tracks indentation levels correctly
    - Verify test passes on UNFIXED code
    - _Requirements: 3.11_

  - [x] 2.9 EOF handling preservation test
    - Observe: Parser emits remaining DEDENT tokens and finalizes AST correctly at EOF on unfixed code
    - Write property-based test: for all inputs ending with nested blocks, parser emits DEDENT tokens and finalizes AST correctly
    - Verify test passes on UNFIXED code
    - _Requirements: 3.12_

## Phase 3: Implementation

- [x] 3. Fix for intent parser robustness bugs

  - [x] 3.1 Fix streaming boundary bugs in tokenizer
    - In `tokenizer.rs`, function `tokenize_multiline_string()`:
      - Save `after_colon` state before advancing: `let start_after_colon = self.state.after_colon;`
      - Restore `after_colon` in all rewind paths: `self.state.after_colon = start_after_colon;`
    - In `tokenizer.rs`, function `tokenize_content()`:
      - Add check for newline after pipe: if at chunk boundary and no newline seen, rewind to start of pipe block
    - _Bug_Condition: isBugCondition_StreamingBoundary(input) where input.contains_pipe_block AND input.chunk_size < 20 AND chunk_boundary_splits_pipe_header_
    - _Expected_Behavior: Parser correctly rewinds all state variables and waits for more data, eventually parsing complete multiline string_
    - _Preservation: Normal pipe blocks with chunk sizes >50 bytes continue to parse correctly_
    - _Requirements: 1.1, 1.2, 2.1, 2.2, 3.1_

  - [x] 3.2 Fix error reporting in parser
    - In `parser.rs`, add `input_lines: Vec<String>` field to `Parser` struct
    - In `write()` method, split chunk by newlines and append to `input_lines`
    - When creating `ParseError`, populate `context` field with `self.input_lines.get(error_line - 1).cloned()`
    - In `parse_tokens()`, add logging: `for error in &self.errors { eprintln!("Parse error at {}:{}: {}", error.line, error.column, error.message); }`
    - Ensure `ParseResult.errors` is populated in all error paths
    - _Bug_Condition: isBugCondition_ErrorReporting(input) where input.has_parse_error AND error_message_lacks_line_column_
    - _Expected_Behavior: Error messages include line number, column number, and source context_
    - _Preservation: Existing parsing behavior for valid YAML unchanged_
    - _Requirements: 1.3, 1.4, 2.3, 2.4_

  - [x] 3.3 Fix memory efficiency in parser
    - In `parser.rs`, function `parse_tokens()`:
      - After processing tokens, add: `if self.pos > 100 { self.tokens.drain(0..self.pos - 100); self.pos = 100; }`
    - In `parser.rs`, function `to_ast_node()`:
      - Change to return reference `&ASTNode` where possible, or use `std::mem::take()` to move out of mutable references
    - In `parser.rs`, function `pop_frame()`:
      - Change to move nodes instead of cloning: `let child_node = match frame.node { FrameNode::Mapping(m) => ASTNode::Mapping(m), FrameNode::Sequence(s) => ASTNode::Sequence(s) };`
    - _Bug_Condition: isBugCondition_MemoryEfficiency(input) where input.token_count > 1000 AND tokens_not_cleaned_up_
    - _Expected_Behavior: Token buffer stays bounded (<= 100 tokens), minimal cloning_
    - _Preservation: Existing parsing behavior and performance for typical inputs unchanged_
    - _Requirements: 1.5, 1.6, 1.7, 2.5, 2.6, 2.7_

  - [x] 3.4 Add YAML feature support in tokenizer
    - In `tokenizer.rs`, function `tokenize_content()`:
      - Add branch: `if char == '>' { return self.tokenize_folded_string(); }`
    - Implement `tokenize_folded_string()` similar to `tokenize_multiline_string()` but joining lines with spaces
    - In both `tokenize_multiline_string()` and `tokenize_folded_string()`:
      - After consuming `|` or `>`, check for chomping indicators: `let chomping = match self.peek_char(0) { '-' => Chomping::Strip, '+' => Chomping::Keep, _ => Chomping::Clip };`
      - Apply chomping rules when building final value
    - _Bug_Condition: isBugCondition_YAMLFeatures(input) where input.contains_folded_scalar_indicator OR input.contains_chomping_indicator_
    - _Expected_Behavior: Folded scalars fold lines with spaces, chomping indicators control trailing newlines_
    - _Preservation: Existing pipe block parsing unchanged_
    - _Requirements: 1.8, 1.9, 2.8, 2.9, 3.1_

  - [x] 3.5 Fix type coercion edge cases in builder
    - In `builder.rs`, function `coerce_value()`:
      - Before `parse::<f64>()`, add special float checks:
        ```rust
        if trimmed.eq_ignore_ascii_case("nan") { return IRValue::Number(f64::NAN); }
        if trimmed.eq_ignore_ascii_case("inf") || trimmed.eq_ignore_ascii_case("infinity") { return IRValue::Number(f64::INFINITY); }
        if trimmed.eq_ignore_ascii_case("-inf") || trimmed.eq_ignore_ascii_case("-infinity") { return IRValue::Number(f64::NEG_INFINITY); }
        ```
      - After "true"/"false" checks, add YAML 1.1 boolean handling:
        ```rust
        if trimmed.eq_ignore_ascii_case("yes") || trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("on") { return IRValue::Boolean(true); }
        if trimmed.eq_ignore_ascii_case("no") || trimmed.eq_ignore_ascii_case("n") || trimmed.eq_ignore_ascii_case("off") { return IRValue::Boolean(false); }
        ```
      - Wrap `parse::<f64>()` in check: if fails and string matches `/^-?\d+\.?\d*e[+-]?$/`, return `IRValue::String(value.to_string())`
    - In `builder.rs`, function `parse_yaml_flow_object()`:
      - Add `depth` parameter (default 0, max 10)
      - When recursing for nested objects, pass `depth + 1` and return `None` if `depth > 10`
      - Improve escape handling: properly handle `\n`, `\t`, `\r`, `\\`, `\"`, `\'`, `\uXXXX`
    - _Bug_Condition: isBugCondition_TypeCoercion(input) where is_special_float(input) OR is_incomplete_scientific(input) OR is_yaml11_boolean(input) OR is_deeply_nested_flow_collection(input)_
    - _Expected_Behavior: Special floats coerce correctly, YAML 1.1 booleans recognized, incomplete scientific notation falls back gracefully, deeply nested flow collections parse correctly_
    - _Preservation: Standard boolean and number coercion unchanged_
    - _Requirements: 1.10, 1.11, 1.12, 2.10, 2.11, 2.12, 2.13, 3.6, 3.7_

  - [x] 3.6 Fix glue heuristic in orchestrator
    - In `orchestrator.rs`, add `enable_glue_heuristic: bool` field to `ParserOptions` (default `true`)
    - Add `pub fn set_glue_heuristic(&mut self, enabled: bool)` method to `Orchestrator`
    - In `process_mapping_intents()`, wrap glue logic in `if self.options.enable_glue_heuristic { ... }`
    - Improve glue heuristic: only merge keys at same indent level that immediately follow intent (no blank lines)
    - In `types.rs`, add `pub enable_glue_heuristic: Option<bool>` to `ParserOptions` struct with default `Some(true)`
    - _Bug_Condition: isBugCondition_GlueHeuristic(input) where is_malformed_multiline_without_pipe(input) AND glue_heuristic_too_aggressive_
    - _Expected_Behavior: Glue heuristic can be disabled, improved heuristic doesn't merge unrelated keys_
    - _Preservation: Existing intent detection behavior unchanged when glue heuristic enabled_
    - _Requirements: 1.13, 1.14, 2.14, 3.8_

  - [x] 3.7 Verify bug condition exploration tests now pass
    - **Property 1: Expected Behavior** - Intent Parser Robustness Fixed
    - **IMPORTANT**: Re-run the SAME tests from task 1 - do NOT write new tests
    - The tests from task 1 encode the expected behavior
    - When these tests pass, it confirms the expected behavior is satisfied
    - Run all bug condition exploration tests from Phase 1
    - **EXPECTED OUTCOME**: All tests PASS (confirms bugs are fixed)
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 2.10, 2.11, 2.12, 2.13, 2.14_

  - [x] 3.8 Verify preservation tests still pass
    - **Property 2: Preservation** - Existing Parser Behavior Unchanged
    - **IMPORTANT**: Re-run the SAME tests from task 2 - do NOT write new tests
    - Run all preservation property tests from Phase 2
    - **EXPECTED OUTCOME**: All tests PASS (confirms no regressions)
    - Confirm all tests still pass after fix (no regressions)
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11, 3.12_

## Phase 4: Validation

- [x] 4. Checkpoint - Ensure all tests pass
  - Run complete test suite (exploration + preservation + existing unit tests)
  - Verify all bug condition tests pass (confirms bugs fixed)
  - Verify all preservation tests pass (confirms no regressions)
  - Verify all existing unit tests pass (confirms backward compatibility)
  - If any tests fail, investigate and fix before proceeding
  - Ask the user if questions arise
