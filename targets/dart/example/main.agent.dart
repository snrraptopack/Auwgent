// Auto-generated Dart bindings for Hello
// Do not edit manually
import 'dart:async';
import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;
import 'main.agent.ir.dart';
typedef Person = sdk.JsonMap;

typedef HelloInput = String;

final class HelloOutput {
  const HelloOutput({
    required this.name,
    required this.age,
  });

  final String name;
  final double age;

  factory HelloOutput.fromJson(sdk.JsonMap json) {
    return HelloOutput(
      name: (json['name'] as String?) ?? '',
      age: ((json['age'] as num?)?.toDouble()) ?? 0,
    );
  }
}

typedef HelloContext = sdk.JsonMap;

typedef HelloGetDetailsToolHandler = FutureOr<HelloGetDetailsToolResultValue> Function();
typedef HelloGetLocationToolHandler = FutureOr<HelloGetLocationToolResultValue> Function(HelloGetLocationToolArgs args);

abstract class HelloTools {
  const HelloTools();

  FutureOr<HelloGetDetailsToolResultValue> getDetails();
  FutureOr<HelloGetLocationToolResultValue> getLocation(HelloGetLocationToolArgs args);

  Map<String, sdk.ToolHandler> toMap() {
    return {
      'get_details': (_) => getDetails(),
      'get_location': (args) => getLocation(HelloGetLocationToolArgs.fromJson(Map<String, Object?>.from((args as Map?) ?? const {}))),
    };
  }
}

final class HelloToolRegistry extends HelloTools {
  const HelloToolRegistry({
    required HelloGetDetailsToolHandler getDetails,
    required HelloGetLocationToolHandler getLocation,
  }) :
      _getDetails = getDetails,
      _getLocation = getLocation;

  final HelloGetDetailsToolHandler _getDetails;
  final HelloGetLocationToolHandler _getLocation;

  @override
  FutureOr<HelloGetDetailsToolResultValue> getDetails() => _getDetails();
  @override
  FutureOr<HelloGetLocationToolResultValue> getLocation(HelloGetLocationToolArgs args) => _getLocation(args);
}

final class HelloResponseTextIntent {
  const HelloResponseTextIntent({
    required this.text,
  });

  final String text;

  factory HelloResponseTextIntent.fromJson(sdk.JsonMap json) {
    return HelloResponseTextIntent(
      text: (json['text'] as String?) ?? '',
    );
  }
}

final class HelloResponseSchemaIntent {
  const HelloResponseSchemaIntent({
    required this.type,
    required this.response,
  });

  final String type;
  final HelloOutput response;

  factory HelloResponseSchemaIntent.fromJson(sdk.JsonMap json) {
    return HelloResponseSchemaIntent(
      type: (json['type'] as String?) ?? '',
      response: HelloOutput.fromJson(Map<String, Object?>.from((json['response'] as Map?) ?? const {})),
    );
  }
}

final class HelloErrorIntent {
  const HelloErrorIntent({
    required this.message,
  });

  final String message;

  factory HelloErrorIntent.fromJson(sdk.JsonMap json) {
    return HelloErrorIntent(
      message: (json['message'] as String?) ?? '',
    );
  }
}

abstract class HelloToolCallIntent {
  const HelloToolCallIntent();

  String get type;
  Object? get args;

  factory HelloToolCallIntent.fromJson(sdk.JsonMap json) {
    final kind = (json['type'] as String?) ?? '';
    if (kind == 'get_details') {
      return HelloGetDetailsToolCallIntentCase.fromJson(json);
    }
    if (kind == 'get_location') {
      return HelloGetLocationToolCallIntentCase.fromJson(json);
    }
    return HelloToolCallIntentUnknown(Map<String, Object?>.from(json));
  }
}

abstract class HelloToolResultIntent {
  const HelloToolResultIntent();

  String get name;
  Object? get args;
  Object? get result;
  bool get overridden;

  factory HelloToolResultIntent.fromJson(sdk.JsonMap json) {
    final kind = (json['name'] as String?) ?? '';
    if (kind == 'get_details') {
      return HelloGetDetailsToolResultIntentCase.fromJson(json);
    }
    if (kind == 'get_location') {
      return HelloGetLocationToolResultIntentCase.fromJson(json);
    }
    return HelloToolResultIntentUnknown(Map<String, Object?>.from(json));
  }
}

