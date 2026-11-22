// Writes the parity manifest (desktop/src/ui-bindings/parity.json; parity note 9, ruling D-25): runs the parity test
// through the desktop's pinned `test` script with TT_PARITY_WRITE=1, which writes the manifest instead of comparing.
// Usage: `pnpm --dir desktop parity:gen` (or `node scripts/gen-parity-manifest.mjs`).
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const desktop = join(dirname(fileURLToPath(import.meta.url)), '..', 'desktop');
const r = spawnSync('pnpm', ['test', 'src/ui-bindings/parity.test.tsx'], {
  cwd: desktop,
  env: { ...process.env, TT_PARITY_WRITE: '1' },
  stdio: 'inherit',
  shell: process.platform === 'win32',
});
process.exit(r.status ?? 1);
