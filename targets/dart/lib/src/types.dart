import 'dart:async';

typedef JsonMap = Map<String, Object?>;

final class NoArgs {
  const NoArgs();

  factory NoArgs.fromJson(JsonMap json) {
    return const NoArgs();
  }

  JsonMap toJson() => const {};
}

final class NoResult {
  const NoResult();

  factory NoResult.fromJson(Object? value) {
    return const NoResult();
  }
}

typedef ToolHandler = FutureOr<Object?> Function(JsonMap args);
typedef IntentHandler =
    FutureOr<Object?> Function(String name, Object? value, String agentName);
typedef PartialIntentHandler =
    FutureOr<void> Function(String name, Object? value, String agentName);
typedef MiddlewareEventHandler = FutureOr<String?> Function(String eventJson);
typedef SessionTransformHandler =
    FutureOr<String?> Function(String primaryName, String sessionJson);
typedef SessionNotifyHandler =
    FutureOr<void> Function(String primaryName, String sessionJson);
typedef LlmStartHandler =
    FutureOr<Object?> Function(
      String inputJson,
      String systemPrompt,
      String contextJson,
    );
typedef LlmEndHandler =
    FutureOr<void> Function(String rawResponse, String systemPrompt);
typedef ErrorHandler =
    FutureOr<bool> Function(
      Object error,
      SessionState? session,
      JsonMap context,
    );

typedef IntentControl = Object?;

enum AuwgentWarningSource {
  onIntent,
  onIntentPartial,
  onMiddlewareEvent,
  onSubEngineStart,
  onSubEngineComplete,
  onLlmStart,
  onLlmEnd,
  middleware,
  run,
}

final class AuwgentWarning {
  const AuwgentWarning({
    required this.timestamp,
    required this.source,
    required this.message,
    this.detail,
    this.agentName,
  });

  final DateTime timestamp;
  final AuwgentWarningSource source;
  final String message;
  final String? detail;
  final String? agentName;
}

final class SessionTurn {
  SessionTurn({required this.input, required this.modelResponse});

  final String input;
  final String modelResponse;

  factory SessionTurn.fromJson(JsonMap json) {
    return SessionTurn(
      input: (json['input'] as String?) ?? '',
      modelResponse: (json['model_response'] as String?) ?? '',
    );
  }

  JsonMap toJson() => {'input': input, 'model_response': modelResponse};
}

final class SessionState {
  SessionState({
    this.systemPrompt,
    this.turns = const [],
    this.stack = const [],
    this.initialInput,
  });

  final String? systemPrompt;
  final List<SessionTurn> turns;
  final List<String> stack;
  final Object? initialInput;

  factory SessionState.fromJson(JsonMap json) {
    final turnsRaw = (json['turns'] as List?) ?? const [];
    return SessionState(
      systemPrompt: json['systemPrompt'] as String?,
      turns: turnsRaw
          .whereType<Map>()
          .map((turn) => SessionTurn.fromJson(Map<String, Object?>.from(turn)))
          .toList(growable: false),
      stack: ((json['stack'] as List?) ?? const [])
          .map((item) => item.toString())
          .toList(growable: false),
      initialInput: json['initialInput'],
    );
  }

  JsonMap toJson() => {
    if (systemPrompt != null) 'systemPrompt': systemPrompt,
    'turns': turns.map((turn) => turn.toJson()).toList(growable: false),
    'stack': stack,
    if (initialInput != null) 'initialInput': initialInput,
  };
}

final class AuwgentConfig {
  const AuwgentConfig({
    this.tools = const {},
    this.middleware = const [],
    this.context,
    this.apiKeys = const {},
    this.libraryPath,
  });

  final Map<String, ToolHandler> tools;
  final List<Object> middleware;
  final JsonMap? context;
  final Map<String, String> apiKeys;
  final String? libraryPath;
}
