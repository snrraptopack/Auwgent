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
await agent.run([
  { type: "text", text: "What is in this image?" },
  { type: "image", path: "./photo.png", mimeType: "image/png" },
])
```

The supported part shapes are:

```ts
type AuwgentTextPart = {
  type: "text"
  text: string
}

type AuwgentImagePart = {
  type: "image"
  data?: ArrayBuffer | Uint8Array | string
  encoding?: "base64" | "utf8"
  path?: string
  url?: string
  ref?: string
  mimeType?: string
  detail?: "auto" | "low" | "high"
}

type AuwgentFilePart = {
  type: "file"
  data?: ArrayBuffer | Uint8Array | string
  encoding?: "base64" | "utf8"
  path?: string
  url?: string
  ref?: string
  mimeType?: string
  name?: string
}

type AuwgentAudioPart = {
  type: "audio"
  data?: ArrayBuffer | Uint8Array | string
  encoding?: "base64" | "utf8"
  path?: string
  url?: string
  ref?: string
  mimeType?: string
  transcript?: string
}

type AuwgentVideoPart = {
  type: "video"
  data?: ArrayBuffer | Uint8Array | string
  encoding?: "base64" | "utf8"
  path?: string
  url?: string
  ref?: string
  mimeType?: string
  transcript?: string
  sampledFrames?: AuwgentImagePart[]
}
```

If the DSL says:

```agent
input: Image | File
```

then TypeScript permits text, image, and file parts:

```ts
await agent.run([
  { type: "text", text: "Summarize this" },
  { type: "file", path: "./report.pdf", mimeType: "application/pdf" },
])
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

## Rust Usage

Text-only generated Rust agents keep the existing string-like path through `Option<AuwgentInput>` for the generated input type.

For media-capable agents, construct input parts from the target SDK types and pass them through the generated agent input type. A direct SDK-level media part looks like:

```rust
use auwgent_sdk::{
    AuwgentImagePart,
    AuwgentInputPart,
    AuwgentMediaSource,
    AuwgentTextPart,
};

let input = vec![
    AuwgentInputPart::Text(AuwgentTextPart {
        text: "What is in this image?".to_string(),
    }),
    AuwgentInputPart::Image(AuwgentImagePart {
        source: AuwgentMediaSource {
            path: Some("./photo.png".to_string()),
            mime_type: Some("image/png".to_string()),
            ..Default::default()
        },
        detail: Some("auto".to_string()),
    }),
];
```

For `input: Image | File`, the generated Rust input type should only allow text, image, and file-compatible values. Audio and video should remain outside that generated type.

## Python Usage

Text-only generated Python agents keep the simple string call:

```python
session = await agent.run("hello")
```

For media-capable agents, pass a list of typed input-part dictionaries:

```python
from auwgent_sdk import AuwgentImagePart, AuwgentTextPart

input_parts: list[AuwgentTextPart | AuwgentImagePart] = [
    {"type": "text", "text": "What is in this image?"},
    {
        "type": "image",
        "path": "./photo.png",
        "mimeType": "image/png",
        "detail": "auto",
    },
]

session = await agent.run(input_parts)
```

For file input:

```python
session = await agent.run([
    {"type": "text", "text": "Summarize this document"},
    {
        "type": "file",
        "path": "./report.pdf",
        "mimeType": "application/pdf",
        "name": "report.pdf",
    },
])
```

## Dart Usage

Text-only generated Dart agents keep the simple string call:

```dart
final session = await agent.run('hello');
```

For media-capable agents, use the shared sealed input part classes:

```dart
final session = await agent.run([
  const sdk.AuwgentTextPart('What is in this image?'),
  const sdk.AuwgentImagePart(
    path: './photo.png',
    mimeType: 'image/png',
    detail: 'auto',
  ),
]);
```

For `input: Image | File`, use text, image, and file parts:

```dart
final session = await agent.run([
  const sdk.AuwgentTextPart('Summarize this document'),
  const sdk.AuwgentFilePart(
    path: './report.pdf',
    mimeType: 'application/pdf',
    name: 'report.pdf',
  ),
]);
```

Each Dart input part exposes `toJson()`, so generated wrappers can serialize the list before crossing the FFI/runtime boundary.

## Current Runtime Boundary

The TypeScript wrapper still calls the native runtime through the existing string-oriented N-API boundary. For non-string input, it serializes the input array to JSON before passing it into native code. The native binding already parses JSON strings into runtime values, so the engine receives structured input without requiring an immediate N-API signature change.

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
- `targets/dart/lib/src/types.dart`
- `targets/rust/src/lib.rs`

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
```

## Notes On Existing Dirty Files

At the time this change was made, some files under `targets/typescript/verfication` and `targets/typescript/package.json` were already modified in the working tree. Those changes were not reset, cleaned, or intentionally folded into the multimodal input work.

That matters because this repo was already dirty before the multimodal edits. The multimodal change should be reviewed by focusing on the compiler, target SDK type additions, TypeScript runtime input typing, and this changelog entry.
