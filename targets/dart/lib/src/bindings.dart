import 'dart:ffi' as ffi;

import 'package:ffi/ffi.dart';

typedef _EngineHandle = ffi.Opaque;
typedef NativeVoidPtr = ffi.Pointer<ffi.Void>;

typedef NativeFreeString = ffi.Void Function(ffi.Pointer<Utf8>, NativeVoidPtr);
typedef DartFreeString = void Function(ffi.Pointer<Utf8>, NativeVoidPtr);

typedef NativeLastErrorMessage = ffi.Pointer<Utf8> Function();
typedef DartLastErrorMessage = ffi.Pointer<Utf8> Function();

typedef NativeEngineNew =
    ffi.Pointer<_EngineHandle> Function(ffi.Pointer<Utf8>);
typedef DartEngineNew = ffi.Pointer<_EngineHandle> Function(ffi.Pointer<Utf8>);

typedef NativeGeneratePromptFromIr =
    ffi.Pointer<Utf8> Function(
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );
typedef DartGeneratePromptFromIr =
    ffi.Pointer<Utf8> Function(
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );

typedef NativeEngineFree = ffi.Void Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineFree = void Function(ffi.Pointer<_EngineHandle>);

typedef NativeEngineSetContext =
    ffi.Bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);
typedef DartEngineSetContext =
    bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);

typedef NativeEngineSetGeminiDriver =
    ffi.Bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);
typedef DartEngineSetGeminiDriver =
    bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);

typedef NativeEngineSetGroqDriver =
    ffi.Bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);
typedef DartEngineSetGroqDriver =
    bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);

typedef NativeEngineSetOpenaiDriver =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );
typedef DartEngineSetOpenaiDriver =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );

typedef NativeEngineSetCustomDriver =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );
typedef DartEngineSetCustomDriver =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );

typedef NativeEngineGeneratePrompt =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);
typedef DartEngineGeneratePrompt =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);

typedef NativeEngineExportSession =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineExportSession =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);

typedef NativeEngineImportSession =
    ffi.Bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);
typedef DartEngineImportSession =
    bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);

typedef NativeEngineClearSession =
    ffi.Bool Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineClearSession = bool Function(ffi.Pointer<_EngineHandle>);

typedef NativeEngineRunText =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );
typedef DartEngineRunText =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );

typedef NativeEngineRunJson =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );
typedef DartEngineRunJson =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );

typedef NativeRunCompleteCallback =
    ffi.Void Function(ffi.Bool, ffi.Pointer<Utf8>, NativeVoidPtr);

typedef NativeEngineRunTextAsync =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.NativeFunction<NativeRunCompleteCallback>>,
      NativeVoidPtr,
    );
typedef DartEngineRunTextAsync =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.NativeFunction<NativeRunCompleteCallback>>,
      NativeVoidPtr,
    );

typedef NativeEngineRunJsonAsync =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.NativeFunction<NativeRunCompleteCallback>>,
      NativeVoidPtr,
    );
typedef DartEngineRunJsonAsync =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.NativeFunction<NativeRunCompleteCallback>>,
      NativeVoidPtr,
    );

typedef NativeEngineProcessIntents =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineProcessIntents =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);

typedef NativeEngineWriteChunk =
    ffi.Bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);
typedef DartEngineWriteChunk =
    bool Function(ffi.Pointer<_EngineHandle>, ffi.Pointer<Utf8>);

typedef NativeEngineEndStream =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineEndStream =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);

typedef NativeEngineDrainJsonl =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineDrainJsonl =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);

typedef NativeEngineDrainJsonlLines =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineDrainJsonlLines =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);

typedef NativeEngineGetMetadata =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineGetMetadata =
    ffi.Pointer<Utf8> Function(ffi.Pointer<_EngineHandle>);

typedef NativeEngineClearListeners =
    ffi.Bool Function(ffi.Pointer<_EngineHandle>);
typedef DartEngineClearListeners = bool Function(ffi.Pointer<_EngineHandle>);

typedef NativeToolCallback =
    ffi.Pointer<Utf8> Function(
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      NativeVoidPtr,
    );
typedef NativeAsyncToolCallback =
    ffi.Void Function(
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      NativeVoidPtr,
    );

typedef NativeMiddlewareEventCallback =
    ffi.Pointer<Utf8> Function(ffi.Pointer<Utf8>, NativeVoidPtr);

