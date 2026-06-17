// Cross-platform build script for Tauri
// Windows: NSIS .exe   macOS: .dmg   Linux: .deb + .AppImage
//
// Auto-detects local proxy (Clash/V2Ray) and sets HTTPS_PROXY so the Rust
// HTTP client (reqwest) can reach GitHub for downloading bundler tools.

import { spawn } from 'node:child_process';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { platform } from 'node:os';
import { connect } from 'node:net';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');

const env = { ...process.env };

// ----- Auto-detect local proxy -----
const PROXY_PORTS = [7890, 10809, 1080, 8118, 8080, 1087, 4780];

async function detectProxy() {
  // respect explicit user setting first
  if (env.HTTPS_PROXY || env.https_proxy || env.HTTP_PROXY || env.http_proxy) {
    console.log('[build] Using existing proxy env vars');
    return;
  }
  for (const port of PROXY_PORTS) {
    const reachable = await new Promise((resolve) => {
      const s = connect({ host: '127.0.0.1', port, timeout: 500 }, () => {
        s.destroy();
        resolve(true);
      });
      s.on('error', () => resolve(false));
      s.on('timeout', () => { s.destroy(); resolve(false); });
    });
    if (reachable) {
      const proxy = `http://127.0.0.1:${port}`;
      env.HTTPS_PROXY = proxy;
      env.HTTP_PROXY = proxy;
      console.log(`[build] Proxy detected: ${proxy}`);
      return;
    }
  }
  console.log('[build] No proxy detected, direct connection');
}

// ----- Build -----
const labels = { win32: 'Windows → .exe (NSIS)', darwin: 'macOS → .dmg', linux: 'Linux → .deb + .AppImage' };

await detectProxy();
console.log(`[build] ${labels[platform()] || platform()}`);

const child = spawn('npm', ['run', 'tauri', 'build'], {
  cwd: root,
  env,
  stdio: 'inherit',
  shell: true,
});

child.on('exit', (code) => {
  process.exit(code ?? 1);
});
