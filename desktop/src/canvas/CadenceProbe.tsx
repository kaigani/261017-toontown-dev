// The control of ruling D-33: with `--canvas-metrics`, the webview's own `requestAnimationFrame` cadence measured by a
// loop that does no work, published in the UI tree (`metrics.cadence`), so a measurement can tell a display or
// compositor cap from frames the canvas loses. It records the intervals of the last 300 frames, how many canvases the
// page holds and whether the start screen is up (the control surface).
import { useEffect, useState } from 'react';

import { canvasLaunch } from './launch';
import { stat } from './metrics';

export function CadenceProbe({ enabled }: { enabled: boolean }) {
  const [value, setValue] = useState<string | null>(null);
  useEffect(() => {
    if (!enabled || !canvasLaunch().metrics) return undefined;
    const frames: number[] = [];
    let last: number | null = null;
    let published = 0;
    let raf = 0;
    const tick = (t: number) => {
      if (last !== null) {
        frames.push(t - last);
        if (frames.length > 300) frames.shift();
      }
      last = t;
      if (t - published >= 500) {
        published = t;
        setValue(
          JSON.stringify({
            frameMs: stat(frames),
            canvases: document.querySelectorAll('canvas').length,
            startScreen: document.querySelector('[data-tt-id="startScreen"]') !== null,
          }),
        );
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
    };
  }, [enabled]);
  if (value === null) return null;
  return <span className="tt-probe" data-tt-role="text" data-tt-id="metrics.cadence" data-tt-value={value} />;
}
