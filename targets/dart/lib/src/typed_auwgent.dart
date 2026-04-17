import 'dart:async';
import 'dart:convert';

import 'auwgent_native.dart';
import 'middleware.dart';
import 'types.dart';

class TypedAuwgent<IR extends JsonMap> {
  TypedAuwgent(this.ir, this.config)
    : native = AuwgentNative.fromIrJson(
        irJson: jsonEncode(ir),
        libraryPath: config.libraryPath,
      ),
      middleware = List<Middleware>.from(
        config.middleware.whereType<Middleware>(),
      ) {
    if (config.context != null) {
      native.setContext(config.context!);
    }

    _registerDrivers(config.apiKeys);
    _registerTools(config.tools);
  }

  final IR ir;
  final AuwgentConfig config;
  final AuwgentNative native;
  final List<Middleware> middleware;

  IntentHandler? _storedIntentHandler;
  PartialIntentHandler? _storedPartialIntentHandler;
  final List<AuwgentWarning> _warnings = [];
  void Function(AuwgentWarning warning)? _warningHandler;

  Map<String, Object?> _sharedContext = {};
  List<String> _agentStack = [];
  String? _lastTurnRawBlock;

  void dispose() => native.dispose();

  void setContext(JsonMap context) => native.setContext(context);

  AuwgentNative get raw => native;

  void registerTool(String name, ToolHandler handler) {
    native.registerTool(name, handler);
  }

  void onIntent(IntentHandler handler) {
    _storedIntentHandler = handler;
  }

  void onIntentPartial(PartialIntentHandler handler) {
    _storedPartialIntentHandler = handler;
  }

  void onWarning(void Function(AuwgentWarning warning) handler) {
    _warningHandler = handler;
  }

  List<AuwgentWarning> getWarnings() => List.unmodifiable(_warnings);

  void clearWarnings() => _warnings.clear();

  String generatePrompt({String? helperName}) {
    return native.generatePrompt(helperName: helperName);
  }

  SessionState exportSession() {
    final raw = native.exportSession();
    return SessionState.fromJson(
      Map<String, Object?>.from(jsonDecode(raw) as Map),
    );
  }

  void importSession(SessionState session) {
    native.importSession(jsonEncode(session.toJson()));
  }

  void clearSession() {
    native.clearSession();
  }

  Future<SessionState> run(Object? input, {List<String>? initialStack}) async {
    _sharedContext = {};
    _lastTurnRawBlock = null;

    try {
      final initialSession = exportSession();
      _runMiddlewareStart(initialSession);
      if (_storedPartialIntentHandler != null) {
        _reportWarning(
          AuwgentWarningSource.onIntentPartial,
          'async FFI currently dispatches final intents only; partial intent handlers are not invoked during run()',
          agentName: ir['name']?.toString(),
        );
      }

      if (input is String) {
        await _awaitRunWithStructuredPolling(
          native.runTextAsync(input, initialStack: initialStack),
        );
      } else {
        await _awaitRunWithStructuredPolling(
          native.runJsonAsync(input, initialStack: initialStack),
        );
      }
      final session = exportSession();
      _agentStack = [...session.stack];
      _runMiddlewareComplete(session);
      return session;
    } catch (error) {
      _reportWarning(
        AuwgentWarningSource.run,
        'native run failed',
        error: error,
      );
      rethrow;
    }
  }

  JsonMap processIntents() {
    return Map<String, Object?>.from(
      jsonDecode(native.processIntents()) as Map,
    );
  }

  void writeChunk(String chunk) => native.writeChunk(chunk);

  JsonMap endStream() {
    return Map<String, Object?>.from(jsonDecode(native.endStream()) as Map);
  }

  String drainJsonl() => native.drainJsonl();

  List<String> drainJsonlLines() {
    final decoded = jsonDecode(native.drainJsonlLines()) as List;
    return decoded.map((line) => line.toString()).toList(growable: false);
  }

  RunMetadata getMetadata() {
    return RunMetadata.fromJson(
      Map<String, Object?>.from(jsonDecode(native.getMetadata()) as Map),
    );
  }

  void clearListeners() => native.clearListeners();

