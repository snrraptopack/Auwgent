// Auto-generated Dart bindings for Hello
// Do not edit manually
import 'dart:async';
import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;
import 'main.agent.ir.dart';
typedef Simple = sdk.JsonMap;

typedef Person = sdk.JsonMap;

typedef HelloInput = String;

abstract class HelloOutput {
  const HelloOutput();

  String get variant;

  factory HelloOutput.fromJson(sdk.JsonMap json) {
    if (_matchesHelloOutputSimple(json)) {
      return HelloOutputSimple.fromJson(json);
    }
    if (_matchesHelloOutputPerson(json)) {
      return HelloOutputPerson.fromJson(json);
    }
    return HelloOutputUnknown(Map<String, Object?>.from(json));
  }
}

final class HelloOutputSimple extends HelloOutput {
  const HelloOutputSimple({
    required this.simple,
  });

  final String simple;

  @override
  String get variant => 'Simple';

  factory HelloOutputSimple.fromJson(sdk.JsonMap json) {
    return HelloOutputSimple(
      simple: (json['simple'] as String?) ?? '',
    );
  }
}

bool _matchesHelloOutputSimple(sdk.JsonMap json) {
  return json.containsKey('simple');
}

final class HelloOutputPerson extends HelloOutput {
  const HelloOutputPerson({
    required this.name,
    required this.age,
  });

  final String name;
  final double age;

  @override
  String get variant => 'Person';

  factory HelloOutputPerson.fromJson(sdk.JsonMap json) {
    return HelloOutputPerson(
      name: (json['name'] as String?) ?? '',
      age: ((json['age'] as num?)?.toDouble()) ?? 0,
    );
  }
}

bool _matchesHelloOutputPerson(sdk.JsonMap json) {
  return json.containsKey('name') && json.containsKey('age');
}

final class HelloOutputUnknown extends HelloOutput {
  const HelloOutputUnknown(this.raw);

  final sdk.JsonMap raw;

  @override
  String get variant => 'Unknown';
}

typedef HelloContext = sdk.JsonMap;

typedef HelloTools = Map<String, sdk.ToolHandler>;

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

typedef HelloIntentValue = Object?;
typedef HelloIntentControl = Object?;
typedef HelloIntentHandler = FutureOr<HelloIntentControl> Function(String name, Object? value, String agentName);
typedef HelloPartialIntentHandler = FutureOr<void> Function(String name, Object? value, String agentName);

abstract class HelloBaseIntentHandler {
  FutureOr<HelloIntentControl> responseText(HelloResponseTextIntent intent, String agentName) => null;
  FutureOr<HelloIntentControl> responseSchema(HelloResponseSchemaIntent intent, String agentName) => null;
  FutureOr<HelloIntentControl> error(HelloErrorIntent intent, String agentName) => null;
}

abstract class HelloBasePartialIntentHandler {
  FutureOr<void> responseText(HelloResponseTextIntent intent, String agentName) {}
  FutureOr<void> responseSchema(HelloResponseSchemaIntent intent, String agentName) {}
  FutureOr<void> error(HelloErrorIntent intent, String agentName) {}
}

final class HelloApiKeys {
  const HelloApiKeys({
    this.geminiApiKey,
  });

  final String? geminiApiKey;

  Map<String, String> toMap() {
    return {
      if (geminiApiKey != null && geminiApiKey!.isNotEmpty) 'geminiApiKey': geminiApiKey!,
    };
  }
}

typedef HelloMiddleware = sdk.Middleware;

final class HelloConfig {
  const HelloConfig({
    this.tools = const {},
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
      tools: tools,
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
