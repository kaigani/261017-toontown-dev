// The preferences window (01 §7.1, §8.3, §9.4; M4 C4.1). Edits are staged and validated inline; OK applies them
// with the commands an agent would use — `prefs.reset` for restored categories, one `prefs.set` for the edited
// values (atomic), then the keymap edits in order — and Cancel discards everything, keymap edits included. A value the
// executor refuses keeps the window open and focuses the field. Theme and colour changes preview live (01 §13.8).
import { useMemo, useRef, useState } from 'react';

import { COMMAND_INDEX } from '../ipc/generated/commands';
import { isMac, nativeNotation, sequenceOf } from '../keys/sequence';
import { Dialog } from '../shell/dialogs';
import type { ShellServices } from '../state/context';
import type { Binding, PrefEntry, Theme, Tip } from '../state/types';
import { applyTheme } from '../theme/theme';
import { CATEGORIES, categoryOf, valueProblem } from './categories';

function coerce(p: PrefEntry, raw: unknown): unknown {
  if ((p.type === 'int' || p.type === 'number') && typeof raw === 'string') return Number(raw);
  return raw;
}

interface KeymapOp {
  readonly command: 'keymap.set' | 'keymap.remove' | 'keymap.reset';
  readonly params: Record<string, unknown>;
}

interface IndexEntry {
  readonly kind: string;
  readonly label?: string;
  readonly summary?: string;
  readonly milestone?: string;
  readonly event?: boolean;
}

const INDEX = COMMAND_INDEX as unknown as Readonly<Record<string, IndexEntry>>;