  void _registerDrivers(Map<String, String> apiKeys) {
    final geminiKey = apiKeys['geminiApiKey'];
    if (geminiKey != null && geminiKey.isNotEmpty) {
      native.setGeminiDriver(geminiKey);
    }

    final openaiKey = apiKeys['openaiApiKey'];
    if (openaiKey != null && openaiKey.isNotEmpty) {
      native.setOpenaiDriver(openaiKey);
    }

    final groqKey = apiKeys['groqApiKey'];
    if (groqKey != null && groqKey.isNotEmpty) {
      native.setGroqDriver(groqKey);
    }

    final modelConfig = ir['modelConfig'];
    if (modelConfig is List) {
      for (final entry in modelConfig.whereType<Map>()) {
        _registerCustomDriversFromEntry(
          Map<String, Object?>.from(entry),
          apiKeys,
        );
      }
    }

    final helpers = ir['helpers'];
    if (helpers is List) {
      for (final helper in helpers.whereType<Map>()) {
        final helperMap = Map<String, Object?>.from(helper);
        final helperModelConfig = helperMap['modelConfig'];
        if (helperModelConfig is List) {
          for (final entry in helperModelConfig.whereType<Map>()) {
            _registerCustomDriversFromEntry(
              Map<String, Object?>.from(entry),
              apiKeys,
            );
          }
        }
      }
    }
  }

  void _registerCustomDriversFromEntry(
    JsonMap entry,
    Map<String, String> apiKeys,
  ) {
    void collectFromModelConfig(Object? configValue) {
      if (configValue is! Map) return;
      final configMap = Map<String, Object?>.from(configValue);
      final model = configMap['model'];
      if (model is! Map) return;
      final modelMap = Map<String, Object?>.from(model);
      if (modelMap['type'] != 'custom') return;

      final id = modelMap['id'];
      final url = modelMap['url'];
      if (id is! String || url is! String) return;

      final keyName = '${id.replaceAll('-', '_')}ApiKey';
      final apiKey = apiKeys[keyName];
      if (apiKey == null || apiKey.isEmpty) return;

      native.setCustomDriver(id: id, apiKey: apiKey, baseUrl: url);
    }

    collectFromModelConfig(entry['defaultConfig']);

    final namedConfig = entry['namedConfig'];
    if (namedConfig is List) {
      for (final configValue in namedConfig) {
        collectFromModelConfig(configValue);
      }
    }
  }

  void _registerTools(Map<String, ToolHandler> tools) {
    for (final entry in tools.entries) {
      native.registerTool(entry.key, entry.value);
    }
  }

  Future<void> _awaitRunWithStructuredPolling(Future<void> runFuture) async {
    Object? failure;
    StackTrace? failureStack;
    var done = false;

    unawaited(
      runFuture.then((_) {
        done = true;
      }, onError: (Object error, StackTrace stackTrace) {
        failure = error;
        failureStack = stackTrace;
        done = true;
      }),
    );

    while (!done) {
      _drainStructuredEvents();
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }

    _drainStructuredEvents();

    if (failure != null) {
      Error.throwWithStackTrace(failure!, failureStack!);
    }
  }

  void _drainStructuredEvents() {
    for (final line in drainJsonlLines()) {
      if (line.trim().isEmpty) {
        continue;
      }

      try {
        final event = Map<String, Object?>.from(jsonDecode(line) as Map);
        _dispatchStructuredEvent(event);
      } catch (error) {
        _reportWarning(
          AuwgentWarningSource.onIntent,
          'failed to decode structured event',
          error: error,
        );
      }
    }
  }

  void _dispatchStructuredEvent(JsonMap event) {
    final eventType = event['event']?.toString();
    switch (eventType) {
      case 'intent':
        final name = event['name']?.toString();
        if (name == null || name.isEmpty) {
          return;
        }
        final agentName =
            event['agent']?.toString() ??
            ir['name']?.toString() ??
            'unknown_agent';
        final payload = event['payload'];
        _dispatchIntent(name, payload, agentName);
        return;
      case 'stream_error':
        _reportWarning(
          AuwgentWarningSource.run,
          'structured stream reported an error',
          error: event['error'],
          agentName: event['agent']?.toString(),
        );
        return;
      default:
        return;
    }
  }