typedef NativeIntentCallback =
    ffi.Pointer<Utf8> Function(
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      NativeVoidPtr,
    );

typedef NativePartialIntentCallback =
    ffi.Void Function(
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      NativeVoidPtr,
    );

typedef NativeSessionTransformCallback =
    ffi.Pointer<Utf8> Function(
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
      NativeVoidPtr,
    );

typedef NativeSessionNotifyCallback =
    ffi.Void Function(ffi.Pointer<Utf8>, ffi.Pointer<Utf8>, NativeVoidPtr);

typedef NativeEngineRegisterToolCallback =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.NativeFunction<NativeToolCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      NativeVoidPtr,
    );
typedef DartEngineRegisterToolCallback =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.NativeFunction<NativeToolCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      NativeVoidPtr,
    );

typedef NativeEngineRegisterToolCallbackAsync =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.NativeFunction<NativeAsyncToolCallback>>,
      NativeVoidPtr,
    );
typedef DartEngineRegisterToolCallbackAsync =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<ffi.NativeFunction<NativeAsyncToolCallback>>,
      NativeVoidPtr,
    );

typedef NativeEngineCompleteToolCall =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );
typedef DartEngineCompleteToolCall =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );

typedef NativeEngineFailToolCall =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );
typedef DartEngineFailToolCall =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<Utf8>,
      ffi.Pointer<Utf8>,
    );

typedef NativeEngineOnMiddlewareEvent =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeMiddlewareEventCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      NativeVoidPtr,
    );
typedef DartEngineOnMiddlewareEvent =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeMiddlewareEventCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      NativeVoidPtr,
    );

typedef NativeEngineOnIntent =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeIntentCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      NativeVoidPtr,
    );
typedef DartEngineOnIntent =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeIntentCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      NativeVoidPtr,
    );

typedef NativeEngineOnIntentPartial =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativePartialIntentCallback>>,
      NativeVoidPtr,
    );
typedef DartEngineOnIntentPartial =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativePartialIntentCallback>>,
      NativeVoidPtr,
    );

typedef NativeEngineOnSubEngineStart =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeSessionTransformCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      NativeVoidPtr,
    );
typedef DartEngineOnSubEngineStart =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeSessionTransformCallback>>,
      ffi.Pointer<ffi.NativeFunction<NativeFreeString>>,
      NativeVoidPtr,
    );

typedef NativeEngineOnSubEngineComplete =
    ffi.Bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeSessionNotifyCallback>>,
      NativeVoidPtr,
    );
typedef DartEngineOnSubEngineComplete =
    bool Function(
      ffi.Pointer<_EngineHandle>,
      ffi.Pointer<ffi.NativeFunction<NativeSessionNotifyCallback>>,
      NativeVoidPtr,
    );