export function PreferencesDialog({
  services,
  prefs,
  keymap,
  tips,
  themes,
  initialCategory,
  close,
}: {
  services: ShellServices;
  prefs: readonly PrefEntry[];
  keymap: readonly Binding[];
  tips: readonly Tip[];
  themes: readonly Theme[];
  initialCategory?: string;
  close: (applied: boolean | null) => void;
}) {
  const [category, setCategory] = useState(initialCategory ?? 'general');
  const [search, setSearch] = useState('');
  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [resetCategories, setResetCategories] = useState<readonly string[]>([]);
  const [keyOps, setKeyOps] = useState<readonly KeymapOp[]>([]);
  const [bindings, setBindings] = useState<readonly Binding[]>(keymap);
  const [problemKey, setProblemKey] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const fields = useRef(new Map<string, HTMLElement>());
  const originalTheme = useRef(prefs.find((p) => p.key === 'interface.theme')?.value);

  const value = (p: PrefEntry): unknown => {
    if (Object.prototype.hasOwnProperty.call(edits, p.key)) return edits[p.key];
    if (resetCategories.includes(categoryOf(p.key))) return p.default;
    return p.value;
  };
  const problems = useMemo(() => {
    const out: Record<string, string> = {};
    for (const p of prefs) {
      if (!Object.prototype.hasOwnProperty.call(edits, p.key)) continue;
      const pr = valueProblem(p, edits[p.key]);
      if (pr) out[p.key] = pr;
    }
    return out;
  }, [edits, prefs]);

  const setValue = (p: PrefEntry, v: unknown) => {
    setEdits((e) => ({ ...e, [p.key]: v }));
    if (p.key === 'interface.theme' && typeof v === 'string') applyTheme(v, themes);
  };

  const cancel = () => {
    // Cancel discards every edit, keymap included; a previewed theme reverts.
    if (typeof originalTheme.current === 'string') applyTheme(originalTheme.current, themes);
    close(null);
  };

  const ok = async () => {
    setRefusal(null);
    const first = Object.keys(problems)[0];
    if (first) {
      setProblemKey(first);
      fields.current.get(first)?.focus();
      return;
    }
    for (const c of resetCategories) {
      const keys = prefs.filter((p) => categoryOf(p.key) === c).map((p) => p.key);
      if (keys.length === 0) continue;
      const env = await services.engine.run('prefs.reset', { keys }).done;
      if (env.status !== 'ok') {
        setRefusal(`${env.error?.code ?? 'error'}: ${env.error?.message ?? ''}`);
        return;
      }
    }
    const values: Record<string, unknown> = {};
    for (const p of prefs) {
      if (Object.prototype.hasOwnProperty.call(edits, p.key)) values[p.key] = coerce(p, edits[p.key]);
    }
    if (Object.keys(values).length > 0) {
      const env = await services.engine.run('prefs.set', { values }).done;
      if (env.status !== 'ok') {
        // 01 §7.1: the window stays open and the offending field gets the focus.
        const d = env.error?.details as Record<string, unknown> | undefined;
        const key = typeof d?.['key'] === 'string' ? d['key'] : null;
        setRefusal(`${env.error?.code ?? 'error'}: ${env.error?.message ?? ''}`);
        if (key) {
          setProblemKey(key);
          const cat = categoryOf(key);
          setCategory(cat);
          fields.current.get(key)?.focus();
        }
        return;
      }
    }
    for (const op of keyOps) {
      const env = await services.engine.run(op.command, op.params).done;
      if (env.status !== 'ok') {
        setRefusal(`${env.error?.code ?? 'error'}: ${env.error?.message ?? ''}`);
        return;
      }
    }
    close(true);
  };

  const shown = prefs.filter((p) => {
    if (search.trim()) {
      const s = search.toLowerCase();
      return p.key.toLowerCase().includes(s) || p.description.toLowerCase().includes(s);
    }
    return categoryOf(p.key) === category;
  });

  return (
    <Dialog
      id="preferences"
      title="Preferences"
      onCancel={cancel}
      onSubmit={() => void ok()}
      submitLabel="OK"
      wide
    >
      <div className="prefs-layout">
        <nav
          className="prefs-categories"
          aria-label="Categories"
          data-tt-role="list"
          data-tt-id="preferences.categories"
        >
          <input
            type="search"
            placeholder="Search settings"
            value={search}
            data-tt-role="field"
            data-tt-id="preferences.search"
            onChange={(e) => {
              setSearch(e.target.value);
            }}
          />
          {CATEGORIES.map((c) => (
            <button
              key={c.id}
              type="button"
              aria-pressed={category === c.id}
              data-tt-role="tab"
              data-tt-id={`preferences.category.${c.id}`}
              onClick={() => {
                setCategory(c.id);
                setSearch('');
              }}
            >
              {c.label}
            </button>
          ))}
        </nav>
        <div className="prefs-body" data-tt-role="group" data-tt-id={`preferences.${category}`}>
          {refusal && (
            <p className="field-problem" role="alert" data-tt-role="text" data-tt-id="preferences.refusal">
              {refusal}
            </p>
          )}
          {category === 'keyboard' && !search ? (
            <KeymapEditor
              services={services}
              bindings={bindings}
              onStage={(op, next) => {
                setKeyOps((ops) => [...ops, op]);
                setBindings(next);
              }}
            />
          ) : category === 'tips' && !search ? (
            <TipsList services={services} tips={tips} />
          ) : (
            <>
              {shown.map((p) => (
                <PrefField
                  key={p.key}
                  pref={p}
                  value={value(p)}
                  problem={problems[p.key] ?? (problemKey === p.key ? refusal : null)}
                  themes={themes}
                  register={(el) => {
                    if (el) fields.current.set(p.key, el);
                  }}
                  onChange={(v) => {
                    setValue(p, v);
                  }}
                />
              ))}
              {!search && (
                <button
                  type="button"
                  data-tt-role="button"
                  data-tt-id={`preferences.${category}.restore`}
                  data-tt-command="prefs.reset"
                  onClick={() => {
                    setResetCategories((r) => (r.includes(category) ? r : [...r, category]));
                    setEdits((e) =>
                      Object.fromEntries(Object.entries(e).filter(([k]) => categoryOf(k) !== category)),
                    );
                  }}
                >
                  Restore defaults
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </Dialog>
  );
}

function PrefField({
  pref,
  value,
  problem,
  themes,
  register,
  onChange,
}: {
  pref: PrefEntry;
  value: unknown;
  problem: string | null;
  themes: readonly Theme[];
  register: (el: HTMLElement | null) => void;
  onChange: (v: unknown) => void;
}) {
  const id = `pref.${pref.key}`;
  const common = {
    id,
    'aria-invalid': problem ? true : undefined,
    'data-tt-role': 'field',
    'data-tt-id': id,
    'data-tt-label': pref.key,
  } as const;
  let input;
  switch (pref.type) {
    case 'bool':
      input = (
        <input
          {...common}
          ref={register}
          type="checkbox"
          checked={value === true}
          onChange={(e) => {
            onChange(e.target.checked);
          }}
        />
      );
      break;
    case 'enum':
      input = (
        <select
          {...common}
          ref={register}
          value={String(value)}
          onChange={(e) => {
            onChange(e.target.value);
          }}
        >
          {(pref.values ?? []).map((v) => (
            <option key={v} value={v}>
              {pref.key === 'interface.theme' ? (themes.find((t) => t.themeId === v)?.name ?? v) : v}
            </option>
          ))}
        </select>
      );
      break;
    case 'object':
      input = (
        <span className="pref-object" ref={register}>
          {Object.entries((value ?? {}) as Record<string, unknown>).map(([k, v]) => (
            <label key={k}>
              <input
                type="checkbox"
                checked={v === true}
                data-tt-role="field"
                data-tt-id={`${id}.${k}`}
                onChange={(e) => {
                  onChange({ ...(value as Record<string, unknown>), [k]: e.target.checked });
                }}
              />
              {k}
            </label>
          ))}
        </span>
      );
      break;
    default:
      input = (
        <input
          {...common}
          ref={register}
          value={typeof value === 'string' || typeof value === 'number' ? String(value) : ''}
          onChange={(e) => {
            onChange(e.target.value);
          }}
        />
      );
  }
  return (
    <div className="pref-field">
      <label htmlFor={id}>
        <code>{pref.key}</code>
      </label>
      {input}
      <small className="pref-description">
        {pref.description}
        {pref.applies !== 'live' && (
          <em>
            {' '}
            {pref.applies === 'restart'
              ? 'Takes effect after a restart.'
              : 'Takes effect at the next launch.'}
          </em>
        )}
      </small>
      {problem && (
        <small className="field-problem" role="alert">
          {problem}
        </small>
      )}
    </div>
  );
}

/** The shortcut editor (01 §8.3). */
export function KeymapEditor({
  services,
  bindings,
  onStage,
}: {
  services: ShellServices;
  bindings: readonly Binding[];
  onStage: (op: KeymapOp, next: readonly Binding[]) => void;
}) {
  const mac = isMac();
  const [filter, setFilter] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [context, setContext] = useState<'global' | 'drawing' | 'staging'>('global');
  const [capturing, setCapturing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [conflict, setConflict] = useState<{ keys: string; with: Binding } | null>(null);

  const commands = Object.entries(INDEX)
    .filter(([, e]) => !e.event)
    .map(([id, e]) => ({ id, label: e.label ?? id, summary: e.summary ?? '' }));
  const keysOf = (id: string) => bindings.filter((b) => b.commandId === id);
  const shown = commands.filter((c) => {
    const f = filter.trim().toLowerCase();
    if (!f) return true;
    const keys = keysOf(c.id)
      .map((b) => `${b.keys} ${nativeNotation(b.keys, mac)}`)
      .join(' ')
      .toLowerCase();
    return (
      c.id.toLowerCase().includes(f) ||
      c.label.toLowerCase().includes(f) ||
      c.summary.toLowerCase().includes(f) ||
      keys.includes(f)
    );
  });
  const byCategory = new Map<string, typeof shown>();
  for (const c of shown) {
    const cat = c.id.split('.')[0] ?? '';
    byCategory.set(cat, [...(byCategory.get(cat) ?? []), c]);
  }

  const stageSet = (keys: string, override: boolean) => {
    if (!selected) return;
    const params: Record<string, unknown> = { commandId: selected, keys, context };
    if (override) params['onConflict'] = 'override';
    const next = bindings.filter(
      (b) => !(b.keys === keys && (b.context === context || b.context === 'global' || context === 'global')),
    );
    onStage({ command: 'keymap.set', params }, [
      ...next,
      { commandId: selected, keys, context, isDefault: false },
    ]);
  };

  const captured = async (keys: string) => {
    setCapturing(false);
    if (!selected) return;
    // The executor's own rules decide what is reserved or malformed: a dry run changes nothing (01 §8.3 item 5).
    const env = await services.engine.run(
      'keymap.set',
      { commandId: selected, keys, context },
      { dryRun: true },
    ).done;
    if (env.status !== 'ok' && env.error?.code !== 'keymap.conflict') {
      setNotice(env.error?.message ?? 'That shortcut cannot be used.');
      return;
    }
    const clash = bindings.find(
      (b) =>
        b.keys === keys &&
        b.commandId !== selected &&
        (b.context === context || b.context === 'global' || context === 'global'),
    );
    if (clash) {
      setConflict({ keys, with: clash });
      return;
    }
    stageSet(keys, false);
  };

  return (
    <div className="keymap-editor" data-tt-role="group" data-tt-id="keymap">
      <input
        type="search"
        placeholder="Filter by label, description or shortcut"
        value={filter}
        data-tt-role="field"
        data-tt-id="keymap.filter"
        onChange={(e) => {
          setFilter(e.target.value);
        }}
      />
      <div className="keymap-list" data-tt-role="list" data-tt-id="keymap.list">
        {[...byCategory.entries()].map(([cat, list]) => (
          <details key={cat} open={filter.trim() !== ''}>
            <summary>{cat}</summary>
            {list.map((c) => (
              <button
                key={c.id}
                type="button"
                aria-pressed={selected === c.id}
                data-tt-role="button"
                data-tt-id={`keymap.command.${c.id}`}
                onClick={() => {
                  setSelected(c.id);
                  setNotice(null);
                  setConflict(null);
                }}
              >
                {c.label}{' '}
                <span className="shortcuts">
                  {keysOf(c.id)
                    .map((b) => nativeNotation(b.keys, mac))
                    .join(', ')}
                </span>
              </button>
            ))}
          </details>
        ))}
      </div>
      {selected && (
        <div className="keymap-details" data-tt-role="group" data-tt-id="keymap.details">
          <h3>{INDEX[selected]?.label ?? selected}</h3>
          <p>{INDEX[selected]?.summary ?? ''}</p>
          <ul>
            {keysOf(selected).length === 0 && <li>none</li>}
            {keysOf(selected).map((b) => (
              <li key={`${b.context}:${b.keys}`}>
                {nativeNotation(b.keys, mac)} ({b.context}){' '}
                <button
                  type="button"
                  data-tt-role="button"
                  data-tt-id={`keymap.remove.${b.keys}`}
                  data-tt-command="keymap.remove"
                  onClick={() => {
                    onStage(
                      {
                        command: 'keymap.remove',
                        params: { commandId: selected, keys: b.keys, context: b.context },
                      },
                      bindings.filter((x) => x !== b),
                    );
                  }}
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
          <label>
            Context{' '}
            <select
              value={context}
              data-tt-role="field"
              data-tt-id="keymap.context"
              onChange={(e) => {
                setContext(e.target.value as 'global' | 'drawing' | 'staging');
              }}
            >
              <option value="global">global</option>
              <option value="drawing">drawing</option>
              <option value="staging">staging</option>
            </select>
          </label>
          <button
            type="button"
            className={capturing ? 'capture capturing' : 'capture'}
            data-tt-role="button"
            data-tt-id="keymap.capture"
            data-tt-command="keymap.set"
            onClick={() => {
              setCapturing(true);
              setNotice(null);
            }}
            onBlur={() => {
              setCapturing(false);
            }}
            onKeyDown={(e) => {
              if (!capturing) return;
              e.preventDefault();
              e.stopPropagation();
              if (e.key === 'Escape' && !e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
                setCapturing(false);
                return;
              }
              const seq = sequenceOf(e, mac);
              if (seq === null) return; // a bare modifier is not accepted; keep listening
              void captured(seq);
            }}
          >
            {capturing ? 'Press a shortcut…' : 'Add shortcut'}
          </button>
          <button
            type="button"
            data-tt-role="button"
            data-tt-id="keymap.reset"
            data-tt-command="keymap.reset"
            onClick={() => {
              onStage({ command: 'keymap.reset', params: { commandId: selected } }, bindings);
            }}
          >
            Reset
          </button>
          {notice && (
            <p className="field-problem" role="alert" data-tt-role="text" data-tt-id="keymap.notice">
              {notice}
            </p>
          )}
          {conflict && (
            <div className="keymap-conflict" role="alert" data-tt-role="group" data-tt-id="keymap.conflict">
              {nativeNotation(conflict.keys, mac)} is bound to{' '}
              {INDEX[conflict.with.commandId]?.label ?? conflict.with.commandId} (
              {conflict.with.commandId.split('.')[0]}, {conflict.with.context}).
              <button
                type="button"
                data-tt-role="button"
                data-tt-id="keymap.conflict.override"
                onClick={() => {
                  stageSet(conflict.keys, true);
                  setConflict(null);
                }}
              >
                Override
              </button>
              <button
                type="button"
                data-tt-role="button"
                data-tt-id="keymap.conflict.cancel"
                onClick={() => {
                  setConflict(null);
                }}
              >
                Cancel
              </button>
            </div>
          )}
        </div>
      )}
      <button
        type="button"
        data-tt-role="button"
        data-tt-id="keymap.resetAll"
        data-tt-command="keymap.reset"
        onClick={() => {
          if (window.confirm('Restore the whole default keymap?')) {
            onStage({ command: 'keymap.reset', params: { all: true } }, bindings);
          }
        }}
      >
        Reset all…
      </button>
    </div>
  );
}

function TipsList({ services, tips }: { services: ShellServices; tips: readonly Tip[] }) {
  return (
    <div className="tips-list" data-tt-role="list" data-tt-id="tips">
      <button
        type="button"
        data-tt-role="button"
        data-tt-id="tips.reset"
        data-tt-command="ui.tips.reset"
        onClick={() => void services.engine.run('ui.tips.reset', {}).done}
      >
        Show all tips again
      </button>
      {tips.map((t) => (
        <label key={t.messageId}>
          <input
            type="checkbox"
            checked={!t.suppressed}
            data-tt-role="field"
            data-tt-id={`tips.${t.messageId}`}
            data-tt-command="ui.tips.set"
            onChange={(e) =>
              void services.engine.run('ui.tips.set', {
                messageId: t.messageId,
                suppressed: !e.target.checked,
              }).done
            }
          />{' '}
          {t.description} <code>{t.messageId}</code>
        </label>
      ))}
    </div>
  );
}
