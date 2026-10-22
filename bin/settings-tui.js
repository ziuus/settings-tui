#!/usr/bin/env node

const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const possiblePaths = [
  '/usr/bin/settings-tui',
  '/usr/local/bin/settings-tui',
  '/usr/bin/settings',
  '/usr/local/bin/settings',
  path.join(__dirname, 'settings-tui'),
  path.join(__dirname, 'settings'),
  path.join(__dirname, '..', 'target', 'release', 'settings-tui'),
  path.join(__dirname, '..', 'target', 'release', 'settings')
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
  try {
    const pkg = require('../package.json');
    const ver = pkg.version || '0.1.15';
    const downloadUrl = `https://github.com/ziuus/settings-tui/releases/download/v${ver}/settings-tui-v${ver}-x86_64-unknown-linux-gnu.tar.gz`;
    const destDir = path.join(__dirname);
    const destTar = path.join(destDir, 'settings-tui.tar.gz');
    process.stderr.write('Downloading precompiled settings-tui binary...\n');
    const curlRes = spawnSync('curl', ['-fSL', downloadUrl, '-o', destTar], { stdio: 'inherit' });
    if (curlRes.status === 0 && fs.existsSync(destTar)) {
      spawnSync('tar', ['-xzf', destTar, '-C', destDir], { stdio: 'inherit' });
      if (fs.existsSync(destTar)) fs.unlinkSync(destTar);
      const downloadedBin = path.join(destDir, 'settings-tui');
      if (fs.existsSync(downloadedBin)) {
        fs.chmodSync(downloadedBin, 0o755);
        binPath = downloadedBin;
      }
    }
  } catch (_err) {
    // download fallback failed
  }
}

if (!binPath) {
  console.error('Error: settings-tui binary could not be located.');
  console.error('Ensure Rust/Cargo is installed or download prebuilt releases from: https://github.com/ziuus/settings-tui/releases');
  process.exit(1);
}

const child = spawnSync(binPath, process.argv.slice(2), {
  stdio: 'inherit'
});

process.exit(child.status !== null ? child.status : 0);
