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
    this.location,
  });

  final String name;
  final double age;
  final String? location;

  factory HelloOutput.fromJson(sdk.JsonMap json) {
    return HelloOutput(
      name: (json['name'] as String?) ?? '',
      age: ((json['age'] as num?)?.toDouble()) ?? 0,
      location: json['location'] as String?,
    );
  }

  sdk.JsonMap toJson() {
    return {
      'name': name,
      'age': age,
      'location': location,
    };
  }

  @override
  String toString() => sdk.prettyJson(toJson());
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

  @override
  String toString() => 'HelloResponseTextIntent(text: $text)';
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

  @override
  String toString() => 'HelloResponseSchemaIntent(type: $type, response: $response)';
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

  @override
  String toString() => 'HelloErrorIntent(message: $message)';
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

  @override
  String toString() => 'HelloGetDetailsToolCallIntentCase(type: get_details, args: $args)';
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

  @override
  String toString() => 'HelloGetDetailsToolResultIntentCase(name: get_details, result: $result, overridden: $overridden)';
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

  @override
  String toString() => 'HelloGetDetailsToolSkippedIntentCase(type: get_details, args: $args)';
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

  sdk.JsonMap toJson() {
    return {
      'id': id,
    };
  }

  @override
  String toString() => sdk.prettyJson(toJson());
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

  @override
  String toString() => 'HelloGetLocationToolCallIntentCase(type: get_location, args: $args)';
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

  @override
  String toString() => 'HelloGetLocationToolResultIntentCase(name: get_location, args: $args, result: $result, overridden: $overridden)';
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

  @override
  String toString() => 'HelloGetLocationToolSkippedIntentCase(type: get_location, args: $args)';
}

final class HelloToolCallIntentUnknown extends HelloToolCallIntent {
  const HelloToolCallIntentUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get type => (raw['type'] as String?) ?? '';

  @override
  Object? get args => raw['args'];

  @override
  String toString() => 'HelloToolCallIntentUnknown(raw: $raw)';
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

  @override
  String toString() => 'HelloToolResultIntentUnknown(raw: $raw)';
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

  @override
  String toString() => 'HelloToolSkippedIntentUnknown(raw: $raw)';
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

  @override
  String toString() => 'HelloToolErrorIntent(tool: $tool, message: $message)';
}

abstract class HelloHelperCallIntent {
  const HelloHelperCallIntent();

  String get type;
  Object? get args;

  factory HelloHelperCallIntent.fromJson(sdk.JsonMap json) {
    final kind = (json['type'] as String?) ?? '';
    if (kind == 'Joker') {
      return HelloJokerHelperCallIntentCase.fromJson(json);
    }
    return HelloHelperCallIntentUnknown(Map<String, Object?>.from(json));
  }
}

abstract class HelloHelperResultIntent {
  const HelloHelperResultIntent();

  String get name;
  Object? get args;
  Object? get result;
  bool get overridden;

  factory HelloHelperResultIntent.fromJson(sdk.JsonMap json) {
    final kind = (json['name'] as String?) ?? '';
    if (kind == 'Joker') {
      return HelloJokerHelperResultIntentCase.fromJson(json);
    }
    return HelloHelperResultIntentUnknown(Map<String, Object?>.from(json));
  }
}

final class HelloJokerHelperArgs {
  const HelloJokerHelperArgs({
    required this.joker_prompt,
  });

  final String joker_prompt;

  factory HelloJokerHelperArgs.fromJson(sdk.JsonMap json) {
    return HelloJokerHelperArgs(
      joker_prompt: (json['joker_prompt'] as String?) ?? '',
    );
  }

  sdk.JsonMap toJson() {
    return {
      'joker_prompt': joker_prompt,
    };
  }

  @override
  String toString() => sdk.prettyJson(toJson());
}

final class HelloJokerHelperCallIntentCase extends HelloHelperCallIntent {
  const HelloJokerHelperCallIntentCase({
    required this.args,
  });

  @override
  final HelloJokerHelperArgs args;

  @override
  String get type => 'Joker';

  factory HelloJokerHelperCallIntentCase.fromJson(sdk.JsonMap json) {
    return HelloJokerHelperCallIntentCase(
      args: HelloJokerHelperArgs.fromJson(Map<String, Object?>.from((json['args'] as Map?) ?? const {})),
    );
  }

  @override
  String toString() => 'HelloJokerHelperCallIntentCase(type: Joker, args: $args)';
}

