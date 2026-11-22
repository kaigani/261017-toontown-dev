// The load report's model (10 §7.3, §10.4; M4 C4.4a, design note 2 §7): what the window shows and copies.

export interface Issue {
  readonly issueId: string;
  readonly code: string;
  readonly severity: string;
  readonly objectId?: string | null;
  readonly path?: string | null;
  readonly entry?: string | null;
  readonly message: string;
}

export interface LoadReport {
  readonly projectId: string;
  readonly path: string | null;
  readonly formatVersion: string;
  readonly migratedFrom: string | null;
  readonly readOnly: boolean;
  readonly issues: readonly Issue[];
  readonly repairedCount: number;
}

const notInfo = (i: Issue) => i.severity !== 'info';

/** Whether a report is worth the window (10 §7.3: a summary when something happened). */
export function worthShowing(r: LoadReport): boolean {
  return r.issues.some(notInfo) || r.repairedCount > 0 || r.migratedFrom !== null || r.readOnly;
}

const plural = (n: number, one: string) => `${String(n)} ${one}${n === 1 ? '' : 's'}`;

/** "Opened with 3 repairs and 2 warnings; migrated from format 0.9; read-only". */
export function summaryOf(r: LoadReport): string {
  const warnings = r.issues.filter((i) => i.severity === 'warning').length;
  const errors = r.issues.filter((i) => i.severity === 'error').length;
  const parts = [
    r.repairedCount > 0 ? plural(r.repairedCount, 'repair') : null,
    errors > 0 ? plural(errors, 'error') : null,
    warnings > 0 ? plural(warnings, 'warning') : null,
  ].filter((p): p is string => p !== null);
  const listed =
    parts.length > 1 ? `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1] ?? ''}` : parts[0];
  const head = listed ? `Opened with ${listed}` : 'Opened';
  const tail = [
    r.migratedFrom !== null ? `migrated from format ${r.migratedFrom}` : null,
    r.readOnly ? 'read-only' : null,
  ].filter((p): p is string => p !== null);
  const all = [head, ...tail];
  return all.join('; ');
}

/** The report as text, for Copy details. */
export function reportText(r: LoadReport): string {
  const lines = [summaryOf(r), `Project: ${r.path ?? r.projectId}`, `Format: ${r.formatVersion}`];
  for (const i of r.issues) {
    const where = i.path ?? i.entry ?? i.objectId ?? '';
    lines.push(`${i.severity} ${i.code}${where ? ` ${where}` : ''}: ${i.message}`);
  }
  return lines.join('\n');
}

export function asReport(v: unknown): LoadReport | null {
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (typeof o['projectId'] !== 'string' || !Array.isArray(o['issues'])) return null;
  return {
    projectId: o['projectId'],
    path: typeof o['path'] === 'string' ? o['path'] : null,
    formatVersion: typeof o['formatVersion'] === 'string' ? o['formatVersion'] : '',
    migratedFrom: typeof o['migratedFrom'] === 'string' ? o['migratedFrom'] : null,
    readOnly: o['readOnly'] === true,
    issues: o['issues'] as Issue[],
    repairedCount: typeof o['repairedCount'] === 'number' ? o['repairedCount'] : 0,
  };
}
