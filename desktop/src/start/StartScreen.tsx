// The start screen (01 §2; M4 C4.1): the new-project form, recent projects (missing ones marked), open other,
// recoverable work with Recover/Discard, the sample project and learning resources (their content is authored
// later; the owners are shown), and "Show at start-up". Escape closes it, leaving no project open.
import { useActions } from '../shell/actionsApi';
import { NewProjectForm } from '../shell/projectForms';
import { useSlice } from '../state/context';
import type { PrefEntry, Preset, RecentEntry, RecoveryEntry } from '../state/types';

export function StartScreen() {
  const actions = useActions();
  const presets = (useSlice('presets') as readonly Preset[] | undefined) ?? [];
  const recent = (useSlice('recent') as readonly RecentEntry[] | undefined) ?? [];
  const recovery = (useSlice('recovery') as readonly RecoveryEntry[] | undefined) ?? [];
  const prefs = (useSlice('prefs') as readonly PrefEntry[] | undefined) ?? [];
  const showAtStart = prefs.find((p) => p.key === 'general.startup.behaviour')?.value === 'startScreen';
  return (
    <div
      className="start-screen"
      role="dialog"
      aria-label="Start screen"
      data-tt-role="startScreen"
      data-tt-id="startScreen"
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === 'Escape') void actions.run('ui.startScreen.set', { visible: false });
      }}
    >
      <section className="start-new" aria-label="New project" data-tt-role="group" data-tt-id="start.new">
        <h2>New project</h2>
        <NewProjectForm
          presets={presets}
          idPrefix="start"
          onCreate={(p) => void actions.perform('project.create', p)}
        />
      </section>
      <section
        className="start-recent"
        aria-label="Recent projects"
        data-tt-role="list"
        data-tt-id="start.recent"
      >
        <h2>Recent projects</h2>
        {recent.length === 0 && <p className="empty">No recent projects.</p>}
        <ul>
          {recent.map((r) => (
            <li key={r.path}>
              <button
                type="button"
                title={r.path}
                className={r.exists ? 'recent' : 'recent missing'}
                data-tt-role="button"
                data-tt-id={`start.recent.${r.path}`}
                data-tt-label={r.name}
                data-tt-command="project.open"
                data-tt-params={JSON.stringify({ path: r.path })}
                onClick={() => void actions.perform('project.open', { path: r.path })}
              >
                {r.name}
                {!r.exists && <span className="missing-mark"> (missing)</span>}
              </button>
            </li>
          ))}
        </ul>
        <button
          type="button"
          data-tt-role="button"
          data-tt-id="start.openOther"
          data-tt-command="project.open"
          onClick={() => void actions.perform('project.open')}
        >
          Open other…
        </button>
      </section>
      {recovery.length > 0 && (
        <section
          className="start-recovery"
          aria-label="Recoverable work"
          data-tt-role="list"
          data-tt-id="start.recovery"
        >
          <h2>Recoverable work</h2>
          <ul>
            {recovery.map((r) => (
              <li key={r.recoveryId}>
                {r.projectName} <time>{r.savedAt}</time>{' '}
                <button
                  type="button"
                  data-tt-role="button"
                  data-tt-id={`start.recover.${r.recoveryId}`}
                  data-tt-command="project.recovery.open"
                  onClick={() => void actions.perform('project.recovery.open', { recoveryId: r.recoveryId })}
                >
                  Recover
                </button>
                <button
                  type="button"
                  data-tt-role="button"
                  data-tt-id={`start.discard.${r.recoveryId}`}
                  data-tt-command="project.recovery.delete"
                  onClick={() => void actions.run('project.recovery.delete', { recoveryId: r.recoveryId })}
                >
                  Discard
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
      <section className="start-learn" aria-label="Learning" data-tt-role="group" data-tt-id="start.learn">
        <h2>Learn</h2>
        <button
          type="button"
          disabled
          title="The sample project is authored with the starter content (M9 C9.3)"
          data-tt-role="button"
          data-tt-id="start.sample"
        >
          Sample project
        </button>
        <button
          type="button"
          data-tt-role="button"
          data-tt-id="start.docs"
          data-tt-command="ui.help.open"
          onClick={() => void actions.run('ui.help.open', {})}
        >
          Documentation
        </button>
        <p className="owner-note">Tutorials are authored with the starter content (M9 C9.3).</p>
      </section>
      <label className="start-show" data-tt-role="group" data-tt-id="start.showAtStartup">
        <input
          type="checkbox"
          checked={showAtStart}
          data-tt-role="field"
          data-tt-id="start.showAtStartup.box"
          onChange={(e) =>
            // 01 §2: unchecking sets newDefaultProject, checking sets startScreen.
            void actions.run('prefs.set', {
              values: { 'general.startup.behaviour': e.target.checked ? 'startScreen' : 'newDefaultProject' },
            })
          }
        />{' '}
        Show at start-up
      </label>
    </div>
  );
}