final class HelloJokerHelperResultIntentCase extends HelloHelperResultIntent {
  const HelloJokerHelperResultIntentCase({
    required this.args,
    required this.result,
    this.overridden = false,
  });

  @override
  final HelloJokerHelperArgs args;
  @override
  final sdk.NoResult result;
  @override
  final bool overridden;

  @override
  String get name => 'Joker';

  factory HelloJokerHelperResultIntentCase.fromJson(sdk.JsonMap json) {
    return HelloJokerHelperResultIntentCase(
      args: HelloJokerHelperArgs.fromJson(Map<String, Object?>.from((json['args'] as Map?) ?? const {})),
      result: const sdk.NoResult(),
      overridden: (json['overridden'] as bool?) ?? false,
    );
  }

  @override
  String toString() => 'HelloJokerHelperResultIntentCase(name: Joker, args: $args, result: $result, overridden: $overridden)';
}

final class HelloHelperCallIntentUnknown extends HelloHelperCallIntent {
  const HelloHelperCallIntentUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get type => (raw['type'] as String?) ?? '';

  @override
  Object? get args => raw['args'];

  @override
  String toString() => 'HelloHelperCallIntentUnknown(raw: $raw)';
}

final class HelloHelperResultIntentUnknown extends HelloHelperResultIntent {
  const HelloHelperResultIntentUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get name => (raw['name'] as String?) ?? '';

  @override
  Object? get args => raw['args'];

  @override
  Object? get result => raw['result'];

  @override
  bool get overridden => (raw['overridden'] as bool?) ?? false;

  @override
  String toString() => 'HelloHelperResultIntentUnknown(raw: $raw)';
}

typedef HelloIntentValue = Object?;
typedef HelloIntentControl = sdk.IntentControl?;
typedef HelloIntentHandler = FutureOr<sdk.IntentControl?> Function(String name, Object? value, String agentName);
typedef HelloPartialIntentHandler = FutureOr<void> Function(String name, Object? value, String agentName);

abstract class HelloBaseIntentHandler {
  FutureOr<void> responseText(HelloResponseTextIntent intent, String agentName) {}
  FutureOr<void> responseSchema(HelloResponseSchemaIntent intent, String agentName) {}
  FutureOr<void> error(HelloErrorIntent intent, String agentName) {}
  FutureOr<sdk.IntentControl?> toolCall(HelloToolCallIntent intent, String agentName) => null;
  FutureOr<void> toolResult(HelloToolResultIntent intent, String agentName) {}
  FutureOr<void> toolError(HelloToolErrorIntent intent, String agentName) {}
  FutureOr<void> toolSkipped(HelloToolSkippedIntent intent, String agentName) {}
  FutureOr<void> helperCall(HelloHelperCallIntent intent, String agentName) {}
  FutureOr<void> helperResult(HelloHelperResultIntent intent, String agentName) {}
}

abstract class HelloBasePartialIntentHandler {
  FutureOr<void> responseText(sdk.PartialTextIntentValue intent, String agentName) {}
  FutureOr<void> responseSchema(sdk.PartialStructuredIntentValue<HelloResponseSchemaIntent> intent, String agentName) {}
  FutureOr<void> error(sdk.PartialStructuredIntentValue<HelloErrorIntent> intent, String agentName) {}
  FutureOr<void> toolCall(sdk.PartialStructuredIntentValue<HelloToolCallIntent> intent, String agentName) {}
  FutureOr<void> toolResult(sdk.PartialStructuredIntentValue<HelloToolResultIntent> intent, String agentName) {}
  FutureOr<void> toolError(sdk.PartialStructuredIntentValue<HelloToolErrorIntent> intent, String agentName) {}
  FutureOr<void> toolSkipped(sdk.PartialStructuredIntentValue<HelloToolSkippedIntent> intent, String agentName) {}
  FutureOr<void> helperCall(sdk.PartialStructuredIntentValue<HelloHelperCallIntent> intent, String agentName) {}
  FutureOr<void> helperResult(sdk.PartialStructuredIntentValue<HelloHelperResultIntent> intent, String agentName) {}
}

abstract class HelloMiddleware implements sdk.Middleware {
  const HelloMiddleware();

  @override
  String get name => runtimeType.toString();

  @override
  Object? get target => null;

  @override
  FutureOr<sdk.SessionState> onRunStart(sdk.SessionState session, sdk.MiddlewareContext ctx) => session;

  @override
  FutureOr<String?> onLLMStart(String prompt, sdk.MiddlewareContext ctx) => null;

