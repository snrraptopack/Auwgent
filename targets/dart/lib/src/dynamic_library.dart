import 'dart:ffi' as ffi;
import 'dart:io';

ffi.DynamicLibrary openAuwgentLibrary([String? path]) {
  final watch = Stopwatch()..start();
  _timingLog('open library start path=${path ?? '<auto>'}', watch);
  if (path != null && path.isNotEmpty) {
    final library = ffi.DynamicLibrary.open(path);
    _timingLog('opened explicit library', watch);
    return library;
  }

  final envPath = Platform.environment['AUWGENT_LIBRARY_PATH'];
  if (envPath != null && envPath.isNotEmpty) {
    final library = ffi.DynamicLibrary.open(envPath);
    _timingLog('opened env library', watch);
    return library;
  }

  final libraryName = _defaultLibraryName();
  for (final candidate in _candidateLibraryPaths(libraryName)) {
    _timingLog('checking $candidate', watch);
    if (File(candidate).existsSync()) {
      final library = ffi.DynamicLibrary.open(candidate);
      _timingLog('opened candidate $candidate', watch);
      return library;
    }
  }

  final library = ffi.DynamicLibrary.open(libraryName);
  _timingLog('opened by library name $libraryName', watch);
  return library;
}

String _defaultLibraryName() {
  if (Platform.isWindows) {
    return 'auwgent_c_abi.dll';
  }
  if (Platform.isMacOS) {
    return 'libauwgent_c_abi.dylib';
  }
  if (Platform.isLinux || Platform.isAndroid) {
    return 'libauwgent_c_abi.so';
  }

  throw UnsupportedError(
    'Unsupported platform for Auwgent Dart FFI: ${Platform.operatingSystem}',
  );
}

Iterable<String> _candidateLibraryPaths(String libraryName) sync* {
  final current = Directory.current.absolute;

  yield File(current.uri.resolve(libraryName).toFilePath()).path;
  yield File(current.uri.resolve('../../$libraryName').toFilePath()).path;
  yield File(current.uri.resolve('../../../$libraryName').toFilePath()).path;
  yield File(
    current.uri.resolve('../../../target/debug/$libraryName').toFilePath(),
  ).path;
  yield File(
    current.uri.resolve('../../../target/release/$libraryName').toFilePath(),
  ).path;
  yield File(
    current.uri
        .resolve('../../../c-abi/target/debug/$libraryName')
        .toFilePath(),
  ).path;
  yield File(
    current.uri
        .resolve('../../../c-abi/target/release/$libraryName')
        .toFilePath(),
  ).path;
  yield File(
    current.uri.resolve('../../../../target/debug/$libraryName').toFilePath(),
  ).path;
  yield File(
    current.uri.resolve('../../../../target/release/$libraryName').toFilePath(),
  ).path;
}

bool _timingEnabled() {
  final value = Platform.environment['AUWGENT_DEBUG_TIMING'];
  return value == '1' ||
      value == 'true' ||
      value == 'TRUE' ||
      value == 'yes' ||
      value == 'YES';
}

void _timingLog(String message, Stopwatch watch) {
  if (_timingEnabled()) {
    stderr.writeln(
      '[auwgent][timing][dart] dynamic_library +${watch.elapsedMilliseconds}ms $message',
    );
  }
}
