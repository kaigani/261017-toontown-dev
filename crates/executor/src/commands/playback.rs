//! Interactive playback (09 §2 without sound and capture; M5 C5.1a, ruling E-1; `reviews/M5-design-timeline-kernel.md`
//! §2.10): `playback.start`, `.stop`, `.state.get`, `.loop.set`, `.rate.set`, `.forceRate.set`, the `playback.range.*`
//! commands (one state with `timeline.playbackRange.*`, SI-202) and their events.
//!
//! **The clock.** One ticker thread per executor ([`clock_thread`]) posts `Request::PlaybackTick` to the document
//! thread at the effective rate while a run is active; a run is started and stopped by deferred effects, so a dry run or
//! a refused call starts nothing. Every tick carries the run's generation and the time since the run began; the document
//! thread applies it through [`advance`], a pure function of the playback state, the range and that time, so the rules
//! are tested deterministically with simulated times (no CI assertion reads a wall clock, D-45). A tick of an earlier
//! generation stops its clock and changes nothing.

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tt_model::Id;
use tt_registry::{CommandError, codes, command};

use super::common::NoParams;
use super::timeline::{RangeMode, RangeResult, current, effective_range, set_range};
use super::timing::{err, scene_id};
use crate::cx::{Cx, Effect};
use crate::rules;

/// The playback rate setting (09 §2.6).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) enum Rate {
    /// The project frame rate.
    #[default]
    Project,
    /// A fixed rate, 1 to 120 frames per second.
    Fps(f64),
}

/// One run of the clock (from a start to a stop).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Run {
    /// The frame the run started at.
    pub start_frame: u32,
    /// The run's time when the last tick was applied, in milliseconds.
    pub elapsed_ms: f64,
    /// Frames shown.
    pub shown: u64,
    /// Frames skipped to keep real time (Force Frame Rate).
    pub dropped: u64,
    /// The run time of the last `timeline.currentFrame.changed` with source `playback` (throttled to ten per second).
    pub last_event_ms: Option<f64>,
    /// A played frame has not been announced yet (the next announcement or the stop carries it).
    pub event_pending: bool,
}

/// Playback state of the session.
#[derive(Debug, Clone, Default)]
pub(crate) struct Playback {
    pub playing: bool,
    pub scene: Option<Id>,
    /// Bumped by every start and stop: a tick of another generation is stale.
    pub generation: u64,
    pub loop_on: bool,
    pub force_rate: bool,
    pub rate: Rate,
    pub run: Run,
    /// The last frame the window reported presented (frame-based playback waits for it).
    pub presented: Option<u32>,
    /// The clock's control channel (set when the executor starts its clock).
    pub clock: Option<Sender<ClockControl>>,
}

/// A command to the clock thread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ClockControl {
    /// Tick generation `generation` every `period` milliseconds.
    Start { generation: u64, period_ms: f64 },
    /// Stop ticking generation `generation`.
    Stop { generation: u64 },
}

/// The effective playback rate in frames per second.
pub(crate) fn effective_fps(rate: Rate, project_fps: f64) -> f64 {
    let fps = match rate {
        Rate::Project => project_fps,
        Rate::Fps(f) => f,
    };
    if fps.is_finite() { fps.clamp(1.0, 120.0) } else { 24.0 }
}

/// The options and time of one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tick {
    /// Loop on.
    pub loop_on: bool,
    /// Force Frame Rate on (time-based).
    pub force_rate: bool,
    /// The effective rate.
    pub fps: f64,
    /// The run's time now, in milliseconds.
    pub elapsed_ms: f64,
    /// The window has not presented the frame shown yet.
    pub waiting: bool,
}

/// What one tick does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Step {
    /// The frame to show (`None`: hold the present frame).
    pub frame: Option<u32>,
    /// Frames skipped by this step.
    pub dropped: u64,
    /// Playback ends on this step (Loop off, at the range end; 09 line 115).
    pub ends: bool,
}

