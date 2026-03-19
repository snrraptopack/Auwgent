# Intent Parser Robustness Bugfix Design

## Overview

This design addresses 14 robustness and correctness issues in the intent parser's streaming YAML implementation. The parser consists of four components: tokenizer (lexical analysis), parser (AST construction), builder (type coercion), and orchestrator (intent detection). The bugs span streaming boundary handling, error reporting, memory efficiency, YAML feature support, and type coercion edge cases. The fix strategy focuses on minimal, targeted changes that preserve existing functionality while adding robustness for real-world LLM output processing.

## Glossary

- **Bug_Condition (C)**: The conditions that trigger parser failures - small chunk boundaries in multiline strings, missing error context, unbounded memory growth, unsupported YAML features, and edge case type coercion failures
- **Property (P)**: The desired behavior - correct streaming rewind logic, rich error messages, bounded memory usage, full YAML multiline support, and robust type coercion
- **Preservation**: All existing parsing behavior for valid YAML with normal chunk sizes must remain unchanged
- **Tokenizer**: The lexical analyzer in `tokenizer.rs` that converts character streams into tokens
- **Parser**: The AST builder in `parser.rs` that constructs syntax trees from token streams
- **Builder**: The type coercer in `builder.rs` that converts AST scalar strings to typed IR values
- **Orchestrator**: The intent detector in `orchestrator.rs` that identifies and emits complete intent blocks
- **Streaming Boundary**: A chunk boundary where input is split mid-token or mid-structure
- **Rewind Logic**: The mechanism that restores tokenizer state when a token is incomplete at a chunk boundary
- **Pipe Block**: A YAML literal block scalar using `|` indicator (preserves newlines)
- **Folded Block**: A YAML folded block scalar using `>` indicator (folds newlines to spaces)
- **Chomping Indicator**: YAML modifiers (`-` strip, `+` keep) that control trailing newlines in block scalars

## Bug Details

### Bug Condition

The parser fails under multiple distinct conditions that can be grouped into five categories:

**Category 1: Streaming Boundary Bugs (Defects 1.1, 1.2)**

The bug manifests when a pipe block (`|`) multiline string is split across very small chunks (e.g., 5-byte chunks). The `tokenize_multiline_string()` function's rewind logic fails to correctly restore all state variables, causing premature termination or data loss.

**Formal Specification:**
```
FUNCTION isBugCondition_StreamingBoundary(input)
  INPUT: input of type (chunk_sequence, chunk_size)
  OUTPUT: boolean
  
  RETURN input.contains_pipe_block
         AND input.chunk_size < 20
         AND (chunk_boundary_splits_pipe_header OR chunk_boundary_within_pipe_content)
         AND NOT tokenizer_correctly_rewinds_all_state
END FUNCTION
```

**Category 2: Error Reporting Bugs (Defects 1.3, 1.4)**

The bug manifests when parsing errors occur. Error messages lack line/column context and source snippets, and the `Vec<ParseError>` is populated but never surfaced to callers.

**Formal Specification:**
```
FUNCTION isBugCondition_ErrorReporting(input)
  INPUT: input of type (yaml_string, has_parse_error)
  OUTPUT: boolean
  
  RETURN input.has_parse_error
         AND (error_message_lacks_line_column OR error_lacks_source_context)
         AND errors_not_surfaced_to_caller
END FUNCTION
```

**Category 3: Memory Efficiency Bugs (Defects 1.5, 1.6, 1.7)**

The bug manifests during streaming parsing when AST nodes are cloned repeatedly, tokens accumulate unbounded, and stack frames clone nodes unnecessarily.

**Formal Specification:**
```
FUNCTION isBugCondition_MemoryEfficiency(input)
  INPUT: input of type (streaming_session, token_count, clone_count)
  OUTPUT: boolean
  
  RETURN input.token_count > 1000
         AND tokens_not_cleaned_up
         AND (excessive_node_cloning OR unnecessary_frame_cloning)
END FUNCTION
```

**Category 4: YAML Feature Support Bugs (Defects 1.8, 1.9)**

The bug manifests when LLM output contains folded scalar indicators (`>`) or block chomping indicators (`|-`, `|+`), which are not recognized by the tokenizer.

