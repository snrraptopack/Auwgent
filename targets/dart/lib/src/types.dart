import 'dart:async';
import 'dart:convert';

typedef JsonMap = Map<String, Object?>;

String prettyJson(Object? value) {
  const encoder = JsonEncoder.withIndent('  ');
  return encoder.convert(value);
}

final class NoArgs {
  const NoArgs();

  factory NoArgs.fromJson(JsonMap json) {
    return const NoArgs();
  }

  JsonMap toJson() => const {};

  @override
  String toString() => prettyJson(toJson());
}

final class NoResult {
  const NoResult();

  factory NoResult.fromJson(Object? value) {
    return const NoResult();
  }

  JsonMap toJson() => const {};

  @override
  String toString() => prettyJson(toJson());
}

typedef ToolHandler = FutureOr<Object?> Function(JsonMap args);
typedef IntentHandler =
    FutureOr<IntentControl?> Function(String name, Object? value, String agentName);
typedef PartialIntentHandler =
    FutureOr<void> Function(String name, Object? value, String agentName);
typedef StructuredIntentDecoder<T> = T Function(JsonMap json);
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

sealed class IntentControl {
  const IntentControl();

  JsonMap toJson();
}

final class SkipIntentControl extends IntentControl {
  const SkipIntentControl();

  @override
  JsonMap toJson() => const {'skip': true};

  @override
  String toString() => prettyJson(toJson());
}

final class ResultIntentControl extends IntentControl {
  const ResultIntentControl(this.result);

  final Object? result;

  @override
  JsonMap toJson() => {'result': result};

  @override
  String toString() => prettyJson(toJson());
}

final class TokenUsage {
  const TokenUsage({
    required this.promptTokens,
    required this.completionTokens,
    required this.totalTokens,
    required this.reasoningTokens,
    required this.cachedTokens,
  });

  final int promptTokens;
  final int completionTokens;
  final int totalTokens;
  final int reasoningTokens;
  final int cachedTokens;

  factory TokenUsage.fromJson(JsonMap json) {
    return TokenUsage(
      promptTokens: ((json['prompt_tokens'] as num?)?.toInt()) ?? 0,
      completionTokens: ((json['completion_tokens'] as num?)?.toInt()) ?? 0,
      totalTokens: ((json['total_tokens'] as num?)?.toInt()) ?? 0,
      reasoningTokens: ((json['reasoning_tokens'] as num?)?.toInt()) ?? 0,
      cachedTokens: ((json['cached_tokens'] as num?)?.toInt()) ?? 0,
    );
  }

  JsonMap toJson() => {
    'prompt_tokens': promptTokens,
    'completion_tokens': completionTokens,
    'total_tokens': totalTokens,
    'reasoning_tokens': reasoningTokens,
    'cached_tokens': cachedTokens,
  };

  @override
  String toString() => prettyJson(toJson());
}

sealed class FinishReason {
  const FinishReason();

  factory FinishReason.fromJson(Object? value) {
    if (value == null) return const NullFinishReason();
    if (value is String) {
      switch (value) {
        case 'stop':
          return const StopFinishReason();
        case 'length':
          return const LengthFinishReason();
        case 'tool_calls':
          return const ToolCallsFinishReason();
        case 'content_filter':
          return const ContentFilterFinishReason();
        default:
          return OtherFinishReason(value);
      }
    }
    return OtherFinishReason(value.toString());
  }
}

final class NullFinishReason extends FinishReason {
  const NullFinishReason();

  @override
  String toString() => 'null';
}

final class StopFinishReason extends FinishReason {
  const StopFinishReason();

  @override
  String toString() => 'stop';
}

final class LengthFinishReason extends FinishReason {
  const LengthFinishReason();

  @override
  String toString() => 'length';
}

final class ToolCallsFinishReason extends FinishReason {
  const ToolCallsFinishReason();

  @override
  String toString() => 'tool_calls';
}

final class ContentFilterFinishReason extends FinishReason {
  const ContentFilterFinishReason();

  @override
  String toString() => 'content_filter';
}

final class OtherFinishReason extends FinishReason {
  const OtherFinishReason(this.value);

  final String value;

  @override
  String toString() => value;
}

final class TurnMetadata {
  const TurnMetadata({
    required this.turnIndex,
    required this.usage,
    required this.finishReason,
    required this.model,
  });

  final int turnIndex;
  final TokenUsage usage;
  final FinishReason? finishReason;
  final String model;

  factory TurnMetadata.fromJson(JsonMap json) {
    return TurnMetadata(
      turnIndex: ((json['turn_index'] as num?)?.toInt()) ?? 0,
      usage: TokenUsage.fromJson(
        Map<String, Object?>.from((json['usage'] as Map?) ?? const {}),
      ),
      finishReason: json.containsKey('finish_reason')
          ? FinishReason.fromJson(json['finish_reason'])
          : null,
      model: (json['model'] as String?) ?? '',
    );
  }

  JsonMap toJson() => {
    'turn_index': turnIndex,
    'usage': usage.toJson(),
    'finish_reason': finishReason?.toString(),
    'model': model,
  };

  @override
  String toString() => prettyJson(toJson());
}

final class AggregateUsage {
  const AggregateUsage({
    required this.promptTokens,
    required this.completionTokens,
    required this.totalTokens,
    required this.reasoningTokens,
    required this.cachedTokens,
  });

