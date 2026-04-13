import 'dart:async';

import 'types.dart';

abstract interface class Middleware {
  String get name;

  Object? get target => null;

  FutureOr<SessionState> onRunStart(
    SessionState session,
    MiddlewareContext ctx,
  ) {
    return session;
  }

  FutureOr<String?> onLLMStart(String prompt, MiddlewareContext ctx) {
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

  FutureOr<bool> onError(
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
  });

  String activeAgent;
  List<String> stack;
  String rootAgent;
  String? rawBlock;
  String? systemPrompt;
  final void Function(JsonMap value) setContext;

  @override
  String toString() =>
      prettyJson({
        'activeAgent': activeAgent,
        'stack': stack,
        'rootAgent': rootAgent,
        'rawBlock': rawBlock,
        'systemPrompt': systemPrompt,
      });
}