**Formal Specification:**
```
FUNCTION isBugCondition_YAMLFeatures(input)
  INPUT: input of type yaml_string
  OUTPUT: boolean
  
  RETURN (input.contains_folded_scalar_indicator OR input.contains_chomping_indicator)
         AND tokenizer_treats_as_regular_scalar
END FUNCTION
```

**Category 5: Type Coercion Bugs (Defects 1.10, 1.11, 1.12, 1.13, 1.14)**

The bug manifests when parsing edge case numbers (NaN, Infinity, incomplete scientific notation), YAML 1.1 boolean variations, deeply nested flow collections, or malformed multiline scalars without proper `|` indicators.

**Formal Specification:**
```
FUNCTION isBugCondition_TypeCoercion(input)
  INPUT: input of type scalar_value
  OUTPUT: boolean
  
  RETURN (is_special_float(input) OR is_incomplete_scientific(input) OR is_yaml11_boolean(input))
         AND coerce_value_fails_or_incorrect
         OR (is_deeply_nested_flow_collection(input) AND parse_yaml_flow_object_breaks)
         OR (is_malformed_multiline_without_pipe(input) AND glue_heuristic_too_aggressive)
END FUNCTION
```

### Examples

**Streaming Boundary Bug:**
- Input: `"response_schema:\n  response: |\n    Ghana gained independence..."` split into 5-byte chunks
- Expected: Parser rewinds at chunk boundaries and eventually parses complete multiline string
- Actual: Parser terminates prematurely or loses content after `|` because rewind doesn't restore `after_colon` state

**Error Reporting Bug:**
- Input: `"key: value\n  invalid indentation\nkey2: value2"`
- Expected: Error message includes "Line 2, Column 3: Unexpected indentation. Context: '  invalid indentation'"
- Actual: Error message is generic, lacks line/column, and is never shown to user

**Memory Efficiency Bug:**
- Input: Streaming session with 10,000 tokens over 100 chunks
- Expected: Token buffer size stays bounded (< 100 tokens), minimal cloning
- Actual: Token buffer grows to 10,000 entries, nodes cloned 50,000+ times

**YAML Feature Support Bug:**
- Input: `"description: >\n  This is a long\n  folded paragraph."`
- Expected: Parser recognizes `>` as folded block, produces "This is a long folded paragraph."
- Actual: Parser treats `>` as scalar value, produces literal `">"`

**Type Coercion Bug:**
- Input: `"value: .5e10"` or `"enabled: yes"` or `"data: {nested: {deep: {value: \"escaped\\\"quote\"}}}`
- Expected: Coerces to `5000000000.0`, `true`, and correctly parsed nested object
- Actual: Falls back to string, fails to recognize boolean, or breaks on escaped quotes

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**
- Parsing valid YAML with proper `|` multiline strings in normal-sized chunks (>50 bytes) must continue to work
- Simple key-value pairs must continue to tokenize and parse correctly
- Nested mappings and sequences must continue to build correct AST structure
- Quoted strings with escape sequences must continue to handle escapes correctly
- Inline flow collections in simple cases must continue to parse correctly
- Standard boolean values ("true", "false") must continue to coerce to boolean type
- Standard number formats (integers, floats, basic scientific notation) must continue to coerce to number type
- Orchestrator intent detection for registered keys must continue to emit intent_ready events correctly
- Streaming input with normal chunk sizes (>50 bytes) must continue to provide partial results
- Comments (if preserve_comments enabled) must continue to be preserved correctly
- Indent/dedent tracking must continue to work correctly for block structure
- EOF handling must continue to emit remaining DEDENT tokens and finalize AST correctly

**Scope:**
All inputs that do NOT involve the five bug categories (streaming boundaries in pipe blocks, error conditions, large token counts, unsupported YAML features, edge case type coercion) should be completely unaffected by this fix. This includes:
- Normal YAML parsing with standard chunk sizes
- All existing test cases
- Performance characteristics for typical inputs

## Hypothesized Root Cause

Based on the bug description and code analysis, the most likely issues are:

1. **Incomplete State Rewind in Multiline Tokenization**: The `tokenize_multiline_string()` function rewinds `pos`, `line`, `column`, and `at_line_start`, but fails to restore `after_colon` state. This causes the parser to misinterpret the continuation of a pipe block as a new key-value pair when resuming after a chunk boundary.

