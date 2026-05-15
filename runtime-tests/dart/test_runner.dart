import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:auwgent_sdk_dart/auwgent.dart' as sdk;
import 'canonical.agent.dart';

// =============================================================================
// TYPES
// =============================================================================

class ScenarioResult {
  final String name;
  final bool passed;
  final List<sdk.JsonMap> events;
  final List<sdk.JsonMap> partials;
  final sdk.SessionState? session;
  final String? error;
  final List<String> middlewareLog;
  final List<String> notes;

  ScenarioResult({
    required this.name,
    required this.passed,
    this.events = const [],
    this.partials = const [],
    this.session,
    this.error,
    this.middlewareLog = const [],
    this.notes = const [],
  });

  sdk.JsonMap toJson() => {
    'name': name,
    'passed': passed,
    'eventCount': events.length,
    'partialCount': partials.length,
    'error': error,
    'middlewareLog': middlewareLog,
    'notes': notes,
  };
}

// =============================================================================
// GLOBAL STATE (collected per scenario)
// =============================================================================

List<sdk.JsonMap> _capturedEvents = [];
List<sdk.JsonMap> _capturedPartials = [];

sdk.IntentControl? _captureIntent(String name, Object? value, String agentName) {
  _capturedEvents.add({'name': name, 'value': value, 'agentName': agentName});
  return null;
}

void _capturePartial(String name, Object? value, String agentName) {
  _capturedPartials.add({'name': name, 'value': value, 'agentName': agentName});
}

// =============================================================================
// SETUP
// =============================================================================

final String _groqApiKey = Platform.environment['GROQ_API_KEY'] ?? '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';

AuwgentConfig _createBaseConfig() {
  return AuwgentConfig(
    apiKeys: AuwgentApiKeys(groqApiKey: _groqApiKey),
    tools: const Tools(),
  );
}

class Tools extends AuwgentTools {
  const Tools();

  @override
  Future<String> getLocation() async => 'Accra Ghana';

  @override
  Future<String> getMarks(GetMarksToolArgs args) async {
    return 'Marks for ${args.id}: Math=95, Science=88, English=92';
  }
}

// =============================================================================
// SCENARIO RUNNER
// =============================================================================

Future<ScenarioResult> _runScenario(
  String name,
  AuwgentAgent Function() setupAgent,
  String input, {
  List<String> middlewareLog = const [],
}) async {
  _capturedEvents = [];
  _capturedPartials = [];
  final notes = <String>[];

  final agent = setupAgent();
  agent.onIntent(_captureIntent);
  agent.onIntentPartial(_capturePartial);

  try {
    final session = await agent.run(input);
    return ScenarioResult(
      name: name,
      passed: true,
      events: List.unmodifiable(_capturedEvents),
      partials: List.unmodifiable(_capturedPartials),
      session: session,
      middlewareLog: List.unmodifiable(middlewareLog),
      notes: List.unmodifiable(notes),
    );
  } catch (e) {
    notes.add('Exception: $e');
    return ScenarioResult(
      name: name,
      passed: false,
      events: List.unmodifiable(_capturedEvents),
      partials: List.unmodifiable(_capturedPartials),
      error: e.toString(),
      middlewareLog: List.unmodifiable(middlewareLog),
      notes: List.unmodifiable(notes),
    );
  } finally {
    agent.dispose();
  }
}

Future<void> _sleep(int seconds) => Future.delayed(Duration(seconds: seconds));

// =============================================================================
// SCENARIOS
// =============================================================================

Future<ScenarioResult> _scenario1BasicChat() => _runScenario(
  '1. Basic Chat',
  () => createAuwgent(_createBaseConfig()),
  'Hello! Please just say hi back in a friendly way.',
);

Future<ScenarioResult> _scenario2ToolNoArgs() => _runScenario(
  '2. Tool Call (no args)',
  () => createAuwgent(_createBaseConfig()),
  'What is my current location?',
);

