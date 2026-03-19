# Bugfix Requirements Document

## Introduction

The intent parser is a custom streaming YAML parser designed to handle LLM-generated output in real-time. It consists of a tokenizer (lexical analysis), parser (AST construction), builder (type coercion), and orchestrator (intent detection). The parser currently has multiple robustness and correctness issues that cause it to fail on valid YAML inputs, produce poor error messages, use memory inefficiently, and lack support for standard YAML features. These issues affect the parser's ability to reliably process LLM output in streaming mode, particularly when chunks are small or when standard YAML multiline features are used.

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN a pipe block (`|`) multiline string is split across very small chunks (e.g., 5-byte chunks) THEN the tokenizer's rewind logic in `tokenize_multiline_string()` fails and the parser terminates prematurely or loses content

1.2 WHEN a pipe block (`|`) is split at a chunk boundary where the newline after `|` hasn't arrived yet THEN the parser returns an incomplete token instead of waiting for more data

1.3 WHEN parsing errors occur THEN error messages lack line/column context and are never surfaced to users, making debugging extremely difficult

1.4 WHEN the parser collects errors in `Vec<ParseError>` THEN these errors are populated but never used by callers, resulting in silent failures

1.5 WHEN AST nodes are processed THEN entire nodes are cloned repeatedly during parsing (in `to_ast_node()` calls), causing inefficient memory usage

1.6 WHEN tokens are consumed during streaming THEN the token buffer in `Parser.tokens` grows unbounded with no cleanup of processed tokens

1.7 WHEN stack frames are popped THEN nodes are cloned unnecessarily, adding to memory overhead

1.8 WHEN LLM output contains folded scalar indicators (`>`) THEN the parser treats `>` as a regular scalar value instead of recognizing it as a multiline folded block indicator

1.9 WHEN LLM output contains block chomping indicators (`|-` or `|+`) THEN the parser ignores these indicators and treats all `|` blocks identically

1.10 WHEN parsing numbers with edge cases (NaN, Infinity, -Infinity, incomplete scientific notation like `1e` or `1e+`, or `.5e10`) THEN type coercion fails or produces incorrect results

1.11 WHEN parsing boolean values with common LLM variations ("yes", "no", "on", "off", "y", "n") THEN type coercion fails to recognize these as booleans

1.12 WHEN parsing deeply nested flow collections (inline JSON-like `{}` or `[]`) THEN the hand-rolled `parse_yaml_flow_object()` function breaks or fails to handle escaped quotes in keys/values

1.13 WHEN LLM output contains malformed multiline scalars without proper `|` indicators THEN the orchestrator's "glue" heuristic aggressively merges unrelated keys, incorrectly combining data

1.14 WHEN the "glue" heuristic is too aggressive THEN there is no way to disable or configure this behavior, leading to data corruption

### Expected Behavior (Correct)

2.1 WHEN a pipe block (`|`) multiline string is split across very small chunks (e.g., 5-byte chunks) THEN the tokenizer SHALL correctly rewind and wait for more data, eventually parsing the complete multiline string without data loss

2.2 WHEN a pipe block (`|`) is split at a chunk boundary where the newline after `|` hasn't arrived yet THEN the parser SHALL rewind to the start of the pipe block and wait for more data instead of returning an incomplete token

2.3 WHEN parsing errors occur THEN error messages SHALL include line number, column number, and source context (the problematic line) to aid debugging

2.4 WHEN the parser collects errors in `Vec<ParseError>` THEN these errors SHALL be surfaced to callers through the `ParseResult` and logged or displayed to users

2.5 WHEN AST nodes are processed THEN the parser SHALL minimize cloning by using references or moving nodes where possible, reducing memory allocations

2.6 WHEN tokens are consumed during streaming THEN the parser SHALL clean up processed tokens from the buffer to prevent unbounded growth

2.7 WHEN stack frames are popped THEN the parser SHALL move nodes instead of cloning them to reduce memory overhead

2.8 WHEN LLM output contains folded scalar indicators (`>`) THEN the parser SHALL recognize `>` as a folded block indicator and fold multiple lines into a single line with spaces (per YAML spec)

2.9 WHEN LLM output contains block chomping indicators (`|-` for strip or `|+` for keep) THEN the parser SHALL correctly handle trailing newlines according to the chomping indicator

2.10 WHEN parsing numbers with edge cases (NaN, Infinity, -Infinity) THEN type coercion SHALL correctly recognize and handle these special float values

2.11 WHEN parsing incomplete scientific notation (e.g., `1e`, `1e+`) or edge cases like `.5e10` THEN type coercion SHALL either parse them correctly or fall back to string type gracefully

2.12 WHEN parsing boolean values with common LLM variations ("yes", "no", "on", "off", "y", "n") THEN type coercion SHALL recognize these as boolean values per YAML 1.1 spec

2.13 WHEN parsing deeply nested flow collections THEN the parser SHALL handle arbitrary nesting depth and escaped quotes in keys/values correctly

2.14 WHEN LLM output contains malformed multiline scalars without proper `|` indicators THEN the parser SHALL either use proper multiline indicators or provide a configurable/disableable heuristic that doesn't aggressively merge unrelated keys

### Unchanged Behavior (Regression Prevention)

3.1 WHEN parsing valid YAML with proper `|` multiline strings in normal-sized chunks THEN the parser SHALL CONTINUE TO parse them correctly as it does now

3.2 WHEN parsing simple key-value pairs THEN the parser SHALL CONTINUE TO tokenize and parse them correctly

3.3 WHEN parsing nested mappings and sequences THEN the parser SHALL CONTINUE TO build the correct AST structure

3.4 WHEN parsing quoted strings with escape sequences THEN the parser SHALL CONTINUE TO handle escapes correctly

3.5 WHEN parsing inline flow collections in simple cases THEN the parser SHALL CONTINUE TO parse them correctly

3.6 WHEN parsing standard boolean values ("true", "false") THEN the parser SHALL CONTINUE TO coerce them to boolean type

3.7 WHEN parsing standard number formats (integers, floats, basic scientific notation) THEN the parser SHALL CONTINUE TO coerce them to number type

3.8 WHEN the orchestrator detects registered intent keys THEN it SHALL CONTINUE TO emit intent_ready events correctly

3.9 WHEN parsing streaming input with normal chunk sizes THEN the parser SHALL CONTINUE TO provide partial results and handle incremental parsing

3.10 WHEN parsing comments (if preserve_comments is enabled) THEN the parser SHALL CONTINUE TO preserve them correctly

3.11 WHEN parsing indent/dedent tokens THEN the parser SHALL CONTINUE TO track indentation levels correctly for block structure

3.12 WHEN the parser encounters EOF THEN it SHALL CONTINUE TO emit remaining DEDENT tokens and finalize the AST correctly