2. **Missing Error Context Capture**: The `ParseError` struct has a `context` field for source snippets, but it's never populated. The tokenizer and parser don't track the original input lines for error reporting.

3. **No Token Buffer Cleanup**: The `Parser.tokens` vector grows unbounded because `self.pos` advances but old tokens are never removed. After processing 10,000 tokens, the vector still holds all 10,000 entries.

4. **Excessive Cloning in AST Operations**: The `to_ast_node()` method clones entire subtrees, and `pop_frame()` clones nodes when attaching to parent. Rust's ownership system allows moving instead.

5. **Tokenizer Only Recognizes `|` Indicator**: The `tokenize_content()` function has a single check `if char == '|'` but no handling for `>` (folded) or chomping modifiers (`-`, `+`).

6. **Type Coercion Doesn't Handle Edge Cases**: The `coerce_value()` function uses `parse::<f64>()` which rejects "NaN", "Infinity", and incomplete scientific notation. It also only checks "true"/"false" for booleans, missing YAML 1.1 variants.

7. **Hand-Rolled Flow Parser is Fragile**: The `parse_yaml_flow_object()` function has limited escape handling and no depth limits, breaking on deeply nested or complex inputs.

8. **Orchestrator Glue Heuristic is Too Aggressive**: The `process_mapping_intents()` function unconditionally merges all non-intent keys into the previous intent's first string field, corrupting data when LLMs output multiple intents or metadata keys.

## Correctness Properties

Property 1: Bug Condition - Streaming Boundary Robustness

_For any_ YAML input containing pipe block multiline strings split across very small chunks (5-20 bytes), the fixed tokenizer SHALL correctly rewind all state variables (including `after_colon`) when encountering incomplete tokens, wait for more data, and eventually parse the complete multiline string without data loss or premature termination.

**Validates: Requirements 2.1, 2.2**

Property 2: Bug Condition - Error Reporting Completeness

_For any_ parsing error, the fixed parser SHALL populate error messages with line number, column number, and source context (the problematic line from the original input), and SHALL surface these errors to callers through the `ParseResult` struct for logging or display.

**Validates: Requirements 2.3, 2.4**

Property 3: Bug Condition - Memory Efficiency

_For any_ streaming parsing session, the fixed parser SHALL clean up processed tokens from the buffer (keeping only the last 100 tokens), minimize AST node cloning by using moves where possible, and move (not clone) nodes when popping stack frames, resulting in bounded memory growth proportional to AST depth (not input size).

**Validates: Requirements 2.5, 2.6, 2.7**

Property 4: Bug Condition - YAML Feature Support

_For any_ YAML input containing folded scalar indicators (`>`) or block chomping indicators (`|-`, `|+`), the fixed tokenizer SHALL recognize these indicators, fold multiple lines into single lines with spaces for `>`, and correctly handle trailing newlines according to chomping indicators.

**Validates: Requirements 2.8, 2.9**

Property 5: Bug Condition - Type Coercion Robustness

_For any_ scalar value with edge cases (NaN, Infinity, -Infinity, incomplete scientific notation, YAML 1.1 booleans, deeply nested flow collections, or escaped quotes), the fixed type coercion SHALL correctly parse special float values, gracefully fall back to string for incomplete notation, recognize YAML 1.1 boolean variants, and handle arbitrary nesting depth with proper escape handling.

**Validates: Requirements 2.10, 2.11, 2.12, 2.13**

Property 6: Bug Condition - Configurable Glue Heuristic

_For any_ LLM output with multiple intents or metadata keys, the fixed orchestrator SHALL provide a configurable option to disable the glue heuristic, preventing aggressive merging of unrelated keys and avoiding data corruption.

**Validates: Requirements 2.14**

Property 7: Preservation - Existing Parsing Behavior

_For any_ input that is valid YAML with normal chunk sizes (>50 bytes) and standard features (no edge cases), the fixed parser SHALL produce exactly the same AST, IR, and intent events as the original parser, preserving all existing functionality.

**Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11, 3.12**

## Fix Implementation

### Changes Required

Assuming our root cause analysis is correct:

**File**: `ir-runtime/src/intent_parser/tokenizer.rs`

**Function**: `tokenize_multiline_string()`

