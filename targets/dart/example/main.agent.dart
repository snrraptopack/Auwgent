// Auto-generated Dart bindings for Hello
// Do not edit manually
import 'dart:async';
import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;
import 'main.agent.ir.dart';
final class Person {
  const Person({
    required this.age,
    required this.name,
    required this.location,
  });

  final double age;
  final String name;
  final String location;

  factory Person.fromJson(sdk.JsonMap json) {
    return Person(
      age: ((json['age'] as num?)?.toDouble()) ?? 0,
      name: (json['name'])?.toString() ?? '',
      location: (json['location'])?.toString() ?? '',
    );
  }

  sdk.JsonMap toJson() {
    return {
      'age': age,
      'name': name,
      'location': location,
    };
  }

  @override
  String toString() => sdk.prettyJson(toJson());
}

typedef AuwgentInput = String;

final class AuwgentOutput {
  const AuwgentOutput({
    required this.name,
    required this.age,
    required this.location,
  });

  final String name;
  final double age;
  final String location;

  factory AuwgentOutput.fromJson(sdk.JsonMap json) {
    return AuwgentOutput(
      name: (json['name'])?.toString() ?? '',
      age: ((json['age'] as num?)?.toDouble()) ?? 0,
      location: (json['location'])?.toString() ?? '',
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

typedef AuwgentContext = sdk.JsonMap;

typedef GetUserNameAgeToolHandler = FutureOr<GetUserNameAgeResult> Function();
typedef GetLocationToolHandler = FutureOr<GetLocationResult> Function();

abstract class AuwgentTools {
  const AuwgentTools();

  FutureOr<GetUserNameAgeResult> getUserNameAge();
  FutureOr<GetLocationResult> getLocation();

  Map<String, sdk.ToolHandler> toMap() {
    return {
      'get_user_name_age': (_) => getUserNameAge(),
      'get_location': (_) => getLocation(),
    };
  }
}

final class AuwgentToolRegistry extends AuwgentTools {
  const AuwgentToolRegistry({
    required GetUserNameAgeToolHandler getUserNameAge,
    required GetLocationToolHandler getLocation,
  }) :
      _getUserNameAge = getUserNameAge,
      _getLocation = getLocation;

  final GetUserNameAgeToolHandler _getUserNameAge;
  final GetLocationToolHandler _getLocation;

  @override
  FutureOr<GetUserNameAgeResult> getUserNameAge() => _getUserNameAge();
  @override
  FutureOr<GetLocationResult> getLocation() => _getLocation();
}

final class ResponseText {
  const ResponseText({
    required this.text,
  });

  final String text;

  factory ResponseText.fromJson(sdk.JsonMap json) {
    return ResponseText(
      text: (json['text'])?.toString() ?? '',
    );
  }

  @override
  String toString() => 'ResponseText(text: $text)';
}

final class ResponseSchema {
  const ResponseSchema({
    required this.type,
    required this.response,
  });

  final String type;
  final AuwgentOutput response;

  factory ResponseSchema.fromJson(sdk.JsonMap json) {
    return ResponseSchema(
      type: (json['type'])?.toString() ?? '',
      response: AuwgentOutput.fromJson(Map<String, Object?>.from((json['response'] as Map?) ?? const {})),
    );
  }

  @override
  String toString() => 'ResponseSchema(type: $type, response: $response)';
}

final class ErrorIntent {
  const ErrorIntent({
    required this.message,
  });

  final String message;

  factory ErrorIntent.fromJson(sdk.JsonMap json) {
    return ErrorIntent(
      message: (json['message'])?.toString() ?? '',
    );
  }

  @override
  String toString() => 'ErrorIntent(message: $message)';
}

abstract class ToolCalls {
  const ToolCalls();

  String get type;
  Object? get args;

  factory ToolCalls.fromJson(sdk.JsonMap json) {
    final kind = (json['type'])?.toString() ?? '';
    if (kind == 'get_user_name_age') {
      return GetUserNameAgeToolCall.fromJson(json);
    }
    if (kind == 'get_location') {
      return GetLocationToolCall.fromJson(json);
    }
    return ToolCallUnknown(Map<String, Object?>.from(json));
  }
}

typedef ToolCall = ToolCalls;

abstract class ToolResults {
  const ToolResults();

  String get name;
  Object? get args;
  Object? get result;
  bool get overridden;

  factory ToolResults.fromJson(sdk.JsonMap json) {
    final kind = (json['name'])?.toString() ?? '';
    if (kind == 'get_user_name_age') {
      return GetUserNameAgeToolResult.fromJson(json);
    }
    if (kind == 'get_location') {
      return GetLocationToolResult.fromJson(json);
    }
    return ToolResultUnknown(Map<String, Object?>.from(json));
  }
}

typedef ToolResult = ToolResults;

typedef GetUserNameAgeResult = Person;

final class GetUserNameAgeToolCall extends ToolCalls {
  const GetUserNameAgeToolCall();

  @override
  sdk.NoArgs get args => const sdk.NoArgs();

  @override
  String get type => 'get_user_name_age';

  factory GetUserNameAgeToolCall.fromJson(sdk.JsonMap json) {
    return const GetUserNameAgeToolCall();
  }

  @override
  String toString() => 'GetUserNameAgeToolCall(type: get_user_name_age, args: $args)';
}

final class GetUserNameAgeToolResult extends ToolResults {
  const GetUserNameAgeToolResult({
    required this.result,
    this.overridden = false,
  });

  @override
  sdk.NoArgs get args => const sdk.NoArgs();
  @override
  final GetUserNameAgeResult result;
  @override
  final bool overridden;

  @override
  String get name => 'get_user_name_age';

  factory GetUserNameAgeToolResult.fromJson(sdk.JsonMap json) {
    return GetUserNameAgeToolResult(
      result: json['result'] as Person,
      overridden: (json['overridden'] as bool?) ?? false,
    );
  }

  @override
  String toString() => 'GetUserNameAgeToolResult(name: get_user_name_age, result: $result, overridden: $overridden)';
}

final class GetUserNameAgeToolSkipped extends ToolSkippeds {
  const GetUserNameAgeToolSkipped();

  @override
  sdk.NoArgs get args => const sdk.NoArgs();

  @override
  String get type => 'get_user_name_age';

  factory GetUserNameAgeToolSkipped.fromJson(sdk.JsonMap json) {
    return const GetUserNameAgeToolSkipped();
  }

  @override
  String toString() => 'GetUserNameAgeToolSkipped(type: get_user_name_age, args: $args)';
}

typedef GetLocationResult = String;

final class GetLocationToolCall extends ToolCalls {
  const GetLocationToolCall();

  @override
  sdk.NoArgs get args => const sdk.NoArgs();

  @override
  String get type => 'get_location';

  factory GetLocationToolCall.fromJson(sdk.JsonMap json) {
    return const GetLocationToolCall();
  }

  @override
  String toString() => 'GetLocationToolCall(type: get_location, args: $args)';
}

final class GetLocationToolResult extends ToolResults {
  const GetLocationToolResult({
    required this.result,
    this.overridden = false,
  });

  @override
  sdk.NoArgs get args => const sdk.NoArgs();
  @override
  final GetLocationResult result;
  @override
  final bool overridden;

  @override
  String get name => 'get_location';

  factory GetLocationToolResult.fromJson(sdk.JsonMap json) {
    return GetLocationToolResult(
      result: (json['result'])?.toString() ?? '',
      overridden: (json['overridden'] as bool?) ?? false,
    );
  }

  @override
  String toString() => 'GetLocationToolResult(name: get_location, result: $result, overridden: $overridden)';
}

final class GetLocationToolSkipped extends ToolSkippeds {
  const GetLocationToolSkipped();

  @override
  sdk.NoArgs get args => const sdk.NoArgs();

  @override
  String get type => 'get_location';

  factory GetLocationToolSkipped.fromJson(sdk.JsonMap json) {
    return const GetLocationToolSkipped();
  }

  @override
  String toString() => 'GetLocationToolSkipped(type: get_location, args: $args)';
}

final class ToolCallUnknown extends ToolCalls {
  const ToolCallUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get type => (raw['type'])?.toString() ?? '';

  @override
  Object? get args => raw['args'];

  @override
  String toString() => 'ToolCallUnknown(raw: $raw)';
}

final class ToolResultUnknown extends ToolResults {
  const ToolResultUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get name => (raw['name'])?.toString() ?? '';

  @override
  Object? get args => raw['args'];

  @override
  Object? get result => raw['result'];

  @override
  bool get overridden => (raw['overridden'] as bool?) ?? false;

  @override
  String toString() => 'ToolResultUnknown(raw: $raw)';
}

abstract class ToolSkippeds {
  const ToolSkippeds();

  String get type;
  Object? get args;

  factory ToolSkippeds.fromJson(sdk.JsonMap json) {
    final kind = (json['type'])?.toString() ?? '';
    if (kind == 'get_user_name_age') {
      return GetUserNameAgeToolSkipped.fromJson(json);
    }
    if (kind == 'get_location') {
      return GetLocationToolSkipped.fromJson(json);
    }
    return ToolSkippedUnknown(Map<String, Object?>.from(json));
  }
}

typedef ToolSkipped = ToolSkippeds;

final class ToolSkippedUnknown extends ToolSkippeds {
  const ToolSkippedUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get type => (raw['type'])?.toString() ?? '';

  @override
  Object? get args => raw['args'];

  @override
  String toString() => 'ToolSkippedUnknown(raw: $raw)';
}

final class ToolErrors {
  const ToolErrors({
    required this.tool,
    required this.message,
  });

  final String tool;
  final String message;

  factory ToolErrors.fromJson(sdk.JsonMap json) {
    return ToolErrors(
      tool: (json['tool'])?.toString() ?? '',
      message: (json['message'])?.toString() ?? '',
    );
  }

  @override
  String toString() => 'ToolErrors(tool: $tool, message: $message)';
}

typedef ToolError = ToolErrors;

abstract class HelperCalls {
  const HelperCalls();

  String get type;
  Object? get args;

  factory HelperCalls.fromJson(sdk.JsonMap json) {
    final kind = (json['type'])?.toString() ?? '';
    if (kind == 'Joker') {
      return JokerHelperCall.fromJson(json);
    }
    return HelperCallUnknown(Map<String, Object?>.from(json));
  }
}

typedef HelperCall = HelperCalls;

abstract class HelperResults {
  const HelperResults();

  String get name;
  Object? get args;
  Object? get result;
  bool get overridden;

  factory HelperResults.fromJson(sdk.JsonMap json) {
    final kind = (json['name'])?.toString() ?? '';
    if (kind == 'Joker') {
      return JokerHelperResult.fromJson(json);
    }
    return HelperResultUnknown(Map<String, Object?>.from(json));
  }
}

typedef HelperResult = HelperResults;

final class JokerHelperArgs {
  const JokerHelperArgs({
    required this.joker_prompt,
  });

  final String joker_prompt;

  factory JokerHelperArgs.fromJson(sdk.JsonMap json) {
    return JokerHelperArgs(
      joker_prompt: (json['joker_prompt'])?.toString() ?? '',
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

final class JokerHelperCall extends HelperCalls {
  const JokerHelperCall({
    required this.args,
  });

  @override
  final JokerHelperArgs args;

  @override
  String get type => 'Joker';

  factory JokerHelperCall.fromJson(sdk.JsonMap json) {
    return JokerHelperCall(
      args: JokerHelperArgs.fromJson(Map<String, Object?>.from((json['args'] as Map?) ?? const {})),
    );
  }

  @override
  String toString() => 'JokerHelperCall(type: Joker, args: $args)';
}

final class JokerHelperResult extends HelperResults {
  const JokerHelperResult({
    required this.args,
    required this.result,
    this.overridden = false,
  });

  @override
  final JokerHelperArgs args;
  @override
  final sdk.NoResult result;
  @override
  final bool overridden;

  @override
  String get name => 'Joker';

  factory JokerHelperResult.fromJson(sdk.JsonMap json) {
    return JokerHelperResult(
      args: JokerHelperArgs.fromJson(Map<String, Object?>.from((json['args'] as Map?) ?? const {})),
      result: const sdk.NoResult(),
      overridden: (json['overridden'] as bool?) ?? false,
    );
  }

  @override
  String toString() => 'JokerHelperResult(name: Joker, args: $args, result: $result, overridden: $overridden)';
}

final class HelperCallUnknown extends HelperCalls {
  const HelperCallUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get type => (raw['type'])?.toString() ?? '';

  @override
  Object? get args => raw['args'];

  @override
  String toString() => 'HelperCallUnknown(raw: $raw)';
}

final class HelperResultUnknown extends HelperResults {
  const HelperResultUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get name => (raw['name'])?.toString() ?? '';

  @override
  Object? get args => raw['args'];

  @override
  Object? get result => raw['result'];

  @override
  bool get overridden => (raw['overridden'] as bool?) ?? false;

  @override
  String toString() => 'HelperResultUnknown(raw: $raw)';
}

typedef AuwgentIntentValue = Object?;
typedef AuwgentIntentControl = sdk.IntentControl?;
typedef AuwgentIntentHandler = FutureOr<sdk.IntentControl?> Function(String name, Object? value, String agentName);
typedef AuwgentPartialIntentHandler = FutureOr<void> Function(String name, Object? value, String agentName);

abstract class AuwgentBaseIntentHandler {
  FutureOr<void> responseText(ResponseText intent, String agentName) {}
  FutureOr<void> responseSchema(ResponseSchema intent, String agentName) {}
  FutureOr<void> error(ErrorIntent intent, String agentName) {}
  FutureOr<sdk.IntentControl?> toolCall(ToolCalls intent, String agentName) => null;
  FutureOr<void> toolResult(ToolResults intent, String agentName) {}
  FutureOr<void> toolError(ToolErrors intent, String agentName) {}
  FutureOr<void> toolSkipped(ToolSkippeds intent, String agentName) {}
  FutureOr<void> helperCall(HelperCalls intent, String agentName) {}
  FutureOr<void> helperResult(HelperResults intent, String agentName) {}
}

abstract class AuwgentBasePartialIntentHandler {
  FutureOr<void> responseText(sdk.PartialTextIntentValue intent, String agentName) {}
  FutureOr<void> responseSchema(sdk.PartialStructuredIntentValue<ResponseSchema> intent, String agentName) {}
  FutureOr<void> error(sdk.PartialStructuredIntentValue<ErrorIntent> intent, String agentName) {}
  FutureOr<void> toolCall(sdk.PartialStructuredIntentValue<ToolCalls> intent, String agentName) {}
  FutureOr<void> toolResult(sdk.PartialStructuredIntentValue<ToolResults> intent, String agentName) {}
  FutureOr<void> toolError(sdk.PartialStructuredIntentValue<ToolErrors> intent, String agentName) {}
  FutureOr<void> toolSkipped(sdk.PartialStructuredIntentValue<ToolSkippeds> intent, String agentName) {}
  FutureOr<void> helperCall(sdk.PartialStructuredIntentValue<HelperCalls> intent, String agentName) {}
  FutureOr<void> helperResult(sdk.PartialStructuredIntentValue<HelperResults> intent, String agentName) {}
}

abstract class AuwgentMiddleware implements sdk.Middleware {
  const AuwgentMiddleware();

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

  FutureOr<void> responseText(ResponseText intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> responseSchema(ResponseSchema intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> errorIntent(ErrorIntent intent, sdk.MiddlewareContext ctx) {}
  FutureOr<sdk.IntentControl?> toolCall(ToolCalls intent, sdk.MiddlewareContext ctx) => null;
  FutureOr<void> toolResult(ToolResults intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> toolError(ToolErrors intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> toolSkipped(ToolSkippeds intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> helperCall(HelperCalls intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> helperResult(HelperResults intent, sdk.MiddlewareContext ctx) {}

  FutureOr<void> partialResponseText(sdk.PartialTextIntentValue intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialResponseSchema(sdk.PartialStructuredIntentValue<ResponseSchema> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialError(sdk.PartialStructuredIntentValue<ErrorIntent> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialToolCall(sdk.PartialStructuredIntentValue<ToolCalls> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialToolResult(sdk.PartialStructuredIntentValue<ToolResults> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialToolError(sdk.PartialStructuredIntentValue<ToolErrors> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialToolSkipped(sdk.PartialStructuredIntentValue<ToolSkippeds> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialHelperCall(sdk.PartialStructuredIntentValue<HelperCalls> intent, sdk.MiddlewareContext ctx) {}
  FutureOr<void> partialHelperResult(sdk.PartialStructuredIntentValue<HelperResults> intent, sdk.MiddlewareContext ctx) {}
  @override
  FutureOr<sdk.IntentControl?> onIntent(String name, Object? value, sdk.MiddlewareContext ctx) => _dispatchMiddlewareIntent(this, name, value, ctx);

  @override
  FutureOr<void> onIntentPartial(String name, Object? value, sdk.MiddlewareContext ctx) {
    _dispatchMiddlewarePartialIntent(this, name, value, ctx);
  }
}

final class AuwgentApiKeys {
  const AuwgentApiKeys({
    this.groqApiKey,
  });

  final String? groqApiKey;

  Map<String, String> toMap() {
    return {
      if (groqApiKey != null && groqApiKey!.isNotEmpty) 'groqApiKey': groqApiKey!,
    };
  }
}

final class AuwgentConfig {
  const AuwgentConfig({
    required this.tools,
    this.middleware = const [],
    this.context,
    this.apiKeys,
    this.libraryPath,
    this.autoDispose = true,
  });

  final AuwgentTools tools;
  final List<AuwgentMiddleware> middleware;
  final sdk.JsonMap? context;
  final AuwgentApiKeys? apiKeys;
  final String? libraryPath;
  final bool autoDispose;

  sdk.AuwgentConfig toAuwgentConfig() {
    return sdk.AuwgentConfig(
      tools: tools.toMap(),
      middleware: middleware,
      context: context,
      apiKeys: apiKeys?.toMap() ?? const {},
      libraryPath: libraryPath,
      autoDispose: autoDispose,
    );
  }
}

final class AuwgentAgent extends sdk.TypedAuwgent<sdk.JsonMap> {
  AuwgentAgent(AuwgentConfig config)
      : super(decodeHelloIr(), config.toAuwgentConfig());

  void onIntentHandler(AuwgentBaseIntentHandler handler) {
    onIntent((name, value, agentName) => _dispatchIntent(handler, name, value, agentName));
  }

  void onIntentPartialHandler(AuwgentBasePartialIntentHandler handler) {
    onIntentPartial((name, value, agentName) {
      _dispatchPartialIntent(handler, name, value, agentName);
    });
  }
}

FutureOr<sdk.IntentControl?> _dispatchIntent(AuwgentBaseIntentHandler handler, String name, Object? value, String agentName) {
  switch (name) {
    case 'response_text':
      handler.responseText(ResponseText.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'response_schema':
      handler.responseSchema(ResponseSchema.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'error':
      handler.error(ErrorIntent.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'tool_call':
      return handler.toolCall(ToolCalls.fromJson(value as sdk.JsonMap), agentName);
    case 'tool_result':
      handler.toolResult(ToolResults.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'tool_error':
      handler.toolError(ToolErrors.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'tool_skipped':
      handler.toolSkipped(ToolSkippeds.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'helper_call':
      handler.helperCall(HelperCalls.fromJson(value as sdk.JsonMap), agentName);
      return null;
    case 'helper_result':
      handler.helperResult(HelperResults.fromJson(value as sdk.JsonMap), agentName);
      return null;
    default:
      return null;
  }
}

void _dispatchPartialIntent(AuwgentBasePartialIntentHandler handler, String name, Object? value, String agentName) {
  switch (name) {
    case 'response_text':
      handler.responseText(sdk.PartialTextIntentValue.fromJson(value as sdk.JsonMap), agentName);
      return;
    case 'response_schema':
      handler.responseSchema(sdk.PartialStructuredIntentValue<ResponseSchema>.fromJson(value as sdk.JsonMap, ResponseSchema.fromJson), agentName);
      return;
    case 'error':
      handler.error(sdk.PartialStructuredIntentValue<ErrorIntent>.fromJson(value as sdk.JsonMap, ErrorIntent.fromJson), agentName);
      return;
    case 'tool_call':
      handler.toolCall(sdk.PartialStructuredIntentValue<ToolCalls>.fromJson(value as sdk.JsonMap, ToolCalls.fromJson), agentName);
      return;
    case 'tool_result':
      handler.toolResult(sdk.PartialStructuredIntentValue<ToolResults>.fromJson(value as sdk.JsonMap, ToolResults.fromJson), agentName);
      return;
    case 'tool_error':
      handler.toolError(sdk.PartialStructuredIntentValue<ToolErrors>.fromJson(value as sdk.JsonMap, ToolErrors.fromJson), agentName);
      return;
    case 'tool_skipped':
      handler.toolSkipped(sdk.PartialStructuredIntentValue<ToolSkippeds>.fromJson(value as sdk.JsonMap, ToolSkippeds.fromJson), agentName);
      return;
    case 'helper_call':
      handler.helperCall(sdk.PartialStructuredIntentValue<HelperCalls>.fromJson(value as sdk.JsonMap, HelperCalls.fromJson), agentName);
      return;
    case 'helper_result':
      handler.helperResult(sdk.PartialStructuredIntentValue<HelperResults>.fromJson(value as sdk.JsonMap, HelperResults.fromJson), agentName);
      return;
    default:
      return;
  }
}

FutureOr<sdk.IntentControl?> _dispatchMiddlewareIntent(AuwgentMiddleware middleware, String name, Object? value, sdk.MiddlewareContext ctx) {
  switch (name) {
    case 'response_text':
      middleware.responseText(ResponseText.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'response_schema':
      middleware.responseSchema(ResponseSchema.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'error':
      middleware.errorIntent(ErrorIntent.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'tool_call':
      return middleware.toolCall(ToolCalls.fromJson(value as sdk.JsonMap), ctx);
    case 'tool_result':
      middleware.toolResult(ToolResults.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'tool_error':
      middleware.toolError(ToolErrors.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'tool_skipped':
      middleware.toolSkipped(ToolSkippeds.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'helper_call':
      middleware.helperCall(HelperCalls.fromJson(value as sdk.JsonMap), ctx);
      return null;
    case 'helper_result':
      middleware.helperResult(HelperResults.fromJson(value as sdk.JsonMap), ctx);
      return null;
    default:
      return null;
  }
}

void _dispatchMiddlewarePartialIntent(AuwgentMiddleware middleware, String name, Object? value, sdk.MiddlewareContext ctx) {
  switch (name) {
    case 'response_text':
      middleware.partialResponseText(sdk.PartialTextIntentValue.fromJson(value as sdk.JsonMap), ctx);
      return;
    case 'response_schema':
      middleware.partialResponseSchema(sdk.PartialStructuredIntentValue<ResponseSchema>.fromJson(value as sdk.JsonMap, ResponseSchema.fromJson), ctx);
      return;
    case 'error':
      middleware.partialError(sdk.PartialStructuredIntentValue<ErrorIntent>.fromJson(value as sdk.JsonMap, ErrorIntent.fromJson), ctx);
      return;
    case 'tool_call':
      middleware.partialToolCall(sdk.PartialStructuredIntentValue<ToolCalls>.fromJson(value as sdk.JsonMap, ToolCalls.fromJson), ctx);
      return;
    case 'tool_result':
      middleware.partialToolResult(sdk.PartialStructuredIntentValue<ToolResults>.fromJson(value as sdk.JsonMap, ToolResults.fromJson), ctx);
      return;
    case 'tool_error':
      middleware.partialToolError(sdk.PartialStructuredIntentValue<ToolErrors>.fromJson(value as sdk.JsonMap, ToolErrors.fromJson), ctx);
      return;
    case 'tool_skipped':
      middleware.partialToolSkipped(sdk.PartialStructuredIntentValue<ToolSkippeds>.fromJson(value as sdk.JsonMap, ToolSkippeds.fromJson), ctx);
      return;
    case 'helper_call':
      middleware.partialHelperCall(sdk.PartialStructuredIntentValue<HelperCalls>.fromJson(value as sdk.JsonMap, HelperCalls.fromJson), ctx);
      return;
    case 'helper_result':
      middleware.partialHelperResult(sdk.PartialStructuredIntentValue<HelperResults>.fromJson(value as sdk.JsonMap, HelperResults.fromJson), ctx);
      return;
    default:
      return;
  }
}

AuwgentAgent createAuwgent(AuwgentConfig config) {
  return AuwgentAgent(config);
}

final auwgent = createAuwgent;