typedef HelloGetDetailsToolResultValue = String;

final class HelloGetDetailsToolCallIntentCase extends HelloToolCallIntent {
  const HelloGetDetailsToolCallIntentCase();

  @override
  sdk.NoArgs get args => const sdk.NoArgs();

  @override
  String get type => 'get_details';

  factory HelloGetDetailsToolCallIntentCase.fromJson(sdk.JsonMap json) {
    return const HelloGetDetailsToolCallIntentCase();
  }
}

final class HelloGetDetailsToolResultIntentCase extends HelloToolResultIntent {
  const HelloGetDetailsToolResultIntentCase({
    required this.result,
    this.overridden = false,
  });

  @override
  sdk.NoArgs get args => const sdk.NoArgs();
  @override
  final HelloGetDetailsToolResultValue result;
  @override
  final bool overridden;

  @override
  String get name => 'get_details';

  factory HelloGetDetailsToolResultIntentCase.fromJson(sdk.JsonMap json) {
    return HelloGetDetailsToolResultIntentCase(
      result: (json['result'] as String?) ?? '',
      overridden: (json['overridden'] as bool?) ?? false,
    );
  }
}

final class HelloGetDetailsToolSkippedIntentCase extends HelloToolSkippedIntent {
  const HelloGetDetailsToolSkippedIntentCase();

  @override
  sdk.NoArgs get args => const sdk.NoArgs();

  @override
  String get type => 'get_details';

  factory HelloGetDetailsToolSkippedIntentCase.fromJson(sdk.JsonMap json) {
    return const HelloGetDetailsToolSkippedIntentCase();
  }
}

final class HelloGetLocationToolArgs {
  const HelloGetLocationToolArgs({
    required this.id,
  });

  final String id;

  factory HelloGetLocationToolArgs.fromJson(sdk.JsonMap json) {
    return HelloGetLocationToolArgs(
      id: (json['id'] as String?) ?? '',
    );
  }
}

typedef HelloGetLocationToolResultValue = String;

final class HelloGetLocationToolCallIntentCase extends HelloToolCallIntent {
  const HelloGetLocationToolCallIntentCase({
    required this.args,
  });

  @override
  final HelloGetLocationToolArgs args;

  @override
  String get type => 'get_location';

  factory HelloGetLocationToolCallIntentCase.fromJson(sdk.JsonMap json) {
    return HelloGetLocationToolCallIntentCase(
      args: HelloGetLocationToolArgs.fromJson(Map<String, Object?>.from((json['args'] as Map?) ?? const {})),
    );
  }
}

final class HelloGetLocationToolResultIntentCase extends HelloToolResultIntent {
  const HelloGetLocationToolResultIntentCase({
    required this.args,
    required this.result,
    this.overridden = false,
  });

  @override
  final HelloGetLocationToolArgs args;
  @override
  final HelloGetLocationToolResultValue result;
  @override
  final bool overridden;

  @override
  String get name => 'get_location';

  factory HelloGetLocationToolResultIntentCase.fromJson(sdk.JsonMap json) {
    return HelloGetLocationToolResultIntentCase(
      args: HelloGetLocationToolArgs.fromJson(Map<String, Object?>.from((json['args'] as Map?) ?? const {})),
      result: (json['result'] as String?) ?? '',
      overridden: (json['overridden'] as bool?) ?? false,
    );
  }
}

final class HelloGetLocationToolSkippedIntentCase extends HelloToolSkippedIntent {
  const HelloGetLocationToolSkippedIntentCase({
    required this.args,
  });

  @override
  final HelloGetLocationToolArgs args;

  @override
  String get type => 'get_location';

  factory HelloGetLocationToolSkippedIntentCase.fromJson(sdk.JsonMap json) {
    return HelloGetLocationToolSkippedIntentCase(
      args: HelloGetLocationToolArgs.fromJson(Map<String, Object?>.from((json['args'] as Map?) ?? const {})),
    );
  }
}

final class HelloToolCallIntentUnknown extends HelloToolCallIntent {
  const HelloToolCallIntentUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get type => (raw['type'] as String?) ?? '';

  @override
  Object? get args => raw['args'];
}