/// The rule of one tick (09 §2.3, §2.7): pure, so it is tested with simulated times.
///
/// - Force Frame Rate on (time-based): the frame is the run's start frame advanced by the frames the elapsed time covers
///   at `fps`; frames passed over are counted as dropped.
/// - Off (frame-based): one frame per tick; while `waiting` (the window has not presented the shown frame) the frame is
///   held, so playback runs slower than the rate (09 line 170).
/// - At the range end: Loop on wraps to the start; off, playback stops on the last frame. Frames outside the range are
///   brought to its start.
pub(crate) fn advance(now: u32, range: (u32, u32), run: &Run, tick: Tick) -> Step {
    let Tick { loop_on, force_rate, fps, elapsed_ms, waiting } = tick;
    let (lo, hi) = (range.0.min(range.1), range.1.max(range.0));
    let n = u64::from(hi - lo + 1);
    if !(lo..=hi).contains(&now) {
        return Step { frame: Some(lo), dropped: 0, ends: false };
    }
    if force_rate {
        let index = |ms: f64| -> u64 {
            let frames = (ms.max(0.0) * fps / 1000.0).floor();
            if frames.is_finite() && frames >= 0.0 { frames as u64 } else { 0 }
        };
        let base = u64::from(run.start_frame.clamp(lo, hi) - lo);
        let before = base + index(run.elapsed_ms);
        let target = base + index(elapsed_ms);
        if target <= before {
            return Step { frame: None, dropped: 0, ends: false };
        }
        let dropped = target - before - 1;
        if loop_on {
            let f = lo + u32::try_from(target % n).unwrap_or(0);
            return Step { frame: Some(f), dropped, ends: false };
        }
        if target >= n {
            let ends = now == hi || target >= n;
            return Step { frame: Some(hi), dropped: dropped.min(n.saturating_sub(1)), ends };
        }
        return Step { frame: Some(lo + u32::try_from(target).unwrap_or(0)), dropped, ends: false };
    }
    if waiting {
        return Step { frame: None, dropped: 0, ends: false };
    }
    if now >= hi {
        return if loop_on {
            Step { frame: Some(lo), dropped: 0, ends: false }
        } else {
            Step { frame: None, dropped: 0, ends: true }
        };
    }
    Step { frame: Some(now + 1), dropped: 0, ends: false }
}

