// The in-memory host's M0 diagnostic commands (C0.4). The real host registers them only in the diagnostics build, so
// the mock includes them only there too; the default bundle drops this module and names none of them.
import type { ErrorInfo } from './types';

type Outcome = { ok: true; value: unknown } | { ok: false; error: ErrorInfo };

export const MOCK_DIAGNOSTIC_HANDLERS: Record<string, () => Outcome> = {
  bench_plan: () => ({ ok: true, value: null }),
  spike_mode: () => ({ ok: true, value: null }),
  bench_report: () => ({
    ok: false,
    error: { code: 'diagnostics.notRequested', message: 'the app was not started with --ipc-bench' },
  }),
};
