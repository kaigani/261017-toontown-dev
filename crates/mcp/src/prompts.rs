//! MCP prompts: workflow templates (11 §12). Each prompt's procedure names the commands it uses; a test checks that
//! every one is registered ("prompt texts never reference commands that do not exist"). Steps whose commands belong
//! to later milestones are part of the procedure all the same: they answer `command.notImplemented` with their
//! milestone until then.

use serde_json::{Map, Value, json};

/// One prompt.
#[derive(Debug, Clone, Copy)]
pub struct Prompt {
    /// Name.
    pub name: &'static str,
    /// Title.
    pub title: &'static str,
    /// What it covers.
    pub description: &'static str,
    /// Arguments: (name, description, required).
    pub arguments: &'static [(&'static str, &'static str, bool)],
    /// The procedure; `{arg}` is replaced by the argument's value.
    pub steps: &'static [&'static str],
    /// The commands the procedure uses.
    pub commands: &'static [&'static str],
}

/// The required prompts.
pub const PROMPTS: &[Prompt] = &[
    Prompt {
        name: "orient",
        title: "Orient yourself",
        description: "Read state, list scenes and elements, render the current frame, summarise the project",
        arguments: &[],
        steps: &[
            "Call `get_state` and note the project, scene, frame, selection and your scopes.",
            "List scenes with `find_objects {type: \"scene\"}` and, per scene, elements with `find_objects {type: \"element\", within: <sceneId>}`.",
            "Read the project with `get_object {ref: \"project\"}` (frame rate, camera size, palettes).",
            "Render the current frame with `render_frame` (`render.frame.run`) to see it.",
            "Summarise: scenes, element counts, the current frame, and anything `doc.validate` reports.",
        ],
        commands: &["app.state.get", "doc.object.find", "doc.object.get", "render.frame.run", "doc.validate"],
    },
    Prompt {
        name: "draw_and_paint",
        title: "Draw and paint a drawing",
        description: "Create a drawing in a cell, add strokes structurally, close gaps, paint zones by id, verify",
        arguments: &[
            ("element", "Drawing element (id or path)", true),
            ("frame", "Frame (1-based)", true),
            ("description", "What to draw", true),
        ],
        steps: &[
            "Create the drawing in the cell: `element.drawing.create {elementId: {element}, frame: {frame}}`.",
            "Add strokes with explicit points: `draw.stroke.add` (or `draw.ellipse.add` for shapes) for: {description}.",
            "Close small gaps so zones form: `colour.gap.close`.",
            "Paint zones by id: `colour.paint.apply`.",
            "Verify with `render_drawing` (`draw.drawing.render`) and `doc.validate {checks: [\"unpaintedZones\"]}`.",
        ],
        commands: &[
            "element.drawing.create",
            "draw.stroke.add",
            "draw.ellipse.add",
            "colour.gap.close",
            "colour.paint.apply",
            "draw.drawing.render",
            "doc.validate",
        ],
    },
    Prompt {
        name: "traditional_animation",
        title: "Traditional animation",
        description: "Key drawings, in-betweens, exposures, onion-skin check via a contact sheet",
        arguments: &[
            ("element", "Drawing element", true),
            ("frames", "Frame range, e.g. 1-24", true),
            ("description", "The motion", true),
        ],
        steps: &[
            "Draw the key drawings of {description} on {element} with `element.drawing.create` and `draw.stroke.add`.",
            "Add in-betweens the same way, then set the timing with `xsheet.exposures.set` over {frames}.",
            "Review the motion in one image: `render_contact_sheet` (`inspect.contactSheet.render`) over {frames}.",
        ],
        commands: &["element.drawing.create", "draw.stroke.add", "xsheet.exposures.set", "inspect.contactSheet.render"],
    },
    Prompt {
        name: "cutout_rig",
        title: "Cut-out rig",
        description: "Elements per body part, pegs and hierarchy, pivots, bones, test poses, evaluated transforms",
        arguments: &[("character", "Character name", true)],
        steps: &[
            "Create one element per body part of {character} with `element.create` and pegs with `anim.peg.create`.",
            "Build the hierarchy with `element.parent.set` and set pivots with `doc.object.patch` on the element transform pivot.",
            "Add bones where needed with `anim.bone.create`; pose with `anim.key.add` at test frames.",
            "Check world transforms with `anim.evaluate`.",
        ],
        commands: &[
            "element.create",
            "anim.peg.create",
            "element.parent.set",
            "doc.object.patch",
            "anim.bone.create",
            "anim.key.add",
            "anim.evaluate",
        ],
    },
    Prompt {
        name: "animate_motion",
        title: "Animate motion",
        description: "Peg keyframes, motion path points, velocity and ease, verified with anim.evaluate and a contact sheet",
        arguments: &[
            ("element", "Element or peg", true),
            ("path", "Motion path description", true),
            ("timing", "Timing", true),
        ],
        steps: &[
            "Set keyframes on {element} with `anim.key.add` along {path}.",
            "Shape the path with motion points (`anim.path.point.add`) and the timing {timing} with `anim.function.preset.apply` (ease).",
            "Verify positions with `anim.evaluate` and the motion with `render_contact_sheet` (`inspect.contactSheet.render`).",
        ],
        commands: &[
            "anim.key.add",
            "anim.path.point.add",
            "anim.function.preset.apply",
            "anim.evaluate",
            "inspect.contactSheet.render",
        ],
    },
    Prompt {
        name: "lip_sync",
        title: "Lip-sync",
        description: "Import sound, detect with transcript, map shapes, apply with an overwrite policy, verify the mouth track",
        arguments: &[
            ("soundElement", "Sound element", true),
            ("mouthElement", "Mouth drawing element", true),
            ("transcript", "Transcript", false),
        ],
        steps: &[
            "Import the sound into {soundElement} with `sound.import`.",
            "Detect mouth shapes with `lipsync.detect` (pass the transcript: {transcript}); wait with `job_wait`.",
            "Map shapes to drawings of {mouthElement} with `lipsync.mapping.set` and apply with `lipsync.apply {overwrite: \"replace\"}`.",
            "Verify sampled frames with `describe_frame` (`inspect.frame.describe`).",
        ],
        commands: &[
            "sound.import",
            "lipsync.detect",
            "job.wait",
            "lipsync.mapping.set",
            "lipsync.apply",
            "inspect.frame.describe",
        ],
    },
    Prompt {
        name: "storyboard_to_animatic",
        title: "Storyboard to animatic",
        description: "Scenes per panel, imported images, timing, transitions, sound, preview",
        arguments: &[("panels", "Panels: image paths, durations, captions (JSON)", true)],
        steps: &[
            "For each panel of {panels}, create a scene with `storyline.scene.create` and import its image with `import.file.run`.",
            "Set each scene's length with `storyline.scene.set` and transitions with `storyline.transition.set`.",
            "Add sound with `sound.import`, then preview with `render_frame` (`render.frame.run`).",
        ],
        commands: &[
            "storyline.scene.create",
            "import.file.run",
            "storyline.scene.set",
            "storyline.transition.set",
            "sound.import",
            "render.frame.run",
        ],
    },
    Prompt {
        name: "effects_pass",
        title: "Effects pass",
        description: "Add an effect, set parameters and seed, render frames to verify",
        arguments: &[("scene", "Scene", true), ("effect", "Effect type", true), ("targets", "Target elements", true)],
        steps: &[
            "Add {effect} in {scene} over {targets} with `fx.block.add`.",
            "Set its parameters with `fx.block.set` (and the seed with `fx.block.seed.set` for particle effects).",
            "Render check frames with `render_frame` (`render.frame.run`).",
        ],
        commands: &["fx.block.add", "fx.block.set", "fx.block.seed.set", "render.frame.run"],
    },
    Prompt {
        name: "export_deliverable",
        title: "Export a deliverable",
        description: "Validate, render check frames, export as a job, verify the manifest",
        arguments: &[
            ("format", "Format (mp4, mov, gif, png sequence)", true),
            ("path", "Output path", true),
            ("range", "Frame range", false),
        ],
        steps: &[
            "Validate with `doc.validate` and fix errors first.",
            "Render a few check frames with `render_frame` (`render.frame.run`).",
            "Export with `export.movie.run` ({format} to {path}, range {range}) and wait with `job_wait`.",
            "Verify the result's manifest (files, frame count).",
        ],
        commands: &["doc.validate", "render.frame.run", "export.movie.run", "job.wait"],
    },
    Prompt {
        name: "review_and_fix",
        title: "Review and fix",
        description: "doc.validate, fix the issues, validate again",
        arguments: &[("scene", "Scene (or the whole project)", false)],
        steps: &[
            "Run `doc.validate` (within {scene} when given).",
            "Apply default fixes with `doc.repair {issueIds: [...]}` (dry-run first) or fix issues with their suggested commands.",
            "Run `doc.validate` again until it is clean.",
        ],
        commands: &["doc.validate", "doc.repair"],
    },
];