/// The clock thread: ticks the active generation at its period, posting each tick with the run's time.
pub(crate) fn clock_thread(control: &Receiver<ClockControl>, tick: &dyn Fn(u64, f64) -> bool) {
    let mut active: Option<(u64, f64, Instant, u64)> = None;
    loop {
        let wait = match active {
            None => None,
            Some((_, period, began, n)) => {
                let due = began + Duration::from_secs_f64(period * (n + 1) as f64 / 1000.0);
                Some(due.saturating_duration_since(Instant::now()))
            }
        };
        let msg = match wait {
            None => control.recv().ok(),
            Some(w) => match control.recv_timeout(w) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => {
                    if let Some((generation, period, began, n)) = active.as_mut() {
                        *n += 1;
                        let ms = began.elapsed().as_secs_f64() * 1000.0;
                        let _ = period;
                        if !tick(*generation, ms) {
                            return;
                        }
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => return,
            },
        };
        match msg {
            None => return,
            Some(ClockControl::Start { generation, period_ms }) => {
                active = Some((generation, period_ms.max(1.0), Instant::now(), 0));
            }
            Some(ClockControl::Stop { generation }) => {
                if active.is_some_and(|(g, ..)| g == generation) {
                    active = None;
                }
            }
        }
    }
}

/// The project frame rate.
fn project_fps(cx: &Cx) -> f64 {
    cx.document().map_or(24.0, |d| *d.project().frame_rate())
}

/// Starts a run of the clock at the session's present rate (a new generation).
fn start_run(cx: &mut Cx, frame: u32) {
    let fps = effective_fps(cx.session().timeline.playback.rate, project_fps(cx));
    let pb = &mut cx.session_mut().timeline.playback;
    pb.generation = pb.generation.wrapping_add(1);
    pb.run = Run { start_frame: frame, ..Run::default() };
    pb.presented = None;
    let generation = pb.generation;
    cx.defer(Effect::Playback(ClockControl::Start { generation, period_ms: 1000.0 / fps }));
}

/// The playback rate saved at the last run (SI-209): `"project"` or a number of frames per second.
pub(crate) fn rate_of(v: Option<&Value>) -> Rate {
    match v.and_then(Value::as_f64) {
        Some(f) if f.is_finite() && (1.0..=120.0).contains(&f) => Rate::Fps(f),
        _ => Rate::Project,
    }
}

/// One tick of the clock on the document thread (the run's generation already checked by the caller).
pub(crate) fn on_tick(s: &mut crate::session::Session, ms: f64) {
    let Some(project) = &s.project else { return };
    let Some(scene) = s.app.scene.clone() else { return };
    let len = super::timeline::length_of(&project.doc, &scene);
    let project_fps = *project.doc.project().frame_rate();
    let (_, lo, hi) = effective_range(&s.timeline.ranges, &scene, len);
    let pb = &s.timeline.playback;
    let fps = effective_fps(pb.rate, project_fps);
    // Frame-based playback waits for the window to present the frame shown, once the window reports presentations.
    let waiting = !pb.force_rate && pb.presented.is_some() && pb.presented != Some(s.app.frame);
    let step = advance(
        s.app.frame,
        (lo, hi),
        &pb.run,
        Tick { loop_on: pb.loop_on, force_rate: pb.force_rate, fps, elapsed_ms: ms, waiting },
    );
    s.timeline.playback.run.elapsed_ms = ms;
    if let Some(f) = step.frame {
        if s.app.frame != f {
            s.app.frame = f;
            s.app.drawing = None;
        }
        s.timeline.playback.run.shown += 1;
        s.timeline.playback.run.dropped += step.dropped;
        s.timeline.frame_source = Some("playback");
        s.emit("shell.playbackFrame", json!({ "sceneId": scene.as_str(), "frame": f, "playing": true }));
    }
    if step.ends {
        let frame = s.app.frame;
        stopped(&mut s.timeline.playback);
        if std::mem::take(&mut s.timeline.playback.run.event_pending) {
            s.emit(
                "timeline.currentFrame.changed",
                json!({ "sceneId": scene.as_str(), "frame": frame, "source": "playback" }),
            );
        }
        s.emit("shell.playbackFrame", json!({ "sceneId": scene.as_str(), "frame": frame, "playing": false }));
        s.emit("playback.stopped", json!({ "sceneId": scene.as_str(), "frame": frame, "reason": "end" }));
    }
}

/// The Timeline's part of `Core::settle` (M5 C5.1a): playback stops on a scene change or a frame navigation (09 lines
/// 138, 210); `timeline.currentFrame.changed` follows every change of the current frame with its source, throttled to
/// ten per second during playback (04 line 1466); the playback-range events follow every change of the current scene's
/// effective range (SI-202); the host-only `shell.timelineView` follows the Timeline's view state (SI-210).
pub(crate) fn settle_timeline(
    s: &mut crate::session::Session,
    scene_before: Option<&Id>,
    frame_before: u32,
    range_before: Option<(RangeMode, u32, u32)>,
    view_before: &Value,
) {
    let source = s.timeline.frame_source.take().unwrap_or("command");
    let scene = s.app.scene.clone();
    let frame = s.app.frame;
    let scene_changed = scene.as_ref() != scene_before;
    let frame_changed = frame != frame_before || scene_changed;
    if s.timeline.playback.playing && (scene_changed || (frame_changed && source != "playback") || s.project.is_none())
    {
        let reason = if scene_changed || s.project.is_none() { "sceneChanged" } else { "navigation" };
        let stopped_scene = s.timeline.playback.scene.clone().or_else(|| scene.clone());
        stopped(&mut s.timeline.playback);
        s.timeline.playback.run.event_pending = false;
        if let Some(sc) = &stopped_scene {
            s.emit("shell.playbackFrame", json!({ "sceneId": sc.as_str(), "frame": frame, "playing": false }));
        }
        s.emit(
            "playback.stopped",
            json!({ "sceneId": stopped_scene.as_ref().map(Id::as_str), "frame": frame, "reason": reason }),
        );
    }
    if frame_changed
        && s.project.is_some()
        && let Some(sc) = &scene
    {
        let playing = s.timeline.playback.playing;
        let run = &mut s.timeline.playback.run;
        let announce = if source == "playback" && playing {
            let due = run.last_event_ms.is_none_or(|t| run.elapsed_ms - t >= 100.0);
            if due {
                run.last_event_ms = Some(run.elapsed_ms);
                run.event_pending = false;
            } else {
                run.event_pending = true;
            }
            due
        } else {
            true
        };
        if announce {
            s.emit(
                "timeline.currentFrame.changed",
                json!({ "sceneId": sc.as_str(), "frame": frame, "source": source }),
            );
        }
    }
    let range = match (&s.project, &scene) {
        (Some(p), Some(sc)) => Some(effective_range(&s.timeline.ranges, sc, super::timeline::length_of(&p.doc, sc))),
        _ => None,
    };
    if range != range_before
        && !scene_changed
        && let (Some((mode, start, end)), Some(sc)) = (range, &scene)
    {
        s.emit("timeline.playbackRange.changed", json!({ "mode": mode, "start": start, "end": end }));
        s.emit("playback.range.changed", json!({ "sceneId": sc.as_str(), "mode": mode, "start": start, "end": end }));
    }
    let view = super::timeline::view_notice(&s.timeline, scene.as_ref());
    if &view != view_before {
        s.emit("shell.timelineView", view);
    }
}

// ----- commands -------------------------------------------------------------------------------------------------------

/// `playback.start` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct StartParams {
    /// The scene (the current scene only).
    #[serde(default)]
    scene_id: Option<String>,
    /// Start here instead of the current frame.
    #[serde(default)]
    #[schemars(range(min = 1))]
    from_frame: Option<i64>,
}

