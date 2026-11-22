// Preference categories of the Preferences window (01 §7.3) and inline validation of staged values.
import type { PrefEntry } from '../state/types';

export const CATEGORY_OF_PREFIX: Readonly<Record<string, string>> = {
  user: 'general',
  library: 'general',
  general: 'general',
  files: 'files',
  project: 'newProject',
  interface: 'interface',
  drawing: 'drawingAids',
  paint: 'drawingAids',
  display: 'display',
  tablet: 'tablet',
  staging: 'staging',
  timing: 'timing',
  edit: 'edit',
  agent: 'agent',
};

export const CATEGORIES: readonly { readonly id: string; readonly label: string }[] = [
  { id: 'general', label: 'General' },
  { id: 'files', label: 'Files and safety' },
  { id: 'newProject', label: 'New-project defaults' },
  { id: 'interface', label: 'Interface' },
  { id: 'drawingAids', label: 'Drawing aids' },
  { id: 'display', label: 'Display' },
  { id: 'tablet', label: 'Tablet' },
  { id: 'staging', label: 'Staging' },
  { id: 'timing', label: 'Timing' },
  { id: 'edit', label: 'Edit' },
  { id: 'agent', label: 'Agent access' },
  { id: 'keyboard', label: 'Keyboard' },
  { id: 'tips', label: 'Tips' },
];

export function categoryOf(key: string): string {
  return CATEGORY_OF_PREFIX[key.split('.')[0] ?? ''] ?? 'general';
}

/** Inline validation of a staged value (the executor validates again on OK). */
export function valueProblem(p: PrefEntry, raw: unknown): string | null {
  switch (p.type) {
    case 'bool':
      return typeof raw === 'boolean' ? null : 'Choose on or off.';
    case 'int':
    case 'number': {
      const n = typeof raw === 'number' ? raw : Number(raw);
      if (typeof raw === 'string' && raw.trim() === '') return 'Enter a number.';
      if (!Number.isFinite(n)) return 'Enter a number.';
      if (p.type === 'int' && !Number.isInteger(n)) return 'Enter a whole number.';
      if (p.min !== undefined && n < p.min) return `At least ${String(p.min)}.`;
      if (p.max !== undefined && n > p.max) return `At most ${String(p.max)}.`;
      return null;
    }
    case 'colour':
      return typeof raw === 'string' && (/^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$/.test(raw) || raw === '')
        ? null
        : 'A colour #RRGGBBAA.';
    case 'enum':
      return typeof raw === 'string' && (p.values ?? []).includes(raw) ? null : 'Choose one of the values.';
    default:
      return null;
  }
}