**Specific Changes**:
1. **Save and Restore `after_colon` State**: Add `let start_after_colon = self.state.after_colon;` before advancing, and restore it in all rewind paths: `self.state.after_colon = start_after_colon;`

2. **Add Folded Scalar Support**: In `tokenize_content()`, add a new branch `if char == '>' { return self.tokenize_folded_string(); }` and implement `tokenize_folded_string()` similar to `tokenize_multiline_string()` but joining lines with spaces instead of newlines

3. **Add Chomping Indicator Support**: In both `tokenize_multiline_string()` and `tokenize_folded_string()`, after consuming `|` or `>`, check for chomping indicators: `let chomping = match self.peek_char(0) { '-' => Chomping::Strip, '+' => Chomping::Keep, _ => Chomping::Clip };` and apply chomping rules when building the final value

**File**: `ir-runtime/src/intent_parser/parser.rs`

**Function**: `Parser` struct and methods

**Specific Changes**:
1. **Add Input Tracking for Error Context**: Add `input_lines: Vec<String>` field to `Parser` struct. In `write()`, split chunk by newlines and append to `input_lines`. When creating `ParseError`, populate `context` field with `self.input_lines.get(error_line - 1).cloned()`

2. **Surface Errors to Callers**: Ensure `ParseResult.errors` is populated in all error paths. Add logging in `parse_tokens()`: `for error in &self.errors { eprintln!("Parse error at {}:{}: {}", error.line, error.column, error.message); }`

3. **Clean Up Token Buffer**: After processing tokens in `parse_tokens()`, add: `if self.pos > 100 { self.tokens.drain(0..self.pos - 100); self.pos = 100; }` to keep only the last 100 tokens

4. **Reduce Cloning in `to_ast_node()`**: Change `FrameNode::to_ast_node(&self)` to return a reference `&ASTNode` where possible, or use `std::mem::take()` to move out of mutable references

5. **Move Nodes in `pop_frame()`**: Change `let child_node = frame.node.as_ast_node();` to `let child_node = match frame.node { FrameNode::Mapping(m) => ASTNode::Mapping(m), FrameNode::Sequence(s) => ASTNode::Sequence(s) };` to move instead of clone

**File**: `ir-runtime/src/intent_parser/builder.rs`

**Function**: `coerce_value()`

**Specific Changes**:
1. **Add Special Float Handling**: Before the `parse::<f64>()` call, add explicit checks:
   ```rust
   if trimmed.eq_ignore_ascii_case("nan") { return IRValue::Number(f64::NAN); }
   if trimmed.eq_ignore_ascii_case("inf") || trimmed.eq_ignore_ascii_case("infinity") { return IRValue::Number(f64::INFINITY); }
   if trimmed.eq_ignore_ascii_case("-inf") || trimmed.eq_ignore_ascii_case("-infinity") { return IRValue::Number(f64::NEG_INFINITY); }
   ```

2. **Add YAML 1.1 Boolean Handling**: After the "true"/"false" checks, add:
   ```rust
   if trimmed.eq_ignore_ascii_case("yes") || trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("on") { return IRValue::Boolean(true); }
   if trimmed.eq_ignore_ascii_case("no") || trimmed.eq_ignore_ascii_case("n") || trimmed.eq_ignore_ascii_case("off") { return IRValue::Boolean(false); }
   ```

3. **Graceful Fallback for Incomplete Scientific Notation**: Wrap `parse::<f64>()` in a check: if it fails and the string matches `/^-?\d+\.?\d*e[+-]?$/`, return `IRValue::String(value.to_string())` instead of attempting to parse

**Function**: `parse_yaml_flow_object()`

**Specific Changes**:
1. **Add Depth Limit**: Add a `depth` parameter (default 0, max 10). When recursing for nested objects, pass `depth + 1` and return `None` if `depth > 10`

2. **Improve Escape Handling**: In the quoted string parsing loop, properly handle all escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\'`, and `\uXXXX` for Unicode escapes

**File**: `ir-runtime/src/intent_parser/orchestrator.rs`

**Function**: `Orchestrator` struct and `process_mapping_intents()`

**Specific Changes**:
1. **Add Configurable Glue Heuristic**: Add `enable_glue_heuristic: bool` field to `ParserOptions` (default `true` for backward compatibility). Add `pub fn set_glue_heuristic(&mut self, enabled: bool)` method to `Orchestrator`