/// `{playing, sceneId, frame}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartResult {
    playing: bool,
    scene_id: String,
    frame: u32,
}

#[command(
    id = "playback.start",
    kind = ui,
    label = "Play",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Play the current scene", params = "{}"),
    gui = "Play menu and the main toolbar: Play/Stop (M5 C5.1)",
)]
pub(crate) fn playback_start(cx: &mut Cx, p: StartParams) -> Result<StartResult, CommandError> {
    let (scene, len) = current(cx)?;
    if let Some(r) = &p.scene_id
        && scene_id(cx, Some(r))? != scene
    {
        return Err(err(
            codes::ARG_INVALID,
            "interactive playback plays the current scene; make that scene current first (08)",
            json!({ "pointer": "/sceneId", "reason": "notCurrentScene" }),
        ));
    }
    if cx.session().timeline.playback.playing {
        return Ok(StartResult { playing: true, scene_id: scene.as_str().to_owned(), frame: cx.app().frame });
    }
    let (_, lo, hi) = effective_range(&cx.session().timeline.ranges, &scene, len);
    let wanted = p.from_frame.map_or(cx.app().frame, |f| u32::try_from(f.clamp(1, i64::from(len))).unwrap_or(1));
    let frame = if (lo..=hi).contains(&wanted) { wanted } else { lo };
    super::timeline::frame_source(cx, "playback");
    super::timeline::put_frame(cx, frame);
    {
        let pb = &mut cx.session_mut().timeline.playback;
        pb.playing = true;
        pb.scene = Some(scene.clone());
    }
    start_run(cx, frame);
    cx.emit("playback.started", json!({ "sceneId": scene.as_str(), "frame": frame, "mode": "scene" }));
    Ok(StartResult { playing: true, scene_id: scene.as_str().to_owned(), frame })
}

