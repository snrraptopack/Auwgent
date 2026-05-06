# Multimodal Input Design

This note proposes a compiler-driven, provider-neutral input model for Auwgent. The goal is a nice DX for text, images, audio, video, and files while still compiling cleanly into each provider's native request format across TypeScript, Rust, Python, and Dart targets.

## Current Context

Today the runtime and compiler are effectively text-first:

```ts
await agent.run("hello")
```

Internally, a session turn stores:

```ts
type Turn = {
  input: string
  model_response: string
}
```

The compiler already has `Text` as a DSL primitive. The lowering path currently treats `input: Text` / string as the default text input shape, and target SDKs mostly route strings through FFI as text while JSON-like inputs go through generic JSON paths.

That is fine for text, but it does not scale to provider multimodal APIs. OpenAI, Gemini, and Anthropic all model multimodal prompts as ordered content blocks/parts, not as one string.

The important design point for Auwgent is that this should not be a runtime "accept anything" API. The compiler should know the declared input modality and generate the correct `run` signature for every target.

## Compiler-Driven Input Contract

The DSL should grow the primitive input datatypes:

```auwgent
input: Text
input: Image
input: File
input: Audio
input: Video
input: Text | Image
input: Image | File
```

These types describe what the user may pass to `run`, and codegen should reflect that directly.

Rules:

1. `Text` means text-only input.
2. Any media input implicitly includes text because multimodal prompts nearly always need text instructions.
3. `Image` means text or image input.
4. `File` means text or file input.
5. `Audio` means text or audio input.
6. `Video` means text or video input.
7. `Image | File` means text, image, or file input.
8. `Text | Image` is allowed but equivalent to `Image` from a capability perspective.

So the compiler should normalize declared input capabilities like this:

```ts
Text         -> { text: true }
Image        -> { text: true, image: true }
File         -> { text: true, file: true }
Audio        -> { text: true, audio: true }
Video        -> { text: true, video: true }
Image | File -> { text: true, image: true, file: true }
```

This normalized capability set should be preserved in IR for media inputs. `Text` remains special: it lowers to `null`, and the runtime understands `null` as default text input.

Recommended IR shape:

```json
{
  "input": null
}
```

for:

```auwgent
input: Text
```

For media input, use the existing type-shape style, not a new `kind` wrapper:

```json
{
  "input": "image"
}
```

for:

```auwgent
input: Image
```

and:

```json
{
  "input": {
    "type": "union",
    "options": ["image", "file"]
  }
}
```

for:

```auwgent
input: Image | File
```

The IR does not include `"text"` in media options. Text is implicit whenever media input is declared.

## Provider Research

### OpenAI

OpenAI's Responses API accepts an `input` string or an array of input items. Message content can be a string or an array of content parts. Supported request parts include `input_text`, `input_image`, and `input_file`; docs also describe `input_audio` in API schemas and Realtime uses audio conversation items/events.

Useful provider shapes:

```ts
{
  role: "user",
  content: [
    { type: "input_text", text: "What is in this image?" },
    { type: "input_image", image_url: "data:image/png;base64,...", detail: "auto" },
    { type: "input_file", file_id: "file_..." }
  ]
}
```

OpenAI file inputs can be passed by `file_id`, `file_url`, or base64 `file_data` for supported file types such as PDFs. Realtime audio is different: it can use full `input_audio` message content or stream audio chunks through `input_audio_buffer.append`.

Sources:

- <https://platform.openai.com/docs/api-reference/responses>
- <https://platform.openai.com/docs/guides/pdf-files?api-mode=responses>
- <https://platform.openai.com/docs/guides/realtime-model-capabilities>
- <https://platform.openai.com/docs/api-reference/realtime>

### Gemini

Gemini's core shape is `contents[]`, where each content item has a `role` and ordered `parts[]`. A `Part` may contain text, inline binary data, or file references.

Useful provider shapes:

```ts
{
  role: "user",
  parts: [
    { text: "Summarize this video" },
    {
      file_data: {
        mime_type: "video/mp4",
        file_uri: "https://generativelanguage.googleapis.com/..."
      }
    }
  ]
}
```

Gemini recommends inline media for small request payloads and the Files API for larger or reusable media. The docs repeatedly call out the 20 MB total request size threshold for inline media across image, audio, and video examples.

Sources:

