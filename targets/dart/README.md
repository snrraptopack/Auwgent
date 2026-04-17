# auwgent-sdk (Dart)

Experimental Dart target for Auwgent built on top of the shared C ABI in [c-abi](../../c-abi).

## Status

This target is experimental.

The goal is to stabilize:

- engine handle lifecycle
- string ownership rules
- session import/export
- prompt generation
- run/stream methods
- middleware and intent hooks
- a generated SDK shape that matches the TypeScript model

## Requirements

- Dart SDK 3.11 or later
- the native Auwgent C ABI library built and available to load

## Dependency Versions

This scaffold uses:

- Dart SDK `3.11.x`
- `ffi: ^2.2.0`

## API Shape

There are two layers to keep in mind:

1. Low-level runtime API

This is the runtime/bootstrap layer. It accepts compiled IR directly:

```dart
import 'dart:convert';
import 'dart:io';

import 'package:auwgent_sdk_dart/auwgent.dart';

void main() {
  final ir = Map<String, Object?>.from(
    jsonDecode(File('demo.agent.json').readAsStringSync()) as Map,
  );

  final agent = createAuwgent(
    ir,
    const AuwgentConfig(
      apiKeys: {
        'openaiApiKey': '...',
      },
    ),
  );

  try {
    print(agent.generatePrompt());
  } finally {
    agent.dispose();
  }
}
```

2. Generated agent API

This is the shape we want the compiler to emit, similar to TypeScript:

```dart
import 'package:my_app/demo.agent.dart';

Future<void> main() async {
  final agent = createDemo(
    const DemoConfig(
      apiKeys: DemoApiKeys(openaiApiKey: '...'),
      tools: {
        'user_name': _getUserName,
      },
    ),
  );

  agent.onIntentHandler(_DemoIntentLogger());

  try {
    final session = await agent.run('what is my name');
    print(session.turns.last.modelResponse);
  } finally {
    agent.dispose();
  }
}

Future<Object?> _getUserName(Map<String, Object?> args) async {
  return 'Theo';
}

final class _DemoIntentLogger extends DemoBaseIntentHandler {
  @override
  Object? responseText(DemoResponseTextIntent intent, String agentName) {
    print('$agentName -> ${intent['text']}');
    return null;
  }
}
```

The generated layer is where helpers, outputs, tools, middleware, and custom intents should feel natural. `createAuwgent(...)` is the general runtime constructor underneath that, and in normal usage the CLI/codegen layer will wrap it for you.

## Async Tools

Tool handlers in Dart may return either a plain value or a `Future`.

```dart
final agent = createDemo(
  DemoConfig(
    tools: {
      'lookup_user': (args) async {
        await Future<void>.delayed(const Duration(milliseconds: 10));
        return {'id': args['id'], 'name': 'Theo'};
      },
    },
  ),
);
```

The Dart wrapper now routes tools through the async C ABI callback path, so tool completion is resolved back into Rust when the `Future` completes.

## Protocol Model

Auwgent currently uses a compiler-driven block protocol for model control.

- Model-facing output is emitted as protocol text such as `[tool_call: ...]`, `[response_text]`, and `[schema: Output]`.
- The runtime parses those protocol blocks into structured intents and typed SDK models.
- Session history keeps the raw model transcript in `model_response` so debugging, replay, and follow-up turns see the exact protocol the model produced.
- User-facing code should generally consume the parsed structured layer rather than manually parsing `model_response`.

Example raw transcript stored in session state:

```json
{
  "model_response": "[schema: Output]\nage: 25\nlocation: \"Tarkwa\"\nname: \"Theo\"\n[/schema]"
}
```

This is intentional. The raw transcript is the internal control representation; typed intents and structured output are the app-facing representation.

## Provider Tool Calling

OpenAI and Gemini are currently used as text-generation transports for the block protocol. Auwgent does not yet rely on provider-native tool calling for normal user-defined tools.

- Today, the compiler teaches models to emit protocol blocks and the runtime executes tools itself.
- Future built-in tools may use provider-native tool APIs directly.
- Until that built-in/native-tools work lands, targets and drivers should be understood as protocol-first rather than native-tool-first.

This distinction matters when reading runtime behavior: a provider may support native tools, but the current Auwgent execution model is intentionally provider-agnostic and protocol-driven.

## Current Direction

The Dart SDK should follow the TypeScript split:

- runtime package exposes a general typed engine wrapper via `createAuwgent(...)`
- compiler codegen emits agent-specific factories and types
- app code mainly interacts with generated `create<AgentName>(...)` APIs, not handwritten IR maps