/// The `prompts/list` entry of a prompt.
#[must_use]
pub fn listing(p: &Prompt) -> Value {
    json!({
        "name": p.name, "title": p.title, "description": p.description,
        "arguments": p.arguments.iter().map(|(n, d, r)| json!({ "name": n, "description": d, "required": r })).collect::<Vec<_>>(),
    })
}

/// The `prompts/get` result.
///
/// # Errors
/// The name of a missing required argument.
pub fn render(p: &Prompt, args: &Map<String, Value>) -> Result<Value, String> {
    let mut text = format!("{}.\n\nProcedure:\n", p.description);
    for (i, step) in p.steps.iter().enumerate() {
        let mut s = (*step).to_owned();
        for (name, _, required) in p.arguments {
            let v = match args.get(*name) {
                Some(Value::String(x)) => x.clone(),
                Some(other) => other.to_string(),
                None if *required => return Err((*name).to_owned()),
                None => "(not given)".to_owned(),
            };
            s = s.replace(&format!("{{{name}}}"), &v);
        }
        text.push_str(&format!("{}. {s}\n", i + 1));
    }
    text.push_str("\nCheck every envelope's `status` and `changes`; use `describe_command` for exact parameters.");
    Ok(json!({
        "description": p.description,
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }],
    }))
}