final class HelloToolResultIntentUnknown extends HelloToolResultIntent {
  const HelloToolResultIntentUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get name => (raw['name'] as String?) ?? '';

  @override
  Object? get args => raw['args'];

  @override
  Object? get result => raw['result'];

  @override
  bool get overridden => (raw['overridden'] as bool?) ?? false;
}

abstract class HelloToolSkippedIntent {
  const HelloToolSkippedIntent();

  String get type;
  Object? get args;

  factory HelloToolSkippedIntent.fromJson(sdk.JsonMap json) {
    final kind = (json['type'] as String?) ?? '';
    if (kind == 'get_details') {
      return HelloGetDetailsToolSkippedIntentCase.fromJson(json);
    }
    if (kind == 'get_location') {
      return HelloGetLocationToolSkippedIntentCase.fromJson(json);
    }
    return HelloToolSkippedIntentUnknown(Map<String, Object?>.from(json));
  }
}

final class HelloToolSkippedIntentUnknown extends HelloToolSkippedIntent {
  const HelloToolSkippedIntentUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get type => (raw['type'] as String?) ?? '';

  @override
  Object? get args => raw['args'];
}

final class HelloToolErrorIntent {
  const HelloToolErrorIntent({
    required this.tool,
    required this.message,
  });

  final String tool;
  final String message;

  factory HelloToolErrorIntent.fromJson(sdk.JsonMap json) {
    return HelloToolErrorIntent(
      tool: (json['tool'] as String?) ?? '',
      message: (json['message'] as String?) ?? '',
    );
  }
}

typedef HelloIntentValue = Object?;
typedef HelloIntentControl = Object?;
typedef HelloIntentHandler = FutureOr<HelloIntentControl> Function(String name, Object? value, String agentName);
typedef HelloPartialIntentHandler = FutureOr<void> Function(String name, Object? value, String agentName);

abstract class HelloBaseIntentHandler {
  FutureOr<HelloIntentControl> responseText(HelloResponseTextIntent intent, String agentName) => null;
  FutureOr<HelloIntentControl> responseSchema(HelloResponseSchemaIntent intent, String agentName) => null;
  FutureOr<HelloIntentControl> error(HelloErrorIntent intent, String agentName) => null;
  FutureOr<HelloIntentControl> toolCall(HelloToolCallIntent intent, String agentName) => null;
  FutureOr<HelloIntentControl> toolResult(HelloToolResultIntent intent, String agentName) => null;
  FutureOr<HelloIntentControl> toolError(HelloToolErrorIntent intent, String agentName) => null;
  FutureOr<HelloIntentControl> toolSkipped(HelloToolSkippedIntent intent, String agentName) => null;
}

abstract class HelloBasePartialIntentHandler {
  FutureOr<void> responseText(HelloResponseTextIntent intent, String agentName) {}
  FutureOr<void> responseSchema(HelloResponseSchemaIntent intent, String agentName) {}
  FutureOr<void> error(HelloErrorIntent intent, String agentName) {}
  FutureOr<void> toolCall(HelloToolCallIntent intent, String agentName) {}
  FutureOr<void> toolResult(HelloToolResultIntent intent, String agentName) {}
  FutureOr<void> toolError(HelloToolErrorIntent intent, String agentName) {}
  FutureOr<void> toolSkipped(HelloToolSkippedIntent intent, String agentName) {}
}

final class HelloApiKeys {
  const HelloApiKeys({
    this.groq_apiApiKey,
  });

  final String? groq_apiApiKey;

  Map<String, String> toMap() {
    return {
      if (groq_apiApiKey != null && groq_apiApiKey!.isNotEmpty) 'groq_apiApiKey': groq_apiApiKey!,
    };
  }
}

typedef HelloMiddleware = sdk.Middleware;

final class HelloConfig {
  const HelloConfig({
    required this.tools,
    this.middleware = const [],
    this.context,
    this.apiKeys,
    this.libraryPath,
  });

  final HelloTools tools;
  final List<HelloMiddleware> middleware;
  final sdk.JsonMap? context;
  final HelloApiKeys? apiKeys;
  final String? libraryPath;

  sdk.AuwgentConfig toAuwgentConfig() {
    return sdk.AuwgentConfig(
      tools: tools.toMap(),
      middleware: middleware,
      context: context,
      apiKeys: apiKeys?.toMap() ?? const {},
      libraryPath: libraryPath,
    );
  }
}

