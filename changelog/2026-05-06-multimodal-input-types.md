# Multimodal Input Types

Date: 2026-05-06

## Summary

The compiler and target SDKs now understand these root input datatypes:

- `Image`
- `File`
- `Audio`
- `Video`

`Text` remains the default input type. It is intentionally represented as `null` in the IR, so text-only agents keep the existing compact runtime shape and generated TypeScript usage stays as `agent.run("...")`.

## IR Shape

This change does not introduce a new `kind` wrapper for media inputs. It follows the existing IR style:

```agent
agent TextOnly {
  default config {
    model: gemini("gemini-2.5-flash")
    prompt: "Answer the user"
  }

  input: Text
}
```

Lowers to:

```json
{
  "input": null
}
```

```agent
agent ImageReader {
  default config {
    model: gemini("gemini-2.5-flash")
    prompt: "Describe the image"
  }

  input: Image
}
```

Lowers to:

```json
{
  "input": "image"
}
```

```agent
agent MediaReader {
  default config {
    model: gemini("gemini-2.5-flash")
    prompt: "Use the provided input"
  }

  input: Image | File
}
```

Lowers to:

```json
{
  "input": {
    "type": "union",
    "options": ["image", "file"]
  }
}
```

`Text` is implicit when media is present:

```agent
input: Text | Image
```

Lowers to:

```json
{
  "input": "image"
}
```

The runtime and generated SDK types understand that media-capable inputs also accept text parts. Text is not emitted into the media IR because `null` is reserved as the text/default signal.

## TypeScript Usage

For text-only agents, generated code remains:

```ts
await agent.run("hello")
```

For media-capable agents, generated code expects an array of input parts:

```ts
import { input } from "./main.agent.types"
import type { Input } from "./main.agent.types"

await agent.run([
  input.text("What is in this image?"),
  input.image({ path: "./photo.png", mimeType: "image/png" }),
] satisfies Input)
```

Base64 is represented as data plus encoding:

```ts
await agent.run([
  input.text("Read this image"),
  input.image({
    data: "iVBORw0KGgoAAAANSUhEUg...",
    encoding: "base64",
    mimeType: "image/png",
  }),
] satisfies Input)
```

The generated types file exposes the agent-specific input type, shared part aliases, and the generated `input` builder:

```ts
import { input } from "./main.agent.types"
import type {
  Input,
  InputPart,
  TextPart,
  ImagePart,
  FilePart,
  AudioPart,
  VideoPart,
} from "./main.agent.types"
```

The base SDK still defines the low-level shapes, but application code should import the public names from the generated file. That keeps the user-facing API tied to what the compiler knows about that specific agent.

If the DSL says:

```agent
input: Image | File
```

then TypeScript permits text, image, and file parts:

```ts
await agent.run([
  input.text("Summarize this"),
  input.file({ path: "./report.pdf", mimeType: "application/pdf" }),
] satisfies Input)
```

It does not permit audio or video parts for that agent.

## Other Targets

The shared media part types were also added to:

- Rust target SDK
- Python target SDK
- Dart target SDK

The compiler codegen maps IR media names to the corresponding target SDK types:

- `image` -> image input part
- `file` -> file input part
- `audio` -> audio input part
- `video` -> video input part

Each generated target layer now exposes those media names back out through generated aliases. User code should import the generated file/module first and treat direct SDK imports as an implementation detail.

## Rust Usage

Text-only generated Rust agents keep the existing string-like path through `Option<Input>` for the generated input type.

For media-capable agents, construct input parts from the generated module. The generated module re-exports the shared media part aliases, so user code does not need to import from `auwgent_sdk_rust` directly.

```rust
use crate::main_agent::{
    input,
    Input,
    MediaSource,
};

let user_input: Input = vec![
    input::text("What is in this image?"),
    input::image(
        MediaSource {
            path: Some("./photo.png".to_string()),
            mime_type: Some("image/png".to_string()),
            ..Default::default()
        },
        Some("auto".to_string()),
    ),
];
```

For `input: Image | File`, user code should still import the input aliases from the generated module. The generated module owns the public input type, even when the underlying representation delegates to shared SDK media parts.

## Python Usage

Text-only generated Python agents keep the simple string call:

```python
session = await agent.run("hello")
```

For media-capable agents, import the generated aliases from the generated Python types file:

```python
from main_types import Input, input

input_parts: Input = [
    input.text("What is in this image?"),
    input.image(path="./photo.png", mimeType="image/png", detail="auto"),
]

session = await agent.run(input_parts)
```

For file input:

```python
from main_types import Input, input

input_parts: Input = [
    input.text("Summarize this document"),
    input.file(path="./report.pdf", mimeType="application/pdf", name="report.pdf"),
]

session = await agent.run(input_parts)
```

## Dart Usage

Text-only generated Dart agents keep the simple string call:

```dart
final session = await agent.run('hello');
```

For media-capable agents, use the shared sealed input part classes:

```dart
import 'main.agent.dart';

final session = await agent.run([
  input.text('What is in this image?'),
  input.image(
    path: './photo.png',
    mimeType: 'image/png',
    detail: 'auto',
  ),
]);
```

For `input: Image | File`, use text, image, and file parts:

```dart
final session = await agent.run([
  input.text('Summarize this document'),
  input.file(
    path: './report.pdf',
    mimeType: 'application/pdf',
    name: 'report.pdf',
  ),
]);
```

Each Dart input part exposes `toJson()`, so generated wrappers can serialize the list before crossing the FFI/runtime boundary.

## Current Runtime Boundary

The TypeScript wrapper still calls the native runtime through the existing string-oriented N-API boundary. For non-string input, it serializes the input array to JSON before passing it into native code. The native binding already parses JSON strings into runtime values, so the engine receives structured input without requiring an immediate N-API signature change.

## Runtime Session And Provider Mapping

Structured media input is no longer treated as a plain JSON string in the conversation history.

When a media-capable generated wrapper calls `agent.run([...])`, the runtime now stores two representations on the turn:

```json
{
  "input": "What is in this image?\n[image: ./photo.png]",
  "inputParts": [
    { "type": "text", "text": "What is in this image?" },
    {
      "type": "image",
      "path": "./photo.png",
      "mimeType": "image/png",
      "detail": "auto"
    }
  ],
  "modelResponse": ""
}
```

`input` is only the compact transcript/display string. It is useful for persistence diffs, logs, binding cursor behavior, and humans reading `data.json`.

`inputParts` is the source of truth for provider submission. When a saved session is loaded again, `SessionState::to_messages()` rebuilds the user message from `inputParts`; it does not parse the compact display string back into media.

This means placeholder text like `[image: ./photo.png]` is not a protocol that providers parse. It is only a fallback/display label. OpenAI and Gemini receive the structured parts directly:

- text parts become provider text parts
- image parts are sent as OpenAI `image_url` content when a URL/data/path can be resolved
- image/file/audio/video parts are sent to Gemini as `inline_data` when data/path can be resolved, or `file_data` for URL inputs
- unresolved non-text media falls back to a compact text label so the model still has a reference instead of losing the part silently

Inline `data` supports both base64 and UTF-8 source strings:

```ts
input.image({
  data: "iVBORw0KGgoAAAANSUhEUg...",
  encoding: "base64",
  mimeType: "image/png",
})

input.file({
  data: "plain text contents",
  encoding: "utf8",
  mimeType: "text/plain",
})
```

For path inputs, native runtimes read the file bytes and encode them for the provider. The wasm runtime cannot read local paths directly, so browser/wasm callers should pass URL, base64 data, or a future uploaded file reference.

## Media Source Flow

Media parts are provider-neutral at the generated API boundary. The generated `input` helper only describes where the bytes or provider-readable media live; the runtime/provider adapter decides how to submit that source to the selected model provider.

Supported source fields:

```ts
input.image({
  path: "./photo.png",
  mimeType: "image/png",
})

input.image({
  url: "https://example.com/photo.png",
  mimeType: "image/png",
})

input.image({
  data: "iVBORw0KGgoAAAANSUhEUg...",
  encoding: "base64",
  mimeType: "image/png",
})

input.file({
  data: "plain text contents",
  encoding: "utf8",
  mimeType: "text/plain",
})
```

### Public Interface Shape

The generated target APIs should expose these shapes through the generated files/modules. User code should not need to import low-level SDK names directly.

