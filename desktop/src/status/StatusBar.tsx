import { useCallback, useEffect, useRef, useState } from 'react';

import { toHostError, useHost, type EngineInfo } from '../ipc';
import { formatMs, measureBurst, pingOnce, type LatencySummary } from '../lib/latency';

export const BURST_COUNT = 500;
const LIVE_INTERVAL_MS = 1000;

export interface StatusMessage {
  text: string;
  kind: 'info' | 'warning' | 'error';
}

/** Status bar (01 §9.1): messages on the left, engine and IPC latency on the right. */
export function StatusBar({ message }: { message: StatusMessage | null }) {
  const host = useHost();
  const [info, setInfo] = useState<EngineInfo | null>(null);
  const [last, setLast] = useState<number | null>(null);
  const [burst, setBurst] = useState<LatencySummary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const seq = useRef(1_000_000);

  useEffect(() => {
    host.appInfo().then(setInfo, (e: unknown) => {
      setError(toHostError(e).message);
    });
  }, [host]);

  useEffect(() => {
    if (busy) return;
    const timer = setInterval(() => {
      pingOnce(host, seq.current++).then(setLast, (e: unknown) => {
        setError(toHostError(e).message);
      });
    }, LIVE_INTERVAL_MS);
    return () => {
      clearInterval(timer);
    };
  }, [host, busy]);

  const runBurst = useCallback(() => {
    setBusy(true);
    measureBurst(host, BURST_COUNT)
      .then(setBurst, (e: unknown) => {
        setError(toHostError(e).message);
      })
      .finally(() => {
        setBusy(false);
      });
  }, [host]);

  return (
    <footer className="status-bar" role="status">
      <span className={`status-message status-${message?.kind ?? 'info'}`}>{message?.text ?? 'Ready'}</span>
      <span className="status-right">
        {error && <span className="status-error">IPC error: {error}</span>}
        <span data-testid="ipc-latency">
          IPC ping ({host.transportKind}): {last === null ? '–' : `${formatMs(last)} ms`}
          {burst &&
            ` · burst n=${burst.n} p50 ${formatMs(burst.p50)} p95 ${formatMs(burst.p95)} max ${formatMs(burst.max)} ms`}
        </span>
        <button type="button" onClick={runBurst} disabled={busy}>
          {busy ? 'Measuring…' : `Measure ${BURST_COUNT}`}
        </button>
        <span className="status-engine">
          {info ? `${info.name} ${info.version} · ${info.platform}-${info.arch}` : '…'}
        </span>
      </span>
    </footer>
  );
}