final class AuwgentBindings {
  AuwgentBindings(this.library)
    : stringFree = library.lookupFunction<NativeFreeString, DartFreeString>(
        'auwgent_string_free',
      ),
      lastErrorMessage = library
          .lookupFunction<NativeLastErrorMessage, DartLastErrorMessage>(
            'auwgent_last_error_message',
          ),
      engineNew = library.lookupFunction<NativeEngineNew, DartEngineNew>(
        'auwgent_engine_new',
      ),
      generatePromptFromIr = _lookupGeneratePromptFromIr(library),
      engineFree = library.lookupFunction<NativeEngineFree, DartEngineFree>(
        'auwgent_engine_free',
      ),
      engineSetContext = library
          .lookupFunction<NativeEngineSetContext, DartEngineSetContext>(
            'auwgent_engine_set_context',
          ),
      engineSetGeminiDriver = library
          .lookupFunction<
            NativeEngineSetGeminiDriver,
            DartEngineSetGeminiDriver
          >('auwgent_engine_set_gemini_driver'),
      engineSetGroqDriver = library
          .lookupFunction<NativeEngineSetGroqDriver, DartEngineSetGroqDriver>(
            'auwgent_engine_set_groq_driver',
          ),
      engineSetOpenaiDriver = library
          .lookupFunction<
            NativeEngineSetOpenaiDriver,
            DartEngineSetOpenaiDriver
          >('auwgent_engine_set_openai_driver'),
      engineSetCustomDriver = library
          .lookupFunction<
            NativeEngineSetCustomDriver,
            DartEngineSetCustomDriver
          >('auwgent_engine_set_custom_driver'),
      engineGeneratePrompt = library
          .lookupFunction<NativeEngineGeneratePrompt, DartEngineGeneratePrompt>(
            'auwgent_engine_generate_prompt',
          ),
      engineExportSession = library
          .lookupFunction<NativeEngineExportSession, DartEngineExportSession>(
            'auwgent_engine_export_session',
          ),
      engineImportSession = library
          .lookupFunction<NativeEngineImportSession, DartEngineImportSession>(
            'auwgent_engine_import_session',
          ),
      engineClearSession = library
          .lookupFunction<NativeEngineClearSession, DartEngineClearSession>(
            'auwgent_engine_clear_session',
          ),
      engineRunText = library
          .lookupFunction<NativeEngineRunText, DartEngineRunText>(
            'auwgent_engine_run_text',
          ),
      engineRunJson = library
          .lookupFunction<NativeEngineRunJson, DartEngineRunJson>(
            'auwgent_engine_run_json',
          ),
      engineRunTextAsync = library
          .lookupFunction<NativeEngineRunTextAsync, DartEngineRunTextAsync>(
            'auwgent_engine_run_text_async',
          ),
      engineRunJsonAsync = library
          .lookupFunction<NativeEngineRunJsonAsync, DartEngineRunJsonAsync>(
            'auwgent_engine_run_json_async',
          ),
      engineProcessIntents = library
          .lookupFunction<NativeEngineProcessIntents, DartEngineProcessIntents>(
            'auwgent_engine_process_intents',
          ),
      engineWriteChunk = library
          .lookupFunction<NativeEngineWriteChunk, DartEngineWriteChunk>(
            'auwgent_engine_write_chunk',
          ),
      engineEndStream = library
          .lookupFunction<NativeEngineEndStream, DartEngineEndStream>(
            'auwgent_engine_end_stream',
          ),
      engineDrainJsonl = library
          .lookupFunction<NativeEngineDrainJsonl, DartEngineDrainJsonl>(
            'auwgent_engine_drain_jsonl',
          ),
      engineDrainJsonlLines = library
          .lookupFunction<
            NativeEngineDrainJsonlLines,
            DartEngineDrainJsonlLines
          >('auwgent_engine_drain_jsonl_lines'),
      engineGetMetadata = library
          .lookupFunction<NativeEngineGetMetadata, DartEngineGetMetadata>(
            'auwgent_engine_get_metadata',
          ),
      engineClearListeners = library
          .lookupFunction<NativeEngineClearListeners, DartEngineClearListeners>(
            'auwgent_engine_clear_listeners',
          ),
      engineRegisterToolCallback = library
          .lookupFunction<
            NativeEngineRegisterToolCallback,
            DartEngineRegisterToolCallback
          >('auwgent_engine_register_tool_callback'),
      engineRegisterToolCallbackAsync = library
          .lookupFunction<
            NativeEngineRegisterToolCallbackAsync,
            DartEngineRegisterToolCallbackAsync
          >('auwgent_engine_register_tool_callback_async'),
      engineCompleteToolCall = library
          .lookupFunction<
            NativeEngineCompleteToolCall,
            DartEngineCompleteToolCall
          >('auwgent_engine_complete_tool_call'),
      engineFailToolCall = library
          .lookupFunction<NativeEngineFailToolCall, DartEngineFailToolCall>(
            'auwgent_engine_fail_tool_call',
          ),
      engineOnMiddlewareEvent = _lookupEngineOnMiddlewareEvent(library),
      engineOnIntent = _lookupEngineOnIntent(library),
      engineOnIntentPartial = _lookupEngineOnIntentPartial(library),
      engineOnSubEngineStart = _lookupEngineOnSubEngineStart(library),
      engineOnSubEngineComplete = _lookupEngineOnSubEngineComplete(library);

  final ffi.DynamicLibrary library;

