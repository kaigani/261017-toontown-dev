//! Enablement rules (11 §2, §8): the text agents read and the check run before a handler. A check receives the
//! invocation context as `&dyn Any`, which downcasts only to `&Cx` (read access).

use std::any::Any;

use tt_registry::{Disabled, Enablement};

use crate::cx::Cx;

fn cx(any: &dyn Any) -> Result<&Cx, Disabled> {
    any.downcast_ref::<Cx>().ok_or_else(|| Disabled::because("no invocation context"))
}

fn can_undo(any: &dyn Any) -> Result<(), Disabled> {
    let cx = cx(any)?;
    if cx.in_atomic_batch() {
        return Err(Disabled::because("history commands cannot run inside an atomic batch"));
    }
    match cx.session().project.as_ref() {
        None => Err(Disabled::no_project()),
        Some(p) if p.history.can_undo() => Ok(()),
        Some(_) => {
            Err(Disabled { code: tt_registry::codes::NOTHING_TO_UNDO, reason: "there is nothing to undo".into() })
        }
    }
}

fn can_redo(any: &dyn Any) -> Result<(), Disabled> {
    let cx = cx(any)?;
    if cx.in_atomic_batch() {
        return Err(Disabled::because("history commands cannot run inside an atomic batch"));
    }
    match cx.session().project.as_ref() {
        None => Err(Disabled::no_project()),
        Some(p) if p.history.can_redo() => Ok(()),
        Some(_) => {
            Err(Disabled { code: tt_registry::codes::NOTHING_TO_REDO, reason: "there is nothing to redo".into() })
        }
    }
}

fn outside_atomic_batch(any: &dyn Any) -> Result<(), Disabled> {
    if cx(any)?.in_atomic_batch() {
        Err(Disabled::because("history commands cannot run inside an atomic batch"))
    } else {
        Ok(())
    }
}

fn has_checkpoints(any: &dyn Any) -> Result<(), Disabled> {
    let cx = cx(any)?;
    if cx.in_atomic_batch() {
        return Err(Disabled::because("history commands cannot run inside an atomic batch"));
    }
    match cx.session().project.as_ref() {
        Some(p) if !p.checkpoints.is_empty() => Ok(()),
        Some(_) => Err(Disabled::because("there are no checkpoints")),
        None => Err(Disabled::no_project()),
    }
}

fn has_scene(any: &dyn Any) -> Result<(), Disabled> {
    if cx(any)?.app().scene.is_some() { Ok(()) } else { Err(Disabled::because("the project has no current scene")) }
}

/// `edit.undo`: an undo step exists (`edit.nothingToUndo` otherwise).
pub const CAN_UNDO: Enablement =
    Enablement { rule: "a project is open and has a step to undo; not inside an atomic batch", check: Some(can_undo) };

/// `edit.redo`: a redo step exists (`edit.nothingToRedo` otherwise).
pub const CAN_REDO: Enablement =
    Enablement { rule: "a project is open and has a step to redo; not inside an atomic batch", check: Some(can_redo) };

/// History moves and checkpoint restores are their own transactions.
pub const OUTSIDE_ATOMIC_BATCH: Enablement =
    Enablement { rule: "a project is open; not inside an atomic batch", check: Some(outside_atomic_batch) };

/// `checkpoint.restore`.
pub const HAS_CHECKPOINTS: Enablement = Enablement {
    rule: "a project is open with at least one checkpoint; not inside an atomic batch",
    check: Some(has_checkpoints),
};

/// Frame and element navigation.
pub const HAS_SCENE: Enablement = Enablement { rule: "a project with a current scene is open", check: Some(has_scene) };