- <https://ai.google.dev/docs/gemini_api_overview/>
- <https://ai.google.dev/gemini-api/docs/vision>
- <https://ai.google.dev/gemini-api/docs/audio>
- <https://ai.google.dev/gemini-api/docs/video-understanding>
- <https://ai.google.dev/api/files>

### Anthropic

Claude Messages API uses `messages[]`, where `content` can be a string or an array of content blocks. Images use image blocks, PDFs/documents use `document` blocks, and reusable uploads can be referenced through the Files API.

Useful provider shapes:

```ts
{
  role: "user",
  content: [
    { type: "text", text: "Explain this PDF" },
    {
      type: "document",
      source: {
        type: "url",
        url: "https://example.com/file.pdf"
      }
    }
  ]
}
```

Anthropic's API is stateless, so full conversational history must be sent each request. That matches Auwgent's current approach of reconstructing provider messages from runtime session state.

Sources:

- <https://docs.anthropic.com/en/api/messages-examples>
- <https://docs.anthropic.com/en/docs/build-with-claude/vision>
- <https://docs.anthropic.com/en/docs/build-with-claude/pdf-support>
- <https://docs.anthropic.com/en/docs/build-with-claude/files>

## Recommended Generated DX

When the compiler sees text-only input:

```auwgent
input: Text
```

the generated API should expose the simple text path:

```ts
await agent.run("hello")
```

When the compiler sees any media input:

```auwgent
input: Image
```

or:

```auwgent
input: Image | File
```

the generated API should expose content parts:

```ts
await agent.run([
  { type: "text", text: "What is in this image?" },
  { type: "image", url: "file://./photo.png", mimeType: "image/png" }
])
```

For media-capable agents, the generated type should reject modalities not declared by the DSL.

Example:

```auwgent
input: Image
```

should allow:

```ts
await agent.run("describe this")

await agent.run([
  { type: "text", text: "describe this" },
  { type: "image", path: "./photo.png" }
])
```

and reject:

```ts
await agent.run([
  { type: "text", text: "summarize" },
  { type: "file", path: "./report.pdf" }
])
```

because `File` was not declared.

For a file-capable agent:

```ts
await agent.run({
  role: "user",
  content: [
    { type: "text", text: "Compare these two files" },
    { type: "file", path: "./report.pdf", mimeType: "application/pdf" },
    { type: "image", path: "./chart.png", mimeType: "image/png" }
  ]
})
```

The object-message form is useful, but for generated agent APIs the array form should be the main multimodal DX.

## Proposed Types

The shared SDK can expose generic building blocks:

```ts
export type AuwgentRunInput<Part extends AuwgentInputPart = AuwgentInputPart> =
  | string
  | AuwgentInputMessage<Part>
  | Part[]

export type AuwgentInputMessage<Part extends AuwgentInputPart = AuwgentInputPart> = {
  role?: "user"
  content: string | Part[]
}

export type AuwgentInputPart =
  | AuwgentTextPart
  | AuwgentImagePart
  | AuwgentAudioPart
  | AuwgentVideoPart
  | AuwgentFilePart

export type AuwgentTextPart = {
  type: "text"
  text: string
}

export type AuwgentBinarySource =
  | { data: ArrayBuffer | Uint8Array | string; encoding?: "base64" | "utf8" }
  | { path: string }
  | { url: string }
  | { ref: string }

export type AuwgentImagePart = AuwgentBinarySource & {
  type: "image"
  mimeType?: string
  detail?: "auto" | "low" | "high"
}

export type AuwgentAudioPart = AuwgentBinarySource & {
  type: "audio"
  mimeType?: string
  transcript?: string
}

export type AuwgentVideoPart = AuwgentBinarySource & {
  type: "video"
  mimeType?: string
  transcript?: string
  sampledFrames?: AuwgentImagePart[]
}

export type AuwgentFilePart = AuwgentBinarySource & {
  type: "file"
  mimeType?: string
  name?: string
}
```

Then generated TypeScript should narrow by compiler-known modality:

```ts
// input: Text
export type AuwgentInput = string

// input: Image
export type AuwgentInput =
  | string
  | import("@snrraptopack/auwgent-sdk").AuwgentRunInput<
      import("@snrraptopack/auwgent-sdk").AuwgentTextPart |
      import("@snrraptopack/auwgent-sdk").AuwgentImagePart
    >

// input: Image | File
export type AuwgentInput =
  | string
  | import("@snrraptopack/auwgent-sdk").AuwgentRunInput<
      import("@snrraptopack/auwgent-sdk").AuwgentTextPart |
      import("@snrraptopack/auwgent-sdk").AuwgentImagePart |
      import("@snrraptopack/auwgent-sdk").AuwgentFilePart
    >
```

