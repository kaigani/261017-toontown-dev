// Validation of the new-project and project-properties fields (01 §3.1, §3.8).
import { nameProblem } from './names';

/** Validation of the new-project fields (01 §3.1): the problem of each field, or none. */
export function projectFieldProblems(f: { name: string; frameRate: string; width: string; height: string }) {
  const problems: Record<string, string> = {};
  const name = nameProblem(f.name);
  if (name) problems['name'] = name;
  const fps = Number(f.frameRate);
  if (!(f.frameRate.trim() !== '' && Number.isFinite(fps) && fps >= 1 && fps <= 120)) {
    problems['frameRate'] = 'Frame rate is 1 to 120 frames per second.';
  }
  for (const k of ['width', 'height'] as const) {
    const v = Number(f[k]);
    if (!(/^\d+$/.test(f[k].trim()) && v >= 16 && v <= 8192))
      problems[k] = 'Camera size is 16 to 8192 pixels.';
  }
  return problems;
}
