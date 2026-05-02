import 'dart:async';
import 'dart:convert';
import 'dart:ffi' as ffi;

import 'package:ffi/ffi.dart';

import 'bindings.dart';
import 'dynamic_library.dart';
import 'types.dart';

typedef _EngineHandle = ffi.Opaque;

final Map<String, AuwgentBindings> _bindingsCache = <String, AuwgentBindings>{};

void _freeCallbackString(ffi.Pointer<Utf8> ptr, ffi.Pointer<ffi.Void> _) {
  malloc.free(ptr);
}

final _freeCallbackStringPtr = ffi.Pointer.fromFunction<NativeFreeString>(
  _freeCallbackString,
);

AuwgentBindings _bindingsForLibrary(String? libraryPath) {
  final cacheKey = libraryPath?.trim().isNotEmpty == true
      ? libraryPath!.trim()
      : '__default__';
  return _bindingsCache.putIfAbsent(
    cacheKey,
    () => AuwgentBindings(openAuwgentLibrary(libraryPath)),
  );
}

final class _RegisteredCallable {
  _RegisteredCallable(this._close);

  final void Function() _close;

  void close() => _close();
}

final class AuwgentNative {
  AuwgentNative._(this._bindings, this._handle);

  factory AuwgentNative.fromIrJson({
    required String irJson,
    String? libraryPath,
  }) {
    final bindings = _bindingsForLibrary(libraryPath);
    final irPtr = irJson.toNativeUtf8();
    try {
      final handle = bindings.engineNew(irPtr);
      if (handle == ffi.nullptr) {
        throw StateError(_readLastError(bindings));
      }
      return AuwgentNative._(bindings, handle);
    } finally {
      malloc.free(irPtr);
    }
  }

  static String generatePromptFromIrJson({
    required String irJson,
    Map<String, Object?>? context,
    String? helperName,
    String? libraryPath,
  }) {
    final bindings = _bindingsForLibrary(libraryPath);
    final irPtr = irJson.toNativeUtf8();
    final contextPtr = context == null
        ? ffi.nullptr
        : jsonEncode(context).toNativeUtf8();
    final helperPtr = helperName?.toNativeUtf8() ?? ffi.nullptr;
    try {
      try {
        final ptr = bindings.generatePromptFromIr(irPtr, contextPtr, helperPtr);
        return _takeRustStringFrom(bindings, ptr);
      } on StateError {
        final native = AuwgentNative.fromIrJson(
          irJson: irJson,
          libraryPath: libraryPath,
        );
        try {
          if (context != null) {
            native.setContext(context);
          }
          return native.generatePrompt(helperName: helperName);
        } finally {
          native.dispose();
        }
      }
    } finally {
      malloc.free(irPtr);
      if (contextPtr != ffi.nullptr) {
        malloc.free(contextPtr);
      }
      if (helperPtr != ffi.nullptr) {
        malloc.free(helperPtr);
      }
    }
  }

  final AuwgentBindings _bindings;
  ffi.Pointer<_EngineHandle> _handle;
  final List<_RegisteredCallable> _callbacks = [];

  bool get isDisposed => _handle == ffi.nullptr;

  void dispose() {
    if (isDisposed) return;
    _releaseCallbacks();
    _bindings.engineFree(_handle);
    _handle = ffi.nullptr;
  }

  void setContext(Map<String, Object?> context) {
    _checkNotDisposed();
    final json = jsonEncode(context);
    final ptr = json.toNativeUtf8();
    try {
      final ok = _bindings.engineSetContext(_handle, ptr);
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(ptr);
    }
  }

  void setGeminiDriver(String apiKey) {
    _checkNotDisposed();
    final apiKeyPtr = apiKey.toNativeUtf8();
    try {
      final ok = _bindings.engineSetGeminiDriver(_handle, apiKeyPtr);
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(apiKeyPtr);
    }
  }

  void setOpenaiDriver(String apiKey, {String? baseUrl}) {
    _checkNotDisposed();
    final apiKeyPtr = apiKey.toNativeUtf8();
    final baseUrlPtr = baseUrl?.toNativeUtf8() ?? ffi.nullptr;
    try {
      final ok = _bindings.engineSetOpenaiDriver(
        _handle,
        apiKeyPtr,
        baseUrlPtr,
      );
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(apiKeyPtr);
      if (baseUrlPtr != ffi.nullptr) {
        malloc.free(baseUrlPtr);
      }
    }
  }

