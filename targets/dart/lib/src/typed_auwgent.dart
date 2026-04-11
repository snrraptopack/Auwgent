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
  final Map<String, SessionState> _helperSessions = {};
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
    _helperSessions.clear();
  }

  Future<SessionState> run(Object? input, {List<String>? initialStack}) async {
    _sharedContext = {};
    _lastTurnRawBlock = null;
    _activateListeners();

    try {
      if (input is String) {
        await native.runTextAsync(input, initialStack: initialStack);
      } else {
        await native.runJsonAsync(input, initialStack: initialStack);
      }
      final session = exportSession();
      _agentStack = [...session.stack];
      return session;
    } catch (error) {
      _reportWarning(
        AuwgentWarningSource.run,
        'native run failed',
        error: error,
      );
      rethrow;
    } finally {
      native.clearListeners();
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

  JsonMap getMetadata() {
    return Map<String, Object?>.from(jsonDecode(native.getMetadata()) as Map);
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

  void _activateListeners() {
    native.onIntent((name, value, agentName) {
      try {
        if (value is Map && value.containsKey('_raw')) {
          _lastTurnRawBlock = value['_raw']?.toString();
          value.remove('_raw');
        }

        final handler = _storedIntentHandler;
        if (handler == null) {
          return null;
        }
        final result = handler(name, value, agentName);
        return _requireSync(result, 'intent callbacks');
      } catch (error) {
        _reportWarning(
          AuwgentWarningSource.onIntent,
          'intent callback failed',
          error: error,
          agentName: agentName,
        );
        return null;
      }
    });

    native.onIntentPartial((name, value, agentName) {
      try {
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
    });

    native.onMiddlewareEvent((eventJson) => _handleMiddlewareEvent(eventJson));

    native.onSubEngineStart((helperName, emptySessionJson) {
      try {
        final session =
            _helperSessions[helperName] ??
            SessionState.fromJson(
              Map<String, Object?>.from(jsonDecode(emptySessionJson) as Map),
            );
        _agentStack = [...session.stack];
        return jsonEncode(session.toJson());
      } catch (error) {
        _reportWarning(
          AuwgentWarningSource.onSubEngineStart,
          'sub-engine start callback failed',
          error: error,
          agentName: helperName,
        );
        return emptySessionJson;
      }
    });

    native.onSubEngineComplete((helperName, completedSessionJson) {
      try {
        final session = SessionState.fromJson(
          Map<String, Object?>.from(jsonDecode(completedSessionJson) as Map),
        );
        _helperSessions[helperName] = session;
        _agentStack = [...session.stack];
      } catch (error) {
        _reportWarning(
          AuwgentWarningSource.onSubEngineComplete,
          'sub-engine complete callback failed',
          error: error,
          agentName: helperName,
        );
      }
    });
  }

  String? _handleMiddlewareEvent(String eventJson) {
    try {
      final event = Map<String, Object?>.from(jsonDecode(eventJson) as Map);
      final ctx = _buildContextFromRuntimeEvent(event);

      switch (event['type']) {
        case 'intent':
          final value = event['value'];
          for (final item in _getMiddleware(ctx)) {
            final control = _requireSync(
              item.onIntent(event['name'].toString(), value, ctx),
              'middleware onIntent hooks',
            );
            if (control != null) {
              _persistMiddlewareContext(ctx);
              return jsonEncode(control);
            }
          }
          _persistMiddlewareContext(ctx);
          return null;
        case 'llm_start':
          var currentPrompt = event['prompt']?.toString() ?? '';
          for (final item in _getMiddleware(ctx)) {
            final modified = _requireSync(
              item.onLLMStart(currentPrompt, ctx),
              'middleware onLLMStart hooks',
            );
            if (modified is String) {
              currentPrompt = modified;
            }
          }
          _persistMiddlewareContext(ctx);
          return jsonEncode({'prompt': currentPrompt, 'stack': ctx.stack});
        case 'llm_end':
          for (final item in _getMiddleware(ctx)) {
            _requireSync(
              item.onLLMEnd(event['response'], ctx),
              'middleware onLLMEnd hooks',
            );
          }
          _persistMiddlewareContext(ctx);
          return null;
        case 'run_start':
          var session = SessionState.fromJson(
            Map<String, Object?>.from(event['session'] as Map),
          );
          for (final item in _getMiddleware(ctx)) {
            session = _requireSync(
              item.onRunStart(session, ctx),
              'middleware onRunStart hooks',
            );
          }
          _agentStack = [...session.stack];
          _persistMiddlewareContext(ctx);
          return jsonEncode({'session': session.toJson()});
        case 'run_complete':
          final session = SessionState.fromJson(
            Map<String, Object?>.from(event['session'] as Map),
          );
          for (final item in _getMiddleware(ctx)) {
            _requireSync(
              item.onRunComplete(session, ctx),
              'middleware onRunComplete hooks',
            );
          }
          _persistMiddlewareContext(ctx);
          return null;
        case 'error':
          final sessionRaw = event['session'];
          final session = sessionRaw is Map
              ? SessionState.fromJson(Map<String, Object?>.from(sessionRaw))
              : null;
          final error = event['error'] ?? {'message': 'Unknown runtime error'};
          for (final item in _getMiddleware(ctx)) {
            final swallow = _requireSync(
              item.onError(error, session, ctx),
              'middleware onError hooks',
            );
            if (swallow == true) {
              _persistMiddlewareContext(ctx);
              return jsonEncode({'swallow': true});
            }
          }
          _persistMiddlewareContext(ctx);
          return null;
        default:
          return null;
      }
    } catch (error) {
      _reportWarning(
        AuwgentWarningSource.onMiddlewareEvent,
        'failed to handle middleware event',
        error: error,
      );
      return null;
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

  MiddlewareContext _buildContextFromRuntimeEvent(JsonMap event) {
    final ctx = _getBuildContext();
    final runtimeCtx = event['context'];
    if (runtimeCtx is Map) {
      final map = Map<String, Object?>.from(runtimeCtx);
      if (map['activeAgent'] is String) {
        ctx.activeAgent = map['activeAgent'] as String;
      }
      if (map['stack'] is List) {
        ctx.stack = (map['stack'] as List)
            .map((item) => item.toString())
            .toList(growable: false);
        _agentStack = [...ctx.stack];
      }
      if (map['rootAgent'] is String) {
        ctx.rootAgent = map['rootAgent'] as String;
      }
      if (map['rawBlock'] is String) {
        ctx.rawBlock = map['rawBlock'] as String;
        _lastTurnRawBlock = ctx.rawBlock;
      }
      if (map['systemPrompt'] is String) {
        ctx.systemPrompt = map['systemPrompt'] as String;
      }
    }
    return ctx;
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