2. **Guard Glue Logic**: Wrap the entire "glue subsequent unmapped keys" section in `if self.options.enable_glue_heuristic { ... }` so it can be disabled

3. **Improve Glue Heuristic**: Instead of unconditionally merging all non-intent keys, only merge keys that are at the same indent level and immediately follow the intent (no blank lines). This prevents merging unrelated metadata keys

**File**: `ir-runtime/src/intent_parser/types.rs`

**Function**: `ParserOptions` struct

**Specific Changes**:
1. **Add New Option Fields**: Add `pub enable_glue_heuristic: Option<bool>` to `ParserOptions` struct with default `Some(true)`

## Testing Strategy

### Validation Approach

The testing strategy follows a two-phase approach: first, surface counterexamples that demonstrate the bugs on unfixed code (exploratory bug condition checking), then verify the fixes work correctly and preserve existing behavior (fix checking and preservation checking).

### Exploratory Bug Condition Checking

**Goal**: Surface counterexamples that demonstrate the bugs BEFORE implementing the fix. Confirm or refute the root cause analysis. If we refute, we will need to re-hypothesize.

**Test Plan**: Write tests that simulate the five bug categories on the UNFIXED code to observe failures and understand the root causes.

**Test Cases**:
1. **Streaming Boundary Test**: Feed `"response_schema:\n  response: |\n    Ghana gained independence..."` in 5-byte chunks to unfixed tokenizer (will fail - loses content or terminates early)
2. **Error Reporting Test**: Parse `"key: value\n  invalid\nkey2: value2"` and inspect `ParseResult.errors` (will fail - errors empty or lack context)
3. **Memory Efficiency Test**: Parse 10,000 tokens in streaming mode and measure `Parser.tokens.len()` and clone count (will fail - unbounded growth)
4. **Folded Scalar Test**: Parse `"description: >\n  This is\n  folded."` (will fail - produces literal `">"` instead of "This is folded.")
5. **Chomping Indicator Test**: Parse `"text: |-\n  No trailing newline"` (will fail - ignores `-` and includes trailing newline)
6. **Special Float Test**: Parse `"value: NaN"` and `"value: Infinity"` (will fail - falls back to string)
7. **YAML 1.1 Boolean Test**: Parse `"enabled: yes"` and `"disabled: no"` (will fail - falls back to string)
8. **Incomplete Scientific Notation Test**: Parse `"value: 1e"` (will fail - panics or produces incorrect result)
9. **Deeply Nested Flow Test**: Parse `"data: {a: {b: {c: {d: {e: \"value\"}}}}}"` (will fail - breaks or stack overflows)
10. **Escaped Quote in Flow Test**: Parse `"data: {key: \"escaped\\\"quote\"}"` (will fail - breaks on escaped quote)
11. **Aggressive Glue Test**: Parse `"intent1:\n  text: Hello\nmetadata: value\nintent2:\n  text: World"` with both intents registered (will fail - metadata merged into intent1)

**Expected Counterexamples**:
- Streaming boundary: Parser returns incomplete token or loses content after chunk boundary in pipe block
- Error reporting: `ParseResult.errors` is empty or lacks line/column/context
- Memory efficiency: Token buffer grows to 10,000+ entries, clone count exceeds 50,000
- YAML features: Folded scalars and chomping indicators not recognized
- Type coercion: Edge cases fall back to string or fail to parse
- Glue heuristic: Unrelated keys merged into previous intent

### Fix Checking

**Goal**: Verify that for all inputs where the bug conditions hold, the fixed parser produces the expected behavior.

**Pseudocode:**
```
FOR ALL input WHERE isBugCondition_StreamingBoundary(input) DO
  result := tokenizer_fixed.tokenize_all(input)
  ASSERT result contains complete multiline string without data loss
END FOR

FOR ALL input WHERE isBugCondition_ErrorReporting(input) DO
  result := parser_fixed.parse(input)
  ASSERT result.errors is not empty
  ASSERT all errors have line, column, and context populated
END FOR

FOR ALL input WHERE isBugCondition_MemoryEfficiency(input) DO
  result := parser_fixed.parse_streaming(input)
  ASSERT parser_fixed.tokens.len() <= 100
  ASSERT clone_count < input.token_count * 2
END FOR

FOR ALL input WHERE isBugCondition_YAMLFeatures(input) DO
  result := tokenizer_fixed.tokenize_all(input)
  ASSERT folded scalars produce space-joined lines
  ASSERT chomping indicators control trailing newlines
END FOR

FOR ALL input WHERE isBugCondition_TypeCoercion(input) DO
  result := builder_fixed.coerce_value(input)
  ASSERT special floats coerce to f64::NAN, f64::INFINITY, f64::NEG_INFINITY
  ASSERT YAML 1.1 booleans coerce to true/false
  ASSERT incomplete scientific notation falls back to string gracefully
  ASSERT deeply nested flow collections parse correctly
END FOR
```