final class HelloAgent extends sdk.TypedAuwgent<sdk.JsonMap> {
  HelloAgent(HelloConfig config)
      : super(decodeHelloIr(), config.toAuwgentConfig());

  void onIntentHandler(HelloBaseIntentHandler handler) {
    onIntent((name, value, agentName) => _dispatchIntent(handler, name, value, agentName));
  }

  void onIntentPartialHandler(HelloBasePartialIntentHandler handler) {
    onIntentPartial((name, value, agentName) {
      _dispatchPartialIntent(handler, name, value, agentName);
    });
  }
}

Object? _dispatchIntent(HelloBaseIntentHandler handler, String name, Object? value, String agentName) {
  switch (name) {
    case 'response_text':
      return handler.responseText(HelloResponseTextIntent.fromJson(value as sdk.JsonMap), agentName);
    case 'response_schema':
      return handler.responseSchema(HelloResponseSchemaIntent.fromJson(value as sdk.JsonMap), agentName);
    case 'error':
      return handler.error(HelloErrorIntent.fromJson(value as sdk.JsonMap), agentName);
    case 'tool_call':
      return handler.toolCall(HelloToolCallIntent.fromJson(value as sdk.JsonMap), agentName);
    case 'tool_result':
      return handler.toolResult(HelloToolResultIntent.fromJson(value as sdk.JsonMap), agentName);
    case 'tool_error':
      return handler.toolError(HelloToolErrorIntent.fromJson(value as sdk.JsonMap), agentName);
    case 'tool_skipped':
      return handler.toolSkipped(HelloToolSkippedIntent.fromJson(value as sdk.JsonMap), agentName);
    default:
      return null;
  }
}

void _dispatchPartialIntent(HelloBasePartialIntentHandler handler, String name, Object? value, String agentName) {
  switch (name) {
    case 'response_text':
      handler.responseText(HelloResponseTextIntent.fromJson(value as sdk.JsonMap), agentName);
      return;
    case 'response_schema':
      handler.responseSchema(HelloResponseSchemaIntent.fromJson(value as sdk.JsonMap), agentName);
      return;
    case 'error':
      handler.error(HelloErrorIntent.fromJson(value as sdk.JsonMap), agentName);
      return;
    case 'tool_call':
      handler.toolCall(HelloToolCallIntent.fromJson(value as sdk.JsonMap), agentName);
      return;
    case 'tool_result':
      handler.toolResult(HelloToolResultIntent.fromJson(value as sdk.JsonMap), agentName);
      return;
    case 'tool_error':
      handler.toolError(HelloToolErrorIntent.fromJson(value as sdk.JsonMap), agentName);
      return;
    case 'tool_skipped':
      handler.toolSkipped(HelloToolSkippedIntent.fromJson(value as sdk.JsonMap), agentName);
      return;
    default:
      return;
  }
}

HelloAgent createHello(HelloConfig config) {
  return HelloAgent(config);
}

final auwgent = createHello;

typedef AuwgentAgent = HelloAgent;
typedef AuwgentConfig = HelloConfig;
typedef AuwgentTools = HelloTools;
typedef AuwgentToolRegistry = HelloToolRegistry;
typedef AuwgentContext = HelloContext;
typedef AuwgentMiddleware = HelloMiddleware;
typedef AuwgentIntentValue = HelloIntentValue;
typedef AuwgentIntentControl = HelloIntentControl;
typedef AuwgentIntentHandler = HelloIntentHandler;
typedef AuwgentPartialIntentHandler = HelloPartialIntentHandler;
typedef ResponseText = HelloResponseTextIntent;
typedef ResponseSchema = HelloResponseSchemaIntent;
typedef ErrorIntent = HelloErrorIntent;
typedef AuwgentBaseIntentHandler = HelloBaseIntentHandler;
typedef AuwgentBasePartialIntentHandler = HelloBasePartialIntentHandler;
typedef AuwgentApiKeys = HelloApiKeys;
typedef ToolCall = HelloToolCallIntent;
typedef ToolResult = HelloToolResultIntent;
typedef ToolError = HelloToolErrorIntent;
typedef ToolSkipped = HelloToolSkippedIntent;