The generated `run` signature should use `AuwgentInput`, not `any`.

Equivalent target-level types:

- Rust: generated enum such as `AuwgentInput::Text(String)`, `AuwgentInput::Parts(Vec<AuwgentInputPart>)`.
- Python: generated `Union[str, Sequence[TextPart | ImagePart | FilePart]]` style typing.
- Dart: generated sealed classes or typed maps around `AuwgentInputPart`, with `String` only for text-only agents.

## Normalization Layer

The runtime should normalize user input before provider mapping:

```ts
type NormalizedMessage = {
  role: "user" | "model" | "system" | "developer" | "tool"
  content: NormalizedPart[]
}

type NormalizedPart =
  | { type: "text"; text: string }
  | { type: "image"; source: MediaSource; mimeType: string; detail?: "auto" | "low" | "high" }
  | { type: "audio"; source: MediaSource; mimeType: string; transcript?: string }
  | { type: "video"; source: MediaSource; mimeType: string; transcript?: string; sampledFrames?: NormalizedPart[] }
  | { type: "file"; source: MediaSource; mimeType: string; name?: string }

type MediaSource =
  | { kind: "inline"; dataBase64: string }
  | { kind: "url"; url: string }
  | { kind: "file"; path: string }
  | { kind: "provider_file"; provider: string; idOrUri: string }
  | { kind: "blob_ref"; ref: string }
```

The provider drivers should never receive arbitrary user input. They should receive normalized messages and be responsible only for converting normalized parts to provider-specific payloads.

Normalization should sit below every target SDK and above provider drivers:

```text
.agent DSL
  -> compiler AST
  -> IR input type/null shape
  -> target codegen run signature
  -> FFI JSON envelope
  -> Rust runtime normalized messages
  -> provider mapper
```

Even if TypeScript, Python, Rust, and Dart expose different native type ergonomics, they should all serialize to the same FFI envelope.

Recommended FFI envelope for media-capable input:

```json
{
  "type": "content",
  "parts": [
    { "type": "text", "text": "Explain this" },
    { "type": "image", "path": "./diagram.png", "mimeType": "image/png" }
  ]
}
```

Text-only agents can continue sending a raw string through the existing text path.

## Provider Mapping

### OpenAI Responses

```ts
function toOpenAI(messages: NormalizedMessage[]) {
  return messages.map((message) => ({
    type: "message",
    role: mapRoleForOpenAI(message.role),
    content: message.content.map((part) => {
      switch (part.type) {
        case "text":
          return { type: "input_text", text: part.text }
        case "image":
          return {
            type: "input_image",
            image_url: imageSourceToUrl(part.source),
            detail: part.detail ?? "auto",
          }
        case "file":
          return fileSourceToOpenAIInputFile(part)
        case "audio":
          return audioSourceToOpenAIInputAudio(part)
        default:
          throw unsupported("openai", part)
      }
    }),
  }))
}
```

OpenAI-specific notes:

- `image_url` can be a normal URL or a data URL.
- Files should prefer `file_id` when already uploaded, `file_url` for remote files, and base64 `file_data` only when appropriate.
- Realtime should not be hidden behind the same synchronous `run()` path for streamed microphone audio. It needs a separate streaming transport surface.

### Gemini

```ts
function toGemini(messages: NormalizedMessage[]) {
  return messages.map((message) => ({
    role: message.role === "model" ? "model" : "user",
    parts: message.content.map((part) => {
      switch (part.type) {
        case "text":
          return { text: part.text }
        case "image":
        case "audio":
        case "video":
        case "file":
          return mediaSourceToGeminiPart(part)
      }
    }),
  }))
}
```

Gemini-specific notes:

- Inline media maps to `inline_data`.
- Uploaded or reusable media maps to `file_data`.
- The driver should auto-upload `path` media to the Gemini Files API when the payload is large or explicitly marked reusable.

### Anthropic

