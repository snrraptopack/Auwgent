const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const extensionRoot = path.resolve(__dirname, '..');
const repoRoot = path.resolve(extensionRoot, '..');
const compilerRoot = path.join(repoRoot, 'auwgent-compiler');
const extensionBinDir = path.join(extensionRoot, 'bin');

const isWindows = process.platform === 'win32';
const binaryName = isWindows ? 'auwgent-lsp.exe' : 'auwgent-lsp';
const sourceBinaryPath = path.join(compilerRoot, 'target', 'release', binaryName);
const bundledBinaryPath = path.join(extensionBinDir, binaryName);

function main() {
	ensureDirectory(extensionBinDir);
	buildRustLsp();
	copyBinary(sourceBinaryPath, bundledBinaryPath);

	console.log(`Bundled Rust LSP: ${bundledBinaryPath}`);
}

function buildRustLsp() {
	console.log('Building Rust LSP in release mode...');

	const result = spawnSync('cargo', ['build', '--release', '-p', 'auwgent-lsp'], {
		cwd: compilerRoot,
		stdio: 'inherit',
		shell: isWindows,
	});

	if (result.error) {
		throw result.error;
	}

	if (result.status !== 0) {
		process.exit(result.status ?? 1);
	}
}

function copyBinary(fromPath, toPath) {
	if (!fs.existsSync(fromPath)) {
		throw new Error(`Expected built Rust LSP binary was not found at: ${fromPath}`);
	}

	fs.copyFileSync(fromPath, toPath);

	if (!fs.existsSync(toPath)) {
		throw new Error(`Failed to copy Rust LSP binary to: ${toPath}`);
	}
}

function ensureDirectory(dirPath) {
	fs.mkdirSync(dirPath, { recursive: true });
}

main();