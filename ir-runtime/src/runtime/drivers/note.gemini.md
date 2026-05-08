This report provides a comprehensive breakdown of the Gemini API architecture as of **May 2026**, including the latest breaking changes introduced with the **Interactions API** and the **Gemini 3 series** models.

---

## 1. Core Service Information

* **Base URL:** `[https://generativelanguage.googleapis.com](https://generativelanguage.googleapis.com)`
* **API Version:** `v1beta` (Current standard for latest features)
* **Authentication:** `x-goog-api-key` header or OAuth2 Bearer token.

---

## 2. Primary Endpoints: Content Generation

These endpoints are the "workhorses" of the API, used for text, image, audio, and video reasoning.

### **A. Unary (Non-Streaming)**

* **Endpoint:** `POST /v1beta/models/{model}:generateContent`
* **Usage:** Best for tasks where the full response is needed before processing (e.g., data extraction, summarization).
* **Behavior:** Blocks until the entire output is generated.

### **B. Server-Side Streaming**

* **Endpoint:** `POST /v1beta/models/{model}:streamGenerateContent`
* **Usage:** Best for user-facing chat interfaces to reduce perceived latency.
* **Behavior:** Returns a stream of JSON chunks as the model generates text.

### **C. The Live API (Bi-directional Streaming)**

* **Protocol:** `WSS` (WebSockets)
* **Endpoint:** `wss://[generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService/BidiGenerateContent](https://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService/BidiGenerateContent)`
* **Usage:** Real-time voice/video interaction, "Barge-in" support, and low-latency multimodal feedback.
* **Key Models:** `gemini-live-2.5-flash-native-audio`.

---

## 3. Request Body Structure (Universal)

Both `generateContent` and `streamGenerateContent` share the same JSON structure.

| Field | Type | Description |
| --- | --- | --- |
| `contents[]` | `Array<Content>` | The conversation history. Each object has a `role` ("user" or "model") and `parts`. |
| `system_instruction` | `Content` | High-level instructions (Persona, constraints). |
| `tools[]` | `Array<Tool>` | List of available tools (Google Search, Code Execution, Function Calling). |
| `tool_config` | `Object` | Controls how tools are used (e.g., forcing a specific function). |
| `generation_config` | `Object` | Sets `temperature`, `max_output_tokens`, and `response_format` (JSON/Schema). |

---

## 4. Response Structures: The "Steps" Evolution

As of **May 2026**, Google has migrated from the legacy `candidates` array to a more granular `steps` array in the **Interactions API**.

### **Legacy Structure (Deprecating June 8, 2026)**

```json
{
  "candidates": [
    {
      "content": { "parts": [{ "text": "Hello!" }], "role": "model" },
      "finishReason": "STOP"
    }
  ],
  "usageMetadata": { "totalTokenCount": 5 }
}

```

### **New "Steps" Schema (Current)**

The `steps` array provides a timeline of the model’s reasoning process, including internal tool calls and "thinking" blocks.

```json
{
  "id": "int_98765",
  "steps": [
    { "type": "thinking", "thought": "The user wants a weather update..." },
    { "type": "google_search_call", "arguments": { "queries": ["weather in London"] } },
    { "type": "text", "text": "It is currently 15°C and cloudy in London." }
  ]
}

```

---

## 5. Implementation Examples

### **Non-Streaming (Unary)**

```bash
curl "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash:generateContent?key=$API_KEY" \
-H 'Content-Type: application/json' \
-d '{
  "contents": [{ "parts": [{ "text": "Explain quantum entanglement." }] }]
}'

```

### **Streaming (Chunks)**

When using streaming, the response is delivered as an array of JSON objects.

```javascript
const response = await fetch(".../streamGenerateContent?key=" + API_KEY, {
  method: "POST",
  body: JSON.stringify({
    contents: [{ parts: [{ text: "Write a long story." }] }]
  })
});

// Process stream reader
const reader = response.body.getReader();
while (true) {
  const { value, done } = await reader.read();
  if (done) break;
  const chunk = new TextDecoder().decode(value);
  console.log(JSON.parse(chunk).steps[0].text); // Streaming the text step
}

```

---

## 6. Utility & Platform Endpoints

