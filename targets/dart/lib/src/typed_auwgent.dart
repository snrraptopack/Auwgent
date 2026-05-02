import 'dart:async';
import 'dart:convert';

import 'auwgent_native.dart';
import 'middleware.dart';
import 'types.dart';

class TypedAuwgent<IR extends JsonMap> {
  TypedAuwgent(this.ir, this.config)
    : _irJson = jsonEncode(ir),
      _context = config.context == null
          ? null
          : Map<String, Object?>.from(config.context!),
      middleware = List<Middleware>.from(
        config.middleware.whereType<Middleware>(),
      );

  final IR ir;
  final AuwgentConfig config;
  final List<Middleware> middleware;
  final String _irJson;

  IntentHandler? _storedIntentHandler;
  PartialIntentHandler? _storedPartialIntentHandler;
  final List<AuwgentWarning> _warnings = [];
  void Function(AuwgentWarning warning)? _warningHandler;

  AuwgentNative? _native;
  Map<String, Object?>? _context;
  int _contextRevision = 0;
  final Map<String, String> _promptCache = {};
  Map<String, Object?> _sharedContext = {};
  List<String> _agentStack = [];
  String? _lastTurnRawBlock;

  void dispose() {
    _native?.dispose();
    _native = null;
  }

  void setContext(JsonMap context) {
    _context = Map<String, Object?>.from(context);
    _contextRevision++;
    _promptCache.clear();
    _native?.setContext(_context!);
  }

  AuwgentNative get native => _ensureNative();

  AuwgentNative get raw => _ensureNative();

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
    final cacheKey = '${helperName ?? ''}#$_contextRevision';
    final cached = _promptCache[cacheKey];
    if (cached != null) {
      return cached;
    }

    final prompt = AuwgentNative.generatePromptFromIrJson(
      irJson: _irJson,
      context: _context,
      helperName: helperName,
      libraryPath: config.libraryPath,
    );
    _promptCache[cacheKey] = prompt;
    return prompt;
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
      final apiKey =
          apiKeys[keyName] ??
          _builtinApiKeyForCustomEndpoint(id: id, url: url, apiKeys: apiKeys);
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

  String? _builtinApiKeyForCustomEndpoint({
    required String id,
    required String url,
    required Map<String, String> apiKeys,
  }) {
    final normalizedId = id
        .replaceAll(RegExp(r'[^A-Za-z0-9]'), '')
        .toLowerCase();
    final normalizedUrl = url.toLowerCase();

    if (normalizedId == 'groq' ||
        normalizedId == 'groqapi' ||
        normalizedUrl.contains('api.groq.com')) {
      return apiKeys['groqApiKey'];
    }

    if (normalizedId == 'openai' ||
        normalizedId == 'openaiapi' ||
        normalizedUrl.contains('api.openai.com')) {
      return apiKeys['openaiApiKey'];
    }

    if (normalizedId == 'gemini' ||
        normalizedId == 'geminiapi' ||
        normalizedUrl.contains('generativelanguage.googleapis.com')) {
      return apiKeys['geminiApiKey'];
    }

    return null;
  }

  void _registerTools(Map<String, ToolHandler> tools) {
    for (final entry in tools.entries) {
      native.registerTool(entry.key, entry.value);
    }
  }

  AuwgentNative _ensureNative() {
    final existing = _native;
    if (existing != null && !existing.isDisposed) {
      return existing;
    }

    final created = AuwgentNative.fromIrJson(
      irJson: _irJson,
      libraryPath: config.libraryPath,
    );
    _native = created;

    if (_context != null) {
      created.setContext(_context!);
    }
    _registerDrivers(config.apiKeys);
    _registerTools(config.tools);

    return created;
  }

  Future<void> _awaitRunWithStructuredPolling(Future<void> runFuture) async {
    Object? failure;
    StackTrace? failureStack;
    var done = false;

    unawaited(
      runFuture.then(
        (_) {
          done = true;
        },
        onError: (Object error, StackTrace stackTrace) {
          failure = error;
          failureStack = stackTrace;
          done = true;
        },
      ),
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
      case 'partial_intent':
        final name = event['name']?.toString();
        if (name == null || name.isEmpty) {
          return;
        }
        final agentName =
            event['agent']?.toString() ??
            ir['name']?.toString() ??
            'unknown_agent';
        final payload = event['payload'];
        _dispatchPartialIntent(name, payload, agentName);
        return;
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

  void _dispatchPartialIntent(String name, Object? payload, String agentName) {
    try {
      Object? value = payload;
      if (value is Map) {
        value = Map<String, Object?>.from(value);
      }

      final handler = _storedPartialIntentHandler;
      if (handler != null) {
        _requireSync(
          handler(name, value, agentName),
          'partial intent callbacks',
        );
      }

      final ctx = _getBuildContext()..activeAgent = agentName;
      for (final item in _getMiddleware(ctx)) {
        _requireSync(
          item.onIntentPartial(name, value, ctx),
          'middleware onIntentPartial hooks',
        );
      }
      _persistMiddlewareContext(ctx);
    } catch (error) {
      _reportWarning(
        AuwgentWarningSource.onIntentPartial,
        'partial intent callback failed',
        error: error,
        agentName: agentName,
      );
    }
  }

  void _runMiddlewareStart(SessionState session) {
    try {
      var currentSession = session;
      final ctx = _getBuildContext()
        ..activeAgent = ir['name']?.toString() ?? '';
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
      final ctx = _getBuildContext()
        ..activeAgent = ir['name']?.toString() ?? '';
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
