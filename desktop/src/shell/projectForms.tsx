// The New Project form (01 §3.1; start screen and dialog share it) and the Project Properties dialog (01 §3.8).
import { useEffect, useRef, useState } from 'react';

import type { Envelope } from '../ipc';
import type { ShellServices } from '../state/context';
import { Dialog } from './dialogs';
import { useModalFocus } from './modalFocus';
import { declaredRange, type NumericRange } from './numeric';
import { NumericField } from './NumericField';
import { projectFieldProblems } from './projectFields';

type ProjectNumber = 'frameRate' | 'width' | 'height';

/** The ranges of 01 §3.1 (frame rate 1-120, camera 16-8192 whole pixels), narrowed by what the command declares. */
function projectRange(command: string, k: ProjectNumber): NumericRange {
  const param = k === 'frameRate' ? 'frameRate' : k === 'width' ? 'cameraWidth' : 'cameraHeight';
  const own: NumericRange = k === 'frameRate' ? { min: 1, max: 120 } : { min: 16, max: 8192, integer: true };
  return { ...own, ...declaredRange(command, param) };
}

const PROJECT_LABELS: Readonly<Record<ProjectNumber, string>> = {
  frameRate: 'Frame rate',
  width: 'Camera width',
  height: 'Camera height',
};

interface PresetEntry {
  readonly id: string;
  readonly name: string;
  readonly kind: string;
  readonly frameRate: number;
  readonly width: number;
  readonly height: number;
}

/** The New Project form (start screen and dialog share it). Resolves with `project.create` parameters. */
export function NewProjectForm({
  presets,
  onCreate,
  idPrefix,
}: {
  presets: readonly PresetEntry[];
  onCreate: (params: Record<string, unknown>) => void;
  idPrefix: string;
}) {
  const initial =
    presets.find((p) => p.kind === 'mostRecent') ?? presets.find((p) => p.kind === 'default') ?? presets[0];
  const [name, setName] = useState('Untitled');
  const [preset, setPreset] = useState<string>(initial?.id ?? 'custom');
  const [frameRate, setFrameRate] = useState(initial?.frameRate ?? 24);
  const [width, setWidth] = useState(initial?.width ?? 1920);
  const [height, setHeight] = useState(initial?.height ?? 1080);
  const problems = projectFieldProblems({
    name,
    frameRate: String(frameRate),
    width: String(width),
    height: String(height),
  });
  const valid = Object.keys(problems).length === 0;

  const choose = (id: string) => {
    setPreset(id);
    const p = presets.find((x) => x.id === id);
    if (p) {
      setFrameRate(p.frameRate);
      setWidth(p.width);
      setHeight(p.height);
    }
  };
  // Editing a value by hand switches to Custom unless it matches a preset exactly (01 §3.1).
  const edited = (fps: number, w: number, h: number) => {
    const match = presets.find((p) => p.frameRate === fps && p.width === w && p.height === h);
    setPreset(match ? match.id : 'custom');
  };
  const submit = () => {
    if (!valid) return;
    const params: Record<string, unknown> = {
      name,
      frameRate,
      cameraWidth: width,
      cameraHeight: height,
    };
    if (preset !== 'custom') params['preset'] = preset;
    onCreate(params);
  };
  const field = (id: string, label: string, value: string, set: (v: string) => void, problem?: string) => (
    <label className="field">
      <span>{label}</span>
      <input
        value={value}
        aria-invalid={problem ? true : undefined}
        data-tt-role="field"
        data-tt-id={`${idPrefix}.${id}`}
        data-tt-gui-only="typing"
        onChange={(e) => {
          set(e.target.value);
        }}
        onKeyDown={(e) => {
          // Enter in any field is Create when the form is valid (01 §2).
          if (e.key === 'Enter') {
            e.preventDefault();
            submit();
          }
        }}
      />
      {problem && <small className="field-problem">{problem}</small>}
    </label>
  );
  return (
    <div className="new-project-form" data-tt-role="form" data-tt-id={`${idPrefix}.form`}>
      {field('name', 'Name', name, setName, problems['name'])}
      <label className="field">
        <span>Format</span>
        <select
          value={preset}
          data-tt-role="field"
          data-tt-id={`${idPrefix}.preset`}
          data-tt-gui-only="typing"
          onChange={(e) => {
            choose(e.target.value);
          }}
        >
          {presets.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name} ({p.width} × {p.height}, {p.frameRate} fps)
            </option>
          ))}
          <option value="custom">Custom</option>
        </select>
      </label>
      {(['frameRate', 'width', 'height'] as const).map((k) => (
        <label key={k} className="field">
          <span>{PROJECT_LABELS[k]}</span>
          {/* The window's one numeric field (D-61): a staged value, committed on Enter or focus loss. */}
          <NumericField
            id={`${idPrefix}.${k}`}
            label={PROJECT_LABELS[k]}
            role="field"
            value={k === 'frameRate' ? frameRate : k === 'width' ? width : height}
            range={projectRange('project.create', k)}
            guiOnly="typing"
            onSubmit={submit}
            onCommit={(v) => {
              const next = { frameRate, width, height, [k]: v };
              setFrameRate(next.frameRate);
              setWidth(next.width);
              setHeight(next.height);
              edited(next.frameRate, next.width, next.height);
            }}
          />
        </label>
      ))}
      <button
        type="button"
        className="button-default"
        disabled={!valid}
        data-tt-role="button"
        data-tt-id={`${idPrefix}.create`}
        data-tt-command="project.create"
        onClick={submit}
      >
        Create
      </button>
    </div>
  );
}