Conceptually, every media part has one required `type` plus one source field:

```ts
type MediaSource =
  | {
      path: string
      mimeType?: string
    }
  | {
      url: string
      mimeType?: string
    }
  | {
      data: string
      encoding: "base64" | "utf8"
      mimeType?: string
    }
  | {
      ref: string
      mimeType?: string
    }

type TextPart = {
  type: "text"
  text: string
}

type ImagePart = MediaSource & {
  type: "image"
  detail?: "auto" | "low" | "high"
}

type FilePart = MediaSource & {
  type: "file"
  name?: string
}

type AudioPart = MediaSource & {
  type: "audio"
  transcript?: string
}

type VideoPart = MediaSource & {
  type: "video"
  transcript?: string
  sampledFrames?: ImagePart[]
}

type InputPart = TextPart | ImagePart | FilePart | AudioPart | VideoPart
```

Generated builders keep users away from writing `type` manually:

```ts
input.text("Summarize this PDF")

input.file({
  path: "./report.pdf",
  mimeType: "application/pdf",
  name: "report.pdf",
})

input.file({
  url: "https://example.com/report.pdf",
  mimeType: "application/pdf",
  name: "report.pdf",
})

input.file({
  data: "JVBERi0xLjQKJc...",
  encoding: "base64",
  mimeType: "application/pdf",
  name: "report.pdf",
})
```

The same source fields apply to image, file, audio, and video. A PDF is modeled as `input.file(...)`, not as a special PDF input type.

### `path`

`path` means "read this file from the machine where the runtime is executing."

For native targets, the provider adapter calls `std::fs::read(path)` before the provider request is sent. Relative paths are resolved by the current working directory of the host process, not by the `.agent` file location and not by the generated types file location.

That means:

```ts
input.image({ path: "./photo.png", mimeType: "image/png" })
```

works only when `./photo.png` exists relative to the process that is running the agent.

For example, if the app is started from:

```sh
C:\Users\babyface\Desktop\auwgent\Auwgent\targets\typescript\verfication
```

then `./photo.png` means:

```sh
C:\Users\babyface\Desktop\auwgent\Auwgent\targets\typescript\verfication\photo.png
```

If the same app is started from the repo root, the same relative path points to:

```sh
C:\Users\babyface\Desktop\auwgent\Auwgent\photo.png
```

So production apps should prefer absolute paths or normalize paths before calling `agent.run`.

`path` is not for media stored in S3, a CDN, a database, a browser `File`, a mobile asset bundle, or another machine. In those cases the runtime cannot read the bytes from `path`.

### `url`

`url` means "the provider can fetch or accept this remote media URI."

Use `url` for already-hosted media:

```ts
input.image({
  url: "https://cdn.example.com/uploads/photo.png",
  mimeType: "image/png",
})
```

Provider behavior differs:

- OpenAI image input can use the URL directly through image URL content.
- Gemini maps URLs to `file_data.file_uri`.
- Some providers may not support arbitrary remote URLs, in which case their adapter should either upload/fetch first or return a clear unsupported-source error.

The runtime should not treat a URL as a local path.

### `data` With `encoding: "base64"`

`encoding: "base64"` means the caller has already encoded the bytes.

The runtime does not encode this again:

```ts
input.image({
  data: "iVBORw0KGgoAAAANSUhEUg...",
  encoding: "base64",
  mimeType: "image/png",
})
```

This is useful when the file is stored somewhere the runtime cannot read directly, but the host app can fetch/read it first and pass the encoded contents into Auwgent.

This does not require a separate public "base64 shape" or a different builder like `input.fileBase64`. It is still just `input.file({ data, encoding: "base64", ... })`.

For binary files, `data` should be the base64 body only, not a full data URL:

```ts
input.file({
  data: "JVBERi0xLjQKJc...",
  encoding: "base64",
  mimeType: "application/pdf",
})
```

not:

```ts
input.file({
  data: "data:application/pdf;base64,JVBERi0xLjQKJc...",
  encoding: "base64",
  mimeType: "application/pdf",
})
```

### `data` With `encoding: "utf8"`

`encoding: "utf8"` means the caller is passing plain text.

The runtime encodes the UTF-8 bytes before provider submission:

