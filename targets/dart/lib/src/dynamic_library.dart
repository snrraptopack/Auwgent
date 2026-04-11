import 'dart:ffi' as ffi;
import 'dart:io';

ffi.DynamicLibrary openAuwgentLibrary([String? path]) {
  if (path != null && path.isNotEmpty) {
    return ffi.DynamicLibrary.open(path);
  }

  final envPath = Platform.environment['AUWGENT_LIBRARY_PATH'];
  if (envPath != null && envPath.isNotEmpty) {
    return ffi.DynamicLibrary.open(envPath);
  }

  final libraryName = _defaultLibraryName();
  for (final candidate in _candidateLibraryPaths(libraryName)) {
    if (File(candidate).existsSync()) {
      return ffi.DynamicLibrary.open(candidate);
    }
  }

  return ffi.DynamicLibrary.open(libraryName);
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
  yield File(current.uri.resolve('../../../target/debug/$libraryName').toFilePath()).path;
  yield File(current.uri.resolve('../../../target/release/$libraryName').toFilePath()).path;
  yield File(current.uri.resolve('../../../c-abi/target/debug/$libraryName').toFilePath()).path;
  yield File(current.uri.resolve('../../../c-abi/target/release/$libraryName').toFilePath()).path;
  yield File(current.uri.resolve('../../../../target/debug/$libraryName').toFilePath()).path;
  yield File(current.uri.resolve('../../../../target/release/$libraryName').toFilePath()).path;
}
