//! Owning milestone of each spec A command that has no typed declaration yet (plan §4).
//!
//! The mapping follows the plan's milestone table: a namespace belongs to the milestone that delivers its section's
//! commands, with the exceptions the plan names explicitly (C2.3 delivers `colour.paint.*` with `draw.*`; C3.2
//! delivers `anim.evaluate`; C3.3 delivers the deterministic `render.frame.run`, `render.limits.get` and the
//! inspection/render-backed `inspect.*` queries; C4.2 delivers `inspect.ui.tree` / `inspect.screenshot`). M1
//! commands declared in `tt-executor` report their own owning checkpoint instead.

/// The milestone (and checkpoint, where the plan names one) that implements a command, e.g. `"M2 C2.3"`.
#[must_use]
pub fn owner_of(id: &str) -> &'static str {
    // Explicit exceptions first.
    let exact: &[(&str, &str)] = &[
        ("anim.evaluate", "M3 C3.2"),
        ("render.frame.run", "M3 C3.3"),
        ("render.limits.get", "M3 C3.3"),
        ("render.range.run", "M3 C3.3"),
        ("inspect.frame.describe", "M3 C3.3"),
        ("inspect.hitTest", "M3 C3.3"),
        ("inspect.coordinates.convert", "M3 C3.3"),
        ("inspect.contactSheet.render", "M3 C3.3"),
        ("inspect.ui.tree", "M4 C4.2"),
        ("inspect.screenshot", "M4 C4.2"),
    ];
    if let Some((_, owner)) = exact.iter().find(|(x, _)| *x == id) {
        return owner;
    }
    if id.starts_with("colour.paint") {
        return "M2 C2.3";
    }
    let namespace = id.split('.').next().unwrap_or_default();
    match namespace {
        // Section 01 shell commands not delivered at kernel level in C1.2 belong to the desktop shell (C4.1).
        "app" | "edit" | "view" | "window" | "workspace" | "prefs" | "keymap" | "ui" => "M4 C4.1",
        "project" => "M1 C1.3",
        "doc" => "M1 C1.3",
        "agent" => "M1 C1.4",
        "checkpoint" | "job" => "M1 C1.2",
        "draw" | "drawAid" => "M2 C2.3",
        "tool" => "M4 C4.2",
        "colour" => "M7 C7.1",
        "element" | "cell" | "xsheet" => "M3 C3.1",
        "timeline" => "M5 C5.1",
        "anim" => "M6",
        "fx" => "M7",
        "sound" | "lipsync" | "media" => "M8",
        "library" | "storyline" => "M9",
        "playback" => "M5",
        "render" | "import" | "export" | "capture" | "print" => "M10",
        "inspect" => "M3 C3.3",
        _ => "unassigned",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_inventory_entry_has_an_owner() {
        for e in crate::inventory::INVENTORY {
            assert_ne!(owner_of(e.id), "unassigned", "{}", e.id);
        }
        assert_eq!(owner_of("colour.paint.apply"), "M2 C2.3");
        assert_eq!(owner_of("colour.swatch.add"), "M7 C7.1");
        assert_eq!(owner_of("draw.stroke.add"), "M2 C2.3");
        assert_eq!(owner_of("render.frame.run"), "M3 C3.3");
    }
}
