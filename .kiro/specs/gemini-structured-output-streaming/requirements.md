# Requirements Document

## Introduction

This specification addresses improvements to the GoogleDriver's structured output streaming implementation to align with the official Google Gemini API documentation. The current implementation uses incorrect property names and doesn't properly handle partial JSON chunks during streaming. This update will ensure the driver correctly implements structured output streaming according to Google's official specifications while maintaining compatibility with existing functionality.

## Glossary

- **GoogleDriver**: The driver implementation that interfaces with Google's Gemini API
- **Structured_Output**: A mode where the LLM response is constrained to match a specific JSON schema
- **Streaming**: The process of receiving LLM responses incrementally as chunks rather than waiting for the complete response
- **JSON_Schema**: A vocabulary that allows you to annotate and validate JSON documents
- **Partial_JSON**: Valid JSON string fragments that can be concatenated to form complete JSON
- **System_Instruction**: The system-level prompt that guides the model's behavior
- **Response_Schema**: The JSON schema that defines the expected structure of the model's response

## Requirements

### Requirement 1: Use Correct API Property Names

**User Story:** As a developer using the GoogleDriver, I want the driver to use the correct Gemini API property names, so that structured output works reliably with the official API.

#### Acceptance Criteria

1. WHEN structured output is enabled, THE GoogleDriver SHALL use `responseJsonSchema` in the generation configuration
2. THE GoogleDriver SHALL NOT use the deprecated `responseSchema` property name
3. WHEN structured output is enabled, THE GoogleDriver SHALL set `responseMimeType` to "application/json"
4. THE GoogleDriver SHALL apply these property names consistently in both streaming and non-streaming modes

### Requirement 2: Handle Partial JSON Chunks During Streaming

**User Story:** As a developer using streaming mode with structured output, I want partial JSON chunks to be properly handled, so that I can reconstruct the complete JSON response correctly.

#### Acceptance Criteria

1. WHEN streaming structured output, THE GoogleDriver SHALL concatenate all text chunks to form the complete JSON response
2. WHEN emitting text deltas during structured output streaming, THE GoogleDriver SHALL emit the raw partial JSON strings
3. WHEN streaming completes, THE GoogleDriver SHALL return the complete concatenated JSON text
4. THE GoogleDriver SHALL NOT attempt to parse or validate partial JSON chunks during streaming

### Requirement 3: Clean System Instruction Handling

**User Story:** As a developer using structured output mode, I want the system instructions to remain clean, so that the model receives clear guidance without redundant schema information.

#### Acceptance Criteria

1. WHEN structured output mode is enabled with `responseJsonSchema`, THE GoogleDriver SHALL NOT append schema information to system instructions
2. WHEN structured output mode is disabled, THE GoogleDriver SHALL preserve existing system instruction behavior
3. THE GoogleDriver SHALL rely on the native `responseJsonSchema` configuration for schema enforcement
4. THE GoogleDriver SHALL maintain the original system instruction content without modification when using structured output

### Requirement 4: Schema Conversion Support

**User Story:** As a developer providing schemas to the GoogleDriver, I want the driver to handle JSON Schema format correctly, so that my schemas work with the Gemini API requirements.

#### Acceptance Criteria

1. WHEN a `responseSchema` is provided in the SyntheticRequest, THE GoogleDriver SHALL use it as the `responseJsonSchema` value
2. THE GoogleDriver SHALL pass the JSON Schema object directly to the Gemini API configuration
3. THE GoogleDriver SHALL NOT modify or transform the provided JSON Schema structure
4. THE GoogleDriver SHALL support all standard JSON Schema properties (type, properties, required, items, enum, anyOf)
