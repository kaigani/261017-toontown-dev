// The Timeline's gesture controller (reviews/M5-design-timeline-view.md section 3): the window computes nothing about the
// document. A press goes to the kernel's timing-gesture interpreter (`timing_gesture`), which answers what is under it;
// the window draws a local preview from that target and the pointer's travel; the release goes to the kernel, which
// commits or cancels (E-10, E-23, E-28). Escape, a lost capture, a blur or an engine change abort: nothing is sent but
// the abort (01 line 495). A ruler drag scrubs: the newest frame is sent while no call is outstanding, frames crossed
// meanwhile are dropped, never queued (E-11).
import type {
  CommandFailure,
  TimingGestureAnswer,
  TimingGestureRequest,
  TimingLayout,
  TimingSample,
} from '../ipc';
import { frameAt, LAYOUT } from './geometry';

/** What the controller needs from the host. */
export interface GestureHost {
  timingGesture(request: TimingGestureRequest): Promise<TimingGestureAnswer>;
  /** Runs a `ui` command (the scrub's `timeline.currentFrame.set`); resolves when it answered. */
  scrub(frame: number): Promise<void>;
}

/** What is under the press, as the kernel classified it (the interpreter's `Target`). */
export type Target = { readonly kind: string } & Readonly<Record<string, unknown>>;

/** The local preview between press and release. */
export interface Preview {
  readonly target: Target;
  readonly layout: TimingLayout;
  readonly press: TimingSample;
  readonly last: TimingSample;
  /** Frames travelled (the kernel's `frame_at(last) - frame_at(first)`). */
  readonly frames: number;
  /** Rows travelled. */
  readonly rows: number;
  /** Whether the travel passed the drag threshold. */
  readonly dragged: boolean;
}

/** What the controller tells the panel. */
export interface GestureEvents {
  preview(p: Preview | null): void;
  /** A refusal or a cancellation, for the one refusal path of the status line (D-87). */
  refusal(error: CommandFailure): void;
  /** The release committed: the envelopes' document revision, for the preview-until-drawn rule (D-53). */
  committed(rev: number | null): void;
  /** The release committed this plan (its commands and their envelopes, in order): what the panel can show at once. */
  applied?(
    commands: readonly (readonly [string, Record<string, unknown>])[],
    envelopes: readonly Record<string, unknown>[],
  ): void;
}

type Phase =
  | { readonly kind: 'idle' }
  | {
      readonly kind: 'pressed';
      readonly layout: TimingLayout;
      readonly samples: TimingSample[];
      grab: Promise<{ grabId: number; target: Target } | null>;
      target: Target | null;
      aborted: boolean;
    };

const asTarget = (v: unknown): Target | null =>
  typeof v === 'object' && v !== null && typeof (v as Record<string, unknown>)['kind'] === 'string'
    ? (v as Target)
    : null;

export class TimelineGesture {
  private phase: Phase = { kind: 'idle' };
  /** The scrub's state: a call outstanding, and the newest frame wanted meanwhile. */
  private scrubbing = false;
  private wanted: number | null = null;
  private sent: number | null = null;

  constructor(
    private readonly host: GestureHost,
    private readonly events: GestureEvents,
  ) {}

  /** Whether a gesture is in progress. */
  get active(): boolean {
    return this.phase.kind === 'pressed';
  }

  press(layout: TimingLayout, sample: TimingSample): void {
    if (this.phase.kind === 'pressed') this.abort();
    const samples = [sample];
    const grab = this.host.timingGesture({ phase: 'press', layout, sample }).then((a) => {
      if (!a.ok) {
        this.events.refusal(a.error);
        return null;
      }
      const target = asTarget(a.value['target']);
      const grabId = a.value['grabId'];
      return typeof grabId === 'number' && target ? { grabId, target } : null;
    });
    const phase: Phase = { kind: 'pressed', layout, samples, grab, target: null, aborted: false };
    this.phase = phase;
    this.sent = null;
    void grab.then((g) => {
      if (this.phase !== phase || phase.aborted || !g) return;
      phase.target = g.target;
      this.showPreview(phase);
      // A press in the ruler (not on a free range's marker) scrubs from the press on.
      const last = phase.samples[phase.samples.length - 1];
      if (g.target.kind === 'ruler' && last) this.wantFrame(frameAt(layout, last.x));
    });
  }