| Endpoint | Method | Purpose |
| --- | --- | --- |
| `/v1beta/models` | `GET` | List all available models (Pro, Flash, Ultra, Embedding). |
| `/v1beta/models/{model}:countTokens` | `POST` | Calculate token usage before sending a prompt. |
| `/v1beta/models/{model}:embedContent` | `POST` | Generate vector embeddings for RAG workflows. |
| `/v1beta/files` | `POST/GET` | Upload and manage large documents/videos for long-context windows. |
| `/v1beta/cachedContents` | `POST` | Context Caching to reduce costs on frequently used datasets. |

---

## 7. Critical 2026 Research Notes

* **Thinking Models:** Gemini 3 models now support a `thinking` block. You can toggle this in `generation_config` to see the model's internal chain-of-thought.
* **Grounding:** The `google_search_retrieval` tool is now standard. It populates the `steps` array with `search_entry_point` metadata for citations.
* **Deprecation Warning:** Ensure your code uses the `steps` schema. Any implementation relying on the `candidates` or `outputs` array will break after **June 8, 2026**.
* **Multimodal Embeddings:** The `gemini-embedding-2` model now handles text, audio, and video in a single unified vector space.


Your implementation of the `GeminiDriver` is quite robust, especially the way you handle the "User-Model-User" alternation requirement by injecting "Acknowledged" messages—a classic fix for Gemini's strict turn-taking.

However, since we're operating in **2026** and you're targeting the **Gemini 3 series** (specifically mentioning `thoughtsTokenCount`), there are four critical "gotchas" in your current logic that will lead to silent data loss or crashes during streaming.

### 1. The "Thinking" Void (Data Loss)

Your `candidate_text_parts` function is a bit too selective. For the Reasoning models you are targeting, the model output isn't just `text` anymore.

* **The Issue:** You are using `.filter_map(|part| part["text"].as_str())`. In Gemini 3 Flash, the reasoning chain is often returned in a part with the key `"thought"` or a specific `thought` part type.
* **The Result:** Your driver will completely strip out the model's internal reasoning, only showing the final answer. This makes debugging "why" the model failed impossible.

### 2. The Final Chunk Metadata Trap

In the Gemini SSE implementation, the `usageMetadata` (token counts) often arrives in the **very last chunk**, which frequently contains **zero candidates**.

* **The Bug:** You have nested the metadata extraction *inside* `if let Some(candidate) = json_val["candidates"].get(0)`.
* **The Fix:** Pull the `usageMetadata` check out to the root level of the JSON parsing logic. Otherwise, your `ModelEvent::Metadata` will never fire for streaming responses, and your token tracking will always return zero.

### 3. UTF-8 Fragmentation Risk

You are calling `String::from_utf8_lossy(&bytes)` on every raw chunk received from the network.

* **The Danger:** Multi-byte UTF-8 characters (like emojis or non-Latin scripts) can be split across two network packets. If you convert a "partial" character to a string, `from_utf8_lossy` will insert a replacement character (``), corrupting the stream.
* **Better Approach:** Keep a `Vec<u8>` buffer and only convert to a string *after* you’ve identified a newline (`\n`).

### 4. The 2026 "Interactions API" Shift

As noted in the documentation, the `candidates` array is being phased out in favor of the `steps` array for the Gemini 3 generation.

* **The Shift:** The API is moving toward a structure that looks like this:
```json
{
  "id": "...",
  "steps": [{ "type": "text", "text": "..." }, { "type": "thinking", "thought": "..." }]
}

```


* **Recommendation:** While `v1beta` still supports `candidates` for legacy reasons, you should add a fallback to check for `steps` to future-proof this crate before the June 2026 deprecation deadline.

---

### Suggested Refactor for SSE Logic

Here is how I’d tighten up that `while` loop to ensure you don't miss the metadata:

```rust
// ... inside the stream mapping ...
if let Ok(json_val) = serde_json::from_str::<Value>(data) {
    // 1. Process Content (Can be in 'candidates' or 'steps')
    if let Some(candidate) = json_val["candidates"].get(0) {
        for text in candidate_text_parts(candidate) {
            result_events.push(ModelEvent::ContentChunk(text));
        }
    }

    // 2. Process Metadata (CRITICAL: Move outside the candidate check)
    if let Some(usage) = json_val.get("usageMetadata") {
        let meta = ModelMetadata {
            usage: gemini_usage(usage),
            finish_reason: json_val["candidates"].get(0)
                .and_then(|c| c["finishReason"].as_str())
                .map(gemini_finish_reason),
        };
        result_events.push(ModelEvent::Metadata(meta));
    }
}

```