```ts
function toAnthropic(messages: NormalizedMessage[]) {
  return messages.map((message) => ({
    role: message.role === "model" ? "assistant" : "user",
    content: message.content.map((part) => {
      switch (part.type) {
        case "text":
          return { type: "text", text: part.text }
        case "image":
          return imageSourceToAnthropicBlock(part)
        case "file":
          return fileSourceToAnthropicDocumentBlock(part)
        default:
          throw unsupported("anthropic", part)
      }
    }),
  }))
}
```

Anthropic-specific notes:

- Images map to image content blocks.
- PDFs/documents map to `document` blocks.
- File IDs can come from Anthropic's Files API where supported.
- Unsupported modality should fail clearly before the HTTP request.

## Runtime Shape

If the compiler says the agent is text-only, the public generated API should remain string-shaped.

If the compiler says the agent accepts media, the runtime should move from string-only turns:

```ts
type Turn = {
  input: string
  model_response: string
}
```

to content turns:

```ts
type Turn = {
  input: AuwgentContent
  modelResponse: AuwgentContent
}

type AuwgentContent = {
  parts: AuwgentInputPart[]
}
```

For text-only generated agents:

```ts
"hello"
```

normalizes to:

```ts
{
  parts: [{ type: "text", text: "hello" }]
}
```

This is not a backward-compatibility escape hatch. It is a compiler-selected API:

- text-only DSL input generates `run(text)`.
- media-capable DSL input generates `run(parts)` plus text convenience where the target language can express it cleanly.

The session should store stable metadata and references, not raw large media bytes:

```ts
{
  input: {
    parts: [
      { type: "text", text: "Summarize this file" },
      {
        type: "file",
        ref: "blob://session/abc123",
        name: "report.pdf",
        mimeType: "application/pdf"
      }
    ]
  }
}
```

## Transport Policy

The DX should be provider-neutral, but transport cannot be ignored. Add a resolver before provider mapping:

```ts
type MediaTransportPolicy = {
  inlineMaxBytes?: number
  preferUpload?: boolean
  reuseUploads?: boolean
  allowRemoteUrls?: boolean
}
```

Default behavior:

1. Text stays inline.
2. Small images can become inline data URLs or provider inline parts.
3. Large local files should upload through the provider file API when supported.
4. Remote URLs should pass through only when the provider supports URL inputs for that modality.
5. Raw bytes should not be persisted into sessions; persist a blob reference or provider file reference.

## Recommended Implementation Plan

1. Add DSL tokens and AST variants for `Image`, `File`, `Audio`, and `Video`.
2. Update checker rules:
   - root `input` can be `Text` or a union of media primitives.
   - media implies text in normalized capabilities.
   - reject structured root input as today unless that design changes separately.
3. Keep `Text` lowering to `null`; lower media input using the existing type-shape style.
4. Update IR schema/runtime type helpers only where needed for the new media primitive strings.
5. Add shared content-part SDK types in TS, Rust, Python, and Dart.
6. Update each target codegen:
   - TS generated `AuwgentInput` and `agent.run(input: AuwgentInput)`.
   - Rust generated `AuwgentInput` enum/struct and `run(input: Option<AuwgentInput>)`.
   - Python generated typing around `str | list[Part]`.
   - Dart generated typed input classes and `run` overload-like helpers.
7. Update FFI boundaries so media-capable targets serialize a common content envelope.
8. Change provider drivers to accept normalized message content instead of flat text messages.
9. Add provider mappers:
   - `openai_content_mapper`
   - `gemini_content_mapper`
   - `anthropic_content_mapper` if Anthropic becomes a first-class driver
10. Add a media resolver:
   - detect MIME type
   - base64 encode inline data
   - upload files when needed
   - produce provider file references
11. Update session export to store content references, not raw media bytes.
12. Add clear compile-time or startup errors for unsupported provider/modality combinations.

## Main Design Rule

Auwgent should expose one compiler-selected friendly input format:

```ts
await agent.run([
  { type: "text", text: "Explain this" },
  { type: "image", path: "./diagram.png" },
  { type: "file", path: "./paper.pdf" }
])
```

Only when the `.agent` file declares media input.

For `input: Text`, keep:

```ts
await agent.run("hello")
```

Provider drivers should own the messy conversion:

- OpenAI: message content parts like `input_text`, `input_image`, `input_file`, and Realtime audio events.
- Gemini: `contents[].parts[]` with `text`, `inline_data`, or `file_data`.
- Anthropic: `messages[].content[]` blocks like `text`, `image`, and `document`.

This keeps user code clean while preserving provider-specific capability boundaries.
