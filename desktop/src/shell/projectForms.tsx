// The New Project form (01 §3.1; start screen and dialog share it) and the Project Properties dialog (01 §3.8).
import { useEffect, useState } from 'react';

import type { Envelope } from '../ipc';
import type { ShellServices } from '../state/context';
import { Dialog } from './dialogs';
import { projectFieldProblems } from './projectFields';

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
  const [frameRate, setFrameRate] = useState(String(initial?.frameRate ?? 24));
  const [width, setWidth] = useState(String(initial?.width ?? 1920));
  const [height, setHeight] = useState(String(initial?.height ?? 1080));
  const problems = projectFieldProblems({ name, frameRate, width, height });
  const valid = Object.keys(problems).length === 0;

  const choose = (id: string) => {
    setPreset(id);
    const p = presets.find((x) => x.id === id);
    if (p) {
      setFrameRate(String(p.frameRate));
      setWidth(String(p.width));
      setHeight(String(p.height));
    }
  };
  // Editing a value by hand switches to Custom unless it matches a preset exactly (01 §3.1).
  const edited = (fps: string, w: string, h: string) => {
    const match = presets.find(
      (p) => String(p.frameRate) === fps && String(p.width) === w && String(p.height) === h,
    );
    setPreset(match ? match.id : 'custom');
  };
  const submit = () => {
    if (!valid) return;
    const params: Record<string, unknown> = {
      name,
      frameRate: Number(frameRate),
      cameraWidth: Number(width),
      cameraHeight: Number(height),
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
      {field(
        'frameRate',
        'Frame rate',
        frameRate,
        (v) => {
          setFrameRate(v);
          edited(v, width, height);
        },
        problems['frameRate'],
      )}
      {field(
        'width',
        'Camera width',
        width,
        (v) => {
          setWidth(v);
          edited(frameRate, v, height);
        },
        problems['width'],
      )}
      {field(
        'height',
        'Camera height',
        height,
        (v) => {
          setHeight(v);
          edited(frameRate, width, v);
        },
        problems['height'],
      )}
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
  const [values, setValues] = useState<{ frameRate: string; width: string; height: string } | null>(null);
  const [asDefault, setAsDefault] = useState(false);
  useEffect(() => {
    void services.engine.run('project.properties.get', {}).done.then((env) => {
      const r = (env.result ?? {}) as Record<string, number | undefined>;
      setValues({
        frameRate: String(r['frameRate'] ?? 24),
        width: String(r['cameraWidth'] ?? 1920),
        height: String(r['cameraHeight'] ?? 1080),
      });
    });
  }, [services]);
  const problems = values ? projectFieldProblems({ name: 'x', ...values }) : { loading: '' };
  const set = (k: 'frameRate' | 'width' | 'height') => (v: string) => {
    setValues((s) => (s ? { ...s, [k]: v } : s));
  };
  return (
    <Dialog
      id="projectProperties"
      title="Project Properties"
      onCancel={() => {
        close(null);
      }}
      submitDisabled={Object.keys(problems).length > 0}
      onSubmit={() => {
        if (!values) return;
        close({
          frameRate: Number(values.frameRate),
          cameraWidth: Number(values.width),
          cameraHeight: Number(values.height),
          setAsDefault: asDefault,
        });
      }}
    >
      {values &&
        (['frameRate', 'width', 'height'] as const).map((k) => (
          <label key={k} className="field">
            <span>{k === 'frameRate' ? 'Frame rate' : k === 'width' ? 'Camera width' : 'Camera height'}</span>
            <input
              value={values[k]}
              data-tt-role="field"
              data-tt-id={`projectProperties.${k}`}
              onChange={(e) => {
                set(k)(e.target.value);
              }}
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
  return (
    <div
      className="dialog progress"
      role="dialog"
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
