// Reads answered out of order (CI run 45035198984, the D-39 row "Pen panel > New" on macOS): the desktop host serves a
// window's calls concurrently (`run_command` is asynchronous), so a read a surface began earlier can answer after one it
// began later. A surface that reads again on each kernel event therefore keeps only its newest read's answers: each
// read takes a ticket, and its answers apply only while no newer read has begun. The newest read began after the
// newest event the surface heard, so what it shows is never older than that event.
import { useState } from 'react';

/** A gate for one surface's reads: call it when a read begins; the function it returns says whether that read is
 *  still the newest one (its answers apply) or a newer read has begun since (its answers are dropped). */
export type ReadGate = () => () => boolean;

export function readGate(): ReadGate {
  let newest = 0;
  return () => {
    newest += 1;
    const ticket = newest;
    return () => ticket === newest;
  };
}

/** The component's read gate, the same for its whole life. */
export function useReadGate(): ReadGate {
  const [gate] = useState(readGate);
  return gate;
}