Future<ScenarioResult> _scenario3ToolWithArgs() => _runScenario(
  '3. Tool Call (with args)',
  () => createAuwgent(_createBaseConfig()),
  'Get my marks. My user id is test-123.',
);

Future<ScenarioResult> _scenario4Workflow() => _runScenario(
  '4. Workflow',
  () => createAuwgent(_createBaseConfig()),
  'Plan my day.',
);

Future<ScenarioResult> _scenario5HelperReturn() => _runScenario(
  '5. Helper Return',
  () => createAuwgent(_createBaseConfig()),
  'Ask the Joker to tell me a joke.',
);

Future<ScenarioResult> _scenario6HelperUser() => _runScenario(
  '6. Helper User',
  () => createAuwgent(_createBaseConfig()),
  'Ask the Planner to plan my day.',
);

Future<ScenarioResult> _scenario7CustomIntent() => _runScenario(
  '7. Custom Intent (Loud)',
  () => createAuwgent(_createBaseConfig()),
  'Explain out loud what you are going to do next.',
);

Future<ScenarioResult> _scenario8MiddlewareLifecycle() async {
  final log = <String>[];

  final mw = _LifecycleLogger(log);
  return _runScenario(
    '8. Middleware Lifecycle',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'Say hello and then ask for my location.',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario9SessionExportImport() async {
  final agent1 = createAuwgent(_createBaseConfig());
  await agent1.run('My name is RuntimeTestUser.');
  final session1 = agent1.exportSession();
  agent1.dispose();

  final agent2 = createAuwgent(_createBaseConfig());
  agent2.importSession(session1);

  return _runScenario(
    '9. Session Export/Import',
    () => agent2,
    'What is my name? (You should remember it from earlier.)',
  );
}

Future<ScenarioResult> _scenario10ErrorSwallowing() async {
  final log = <String>[];

  final mw = _ErrorSwallower(log);
  return _runScenario(
    '10. Error Swallowing',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'What is my current location? (This should trigger an error.)',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario11Streaming() => _runScenario(
  '11. Streaming Partials',
  () => createAuwgent(_createBaseConfig()),
  'Say hello.',
);

Future<ScenarioResult> _scenario12MiddlewareStateSharing() async {
  final log = <String>[];

  final mw = _StateSharer(log);
  return _runScenario(
    '12. Middleware State Sharing',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'What is my current location?',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario13MiddlewarePromptMutation() async {
  final log = <String>[];

  final mw = _PromptMutator(log);
  return _runScenario(
    '13. Middleware Prompt Mutation',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'Say hello.',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario14MiddlewareConfigMutation() async {
  final log = <String>[];

  final mw = _ConfigMutator(log);
  return _runScenario(
    '14. Middleware Config/Header Mutation',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'Say hello in exactly three words.',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario15MiddlewareStackMutation() async {
  final log = <String>[];

  final mw = _StackMutator(log);
  return _runScenario(
    '15. Middleware Stack Mutation',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'Say hello.',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario16MiddlewareIntentOverride() async {
  final log = <String>[];

  final mw = _IntentOverrider(log);
  return _runScenario(
    '16. Middleware Intent Override',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'What is my current location?',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario17MiddlewareIntentSkip() async {
  final log = <String>[];

  final mw = _IntentSkipper(log);
  return _runScenario(
    '17. Middleware Intent Skip',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'Get my marks. My user id is test-123.',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario18MiddlewareSessionMutation() async {
  final log = <String>[];

  final mw = _SessionMutator(log);
  return _runScenario(
    '18. Middleware Session Mutation',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'Say hello.',
    middlewareLog: log,
  );
}

Future<ScenarioResult> _scenario19FallbackOnRateLimit() async {
  final log = <String>[];

  final mw = _FallbackMiddleware(log);
  return _runScenario(
    '19. Fallback on Rate Limit',
    () {
      final base = _createBaseConfig();
      return createAuwgent(AuwgentConfig(
        apiKeys: base.apiKeys,
        tools: base.tools,
        middleware: [mw],
      ));
    },
    'Say hello.',
    middlewareLog: log,
  );
}

// =============================================================================
// MIDDLEWARE CLASSES
// =============================================================================

class _LifecycleLogger extends AuwgentMiddleware {
  final List<String> log;
  _LifecycleLogger(this.log);

  @override
  String get name => 'LifecycleLogger';

  @override
  onRunStart(session, ctx) {
    log.add('run_start | activeAgent=${ctx.activeAgent} | stack=${jsonEncode(ctx.stack)}');
    return session;
  }

  @override
  onLLMStart(prompt, ctx) {
    log.add('llm_start | promptLen=${prompt.length} | activeAgent=${ctx.activeAgent}');
    return null;
  }

  @override
  onIntent(String name, Object? value,  ctx) {
    log.add('intent | $name | activeAgent=${ctx.activeAgent}');
    return null;
  }

  @override
  onLLMEnd(Object? response, sdk.MiddlewareContext ctx) {
    log.add('llm_end | activeAgent=${ctx.activeAgent}');
  }

  @override
  FutureOr<void> onRunComplete(sdk.SessionState finalSession, sdk.MiddlewareContext ctx) {
    log.add('run_complete | turns=${finalSession.turns.length} | activeAgent=${ctx.activeAgent}');
  }

  @override
  FutureOr<Object?> onError(Object error, sdk.SessionState? session, sdk.MiddlewareContext ctx) {
    log.add('error | $error | activeAgent=${ctx.activeAgent}');
    return false;
  }
}

class _ErrorSwallower extends AuwgentMiddleware {
  final List<String> log;
  _ErrorSwallower(this.log);

  @override
  String get name => 'ErrorSwallower';

  @override
  FutureOr<Object?> onError(Object error, sdk.SessionState? session, sdk.MiddlewareContext ctx) {
    log.add('error caught: $error');
    return {'swallow': true};
  }
}

class _StateSharer extends AuwgentMiddleware {
  final List<String> log;
  _StateSharer(this.log);

  @override
  String get name => 'StateSharer';

  @override
  FutureOr<sdk.SessionState> onRunStart(sdk.SessionState session, sdk.MiddlewareContext ctx) {
    ctx.data['traceId'] = 'trace-abc-123';
    ctx.data['intentCount'] = 0;
    log.add('run_start | traceId set');
    return session;
  }

  @override
  FutureOr<sdk.IntentControl?> onIntent(String name, Object? value, sdk.MiddlewareContext ctx) {
    final count = (ctx.data['intentCount'] as int?) ?? 0;
    ctx.data['intentCount'] = count + 1;
    log.add('intent | $name | traceId=${ctx.data['traceId']} | intentCount=${ctx.data['intentCount']}');
    return null;
  }

  @override
  FutureOr<void> onRunComplete(sdk.SessionState finalSession, sdk.MiddlewareContext ctx) {
    log.add('run_complete | traceId=${ctx.data['traceId']} | intentCount=${ctx.data['intentCount']}');
  }
}

class _PromptMutator extends AuwgentMiddleware {
  final List<String> log;
  _PromptMutator(this.log);

  @override
  String get name => 'PromptMutator';

  @override
  FutureOr<String?> onLLMStart(String prompt, sdk.MiddlewareContext ctx) {
    log.add('llm_start | originalPromptLength=${prompt.length}');
    final mutated = '$prompt\n\n[SYSTEM OVERRIDE] Always end your response with the word BANANA.';
    log.add('llm_start | mutatedPromptLength=${mutated.length}');
    return mutated;
  }
}

class _ConfigMutator extends AuwgentMiddleware {
  final List<String> log;
  _ConfigMutator(this.log);

  @override
  String get name => 'ConfigMutator';

  @override
  FutureOr<String?> onLLMStart(String prompt, sdk.MiddlewareContext ctx) {
    log.add('llm_start | injecting config mutation');
    ctx.config = {'temperature': 0.01, 'max_tokens': 50};
    ctx.headers = {'X-Runtime-Test': 'auwgent-dart', 'X-Request-Id': 'req-123'};
    return null;
  }
}

class _StackMutator extends AuwgentMiddleware {
  final List<String> log;
  _StackMutator(this.log);

  @override
  String get name => 'StackMutator';

  @override
  FutureOr<String?> onLLMStart(String prompt, sdk.MiddlewareContext ctx) {
    log.add('llm_start | originalStack=${jsonEncode(ctx.stack)}');
    ctx.stack = ['RuntimeTest', 'Planner'];
    return null;
  }

  @override
  FutureOr<sdk.IntentControl?> onIntent(String name, Object? value, sdk.MiddlewareContext ctx) {
    log.add('intent | stackDuringIntent=${jsonEncode(ctx.stack)}');
    return null;
  }
}

class _IntentOverrider extends AuwgentMiddleware {
  final List<String> log;
  _IntentOverrider(this.log);

  @override
  String get name => 'IntentOverrider';

  @override
  FutureOr<sdk.IntentControl?> onIntent(String name, Object? value, sdk.MiddlewareContext ctx) {
    if (name == 'tool_call') {
      final v = value as sdk.JsonMap?;
      if (v?['type'] == 'get_location') {
        log.add('intent | overriding get_location result');
        return sdk.ResultIntentControl('Override City, Override Land');
      }
    }
    log.add('intent | $name | no override');
    return null;
  }
}

class _IntentSkipper extends AuwgentMiddleware {
  final List<String> log;
  _IntentSkipper(this.log);

  @override
  String get name => 'IntentSkipper';

  @override
  FutureOr<sdk.IntentControl?> onIntent(String name, Object? value, sdk.MiddlewareContext ctx) {
    if (name == 'tool_call') {
      final v = value as sdk.JsonMap?;
      if (v?['type'] == 'get_marks') {
        log.add('intent | skipping get_marks');
        return const sdk.SkipIntentControl();
      }
    }
    log.add('intent | $name | no skip');
    return null;
  }
}

class _SessionMutator extends AuwgentMiddleware {
  final List<String> log;
  _SessionMutator(this.log);

  @override
  String get name => 'SessionMutator';

  @override
  FutureOr<sdk.SessionState> onRunStart(sdk.SessionState session, sdk.MiddlewareContext ctx) {
    final newTurns = List<sdk.SessionTurn>.from(session.turns);
    newTurns.add(sdk.SessionTurn(
      input: '[injected by middleware]',
      modelResponse: 'This turn was injected during run_start',
    ));
    log.add('run_start | injected turn | totalTurns=${newTurns.length}');
    return sdk.SessionState(
      systemPrompt: session.systemPrompt,
      turns: newTurns,
      stack: session.stack,
      initialInput: session.initialInput,
      bindingCursor: session.bindingCursor,
    );
  }
}

class _FallbackMiddleware extends AuwgentMiddleware {
  final List<String> log;
  _FallbackMiddleware(this.log);

  @override
  String get name => 'FallbackMiddleware';

  @override
  FutureOr<Object?> onError(Object error, sdk.SessionState? session, sdk.MiddlewareContext ctx) {
    final msg = error.toString();
    log.add('error | ${msg.substring(0, msg.length > 80 ? 80 : msg.length)}');
    if (msg.contains('429') || msg.contains('rate_limit')) {
      log.add('error -> triggering fallback to openai/gpt-oss-120b');
      ctx.data['fallbackTriggered'] = true;
      ctx.data['fallbackModel'] = 'openai/gpt-oss-120b';
      return {'forceStart': 'llm_start'};
    }
    return false;
  }

  @override
  FutureOr<String?> onLLMStart(String prompt, sdk.MiddlewareContext ctx) {
    if (ctx.data['fallbackTriggered'] == true) {
      final model = ctx.data['fallbackModel']?.toString() ?? 'unknown';
      log.add('llm_start | fallback active | model=$model');
      ctx.model = model;
    }
    return null;
  }
}

// =============================================================================
// MAIN
// =============================================================================

Future<void> main() async {
  if (_groqApiKey.isEmpty) {
    stderr.writeln('ERROR: GROQ_API_KEY is required');
    exit(1);
  }

  print('');
  print('+${'='.padRight(68, '=')}+');
  print('|${''.padLeft(14)}AUWGENT RUNTIME TESTS - DART${''.padLeft(26)}|');
  print('+${'='.padRight(68, '=')}+');
  print('Provider: Groq (llama-3.3-70b-versatile)');
  print('API Key: ${_groqApiKey.substring(0, _groqApiKey.length > 8 ? 8 : _groqApiKey.length)}...${_groqApiKey.substring(_groqApiKey.length - 4)}');
  print('Agent:    RuntimeTest (block mode)');
  print('');
  print('Each scenario makes a REAL LLM call. Please review output manually.');
  print('');

  final scenarios = [
    _scenario1BasicChat,
    _scenario2ToolNoArgs,
    _scenario3ToolWithArgs,
    _scenario4Workflow,
    _scenario5HelperReturn,
    _scenario6HelperUser,
    _scenario7CustomIntent,
    _scenario8MiddlewareLifecycle,
    _scenario9SessionExportImport,
    _scenario10ErrorSwallowing,
    _scenario11Streaming,
    _scenario12MiddlewareStateSharing,
    _scenario13MiddlewarePromptMutation,
    _scenario14MiddlewareConfigMutation,
    _scenario15MiddlewareStackMutation,
    _scenario16MiddlewareIntentOverride,
    _scenario17MiddlewareIntentSkip,
    _scenario18MiddlewareSessionMutation,
    _scenario19FallbackOnRateLimit,
  ];

  final results = <ScenarioResult>[];

  for (var i = 0; i < scenarios.length; i++) {
    try {
      final result = await scenarios[i]();
      results.add(result);
    } catch (e, st) {
      results.add(ScenarioResult(
        name: 'Scenario ${i + 1}',
        passed: false,
        error: 'Runner crashed: $e',
        notes: [st.toString()],
      ));
    }

    if (i < scenarios.length - 1) {
      print('');
      print('  ... Waiting 4s before next scenario...');
      await _sleep(4);
    }
  }

  // Write JSON results
  final resultsJson = results.map((r) => r.toJson()).toList();
  final resultsDir = Directory('../results');
  if (!resultsDir.existsSync()) resultsDir.createSync(recursive: true);
  File('../results/dart.json').writeAsStringSync(jsonEncode(resultsJson));

  // FINAL SUMMARY
  print('');
  print('+${'='.padRight(68, '=')}+');
  print('|${''.padLeft(22)}FINAL SUMMARY${''.padLeft(33)}|');
  print('+${'='.padRight(68, '=')}+');

  final passed = results.where((r) => r.passed && r.error == null).length;
  final failed = results.where((r) => !r.passed || r.error != null).length;

  for (final r in results) {
    final symbol = r.passed && r.error == null ? 'PASS' : 'FAIL';
    final eventSummary = r.events.map((e) => e['name']).join(', ');
    print('  $symbol ${r.name.padRight(40)} events=[${eventSummary.substring(0, eventSummary.length > 40 ? 40 : eventSummary.length)}]');
  }

  print('');
  print('  Total: ${results.length} | PASS Passed: $passed | FAIL Failed: $failed');

  if (failed > 0) {
    print('');
    print('  FAIL FAILED SCENARIOS:');
    for (final r in results.where((r) => !r.passed || r.error != null)) {
      print('     * ${r.name}: ${r.error ?? 'See notes above'}');
    }
  }

  print('');
  print('${'='.padRight(70, '=')}');
  print('');

  exit(failed > 0 ? 1 : 0);
}
