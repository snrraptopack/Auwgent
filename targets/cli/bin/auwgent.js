#!/usr/bin/env node
const { spawnSync } = require('child_process');
const path = require('path');
const os = require('os');
const fs = require('fs');

const platform = os.platform();
const arch = os.arch();
const ext = platform === 'win32' ? '.exe' : '';
const binName = `auwgent${ext}`;

let binPath;

// Map platform/arch to our internal package names
const pkgMap = {
  'win32-x64': '@snrraptopack/auwgent-cli-win32-x64-msvc',
  'darwin-x64': '@snrraptopack/auwgent-cli-darwin-x64',
  'darwin-arm64': '@snrraptopack/auwgent-cli-darwin-arm64',
  'linux-x64': '@snrraptopack/auwgent-cli-linux-x64-gnu'
};

const pkgName = pkgMap[`${platform}-${arch}`];

try {
  // 1. Try to find the binary in the architecture-specific package
  if (pkgName) {
    binPath = require.resolve(`${pkgName}/bin/${binName}`);
  }
} catch (e) {
  // If not found in architecture package, fall back to local bin (for dev/old installs)
  binPath = path.join(__dirname, binName);
}

if (!binPath || !fs.existsSync(binPath)) {
  console.error(`Auwgent CLI binary not found for ${platform}-${arch}.`);
  console.error("Please reinstall via: npm install -g @snrraptopack/auwgent-cli");
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status ?? 1);
