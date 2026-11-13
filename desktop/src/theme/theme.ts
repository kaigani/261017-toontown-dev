// Themes (01 §5.8; M4 C4.1): light, dark, follow-system and user themes, applied at once as CSS custom properties
// (`--tt-<token>`), no restart. The built-in palettes live in styles.css under `[data-theme]`; a user theme sets its
// tokens inline over the matching built-in base.
import type { Theme } from '../state/types';

/** The colour tokens a theme defines (crates/executor/src/themes.rs TOKENS). */
export const TOKENS = [
  'background',
  'surface',
  'surfaceAlt',
  'border',
  'text',
  'textMuted',
  'accent',
  'accentText',
  'selection',
  'focus',
  'danger',
  'warning',
  'success',
  'tooltipBackground',
  'tooltipText',
  'tabActive',
  'tabInactive',
] as const;

let systemListener: ((e: MediaQueryListEvent) => void) | null = null;
let systemQuery: MediaQueryList | null = null;

function prefersDark(): boolean {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-color-scheme: dark)').matches
    : false;
}

function setBase(dark: boolean): void {
  document.documentElement.setAttribute('data-theme', dark ? 'dark' : 'light');
}

function clearTokens(): void {
  for (const t of TOKENS) document.documentElement.style.removeProperty(`--tt-${t}`);
}

/** CSS colour from `#RRGGBBAA`. */
export function cssColour(c: string): string {
  const m = /^#([0-9A-Fa-f]{6})([0-9A-Fa-f]{2})?$/.exec(c);
  if (!m) return c;
  const hex = m[1] ?? '000000';
  const a = m[2] === undefined ? 1 : parseInt(m[2], 16) / 255;
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  return `rgba(${String(r)}, ${String(g)}, ${String(b)}, ${a.toFixed(3)})`;
}

/** Applies a theme id (the value of `interface.theme`) at once. Unknown ids fall back to following the system. */
export function applyTheme(themeId: string, themes: readonly Theme[]): string {
  if (systemQuery && systemListener) systemQuery.removeEventListener('change', systemListener);
  systemQuery = null;
  systemListener = null;
  clearTokens();
  const user = themes.find((t) => t.themeId === themeId && !t.builtIn);
  if (themeId === 'light' || themeId === 'dark') {
    setBase(themeId === 'dark');
    return themeId;
  }
  if (user) {
    setBase(user.dark);
    for (const [token, colour] of Object.entries(user.colours ?? {})) {
      document.documentElement.style.setProperty(`--tt-${token}`, cssColour(colour));
    }
    return user.themeId;
  }
  // Follow the operating system's appearance, live (01 §5.8).
  setBase(prefersDark());
  if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
    systemQuery = window.matchMedia('(prefers-color-scheme: dark)');
    systemListener = (e) => {
      setBase(e.matches);
    };
    systemQuery.addEventListener('change', systemListener);
  }
  return 'system';
}