/// `{frame}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct StopResult {
    frame: u32,
}

#[command(
    id = "playback.stop",
    kind = ui,
    label = "Stop",
    scopes = [ui],
    needs_project,
    example(title = "Stop playback", params = "{}"),
    gui = "Play menu and the main toolbar: Play/Stop (M5 C5.1)",
)]
pub(crate) fn playback_stop(cx: &mut Cx, _p: NoParams) -> Result<StopResult, CommandError> {
    let frame = cx.app().frame;
    if cx.session().timeline.playback.playing {
        let generation = cx.session().timeline.playback.generation;
        stopped(&mut cx.session_mut().timeline.playback);
        cx.defer(Effect::Playback(ClockControl::Stop { generation }));
        let scene = cx.app().scene.as_ref().map(|s| s.as_str().to_owned());
        let pending = std::mem::take(&mut cx.session_mut().timeline.playback.run.event_pending);
        if pending && let Some(s) = &scene {
            cx.emit("timeline.currentFrame.changed", json!({ "sceneId": s, "frame": frame, "source": "playback" }));
        }
        cx.emit("playback.stopped", json!({ "sceneId": scene, "frame": frame, "reason": "user" }));
    }
    Ok(StopResult { frame })
}

/// Marks playback stopped (a new generation: the clock's late ticks are stale).
pub(crate) fn stopped(pb: &mut Playback) {
    pb.playing = false;
    pb.generation = pb.generation.wrapping_add(1);
}

/// `PlaybackState` (09 §11.4).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaybackState {
    playing: bool,
    /// `scene` (storyline playback is 08's).
    mode: String,
    scene_id: Option<String>,
    frame: u32,
    range: Value,
    rate: Value,
    #[serde(rename = "loop")]
    loop_on: bool,
    force_rate: bool,
    /// Sound during playback (M8): off until then.
    sound: bool,
    /// Sound scrubbing (M8): off until then.
    scrubbing: bool,
    measured_fps: f64,
    dropped_frames: u64,
}

#[command(
    id = "playback.state.get",
    kind = query,
    label = "Get playback state",
    scopes = [read],
    example(title = "Playback state and options", params = "{}"),
    gui = "Play menu and the main toolbar: Play/Stop's state, Loop, Playback Rate, Force Frame Rate (M5 C5.1)",
)]
pub(crate) fn playback_state_get(cx: &mut Cx, _p: NoParams) -> Result<PlaybackState, CommandError> {
    let pb = cx.session().timeline.playback.clone();
    let fps = effective_fps(pb.rate, project_fps(cx));
    let range = match current(cx) {
        Ok((scene, len)) => {
            let (mode, start, end) = effective_range(&cx.session().timeline.ranges, &scene, len);
            json!({ "mode": mode, "start": start, "end": end, "sceneLength": len })
        }
        Err(_) => Value::Null,
    };
    let setting = match pb.rate {
        Rate::Project => json!("project"),
        Rate::Fps(f) => json!(f),
    };
    let measured =
        if pb.playing && pb.run.elapsed_ms > 0.0 { (pb.run.shown as f64) * 1000.0 / pb.run.elapsed_ms } else { 0.0 };
    Ok(PlaybackState {
        playing: pb.playing,
        mode: "scene".to_owned(),
        scene_id: cx.app().scene.as_ref().map(|s| s.as_str().to_owned()),
        frame: cx.app().frame,
        range,
        rate: json!({ "setting": setting, "fps": fps }),
        loop_on: pb.loop_on,
        force_rate: pb.force_rate,
        sound: false,
        scrubbing: false,
        measured_fps: (measured * 10.0).round() / 10.0,
        dropped_frames: pb.run.dropped,
    })
}

