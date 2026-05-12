import 'dart:async';

import 'types.dart';

/// Result from `onLLMStart` when mutating provider request fields.
final class MiddlewareLLMStartResult {
  MiddlewareLLMStartResult({
    this.prompt,
    this.stack,
    this.config,
    this.provider,
    this.url,
    this.headers,
  });

  final String? prompt;
  final List<String>? stack;
  final JsonMap? config;
  final String? provider;
  final String? url;
  final JsonMap? headers;

  JsonMap toJson() => {
        if (prompt != null) 'prompt': prompt,
        if (stack != null) 'stack': stack,
        if (config != null) 'config': config,
        if (provider != null) 'provider': provider,
        if (url != null) 'url': url,
        if (headers != null) 'headers': headers,
      };
}

/// Result from `onError` when controlling error handling.
final class MiddlewareErrorResult {
  MiddlewareErrorResult({this.swallow = false, this.forceStart});

  final bool swallow;
  final String? forceStart;

  JsonMap toJson() => {
        'swallow': swallow,
        if (forceStart != null) 'forceStart': forceStart,
      };
}

abstract interface class Middleware {
  String get name;

  Object? get target => null;

  FutureOr<SessionState> onRunStart(
    SessionState session,
    MiddlewareContext ctx,
  ) {
    return session;
  }

  /// Return `null` to proceed, a [String] to replace the prompt,
  /// or a [MiddlewareLLMStartResult] to mutate config/provider/headers.
  FutureOr<Object?> onLLMStart(String prompt, MiddlewareContext ctx) {
    return null;
  }

  FutureOr<IntentControl?> onIntent(
    String name,
    Object? value,
    MiddlewareContext ctx,
  ) {
    return null;
  }

  FutureOr<void> onIntentPartial(
    String name,
    Object? value,
    MiddlewareContext ctx,
  ) {}

  FutureOr<void> onLLMEnd(Object? response, MiddlewareContext ctx) {}

  FutureOr<void> onRunComplete(
    SessionState finalSession,
    MiddlewareContext ctx,
  ) {}

  /// Return `true`/`false` to swallow or propagate, or a [MiddlewareErrorResult]
  /// for fine-grained control including `forceStart`.
  FutureOr<Object?> onError(
    Object error,
    SessionState? session,
    MiddlewareContext ctx,
  ) {
    return false;
  }
}

final class MiddlewareContext {
  MiddlewareContext({
    required this.activeAgent,
    required this.stack,
    required this.rootAgent,
    required this.rawBlock,
    required this.systemPrompt,
    required this.setContext,
    Map<String, Object?>? data,
  }) : data = data == null ? <String, Object?>{} : Map<String, Object?>.from(data);

  String activeAgent;
  List<String> stack;
  String rootAgent;
  String? rawBlock;
  String? systemPrompt;
  String? model;
  String? provider;
  JsonMap? config;
  String? url;
  JsonMap? headers;
  final void Function(JsonMap value) setContext;
  final Map<String, Object?> data;

  Object? operator [](String key) => data[key];

  void operator []=(String key, Object? value) {
    data[key] = value;
  }

  @override
  String toString() =>
      prettyJson({
        'activeAgent': activeAgent,
        'stack': stack,
        'rootAgent': rootAgent,
        'rawBlock': rawBlock,
        'systemPrompt': systemPrompt,
        'model': model,
        'provider': provider,
        'config': config,
        'url': url,
        'headers': headers,
        'data': data,
      });
}