  void _dispatchIntent(String name, Object? payload, String agentName) {
    try {
      Object? value = payload;
      if (value is Map) {
        final mapValue = Map<String, Object?>.from(value);
        if (mapValue.containsKey('_raw')) {
          _lastTurnRawBlock = mapValue['_raw']?.toString();
          mapValue.remove('_raw');
        }
        value = mapValue;
      }

      final handler = _storedIntentHandler;
      if (handler != null) {
        final result = handler(name, value, agentName);
        _requireSync(result, 'intent callbacks');
      }

      final ctx = _getBuildContext()..activeAgent = agentName;
      for (final item in _getMiddleware(ctx)) {
        _requireSync(
          item.onIntent(name, value, ctx),
          'middleware onIntent hooks',
        );
      }
      _persistMiddlewareContext(ctx);
    } catch (error) {
      _reportWarning(
        AuwgentWarningSource.onIntent,
        'intent callback failed',
        error: error,
        agentName: agentName,
      );
    }
  }

  void _runMiddlewareStart(SessionState session) {
    try {
      var currentSession = session;
      final ctx = _getBuildContext()..activeAgent = ir['name']?.toString() ?? '';
      for (final item in _getMiddleware(ctx)) {
        currentSession = _requireSync(
          item.onRunStart(currentSession, ctx),
          'middleware onRunStart hooks',
        );
      }
      _persistMiddlewareContext(ctx);
      importSession(currentSession);
      _agentStack = [...currentSession.stack];
    } catch (error) {
      _reportWarning(
        AuwgentWarningSource.run,
        'run start middleware failed',
        error: error,
        agentName: ir['name']?.toString(),
      );
    }
  }

  void _runMiddlewareComplete(SessionState session) {
    try {
      final ctx = _getBuildContext()..activeAgent = ir['name']?.toString() ?? '';
      for (final item in _getMiddleware(ctx)) {
        _requireSync(
          item.onRunComplete(session, ctx),
          'middleware onRunComplete hooks',
        );
      }
      _persistMiddlewareContext(ctx);
    } catch (error) {
      _reportWarning(
        AuwgentWarningSource.run,
        'run complete middleware failed',
        error: error,
        agentName: ir['name']?.toString(),
      );
    }
  }

  MiddlewareContext _getBuildContext() {
    final activeAgent = _agentStack.isNotEmpty
        ? _agentStack.last
        : ir['name']?.toString() ?? 'agent';
    return MiddlewareContext(
      activeAgent: activeAgent,
      stack: [..._agentStack],
      rootAgent: ir['name']?.toString() ?? activeAgent,
      rawBlock: _lastTurnRawBlock,
      systemPrompt: null,
      setContext: native.setContext,
    );
  }

  void _persistMiddlewareContext(MiddlewareContext ctx) {
    _sharedContext = {
      ..._sharedContext,
      'activeAgent': ctx.activeAgent,
      'stack': ctx.stack,
      'rootAgent': ctx.rootAgent,
      'rawBlock': ctx.rawBlock,
      'systemPrompt': ctx.systemPrompt,
    };
  }

  Iterable<Middleware> _getMiddleware(MiddlewareContext ctx) sync* {
    for (final item in middleware) {
      final target = item.target;
      if (target == null) {
        yield item;
        continue;
      }
      if (target is List) {
        if (target.contains(ctx.activeAgent)) {
          yield item;
        }
        continue;
      }
      if (target == ctx.activeAgent) {
        yield item;
      }
    }
  }

  T _requireSync<T>(Object? value, String label) {
    if (value is Future) {
      throw StateError('$label must return synchronously for now.');
    }
    return value as T;
  }

  void _reportWarning(
    AuwgentWarningSource source,
    String message, {
    Object? error,
    String? agentName,
  }) {
    final warning = AuwgentWarning(
      timestamp: DateTime.now().toUtc(),
      source: source,
      message: message,
      detail: error?.toString(),
      agentName: agentName,
    );
    _warnings.add(warning);
    try {
      _warningHandler?.call(warning);
    } catch (_) {}
  }
}

TypedAuwgent<IR> createAuwgent<IR extends JsonMap>(
  IR ir,
  AuwgentConfig config,
) {
  return TypedAuwgent<IR>(ir, config);
}