### Preservation Checking

**Goal**: Verify that for all inputs where the bug conditions do NOT hold, the fixed parser produces the same result as the original parser.

**Pseudocode:**
```
FOR ALL input WHERE NOT (isBugCondition_StreamingBoundary(input) OR isBugCondition_ErrorReporting(input) OR isBugCondition_MemoryEfficiency(input) OR isBugCondition_YAMLFeatures(input) OR isBugCondition_TypeCoercion(input)) DO
  ASSERT parser_original.parse(input) = parser_fixed.parse(input)
END FOR
```

**Testing Approach**: Property-based testing is recommended for preservation checking because:
- It generates many test cases automatically across the input domain
- It catches edge cases that manual unit tests might miss
- It provides strong guarantees that behavior is unchanged for all non-buggy inputs

**Test Plan**: Run all existing test cases on both unfixed and fixed code, asserting identical results. Add property-based tests that generate random valid YAML and verify identical parsing.

**Test Cases**:
1. **Normal Pipe Block Preservation**: Parse `"text: |\n  Line 1\n  Line 2"` with 50-byte chunks (should produce identical result)
2. **Simple Key-Value Preservation**: Parse `"key: value\nkey2: value2"` (should produce identical AST)
3. **Nested Structure Preservation**: Parse `"parent:\n  child:\n    - item1\n    - item2"` (should produce identical AST)
4. **Quoted String Preservation**: Parse `"text: \"escaped\\nstring\""` (should produce identical scalar with newline)
5. **Standard Boolean Preservation**: Parse `"enabled: true\ndisabled: false"` (should produce identical boolean coercion)
6. **Standard Number Preservation**: Parse `"int: 42\nfloat: 3.14\nsci: 1.5e10"` (should produce identical number coercion)
7. **Intent Detection Preservation**: Parse `"intent1:\n  text: Hello"` with intent1 registered (should emit identical intent_ready event)

### Unit Tests

- Test streaming boundary rewind for pipe blocks with 5-byte, 10-byte, and 20-byte chunks
- Test error reporting populates line, column, and context for various error types
- Test token buffer cleanup keeps only last 100 tokens after processing 1000 tokens
- Test folded scalar (`>`) produces space-joined lines
- Test chomping indicators (`|-`, `|+`) control trailing newlines correctly
- Test special float coercion (NaN, Infinity, -Infinity)
- Test YAML 1.1 boolean coercion (yes, no, on, off, y, n)
- Test incomplete scientific notation falls back to string
- Test deeply nested flow collections (depth 10) parse correctly
- Test escaped quotes in flow collections parse correctly
- Test glue heuristic can be disabled via option
- Test glue heuristic only merges adjacent keys at same indent

### Property-Based Tests

- Generate random YAML with pipe blocks and random chunk sizes (1-100 bytes), verify complete parsing without data loss
- Generate random valid YAML and verify fixed parser produces identical AST to original parser
- Generate random scalar values and verify type coercion produces valid IRValue (no panics)
- Generate random flow collections with varying nesting depth (0-15) and verify parsing succeeds or gracefully fails at depth limit
- Generate random YAML with multiple intents and verify glue heuristic doesn't corrupt data when disabled

### Integration Tests

- Test full orchestrator flow with small chunks (5 bytes) and pipe blocks, verify intent_ready fires with complete data
- Test orchestrator with errors, verify errors are logged to stderr
- Test orchestrator with folded scalars and chomping indicators, verify correct IR output
- Test orchestrator with edge case type coercion, verify correct typed values in intent JSON
- Test orchestrator with glue heuristic disabled, verify multiple intents and metadata keys preserved correctly