  move(sample: TimingSample): void {
    const phase = this.phase;
    if (phase.kind !== 'pressed') return;
    phase.samples.push(sample);
    this.showPreview(phase);
    if (phase.layout.surface === 'ruler' && phase.target?.kind === 'ruler')
      this.wantFrame(frameAt(phase.layout, sample.x));
  }

  async release(sample: TimingSample): Promise<void> {
    const phase = this.phase;
    if (phase.kind !== 'pressed') return;
    phase.samples.push(sample);
    this.phase = { kind: 'idle' };
    this.wanted = null;
    const g = await phase.grab;
    if (!g || phase.aborted) {
      this.events.preview(null);
      return;
    }
    const a = await this.host.timingGesture({ phase: 'release', grabId: g.grabId, samples: phase.samples });
    // A refusal or a cancellation clears the preview at once (the C4.3b rule).
    if (!a.ok) {
      this.events.preview(null);
      this.events.refusal(a.error);
      return;
    }
    if (a.value['cancelled'] === true) {
      this.events.preview(null);
      const e = a.value['error'] as CommandFailure | undefined;
      this.events.refusal(e ?? { code: 'timeline.targetChanged', message: 'the gesture was cancelled' });
      return;
    }
    const envelopes = Array.isArray(a.value['envelopes'])
      ? (a.value['envelopes'] as Record<string, unknown>[])
      : [];
    const failed = envelopes.find((e) => e['status'] !== 'ok');
    if (failed) {
      this.events.preview(null);
      const e = failed['error'] as CommandFailure | undefined;
      this.events.refusal(e ?? { code: 'internal', message: 'the gesture was refused' });
      return;
    }
    const plan = a.value['plan'] as { commands?: unknown } | undefined;
    const commands = Array.isArray(plan?.commands)
      ? (plan.commands as unknown[]).filter(
          (c): c is [string, Record<string, unknown>] =>
            Array.isArray(c) && typeof c[0] === 'string' && typeof c[1] === 'object' && c[1] !== null,
        )
      : [];
    this.events.applied?.(commands, envelopes);
    const last = envelopes[envelopes.length - 1];
    const rev = last && typeof last['documentRev'] === 'number' ? last['documentRev'] : null;
    // The preview holds until the panel has drawn a read at this revision (D-53): the panel clears it.
    this.events.committed(rev);
  }

  /** Escape, a lost capture, a blur, an engine change or the panel unmounting: the grab is dropped. */
  abort(): void {
    const phase = this.phase;
    if (phase.kind !== 'pressed') return;
    phase.aborted = true;
    this.phase = { kind: 'idle' };
    this.wanted = null;
    this.events.preview(null);
    void phase.grab.then((g) => {
      if (g) void this.host.timingGesture({ phase: 'abort', grabId: g.grabId });
    });
  }

  private showPreview(phase: Extract<Phase, { kind: 'pressed' }>): void {
    const target = phase.target;
    const press = phase.samples[0];
    const last = phase.samples[phase.samples.length - 1];
    if (!target || !press || !last) return;
    const travel = phase.samples.reduce(
      (m, s) => Math.max(m, Math.abs(s.x - press.x), Math.abs(s.y - press.y)),
      0,
    );
    const rh = LAYOUT.rowHeight;
    const row = (y: number) => Math.floor((y + phase.layout.scrollTop) / rh);
    this.events.preview({
      target,
      layout: phase.layout,
      press,
      last,
      frames: frameAt(phase.layout, last.x) - frameAt(phase.layout, press.x),
      rows: row(last.y) - row(press.y),
      dragged: travel >= LAYOUT.dragThreshold,
    });
  }

  /** The newest-only scrub (E-11): one call outstanding at most; the frame wanted when it answers is sent next. */
  private wantFrame(frame: number): void {
    this.wanted = frame;
    if (this.scrubbing) return;
    this.wanted = null;
    if (frame === this.sent) return;
    const next = frame;
    this.sent = next;
    this.scrubbing = true;
    void this.host
      .scrub(next)
      .catch(() => undefined)
      .then(() => {
        this.scrubbing = false;
        if (this.wanted !== null) this.wantFrame(this.wanted);
      });
  }
}