  @override
  FutureOr<void> onLLMEnd(Object? response, sdk.MiddlewareContext ctx) {}

  @override
  FutureOr<void> onRunComplete(sdk.SessionState finalSession, sdk.MiddlewareContext ctx) {}

  @override
  FutureOr<bool> onError(Object error, sdk.SessionState? session, sdk.MiddlewareContext ctx) => false;

  FutureOr<void> responseText(HelloResponseTextIntent intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> responseSchema(HelloResponseSchemaIntent intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> errorIntent(HelloErrorIntent intent, sdk.MiddlewareContext ctx) {}
  FutureOr<sdk.IntentControl?> toolCall(HelloToolCallIntent intent, sdk.MiddlewareContext ctx) => null;
  FutureOr<void> toolResult(HelloToolResultIntent intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> toolError(HelloToolErrorIntent intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> toolSkipped(HelloToolSkippedIntent intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> helperCall(HelloHelperCallIntent intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> helperResult(HelloHelperResultIntent intent, sdk.MiddlewareContext ctx) {}

  FutureOr<void> partialResponseText(sdk.PartialTextIntentValue intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialResponseSchema(sdk.PartialStructuredIntentValue<HelloResponseSchemaIntent> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialError(sdk.PartialStructuredIntentValue<HelloErrorIntent> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialToolCall(sdk.PartialStructuredIntentValue<HelloToolCallIntent> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialToolResult(sdk.PartialStructuredIntentValue<HelloToolResultIntent> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialToolError(sdk.PartialStructuredIntentValue<HelloToolErrorIntent> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialToolSkipped(sdk.PartialStructuredIntentValue<HelloToolSkippedIntent> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialHelperCall(sdk.PartialStructuredIntentValue<HelloHelperCallIntent> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialHelperResult(sdk.PartialStructuredIntentValue<HelloHelperResultIntent> intent, sdk.MiddlewareContext ctx) {}
  @override
  FutureOr<sdk.IntentControl?> onIntent(String name, Object? value, sdk.MiddlewareContext ctx) => _dispatchMiddlewareIntent(this, name, value, ctx);

  @override
  FutureOr<void> onIntentPartial(String name, Object? value, sdk.MiddlewareContext ctx) {
    _dispatchMiddlewarePartialIntent(this, name, value, ctx);
  }
}

final class HelloApiKeys {
  const HelloApiKeys({
    this.groqApiKey,
  });

  final String? groqApiKey;

  Map<String, String> toMap() {
    return {
      if (groqApiKey != null && groqApiKey!.isNotEmpty) 'groqApiKey': groqApiKey!,
    };
  }
}

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

FutureOr<sdk.IntentControl?> _dispatchIntent(HelloBaseIntentHandler handler, String name, Object? value, String agentName) {
  switch (name) {
    case 'response_text':
      handler.responseText(HelloResponseTextIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'response_schema':
      handler.responseSchema(HelloResponseSchemaIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'error':
      handler.error(HelloErrorIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'tool_call':
      return handler.toolCall(HelloToolCallIntent.fromJson(value as sdk.JsonMap), agentName);
    case 'tool_result':
      handler.toolResult(HelloToolResultIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'tool_error':
      handler.toolError(HelloToolErrorIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'tool_skipped':
      handler.toolSkipped(HelloToolSkippedIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'helper_call':
      handler.helperCall(HelloHelperCallIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'helper_result':
      handler.helperResult(HelloHelperResultIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    default:
      return null;
  }
}

void _dispatchPartialIntent(HelloBasePartialIntentHandler handler, String name, Object? value, String agentName) {
  switch (name) {
    case 'response_text':
      handler.responseText(sdk.PartialTextIntentValue.fromJson(value as sdk.JsonMap), agentName);
      return;
    case 'response_schema':
      handler.responseSchema(sdk.PartialStructuredIntentValue<HelloResponseSchemaIntent>.fromJson(value as sdk.JsonMap, HelloResponseSchemaIntent.fromJson), agentName);
      return;
    case 'error':
      handler.error(sdk.PartialStructuredIntentValue<HelloErrorIntent>.fromJson(value as sdk.JsonMap, HelloErrorIntent.fromJson), agentName);
      return;
    case 'tool_call':
      handler.toolCall(sdk.PartialStructuredIntentValue<HelloToolCallIntent>.fromJson(value as sdk.JsonMap, HelloToolCallIntent.fromJson), agentName);
      return;
    case 'tool_result':
      handler.toolResult(sdk.PartialStructuredIntentValue<HelloToolResultIntent>.fromJson(value as sdk.JsonMap, HelloToolResultIntent.fromJson), agentName);
      return;
    case 'tool_error':
      handler.toolError(sdk.PartialStructuredIntentValue<HelloToolErrorIntent>.fromJson(value as sdk.JsonMap, HelloToolErrorIntent.fromJson), agentName);
      return;
    case 'tool_skipped':
      handler.toolSkipped(sdk.PartialStructuredIntentValue<HelloToolSkippedIntent>.fromJson(value as sdk.JsonMap, HelloToolSkippedIntent.fromJson), agentName);
      return;
    case 'helper_call':
      handler.helperCall(sdk.PartialStructuredIntentValue<HelloHelperCallIntent>.fromJson(value as sdk.JsonMap, HelloHelperCallIntent.fromJson), agentName);
      return;
    case 'helper_result':
      handler.helperResult(sdk.PartialStructuredIntentValue<HelloHelperResultIntent>.fromJson(value as sdk.JsonMap, HelloHelperResultIntent.fromJson), agentName);
      return;
    default:
      return;
  }
}

FutureOr<sdk.IntentControl?> _dispatchMiddlewareIntent(HelloMiddleware middleware, String name, Object? value, sdk.MiddlewareContext ctx) {
  switch (name) {
    case 'response_text':
      middleware.responseText(HelloResponseTextIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'response_schema':
      middleware.responseSchema(HelloResponseSchemaIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'error':
      middleware.errorIntent(HelloErrorIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'tool_call':
      return middleware.toolCall(HelloToolCallIntent.fromJson(value as sdk.JsonMap), ctx);
    case 'tool_result':
      middleware.toolResult(HelloToolResultIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'tool_error':
      middleware.toolError(HelloToolErrorIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'tool_skipped':
      middleware.toolSkipped(HelloToolSkippedIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'helper_call':
      middleware.helperCall(HelloHelperCallIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'helper_result':
      middleware.helperResult(HelloHelperResultIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    default:
      return null;
  }
}

void _dispatchMiddlewarePartialIntent(HelloMiddleware middleware, String name, Object? value, sdk.MiddlewareContext ctx) {
  switch (name) {
    case 'response_text':
      middleware.partialResponseText(sdk.PartialTextIntentValue.fromJson(value as sdk.JsonMap), ctx);
      return;
    case 'response_schema':
      middleware.partialResponseSchema(sdk.PartialStructuredIntentValue<HelloResponseSchemaIntent>.fromJson(value as sdk.JsonMap, HelloResponseSchemaIntent.fromJson), ctx);
      return;
    case 'error':
      middleware.partialError(sdk.PartialStructuredIntentValue<HelloErrorIntent>.fromJson(value as sdk.JsonMap, HelloErrorIntent.fromJson), ctx);
      return;
    case 'tool_call':
      middleware.partialToolCall(sdk.PartialStructuredIntentValue<HelloToolCallIntent>.fromJson(value as sdk.JsonMap, HelloToolCallIntent.fromJson), ctx);
      return;
    case 'tool_result':
      middleware.partialToolResult(sdk.PartialStructuredIntentValue<HelloToolResultIntent>.fromJson(value as sdk.JsonMap, HelloToolResultIntent.fromJson), ctx);
      return;
    case 'tool_error':
      middleware.partialToolError(sdk.PartialStructuredIntentValue<HelloToolErrorIntent>.fromJson(value as sdk.JsonMap, HelloToolErrorIntent.fromJson), ctx);
      return;
    case 'tool_skipped':
      middleware.partialToolSkipped(sdk.PartialStructuredIntentValue<HelloToolSkippedIntent>.fromJson(value as sdk.JsonMap, HelloToolSkippedIntent.fromJson), ctx);
      return;
    case 'helper_call':
      middleware.partialHelperCall(sdk.PartialStructuredIntentValue<HelloHelperCallIntent>.fromJson(value as sdk.JsonMap, HelloHelperCallIntent.fromJson), ctx);
      return;
    case 'helper_result':
      middleware.partialHelperResult(sdk.PartialStructuredIntentValue<HelloHelperResultIntent>.fromJson(value as sdk.JsonMap, HelloHelperResultIntent.fromJson), ctx);
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
typedef HelperCall = HelloHelperCallIntent;
typedef HelperResult = HelloHelperResultIntent;
