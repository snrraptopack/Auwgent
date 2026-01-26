# Implementation Tasks

## 1. Update Non-Streaming Execute Method

### 1.1 Fix Property Names in execute()
- [x] Replace `generationConfig.responseSchema` with `generationConfig.responseJsonSchema`
- [x] Verify `responseMimeType` is set to "application/json" when schema is present
- [x] Remove the line that appends schema to `systemInstruction`

**Details:**
- Location: `GoogleDriver.execute()` method
- Current code appends schema to system instruction - this should be removed
- The Gemini API natively handles schema enforcement via `responseJsonSchema`

## 2. Update Streaming Execute Method

### 2.1 Fix Property Names in executeStream()
- [x] Replace `generationConfig.responseSchema` with `generationConfig.responseJsonSchema`
- [x] Verify `responseMimeType` is set to "application/json" when schema is present
- [x] Remove the line that appends schema to `systemInstruction`

**Details:**
- Location: `GoogleDriver.executeStream()` method
- Same changes as non-streaming method
- Streaming chunk handling already works correctly (concatenates text)

## 3. Testing and Validation

### 3.1 Test Structured Output in Non-Streaming Mode
- [x] Create test case with a JSON schema
- [x] Verify response matches schema
- [x] Verify system instruction is not polluted with schema text

### 3.2 Test Structured Output in Streaming Mode
- [x] Create test case with a JSON schema
- [x] Verify partial JSON chunks are emitted correctly
- [x] Verify final concatenated result is valid JSON matching schema

### 3.3 Test Tool Calling Still Works
- [x] Verify tools work without structured output
- [x] Verify structured output is disabled when tools are present

### 3.4 Test Regular Text Generation
- [x] Verify text generation works without schema
- [x] Verify no regression in basic functionality
