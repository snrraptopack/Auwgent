const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const expectedEntries = [
  'bin/auwgent.js',
  'bin/auwgent-lsp.js',
  'install.js',
  'verify-release-assets.js',
  'assets/auwgent-x86_64-pc-windows-msvc.zip',
  'assets/auwgent-x86_64-unknown-linux-gnu.tar.gz',
  'assets/auwgent-x86_64-apple-darwin.tar.gz',
  'assets/auwgent-aarch64-apple-darwin.tar.gz'
];

function parsePackJson(output) {
  const start = output.indexOf('[');
  if (start < 0) {
    throw new Error('npm pack did not return JSON output.');
  }
  const json = output.slice(start);
  return JSON.parse(json);
}

function main() {
  const output = execSync('npm pack --json', { encoding: 'utf8', cwd: __dirname });
  const rows = parsePackJson(output);
  if (!Array.isArray(rows) || rows.length === 0) {
    throw new Error('npm pack --json returned no package metadata.');
  }

  const packInfo = rows[0];
  const files = new Set((packInfo.files || []).map((f) => f.path));
  const missing = expectedEntries.filter((entry) => !files.has(entry));

  if (packInfo.filename) {
    const tgz = path.join(__dirname, packInfo.filename);
    if (fs.existsSync(tgz)) {
      fs.unlinkSync(tgz);
    }
  }

  if (missing.length > 0) {
    throw new Error(`Package smoke-check failed. Missing entries: ${missing.join(', ')}`);
  }

  console.log('Package smoke-check passed. Required files are included in the npm tarball.');
}

try {
  main();
} catch (err) {
  console.error(err.message || err);
  process.exit(1);
}