  void setGroqDriver(String apiKey) {
    _checkNotDisposed();
    final apiKeyPtr = apiKey.toNativeUtf8();
    try {
      final ok = _bindings.engineSetGroqDriver(_handle, apiKeyPtr);
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(apiKeyPtr);
    }
  }

  void setCustomDriver({
    required String id,
    required String apiKey,
    required String baseUrl,
  }) {
    _checkNotDisposed();
    final idPtr = id.toNativeUtf8();
    final apiKeyPtr = apiKey.toNativeUtf8();
    final baseUrlPtr = baseUrl.toNativeUtf8();
    try {
      final ok = _bindings.engineSetCustomDriver(
        _handle,
        idPtr,
        apiKeyPtr,
        baseUrlPtr,
      );
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(idPtr);
      malloc.free(apiKeyPtr);
      malloc.free(baseUrlPtr);
    }
  }

  void registerTool(
    String name,
    FutureOr<Object?> Function(Map<String, Object?> args) handler,
  ) {
    _checkNotDisposed();
    final toolNamePtr = name.toNativeUtf8();
    final callable = ffi.NativeCallable<NativeAsyncToolCallback>.listener((
      ffi.Pointer<Utf8> requestIdPtr,
      ffi.Pointer<Utf8> toolNamePtr,
      ffi.Pointer<Utf8> argsJsonPtr,
      ffi.Pointer<ffi.Void> _,
    ) {
      try {
        final requestId = requestIdPtr.toDartString();
        final toolName = toolNamePtr.toDartString();
        final decoded = jsonDecode(argsJsonPtr.toDartString());
        final args = decoded is Map
            ? Map<String, Object?>.from(decoded)
            : <String, Object?>{};
        Future.sync(() => handler(args)).then(
          (result) {
            _completeToolCall(requestId, result);
          },
          onError: (Object error, StackTrace stackTrace) {
            _failToolCall(
              requestId,
              'tool `$toolName` failed: $error\n$stackTrace',
            );
          },
        );
      } catch (error, stackTrace) {
        final requestId = requestIdPtr == ffi.nullptr
            ? ''
            : requestIdPtr.toDartString();
        _failToolCall(requestId, '$error\n$stackTrace');
      } finally {
        if (requestIdPtr != ffi.nullptr) {
          _bindings.stringFree(requestIdPtr, ffi.nullptr);
        }
        if (toolNamePtr != ffi.nullptr) {
          _bindings.stringFree(toolNamePtr, ffi.nullptr);
        }
        if (argsJsonPtr != ffi.nullptr) {
          _bindings.stringFree(argsJsonPtr, ffi.nullptr);
        }
      }
    });

    try {
      final ok = _bindings.engineRegisterToolCallbackAsync(
        _handle,
        toolNamePtr,
        callable.nativeFunction,
        ffi.nullptr,
      );
      if (!ok) {
        callable.close();
        throw StateError(_readLastError(_bindings));
      }
      _callbacks.add(_RegisteredCallable(callable.close));
    } finally {
      malloc.free(toolNamePtr);
    }
  }

  void onMiddlewareEvent(FutureOr<String?> Function(String eventJson) handler) {
    _checkNotDisposed();
    final callable =
        ffi.NativeCallable<NativeMiddlewareEventCallback>.isolateLocal((
          ffi.Pointer<Utf8> eventJsonPtr,
          ffi.Pointer<ffi.Void> _,
        ) {
          try {
            return _encodeSyncString(
              handler(eventJsonPtr.toDartString()),
              'middleware callbacks',
            );
          } catch (_) {
            return ffi.nullptr;
          }
        });

    final ok = _bindings.engineOnMiddlewareEvent(
      _handle,
      callable.nativeFunction,
      _freeCallbackStringPtr,
      ffi.nullptr,
    );
    if (!ok) {
      callable.close();
      throw StateError(_readLastError(_bindings));
    }
    _callbacks.add(_RegisteredCallable(callable.close));
  }