#[command(
    id = "playback.loop.set",
    kind = toggle,
    label = "Loop",
    scopes = [ui],
    example(title = "Loop playback", params = r#"{"enabled": true}"#),
    gui = "Play menu and the main toolbar: Loop (M5 C5.1)",
)]
pub(crate) fn playback_loop_set(
    cx: &mut Cx,
    p: super::timeline::Enabled,
) -> Result<super::timeline::Enabled, CommandError> {
    cx.session_mut().timeline.playback.loop_on = p.enabled;
    Ok(p)
}

#[command(
    id = "playback.forceRate.set",
    kind = toggle,
    label = "Force frame rate",
    scopes = [ui],
    example(title = "Drop frames to keep real time", params = r#"{"enabled": true}"#),
    gui = "Play menu: Force Frame Rate (M5 C5.1)",
)]
pub(crate) fn playback_force_rate_set(
    cx: &mut Cx,
    p: super::timeline::Enabled,
) -> Result<super::timeline::Enabled, CommandError> {
    let changed = cx.session().timeline.playback.force_rate != p.enabled;
    cx.session_mut().timeline.playback.force_rate = p.enabled;
    if changed && cx.session().timeline.playback.playing {
        let frame = cx.app().frame;
        start_run(cx, frame);
    }
    Ok(p)
}

/// A rate: `"project"` or 1 to 120 frames per second.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum RateParam {
    /// Frames per second.
    Fps(#[schemars(range(min = 1, max = 120))] f64),
    /// `"project"`: the project frame rate.
    Named(ProjectRate),
}

/// The literal `"project"`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProjectRate {
    /// The project frame rate.
    Project,
}

fn project_rate() -> RateParam {
    RateParam::Named(ProjectRate::Project)
}

/// `playback.rate.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RateParams {
    /// Frames per second, or `"project"`.
    #[serde(default = "project_rate")]
    fps: RateParam,
}

/// `{fps, effectiveFps}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateResult {
    fps: RateParam,
    effective_fps: f64,
}

/// The user settings key of the playback rate (SI-209).
pub(crate) const RATE_SETTING: &str = "playbackRate";

#[command(
    id = "playback.rate.set",
    kind = ui,
    label = "Playback rate",
    scopes = [ui],
    example(title = "Play at 12 frames per second", params = r#"{"fps": 12}"#),
    example(title = "Use the project frame rate", params = r#"{"fps": "project"}"#),
    gui = "Play menu: Playback Rate (M5 C5.1)",
)]
pub(crate) fn playback_rate_set(cx: &mut Cx, p: RateParams) -> Result<RateResult, CommandError> {
    let rate = match p.fps {
        RateParam::Fps(f) if f.is_finite() && (1.0..=120.0).contains(&f) => Rate::Fps(f),
        RateParam::Fps(f) => {
            return Err(err(
                codes::ARG_INVALID,
                format!("{f} frames per second is outside 1 to 120"),
                json!({ "pointer": "/fps", "allowed": { "minimum": 1, "maximum": 120 } }),
            ));
        }
        RateParam::Named(_) => Rate::Project,
    };
    let changed = cx.session().timeline.playback.rate != rate;
    cx.session_mut().timeline.playback.rate = rate;
    let stored = match rate {
        Rate::Project => json!("project"),
        Rate::Fps(f) => json!(f),
    };
    cx.defer(Effect::Settings(crate::cx::SettingsChange::LastUsed(RATE_SETTING.to_owned(), stored)));
    if changed && cx.session().timeline.playback.playing {
        let frame = cx.app().frame;
        start_run(cx, frame);
    }
    Ok(RateResult { fps: p.fps, effective_fps: effective_fps(rate, project_fps(cx)) })
}

// ----- playback.range.* --------------------------------------------------------------------------------------------------

/// `playback.range.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RangeGetParams {
    /// The scene (default the current scene).
    #[serde(default)]
    scene_id: Option<String>,
}