  final DartFreeString stringFree;
  final DartLastErrorMessage lastErrorMessage;
  final DartEngineNew engineNew;
  final DartGeneratePromptFromIr generatePromptFromIr;
  final DartEngineFree engineFree;
  final DartEngineSetContext engineSetContext;
  final DartEngineSetGeminiDriver engineSetGeminiDriver;
  final DartEngineSetOpenaiDriver engineSetOpenaiDriver;
  final DartEngineSetGroqDriver engineSetGroqDriver;
  final DartEngineSetCustomDriver engineSetCustomDriver;
  final DartEngineGeneratePrompt engineGeneratePrompt;
  final DartEngineExportSession engineExportSession;
  final DartEngineImportSession engineImportSession;
  final DartEngineClearSession engineClearSession;
  final DartEngineRunText engineRunText;
  final DartEngineRunJson engineRunJson;
  final DartEngineRunTextAsync engineRunTextAsync;
  final DartEngineRunJsonAsync engineRunJsonAsync;
  final DartEngineProcessIntents engineProcessIntents;
  final DartEngineWriteChunk engineWriteChunk;
  final DartEngineEndStream engineEndStream;
  final DartEngineDrainJsonl engineDrainJsonl;
  final DartEngineDrainJsonlLines engineDrainJsonlLines;
  final DartEngineGetMetadata engineGetMetadata;
  final DartEngineClearListeners engineClearListeners;
  final DartEngineRegisterToolCallback engineRegisterToolCallback;
  final DartEngineRegisterToolCallbackAsync engineRegisterToolCallbackAsync;
  final DartEngineCompleteToolCall engineCompleteToolCall;
  final DartEngineFailToolCall engineFailToolCall;
  final DartEngineOnMiddlewareEvent engineOnMiddlewareEvent;
  final DartEngineOnIntent engineOnIntent;
  final DartEngineOnIntentPartial engineOnIntentPartial;
  final DartEngineOnSubEngineStart engineOnSubEngineStart;
  final DartEngineOnSubEngineComplete engineOnSubEngineComplete;
}

Never _missingSymbol(String symbol, Object error) {
  throw StateError(
    "Auwgent native library is missing required symbol '$symbol'. "
    'Rebuild/copy the current C ABI library so the Dart SDK and native DLL match. '
    'Original lookup error: $error',
  );
}

DartGeneratePromptFromIr _lookupGeneratePromptFromIr(
  ffi.DynamicLibrary library,
) {
  const symbol = 'auwgent_generate_prompt_from_ir';
  try {
    return library
        .lookupFunction<NativeGeneratePromptFromIr, DartGeneratePromptFromIr>(
          symbol,
        );
  } catch (error) {
    return (irJson, contextJson, helperName) => _missingSymbol(symbol, error);
  }
}

DartEngineOnMiddlewareEvent _lookupEngineOnMiddlewareEvent(
  ffi.DynamicLibrary library,
) {
  const symbol = 'auwgent_engine_on_middleware_event';
  try {
    return library.lookupFunction<
      NativeEngineOnMiddlewareEvent,
      DartEngineOnMiddlewareEvent
    >(symbol);
  } catch (error) {
    return (handle, callback, freeResult, userData) =>
        _missingSymbol(symbol, error);
  }
}

DartEngineOnIntent _lookupEngineOnIntent(ffi.DynamicLibrary library) {
  const symbol = 'auwgent_engine_on_intent';
  try {
    return library.lookupFunction<NativeEngineOnIntent, DartEngineOnIntent>(
      symbol,
    );
  } catch (error) {
    return (handle, callback, freeResult, userData) =>
        _missingSymbol(symbol, error);
  }
}

DartEngineOnIntentPartial _lookupEngineOnIntentPartial(
  ffi.DynamicLibrary library,
) {
  const symbol = 'auwgent_engine_on_intent_partial';
  try {
    return library
        .lookupFunction<NativeEngineOnIntentPartial, DartEngineOnIntentPartial>(
          symbol,
        );
  } catch (error) {
    return (handle, callback, userData) => _missingSymbol(symbol, error);
  }
}

DartEngineOnSubEngineStart _lookupEngineOnSubEngineStart(
  ffi.DynamicLibrary library,
) {
  const symbol = 'auwgent_engine_on_sub_engine_start';
  try {
    return library.lookupFunction<
      NativeEngineOnSubEngineStart,
      DartEngineOnSubEngineStart
    >(symbol);
  } catch (error) {
    return (handle, callback, freeResult, userData) =>
        _missingSymbol(symbol, error);
  }
}

DartEngineOnSubEngineComplete _lookupEngineOnSubEngineComplete(
  ffi.DynamicLibrary library,
) {
  const symbol = 'auwgent_engine_on_sub_engine_complete';
  try {
    return library.lookupFunction<
      NativeEngineOnSubEngineComplete,
      DartEngineOnSubEngineComplete
    >(symbol);
  } catch (error) {
    return (handle, callback, userData) => _missingSymbol(symbol, error);
  }
}
