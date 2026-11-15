//! Independent host command reachability regression.
#[test]
fn part_b_product_canvas_commands_are_registered() {
    for name in ["canvas_scene", "canvas_load", "gesture_commit", "gesture_abort", "canvas_presence"] {
        assert!(tt_desktop::COMMANDS.iter().any(|c| c.ends_with(name)), "missing {name}");
    }
}