/// `{mode, start, end, sceneLength}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RangeGetResult {
    mode: RangeMode,
    start: u32,
    end: u32,
    scene_length: f64,
}

fn scene_and_length(cx: &Cx, r: Option<&str>) -> Result<(Id, u32), CommandError> {
    let scene = scene_id(cx, r)?;
    let len = super::timeline::length_of(cx.document()?, &scene);
    Ok((scene, len))
}

#[command(
    id = "playback.range.get",
    kind = query,
    label = "Get playback range",
    scopes = [read],
    needs_project,
    example(title = "The current scene's playback range", params = "{}"),
    agent_only = "agents read a scene's range with its length (09 §11.2); the Timeline ruler reads timeline.playbackRange.get, the same state (SI-202)",
)]
pub(crate) fn playback_range_get(cx: &mut Cx, p: RangeGetParams) -> Result<RangeGetResult, CommandError> {
    let (scene, len) = scene_and_length(cx, p.scene_id.as_deref())?;
    let (mode, start, end) = effective_range(&cx.session().timeline.ranges, &scene, len);
    Ok(RangeGetResult { mode, start, end, scene_length: f64::from(len) })
}

/// `playback.range.mode.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RangeModeParams {
    /// The scene (default the current scene).
    #[serde(default)]
    scene_id: Option<String>,
    /// Automatic fit or free.
    mode: RangeMode,
}

#[command(
    id = "playback.range.mode.set",
    kind = action,
    label = "Playback range mode",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Free range (restores the last free values)", params = r#"{"mode": "free"}"#),
    gui = "Play menu: Playback Range > Automatic Fit / Free (M5 C5.1)",
)]
pub(crate) fn playback_range_mode_set(cx: &mut Cx, p: RangeModeParams) -> Result<RangeResult, CommandError> {
    let (scene, len) = scene_and_length(cx, p.scene_id.as_deref())?;
    let r = set_range(cx, &scene, len, Some(p.mode), None, None);
    Ok(RangeResult { mode: r.mode, start: r.start, end: r.end })
}

/// `playback.range.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct RangeSetParams {
    /// The scene (default the current scene).
    #[serde(default)]
    scene_id: Option<String>,
    /// Free start.
    #[serde(default)]
    start: Option<i64>,
    /// Free end.
    #[serde(default)]
    end: Option<i64>,
}

/// `{mode, start, end, adjusted}`.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct RangeSetResult {
    mode: RangeMode,
    start: u32,
    end: u32,
    adjusted: bool,
}

#[command(
    id = "playback.range.set",
    kind = action,
    label = "Set playback range",
    scopes = [ui],
    needs_project,
    enablement = rules::HAS_SCENE,
    example(title = "Play frames 20 to 30", params = r#"{"start": 20, "end": 30}"#),
    gui = "Play menu: Change Playback Start / End Frame (M5 C5.1)",
)]
pub(crate) fn playback_range_set(cx: &mut Cx, p: RangeSetParams) -> Result<RangeSetResult, CommandError> {
    let (scene, len) = scene_and_length(cx, p.scene_id.as_deref())?;
    let r = set_range(cx, &scene, len, Some(RangeMode::Free), p.start, p.end);
    Ok(RangeSetResult { mode: r.mode, start: r.start, end: r.end, adjusted: r.adjusted })
}

// ----- events -------------------------------------------------------------------------------------------------------------

/// `playback.started` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Started {
    scene_id: String,
    frame: f64,
    mode: String,
}

#[command(
    id = "playback.started",
    kind = event,
    label = "Playback started",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn playback_started(_cx: &mut Cx, _p: NoParams) -> Result<Started, CommandError> {
    Err(super::common::event_not_invocable("playback.started"))
}

/// `playback.stopped` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Stopped {
    scene_id: String,
    frame: f64,
    /// `user`, `end`, `navigation` or `sceneChanged`.
    reason: String,
}