function usePresets(services: ShellServices): readonly PresetEntry[] {
  const slice = services.store.getState().slices.presets;
  return Array.isArray(slice) ? (slice as PresetEntry[]) : [];
}

export function NewProjectDialog({
  services,
  close,
}: {
  services: ShellServices;
  close: (r: Record<string, unknown> | null) => void;
}) {
  const presets = usePresets(services);
  return (
    <Dialog
      id="newProject"
      title="New Project"
      onCancel={() => {
        close(null);
      }}
    >
      <NewProjectForm presets={presets} idPrefix="newProject" onCreate={close} />
    </Dialog>
  );
}

export function ProjectPropertiesDialog({
  services,
  close,
}: {
  services: ShellServices;
  close: (r: Record<string, unknown> | null) => void;
}) {
  const [values, setValues] = useState<Record<ProjectNumber, number> | null>(null);
  const [asDefault, setAsDefault] = useState(false);
  useEffect(() => {
    void services.engine.run('project.properties.get', {}).done.then((env) => {
      const r = (env.result ?? {}) as Record<string, number | undefined>;
      setValues({
        frameRate: r['frameRate'] ?? 24,
        width: r['cameraWidth'] ?? 1920,
        height: r['cameraHeight'] ?? 1080,
      });
    });
  }, [services]);
  const problems = values
    ? projectFieldProblems({
        name: 'x',
        frameRate: String(values.frameRate),
        width: String(values.width),
        height: String(values.height),
      })
    : { loading: '' };
  const set = (k: ProjectNumber) => (v: number) => {
    setValues((s) => (s ? { ...s, [k]: v } : s));
  };
  return (
    <Dialog
      id="projectProperties"
      title="Project Properties"
      command="project.properties.set"
      onCancel={() => {
        close(null);
      }}
      submitDisabled={Object.keys(problems).length > 0}
      onSubmit={() => {
        if (!values) return;
        close({
          frameRate: values.frameRate,
          cameraWidth: values.width,
          cameraHeight: values.height,
          setAsDefault: asDefault,
        });
      }}
    >
      {values &&
        (['frameRate', 'width', 'height'] as const).map((k) => (
          <label key={k} className="field">
            <span>{PROJECT_LABELS[k]}</span>
            <NumericField
              id={`projectProperties.${k}`}
              label={PROJECT_LABELS[k]}
              role="field"
              value={values[k]}
              range={projectRange('project.properties.set', k)}
              onCommit={set(k)}
            />
          </label>
        ))}
      <label>
        <input
          type="checkbox"
          checked={asDefault}
          data-tt-role="field"
          data-tt-id="projectProperties.setAsDefault"
          onChange={(e) => {
            setAsDefault(e.target.checked);
          }}
        />{' '}
        Set as default for new projects
      </label>
    </Dialog>
  );
}

/** The progress window of a job (01 §9.5): step text, fraction, Cancel (`job.cancel`, never a request
 *  cancellation). */
export function JobProgress({
  services,
  jobId,
  title,
  cancellable,
  close,
}: {
  services: ShellServices;
  jobId: string;
  title: string;
  cancellable: boolean;
  close: (env: Envelope | null) => void;
}) {
  const [step, setStep] = useState('');
  const [fraction, setFraction] = useState<number | null>(null);
  useEffect(() => {
    const off = services.store.onEvent((e) => {
      const p = (e.payload ?? {}) as Record<string, unknown>;
      if (e.name === 'job.progress' && p['jobId'] === jobId) {
        if (typeof p['step'] === 'string') setStep(p['step']);
        if (typeof p['fraction'] === 'number') setFraction(p['fraction']);
      }
    });
    // The wait is a query on this request; the job is not the request's (ADR-0020 decision 8).
    const wait = async (): Promise<Envelope> => {
      for (;;) {
        const env = await services.engine.run('job.wait', { jobId, timeoutMs: 20_000 }).done;
        if (env.status === 'ok' || env.error?.code !== 'job.waitTimeout') return env;
      }
    };
    void wait().then((env) => {
      off();
      close(env);
    });
    return off;
  }, [services, jobId, close]);
  // D-49 item 3: an application modal like the others; Escape cancels the job where it can be cancelled.
  const ref = useRef<HTMLDivElement>(null);
  useModalFocus(
    ref,
    cancellable
      ? () => {
          void services.engine.run('job.cancel', { jobId }).done;
        }
      : undefined,
  );
  return (
    <div
      ref={ref}
      tabIndex={-1}
      className="dialog progress"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      data-tt-role="dialog"
      data-tt-id="progress"
      data-tt-label={title}
    >
      <h2>{title}</h2>
      <p data-tt-role="text" data-tt-id="progress.step">
        {step || 'Working…'}
      </p>
      <progress value={fraction ?? undefined} max={1} />
      <div className="buttons">
        {cancellable && (
          <button
            type="button"
            data-tt-role="button"
            data-tt-id="progress.cancel"
            data-tt-command="job.cancel"
            onClick={() => {
              void services.engine.run('job.cancel', { jobId }).done;
            }}
          >
            Cancel
          </button>
        )}
      </div>
    </div>
  );
}