### A Quick Note on Base64

Your manual `encode_base64` implementation is clever, but unless you're strictly forbidden from adding dependencies to this crate, using the `base64` crate is much safer. Manual bit-shifting in the padding logic is a common source of "invalid padding" errors when sending files to the Gemini API.

Since you're working on the reasoning layer of AI, are you planning to have the driver automatically handle the `thought` parts by wrapping them in a specific UI component, or should they be treated as standard text chunks?

```</Value>

```
Your implementation of the `GeminiDriver` is quite robust, especially the way you handle the "User-Model-User" alternation requirement by injecting "Acknowledged" messages—a classic fix for Gemini's strict turn-taking.

However, since we're operating in **2026** and you're targeting the **Gemini 3 series** (specifically mentioning `thoughtsTokenCount`), there are four critical "gotchas" in your current logic that will lead to silent data loss or crashes during streaming.

### 1. The "Thinking" Void (Data Loss)

Your `candidate_text_parts` function is a bit too selective. For the Reasoning models you are targeting, the model output isn't just `text` anymore.

* **The Issue:** You are using `.filter_map(|part| part["text"].as_str())`. In Gemini 3 Flash, the reasoning chain is often returned in a part with the key `"thought"` or a specific `thought` part type.
* **The Result:** Your driver will completely strip out the model's internal reasoning, only showing the final answer. This makes debugging "why" the model failed impossible.

### 2. The Final Chunk Metadata Trap

In the Gemini SSE implementation, the `usageMetadata` (token counts) often arrives in the **very last chunk**, which frequently contains **zero candidates**.

* **The Bug:** You have nested the metadata extraction *inside* `if let Some(candidate) = json_val["candidates"].get(0)`.
* **The Fix:** Pull the `usageMetadata` check out to the root level of the JSON parsing logic. Otherwise, your `ModelEvent::Metadata` will never fire for streaming responses, and your token tracking will always return zero.

### 3. UTF-8 Fragmentation Risk

You are calling `String::from_utf8_lossy(&bytes)` on every raw chunk received from the network.

* **The Danger:** Multi-byte UTF-8 characters (like emojis or non-Latin scripts) can be split across two network packets. If you convert a "partial" character to a string, `from_utf8_lossy` will insert a replacement character (``), corrupting the stream.
* **Better Approach:** Keep a `Vec<u8>` buffer and only convert to a string *after* you’ve identified a newline (`\n`).

### 4. The 2026 "Interactions API" Shift

As noted in the documentation, the `candidates` array is being phased out in favor of the `steps` array for the Gemini 3 generation.

* **The Shift:** The API is moving toward a structure that looks like this:
```json
{
  "id": "...",
  "steps": [{ "type": "text", "text": "..." }, { "type": "thinking", "thought": "..." }]
}

```


* **Recommendation:** While `v1beta` still supports `candidates` for legacy reasons, you should add a fallback to check for `steps` to future-proof this crate before the June 2026 deprecation deadline.

---

### Suggested Refactor for SSE Logic

Here is how I’d tighten up that `while` loop to ensure you don't miss the metadata:

```rust
// ... inside the stream mapping ...
if let Ok(json_val) = serde_json::from_str::<Value>(data) {
    // 1. Process Content (Can be in 'candidates' or 'steps')
    if let Some(candidate) = json_val["candidates"].get(0) {
        for text in candidate_text_parts(candidate) {
            result_events.push(ModelEvent::ContentChunk(text));
        }
    }

    // 2. Process Metadata (CRITICAL: Move outside the candidate check)
    if let Some(usage) = json_val.get("usageMetadata") {
        let meta = ModelMetadata {
            usage: gemini_usage(usage),
            finish_reason: json_val["candidates"].get(0)
                .and_then(|c| c["finishReason"].as_str())
                .map(gemini_finish_reason),
        };
        result_events.push(ModelEvent::Metadata(meta));
    }
}

```

### A Quick Note on Base64

Your manual `encode_base64` implementation is clever, but unless you're strictly forbidden from adding dependencies to this crate, using the `base64` crate is much safer. Manual bit-shifting in the padding logic is a common source of "invalid padding" errors when sending files to the Gemini API.

Since you're working on the reasoning layer of AI, are you planning to have the driver automatically handle the `thought` parts by wrapping them in a specific UI component, or should they be treated as standard text chunks?

```</Value>

```