  void onIntent(
    FutureOr<IntentControl?> Function(
      String name,
      Object? value,
      String agentName,
    )
    handler,
  ) {
    _checkNotDisposed();
    final callable = ffi.NativeCallable<NativeIntentCallback>.isolateLocal((
      ffi.Pointer<Utf8> namePtr,
      ffi.Pointer<Utf8> valueJsonPtr,
      ffi.Pointer<Utf8> agentNamePtr,
      ffi.Pointer<ffi.Void> _,
    ) {
      try {
        return _encodeSyncJson(
          handler(
            namePtr.toDartString(),
            jsonDecode(valueJsonPtr.toDartString()),
            agentNamePtr.toDartString(),
          ),
          'intent callbacks',
        );
      } catch (_) {
        return ffi.nullptr;
      }
    });

    final ok = _bindings.engineOnIntent(
      _handle,
      callable.nativeFunction,
      _freeCallbackStringPtr,
      ffi.nullptr,
    );
    if (!ok) {
      callable.close();
      throw StateError(_readLastError(_bindings));
    }
    _callbacks.add(_RegisteredCallable(callable.close));
  }

  void onIntentPartial(
    FutureOr<void> Function(String name, Object? value, String agentName)
    handler,
  ) {
    _checkNotDisposed();
    final callable = ffi.NativeCallable<NativePartialIntentCallback>.listener((
      ffi.Pointer<Utf8> namePtr,
      ffi.Pointer<Utf8> valueJsonPtr,
      ffi.Pointer<Utf8> agentNamePtr,
      ffi.Pointer<ffi.Void> _,
    ) {
      try {
        final result = handler(
          namePtr.toDartString(),
          jsonDecode(valueJsonPtr.toDartString()),
          agentNamePtr.toDartString(),
        );
        if (result is Future) {
          throw StateError(
            'Dart partial intent callbacks must return synchronously for now.',
          );
        }
      } catch (_) {}
    });

    final ok = _bindings.engineOnIntentPartial(
      _handle,
      callable.nativeFunction,
      ffi.nullptr,
    );
    if (!ok) {
      callable.close();
      throw StateError(_readLastError(_bindings));
    }
    _callbacks.add(_RegisteredCallable(callable.close));
  }

  void onSubEngineStart(SessionTransformHandler handler) {
    _registerSessionTransform(
      _bindings.engineOnSubEngineStart,
      handler,
      'sub-engine start',
    );
  }

  void onSubEngineComplete(SessionNotifyHandler handler) {
    _registerSessionNotify(
      _bindings.engineOnSubEngineComplete,
      handler,
      'sub-engine complete',
    );
  }

  String generatePrompt({String? helperName}) {
    _checkNotDisposed();
    final helperPtr = helperName?.toNativeUtf8() ?? ffi.nullptr;
    try {
      final ptr = _bindings.engineGeneratePrompt(_handle, helperPtr);
      return _takeRustString(ptr);
    } finally {
      if (helperPtr != ffi.nullptr) {
        malloc.free(helperPtr);
      }
    }
  }

  String exportSession() {
    _checkNotDisposed();
    return _takeRustString(_bindings.engineExportSession(_handle));
  }

  void importSession(String sessionJson) {
    _checkNotDisposed();
    final ptr = sessionJson.toNativeUtf8();
    try {
      final ok = _bindings.engineImportSession(_handle, ptr);
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(ptr);
    }
  }

  void clearSession() {
    _checkNotDisposed();
    final ok = _bindings.engineClearSession(_handle);
    if (!ok) {
      throw StateError(_readLastError(_bindings));
    }
  }

  void runText(String input, {List<String>? initialStack}) {
    _checkNotDisposed();
    final inputPtr = input.toNativeUtf8();
    final stackPtr = initialStack == null
        ? ffi.nullptr
        : jsonEncode(initialStack).toNativeUtf8();
    try {
      final ok = _bindings.engineRunText(_handle, inputPtr, stackPtr);
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(inputPtr);
      if (stackPtr != ffi.nullptr) {
        malloc.free(stackPtr);
      }
    }
  }

  void runJson(Object? input, {List<String>? initialStack}) {
    _checkNotDisposed();
    final inputPtr = jsonEncode(input).toNativeUtf8();
    final stackPtr = initialStack == null
        ? ffi.nullptr
        : jsonEncode(initialStack).toNativeUtf8();
    try {
      final ok = _bindings.engineRunJson(_handle, inputPtr, stackPtr);
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(inputPtr);
      if (stackPtr != ffi.nullptr) {
        malloc.free(stackPtr);
      }
    }
  }

