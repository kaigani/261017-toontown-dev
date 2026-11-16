// A surface's command references and the kernel's availability of each (ruling O-10; C43B-R1S-09): the surface
// registers its references with the shared store under its own owner, and reads the answers from the `availability`
// slice. Until the kernel has answered, a reference has no answer and its control is disabled (the safe default).

import { useEffect, useId } from 'react';

import { useShell, useSlice } from '../state/context';
import { refKey, type Availability, type CommandRef } from './availability';

export function useAvailability(refs: readonly CommandRef[]): (ref: CommandRef) => Availability | undefined {
  const { store } = useShell();
  const owner = useId();
  useEffect(() => {
    store.watch(refs, owner);
  }, [store, owner, refs]);
  useEffect(
    () => () => {
      store.watch([], owner);
    },
    [store, owner],
  );
  const map = (useSlice('availability') as Readonly<Record<string, Availability>> | undefined) ?? {};
  return (ref) => map[refKey(ref)];
}

/** Whether the kernel has said this reference can run now. */
export const enabledIn = (answer: Availability | undefined): boolean => answer?.enabled === true;