#[command(
    id = "playback.stopped",
    kind = event,
    label = "Playback stopped",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn playback_stopped(_cx: &mut Cx, _p: NoParams) -> Result<Stopped, CommandError> {
    Err(super::common::event_not_invocable("playback.stopped"))
}

/// `playback.range.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RangeChanged {
    scene_id: String,
    mode: RangeMode,
    start: u32,
    end: u32,
}

#[command(
    id = "playback.range.changed",
    kind = event,
    label = "Playback range changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn playback_range_changed(_cx: &mut Cx, _p: NoParams) -> Result<RangeChanged, CommandError> {
    Err(super::common::event_not_invocable("playback.range.changed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(loop_on: bool, force_rate: bool, fps: f64, elapsed_ms: f64, waiting: bool) -> Tick {
        Tick { loop_on, force_rate, fps, elapsed_ms, waiting }
    }

    fn run(start: u32, elapsed: f64) -> Run {
        Run { start_frame: start, elapsed_ms: elapsed, ..Run::default() }
    }

    #[test]
    fn frame_based_steps_one_frame_and_holds_while_waiting() {
        let r = run(1, 0.0);
        assert_eq!(advance(3, (1, 10), &r, t(false, false, 24.0, 41.7, false)).frame, Some(4));
        assert_eq!(
            advance(3, (1, 10), &r, t(false, false, 24.0, 41.7, true)).frame,
            None,
            "held: playback runs slower"
        );
    }

    #[test]
    fn the_range_end_stops_or_wraps() {
        let r = run(1, 0.0);
        let end = advance(10, (1, 10), &r, t(false, false, 24.0, 0.0, false));
        assert_eq!((end.frame, end.ends), (None, true), "stops on the last frame (09 line 115)");
        let wrap = advance(10, (1, 10), &r, t(true, false, 24.0, 0.0, false));
        assert_eq!((wrap.frame, wrap.ends), (Some(1), false), "wraps with Loop on (09 line 114)");
        let one = advance(5, (5, 5), &r, t(true, false, 24.0, 0.0, false));
        assert_eq!(one.frame, Some(5), "a one-frame range repeats its frame (09 line 209)");
        assert_eq!(
            advance(12, (1, 10), &r, t(false, false, 24.0, 0.0, false)).frame,
            Some(1),
            "outside: the range start"
        );
    }

    #[test]
    fn time_based_skips_frames_to_keep_real_time() {
        // 24 fps: 125 ms covers 3 frames; the last tick was at 41.7 ms (frame index 1): two frames passed over.
        let r = run(1, 41.7);
        let s = advance(2, (1, 100), &r, t(false, true, 24.0, 125.0, false));
        assert_eq!((s.frame, s.dropped), (Some(4), 1));
        // No new frame due yet: hold.
        assert_eq!(advance(2, (1, 100), &run(1, 41.7), t(false, true, 24.0, 50.0, false)).frame, None);
        // Past the end without Loop: the last frame, and playback ends.
        let end = advance(98, (1, 100), &run(1, 4000.0), t(false, true, 24.0, 5000.0, false));
        assert_eq!((end.frame, end.ends), (Some(100), true));
        // With Loop: wraps by the elapsed frames (24 fps, 5 s = 120 frames into a 100-frame range: index 20, frame 21).
        let wrap = advance(98, (1, 100), &run(1, 4000.0), t(true, true, 24.0, 5000.0, false));
        assert_eq!(wrap.frame, Some(21));
    }

    #[test]
    fn rates_are_clamped_to_the_spec_range() {
        assert!((effective_fps(Rate::Project, 24.0) - 24.0).abs() < 1e-9);
        assert!((effective_fps(Rate::Fps(12.0), 24.0) - 12.0).abs() < 1e-9);
        assert!((effective_fps(Rate::Project, 500.0) - 120.0).abs() < 1e-9);
    }
}