  Future<void> runTextAsync(String input, {List<String>? initialStack}) {
    _checkNotDisposed();
    final completer = Completer<void>();
    final inputPtr = input.toNativeUtf8();
    final stackPtr = initialStack == null
        ? ffi.nullptr
        : jsonEncode(initialStack).toNativeUtf8();

    late final ffi.NativeCallable<NativeRunCompleteCallback> callable;
    callable = ffi.NativeCallable<NativeRunCompleteCallback>.listener((
      bool success,
      ffi.Pointer<Utf8> errorMessagePtr,
      ffi.Pointer<ffi.Void> _,
    ) {
      try {
        callable.close();
        if (success) {
          completer.complete();
        } else {
          final message = errorMessagePtr == ffi.nullptr
              ? 'Unknown async run error'
              : errorMessagePtr.toDartString();
          completer.completeError(StateError(message));
        }
      } finally {
        if (errorMessagePtr != ffi.nullptr) {
          _bindings.stringFree(errorMessagePtr, ffi.nullptr);
        }
      }
    });

    try {
      final ok = _bindings.engineRunTextAsync(
        _handle,
        inputPtr,
        stackPtr,
        callable.nativeFunction,
        ffi.nullptr,
      );
      if (!ok) {
        callable.close();
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(inputPtr);
      if (stackPtr != ffi.nullptr) {
        malloc.free(stackPtr);
      }
    }

    return completer.future;
  }

  Future<void> runJsonAsync(Object? input, {List<String>? initialStack}) {
    _checkNotDisposed();
    final completer = Completer<void>();
    final inputPtr = jsonEncode(input).toNativeUtf8();
    final stackPtr = initialStack == null
        ? ffi.nullptr
        : jsonEncode(initialStack).toNativeUtf8();

    late final ffi.NativeCallable<NativeRunCompleteCallback> callable;
    callable = ffi.NativeCallable<NativeRunCompleteCallback>.listener((
      bool success,
      ffi.Pointer<Utf8> errorMessagePtr,
      ffi.Pointer<ffi.Void> _,
    ) {
      try {
        callable.close();
        if (success) {
          completer.complete();
        } else {
          final message = errorMessagePtr == ffi.nullptr
              ? 'Unknown async run error'
              : errorMessagePtr.toDartString();
          completer.completeError(StateError(message));
        }
      } finally {
        if (errorMessagePtr != ffi.nullptr) {
          _bindings.stringFree(errorMessagePtr, ffi.nullptr);
        }
      }
    });

    try {
      final ok = _bindings.engineRunJsonAsync(
        _handle,
        inputPtr,
        stackPtr,
        callable.nativeFunction,
        ffi.nullptr,
      );
      if (!ok) {
        callable.close();
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(inputPtr);
      if (stackPtr != ffi.nullptr) {
        malloc.free(stackPtr);
      }
    }

    return completer.future;
  }

  String processIntents() {
    _checkNotDisposed();
    return _takeRustString(_bindings.engineProcessIntents(_handle));
  }

  void writeChunk(String chunk) {
    _checkNotDisposed();
    final ptr = chunk.toNativeUtf8();
    try {
      final ok = _bindings.engineWriteChunk(_handle, ptr);
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(ptr);
    }
  }

  String endStream() {
    _checkNotDisposed();
    return _takeRustString(_bindings.engineEndStream(_handle));
  }

  String drainJsonl() {
    _checkNotDisposed();
    return _takeRustString(_bindings.engineDrainJsonl(_handle));
  }

  String drainJsonlLines() {
    _checkNotDisposed();
    return _takeRustString(_bindings.engineDrainJsonlLines(_handle));
  }

  String getMetadata() {
    _checkNotDisposed();
    return _takeRustString(_bindings.engineGetMetadata(_handle));
  }

  void clearListeners() {
    _checkNotDisposed();
    final ok = _bindings.engineClearListeners(_handle);
    if (!ok) {
      throw StateError(_readLastError(_bindings));
    }
    _releaseCallbacks();
  }

  void _registerSessionTransform(
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeSessionTransformCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      ffi.Pointer<ffi.Void>,
    )
    register,
    SessionTransformHandler handler,
    String label,
  ) {
    _checkNotDisposed();
    final callable =
        ffi.NativeCallable<NativeSessionTransformCallback>.isolateLocal((
          ffi.Pointer<Utf8> primaryNamePtr,
          ffi.Pointer<Utf8> sessionJsonPtr,
          ffi.Pointer<ffi.Void> _,
        ) {
          try {
            return _encodeSyncString(
              handler(
                primaryNamePtr.toDartString(),
                sessionJsonPtr.toDartString(),
              ),
              '$label callbacks',
            );
          } catch (_) {
            return ffi.nullptr;
          }
        });

    final ok = register(
      _handle,
      callable.nativeFunction,
      _freeCallbackStringPtr,
      ffi.nullptr,
    );
    if (!ok) {
      callable.close();
      throw StateError(_readLastError(_bindings));
    }
    _callbacks.add(_RegisteredCallable(callable.close));
  }

  void _registerSessionNotify(
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeSessionNotifyCallback>>,
      ffi.Pointer<ffi.Void>,
    )
    register,
    SessionNotifyHandler handler,
    String label,
  ) {
    _checkNotDisposed();
    final callable =
        ffi.NativeCallable<NativeSessionNotifyCallback>.isolateLocal((
          ffi.Pointer<Utf8> primaryNamePtr,
          ffi.Pointer<Utf8> sessionJsonPtr,
          ffi.Pointer<ffi.Void> _,
        ) {
          try {
            final result = handler(
              primaryNamePtr.toDartString(),
              sessionJsonPtr.toDartString(),
            );
            if (result is Future) {
              throw StateError(
                'Dart $label callbacks must return synchronously for now.',
              );
            }
          } catch (_) {}
        });

    final ok = register(_handle, callable.nativeFunction, ffi.nullptr);
    if (!ok) {
      callable.close();
      throw StateError(_readLastError(_bindings));
    }
    _callbacks.add(_RegisteredCallable(callable.close));
  }

