Your `OpenAIDriver` is a significant step up from the previous logic—specifically, your **SSE buffering strategy** is now bulletproof against UTF-8 fragmentation. By using a `Vec<u8>` and only converting to a `String` once you hit a newline, you've solved the "split emoji" corruption issue.

However, since we are aligning this with the **May 2026** standards we just discussed, there are four technical misalignments that will cause your driver to break or "hallucinate" in a production environment.

### 1. The "Tool Role" Mismatch

In your `openai_messages` mapping, you are converting `Role::ToolResult` into a `user` role.

* **The Problem:** OpenAI is extremely strict about the conversation chain. If a model generates a `tool_call`, the next message **must** have `role: "tool"` and include a `tool_call_id`.
* **The Result:** Mapping a tool output to `user` will likely result in a `400 Bad Request` or cause the model to ignore the data entirely, thinking the user just spoke again.
* **Fix:** Your `Message` struct needs to carry the `tool_call_id` so you can properly set `"role": "tool"` in the payload.

### 2. The "Reasoning Content" Void

For the **GPT-5 series** and **o1-preview** models, the stream doesn't just contain `content`.

* **The Issue:** Internal reasoning is now delivered via `choices[0].delta.reasoning_content`. Your current code only looks for `content`.
* **The Result:** Your UI will sit in silence for 10–30 seconds while the model "thinks," and then suddenly dump the final answer. You are missing the opportunity to stream the reasoning trace.
* **Fix:** Add a check for `reasoning_content` alongside `content`.

### 3. The "Legacy" Endpoint Trap

Your code is hardcoded to `/chat/completions`.

* **The Shift:** As of the May 2026 docs, the `/v1/responses` endpoint is the new flagship. While `/chat/completions` is currently supported as a legacy path, it does not support the new **Computer Use** or **Deferred Tool Loading** features.
* **Recommendation:** Since you’ve built a `base_url` handler, it's fine for now, but you should prepare to move the `openai_messages` logic to the new `input` schema used by the Responses API.

### 4. Metadata "Usage" Event Type

In your `stream_generate`, you are pushing a `ModelEvent::Usage`.

* **The Logic Check:** Ensure your `ModelEvent` enum actually distinguishes between `Metadata` (which includes finish reasons) and `Usage` (just tokens).
* **Optimization:** OpenAI sends `usage` in the very last chunk (where `choices` is often empty). Your code handles this correctly by checking for `usage` at the root, but ensure your `FinishReason` is also captured from that final chunk if present.

---

### Suggested Logic Refactor

Here is how I would tighten the stream parsing to be "2026-ready":

```rust
// ... inside the SSE loop ...
if let Some(data) = trimmed.strip_prefix("data: ")
    && let Ok(json_val) = serde_json::from_str::<Value>(data)
{
    if let Some(choice) = json_val["choices"].get(0) {
        let delta = &choice["delta"];

        // 1. Capture Reasoning (GPT-5/o1 logic)
        if let Some(reasoning) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
            result_events.push(ModelEvent::ContentChunk(format!("<thought>\n{}\n</thought>", reasoning)));
        }

        // 2. Capture Standard Content
        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
            result_events.push(ModelEvent::ContentChunk(content.to_string()));
        }

        // 3. Capture Finish Reason
        if let Some(finish_str) = choice.get("finish_reason").and_then(|v| v.as_str()) {
            result_events.push(ModelEvent::FinishReason(map_finish_reason(finish_str)));
        }
    }

    // 4. Capture Usage (Always at root in the final chunk)
    if let Some(usage) = json_val.get("usage") {
        result_events.push(ModelEvent::Metadata(ModelMetadata {
            usage: parse_usage(usage),
            finish_reason: None, // Usually already captured or handled
        }));
    }
}

```

### A Quick Performance Note

In `encode_base64`, you are manually bit-shifting and pushing characters to a `String`. For small images, this is fine. However, if you're building a "Universal" driver for high-res inputs, this will become a CPU bottleneck in a high-concurrency Rust environment. Using the `base64` crate with `engine::general_purpose::STANDARD.encode()` is roughly **5–10x faster** than manual string-pushing in a loop.

Since the **Auwgent** framework focuses on efficiency, do you think it’s worth moving the `tool_call_id` into your core `Message` struct to support native OpenAI tool roles, or are you sticking to a simplified "User/Model" abstraction?