  final int promptTokens;
  final int completionTokens;
  final int totalTokens;
  final int reasoningTokens;
  final int cachedTokens;

  factory AggregateUsage.fromJson(JsonMap json) {
    return AggregateUsage(
      promptTokens: ((json['prompt_tokens'] as num?)?.toInt()) ?? 0,
      completionTokens: ((json['completion_tokens'] as num?)?.toInt()) ?? 0,
      totalTokens: ((json['total_tokens'] as num?)?.toInt()) ?? 0,
      reasoningTokens: ((json['reasoning_tokens'] as num?)?.toInt()) ?? 0,
      cachedTokens: ((json['cached_tokens'] as num?)?.toInt()) ?? 0,
    );
  }

  JsonMap toJson() => {
    'prompt_tokens': promptTokens,
    'completion_tokens': completionTokens,
    'total_tokens': totalTokens,
    'reasoning_tokens': reasoningTokens,
    'cached_tokens': cachedTokens,
  };

  @override
  String toString() => prettyJson(toJson());
}

final class RunMetadata {
  const RunMetadata({
    required this.aggregate,
    required this.turns,
  });

  final AggregateUsage aggregate;
  final List<TurnMetadata> turns;

  factory RunMetadata.fromJson(JsonMap json) {
    final turnsRaw = (json['turns'] as List?) ?? const [];
    return RunMetadata(
      aggregate: AggregateUsage.fromJson(
        Map<String, Object?>.from((json['aggregate'] as Map?) ?? const {}),
      ),
      turns: turnsRaw
          .whereType<Map>()
          .map((turn) => TurnMetadata.fromJson(Map<String, Object?>.from(turn)))
          .toList(growable: false),
    );
  }

  JsonMap toJson() => {
    'aggregate': aggregate.toJson(),
    'turns': turns.map((turn) => turn.toJson()).toList(growable: false),
  };

  @override
  String toString() => prettyJson(toJson());
}

final class PartialIntentEnvelope {
  const PartialIntentEnvelope({
    required this.partial,
    required this.complete,
    required this.mode,
    required this.segment,
    required this.raw,
  });

  final bool partial;
  final bool complete;
  final String mode;
  final int segment;
  final String raw;

  factory PartialIntentEnvelope.fromJson(JsonMap json) {
    return PartialIntentEnvelope(
      partial: (json['partial'] as bool?) ?? true,
      complete: (json['complete'] as bool?) ?? false,
      mode: (json['mode'] as String?) ?? 'structured',
      segment: ((json['segment'] as num?)?.toInt()) ?? 0,
      raw: (json['raw'] as String?) ?? '',
    );
  }

  JsonMap toJson() => {
    'partial': partial,
    'complete': complete,
    'mode': mode,
    'segment': segment,
    'raw': raw,
  };

  @override
  String toString() => prettyJson(toJson());
}

final class PartialTextIntentValue {
  const PartialTextIntentValue({
    required this.envelope,
    required this.text,
    this.delta,
  });

  final PartialIntentEnvelope envelope;
  final String text;
  final String? delta;

  bool get partial => envelope.partial;
  bool get complete => envelope.complete;
  String get mode => envelope.mode;
  int get segment => envelope.segment;
  String get raw => envelope.raw;

  factory PartialTextIntentValue.fromJson(JsonMap json) {
    return PartialTextIntentValue(
      envelope: PartialIntentEnvelope.fromJson(json),
      text: (json['text'] as String?) ?? '',
      delta: json['delta'] as String?,
    );
  }

  JsonMap toJson() => {
    ...envelope.toJson(),
    'text': text,
    if (delta != null) 'delta': delta,
  };

  @override
  String toString() => prettyJson(toJson());
}

final class PartialStructuredIntentValue<T> {
  const PartialStructuredIntentValue({
    required this.envelope,
    required this.value,
  });

  final PartialIntentEnvelope envelope;
  final T value;

  bool get partial => envelope.partial;
  bool get complete => envelope.complete;
  String get mode => envelope.mode;
  int get segment => envelope.segment;
  String get raw => envelope.raw;

  factory PartialStructuredIntentValue.fromJson(
    JsonMap json,
    StructuredIntentDecoder<T> decode,
  ) {
    final payload = Map<String, Object?>.from(json)
      ..remove('partial')
      ..remove('complete')
      ..remove('mode')
      ..remove('segment')
      ..remove('raw');

    return PartialStructuredIntentValue(
      envelope: PartialIntentEnvelope.fromJson(json),
      value: decode(payload),
    );
  }

  JsonMap toJson() => {
    ...envelope.toJson(),
    'value': value,
  };

  @override
  String toString() => prettyJson(toJson());
}

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

  @override
  String toString() => prettyJson(toJson());
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

  @override
  String toString() => prettyJson(toJson());
}

final class AuwgentConfig {
  const AuwgentConfig({
    this.tools = const {},
    this.middleware = const [],
    this.context,
    this.apiKeys = const {},
    this.libraryPath,
    this.autoDispose = true,
  });

  final Map<String, ToolHandler> tools;
  final List<Object> middleware;
  final JsonMap? context;
  final Map<String, String> apiKeys;
  final String? libraryPath;
  final bool autoDispose;
}