  void _releaseCallbacks() {
    for (final callback in _callbacks) {
      callback.close();
    }
    _callbacks.clear();
  }

  void _checkNotDisposed() {
    if (isDisposed) {
      throw StateError('AuwgentNative has been disposed');
    }
  }

  String _takeRustString(ffi.Pointer<Utf8> ptr) {
    return _takeRustStringFrom(_bindings, ptr);
  }

  static String _takeRustStringFrom(
    AuwgentBindings bindings,
    ffi.Pointer<Utf8> ptr,
  ) {
    if (ptr == ffi.nullptr) {
      throw StateError(_readLastError(bindings));
    }
    try {
      return ptr.toDartString();
    } finally {
      bindings.stringFree(ptr, ffi.nullptr);
    }
  }

  static ffi.Pointer<Utf8> _encodeSyncJson(
    FutureOr<Object?> value,
    String label,
  ) {
    if (value is Future) {
      throw StateError('Dart $label must return synchronously for now.');
    }
    if (value == null) {
      return ffi.nullptr;
    }
    if (value is IntentControl) {
      return jsonEncode(value.toJson()).toNativeUtf8(allocator: malloc);
    }
    return jsonEncode(value).toNativeUtf8(allocator: malloc);
  }

  static ffi.Pointer<Utf8> _encodeSyncString(
    FutureOr<String?> value,
    String label,
  ) {
    if (value is Future) {
      throw StateError('Dart $label must return synchronously for now.');
    }
    if (value == null) {
      return ffi.nullptr;
    }
    return value.toNativeUtf8(allocator: malloc);
  }

  static String _readLastError(AuwgentBindings bindings) {
    final ptr = bindings.lastErrorMessage();
    if (ptr == ffi.nullptr) {
      return 'Unknown Auwgent native error';
    }
    try {
      return ptr.toDartString();
    } finally {
      bindings.stringFree(ptr, ffi.nullptr);
    }
  }

  void _completeToolCall(String requestId, Object? result) {
    if (isDisposed) return;
    final requestIdPtr = requestId.toNativeUtf8();
    final resultJsonPtr = jsonEncode(result).toNativeUtf8();
    try {
      final ok = _bindings.engineCompleteToolCall(
        _handle,
        requestIdPtr,
        resultJsonPtr,
      );
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(requestIdPtr);
      malloc.free(resultJsonPtr);
    }
  }

  void _failToolCall(String requestId, String message) {
    if (isDisposed) return;
    final requestIdPtr = requestId.toNativeUtf8();
    final messagePtr = message.toNativeUtf8();
    try {
      final ok = _bindings.engineFailToolCall(
        _handle,
        requestIdPtr,
        messagePtr,
      );
      if (!ok) {
        throw StateError(_readLastError(_bindings));
      }
    } finally {
      malloc.free(requestIdPtr);
      malloc.free(messagePtr);
    }
  }
}
