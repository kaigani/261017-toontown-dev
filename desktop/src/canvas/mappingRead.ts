// The canvas's reads of its view mapping (`draw.viewMapping.get`), kept to the newest read's answer (the D-89 rule;
// macOS byte gate, run 45082297045): the desktop host serves a window's calls concurrently, so the read made at mount can
// answer after the read that followed the canvas's size report. Only the newest read's answer applies.
import { readGate } from '../state/readGate';

/** Returns the function that starts one mapping read: `read` asks the kernel, `apply` takes an answer while `live()`
 *  holds and no newer read has begun since (an older answer arriving last is dropped). */
export function mappingReader<M>(
  read: () => Promise<M | null>,
  apply: (mapping: M) => void,
  live: () => boolean,
): () => void {
  const gate = readGate();
  return () => {
    const newest = gate();
    void read()
      .then((m) => {
        if (m !== null && live() && newest()) apply(m);
      })
      .catch(() => undefined);
  };
}