```ts
input.file({
  data: "plain text contents",
  encoding: "utf8",
  mimeType: "text/plain",
})
```

This is mostly useful for text-like files. It should not be used for binary image/audio/video bytes unless the bytes were intentionally converted into a valid UTF-8 string, which is usually not what is wanted.

### Internal Base64 Encoding

The runtime still needs an internal base64 encoder, but not because the user selected `encoding: "base64"`.

It is used only when the runtime has raw bytes or text that must be converted before provider submission:

- `path`: native runtime reads local file bytes, then encodes those bytes
- `encoding: "utf8"`: runtime encodes the UTF-8 text bytes

It is not used for:

- `encoding: "base64"`: the caller already encoded the bytes
- `url`: the provider adapter passes the URL or maps it to the provider's URL/file URI shape
- `ref`: future upload registry should resolve the reference before provider submission

### Future `ref`

The source shape already reserves the idea of `ref` for runtime-managed or app-managed uploaded files:

```ts
input.file({
  ref: "upload_123",
  mimeType: "application/pdf",
})
```

`ref` should mean "resolve this through an upload/file registry." That registry does not exist yet in this change. Until it does, users should use `path`, `url`, or `data`.

### Provider Submission Summary

The provider never receives the generated `input.image(...)` helper object directly. The flow is:

```text
user code
  -> generated input part array
  -> runtime session turn
       input: compact display string
       inputParts: structured source objects
  -> provider adapter
       OpenAI/Gemini/etc-specific message content
  -> model provider
```

For saved sessions, `inputParts` is persisted. When the session is loaded again, provider messages are rebuilt from `inputParts`, not from the compact `input` string.

## Files Changed

Compiler:

- `auwgent-compiler/crates/auwgent-ast/src/lib.rs`
- `auwgent-compiler/crates/auwgent-lexer/src/lib.rs`
- `auwgent-compiler/crates/auwgent-parser/src/types.rs`
- `auwgent-compiler/crates/auwgent-checker/src/lib.rs`
- `auwgent-compiler/crates/auwgent-ir/src/lib.rs`
- `auwgent-compiler/crates/auwgent-codegen/src/typescript.rs`
- `auwgent-compiler/crates/auwgent-codegen/src/python.rs`
- `auwgent-compiler/crates/auwgent-codegen/src/dart.rs`
- `auwgent-compiler/crates/auwgent-codegen/src/rust.rs`

Target SDKs:

- `targets/typescript/types.ts`
- `targets/typescript/auwgent.ts`
- `targets/python/auwgent_sdk.py`
- `targets/python/auwgent_sdk/__init__.py`
- `targets/dart/lib/src/types.dart`
- `targets/rust/src/lib.rs`

Runtime:

- `ir-runtime/src/runtime/session.rs`
- `ir-runtime/src/runtime/engine/runtime_loop.rs`
- `ir-runtime/src/runtime/drivers/openai.rs`
- `ir-runtime/src/runtime/drivers/gemini.rs`
- `ir-runtime/tests/multimodal_session_test.rs`

Design note:

- `MULTIMODAL_INPUT_DESIGN.md`

## Verification

The change was checked with:

```sh
cargo check --manifest-path auwgent-compiler/Cargo.toml
cargo test --manifest-path auwgent-compiler/Cargo.toml -p auwgent-ir -p auwgent-checker -p auwgent-parser -p auwgent-lexer -p auwgent-analysis -p auwgent-codegen
cargo check --manifest-path targets/rust/Cargo.toml
dart analyze targets/dart
python -m py_compile targets/python/auwgent_sdk.py
bun run test
cargo test --manifest-path ir-runtime/Cargo.toml
cargo check --manifest-path targets/wasm-runtime/Cargo.toml --target wasm32-unknown-unknown
```

## Notes On Existing Dirty Files

At the time this change was made, some files under `targets/typescript/verfication` and `targets/typescript/package.json` were already modified in the working tree. Those changes were not reset, cleaned, or intentionally folded into the multimodal input work.

That matters because this repo was already dirty before the multimodal edits. The multimodal change should be reviewed by focusing on the compiler, target SDK type additions, TypeScript runtime input typing, and this changelog entry.
