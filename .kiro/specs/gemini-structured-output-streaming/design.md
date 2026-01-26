# Design Document: Gemini Structured Output Streaming

## Overview

This design addresses the improvements needed for the GoogleDriver's structured output streaming implementation to align with the official Google Gemini API specifications. The changes focus on three key areas:

1. **Correct API Property Names**: Replace `responseSchema` with `responseJsonSchema` in the generation configuration
2. **Proper Streaming Handling**: Ensure partial JSON chunks are correctly concatenated during streaming
3. **Clean System Instructions**: Remove redundant schema information from system instructions when using structured output mode

## Architecture

The GoogleDriver follows a request-response pattern where:
- Input: `SyntheticRequest` containing messages, optional schema, optional tools, and configuration
- Output: `DriverResult` containing either text or tool parameters
- Streaming: Async generator yielding `StreamChunk` objects and returning final `DriverResult`

The key architectural change is in how structured output configuration is applied:

**Current Flow:**
```
SyntheticRequest.responseSchema → generationConfig.responseSchema + system instruction append
```

**New Flow:**
```
SyntheticRequest.responseSchema → generationConfig.responseJsonSchema (no system instruction modification)
```

## Components and Interfaces

### Modified Components

#### 1. GoogleDriver.execute() Method

**Changes:**
- Replace `generationConfig.responseSchema` with `generationConfig.responseJsonSchema`
- Remove the line that appends schema to system instructions
- Keep all other logic unchanged (tool handling, message mapping, etc.)

**Configuration Logic:**
```typescript
if (request.responseSchema && !hasTools) {
    generationConfig.responseMimeType = "application/json";
    generationConfig.responseJsonSchema = request.responseSchema;
    // REMOVED: systemInstruction += schema append
}
```

#### 2. GoogleDriver.executeStream() Method

**Changes:**
- Replace `generationConfig.responseSchema` with `generationConfig.responseJsonSchema`
- Remove the line that appends schema to system instructions
- Streaming chunk handling remains the same (text deltas are already concatenated correctly)

**Streaming Behavior:**
- Text chunks are emitted as-is (partial JSON strings)
- Full text is accumulated in `fullText` variable
- Final result returns the complete concatenated text

### Unchanged Components

#### 1. Message Mapping
- System messages → `systemInstruction` config
- User/assistant messages → `contents` array
- Role mapping: assistant → model, user → user

#### 2. Tool Configuration
- Tools mapped to `functionDeclarations` format
- Structured output disabled when tools are present
- Tool call detection and streaming lifecycle unchanged

#### 3. Response Handling
- Text responses returned in `DriverResult.text`
- Tool calls returned in `DriverResult.toolParams`
- Stream chunks follow existing `StreamChunk` type definitions

## Data Models

No changes to data models. The existing interfaces remain:

**SyntheticRequest:**
- `messages`: Array of SyntheticMessage
- `responseSchema`: Optional JsonSchema
- `tools`: Optional array of SyntheticToolDef
- `config`: Model configuration

**JsonSchema:**
- `type`, `properties`, `required`, `items`, `enum`, `anyOf`
- Passed directly to Gemini API without transformation

**DriverResult:**
- `text`: Optional string response
- `toolParams`: Optional tool call information

**StreamChunk:**
- Text deltas, tool lifecycle events
- No changes to chunk types or structure

