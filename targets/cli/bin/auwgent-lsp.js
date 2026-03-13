#!/usr/bin/env node
const { spawnSync } = require('child_process');
const path = require('path');
const os = require('os');
const fs = require('fs');

const ext = os.platform() === 'win32' ? '.exe' : '';
const binPath = path.join(__dirname, `auwgent-lsp${ext}`);

if (!fs.existsSync(binPath)) {
  console.error("Auwgent LSP binary not found. Please reinstall via: npm install -g @snrraptopack/auwgent-cli");
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), { stdio: 'inherit' });
process.exit(result.status ?? 1);
