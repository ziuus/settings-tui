#!/usr/bin/env node

const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const possiblePaths = [
  path.join(__dirname, '..', 'target', 'release', 'settings-tui'),
  path.join(__dirname, 'settings-tui'),
  '/usr/local/bin/settings-tui',
  '/usr/bin/settings-tui'
];

let binPath = possiblePaths.find(p => fs.existsSync(p) && fs.statSync(p).isFile());

if (!binPath) {
  try {
    const buildRes = spawnSync('cargo', ['build', '--release'], {
      cwd: path.join(__dirname, '..'),
      stdio: 'inherit'
    });
    if (buildRes.status === 0) {
      const builtPath = path.join(__dirname, '..', 'target', 'release', 'settings-tui');
      if (fs.existsSync(builtPath)) {
        binPath = builtPath;
      }
    }
  } catch (_err) {
    // cargo not found or failed
  }
}

if (!binPath) {
  console.error('Error: settings-tui binary could not be located.');
  console.error('Ensure Rust/Cargo is installed or install settings-tui via: cargo install settings-tui');
  process.exit(1);
}

const child = spawnSync(binPath, process.argv.slice(2), {
  stdio: 'inherit'
});

process.exit(child.status !== null ? child.status : 0);
