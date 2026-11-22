//! The Exposure Sheet's view-state commands (04 §17.4; M5): `xsheet.currentCell.set` (C5.1a, the Timeline's and the
//! Exposure Sheet's cell click). C5.2 adds `xsheet.display.set`, `xsheet.columnWidth.set`, `xsheet.panel.set` and the
//! light-table commands here.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_registry::{CommandError, command};

use super::timing::{element_id, err};
use crate::cx::Cx;

// ----- xsheet.currentCell.set (M5; the Timeline's cell click, 04 line 662) -----------------------------------------------

/// `xsheet.currentCell.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CurrentCellParams {
    /// The element.
    element_id: String,
    /// The frame (frames past the scene end may be current: 04 line 676).
    #[schemars(range(min = 1, max = 100_000))]
    frame: i64,
}

/// `{elementId, frame}`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentCellResult {
    element_id: String,
    #[schemars(range(min = 1))]
    frame: u32,
}

#[command(
    id = "xsheet.currentCell.set",
    kind = ui,
    label = "Set current cell",
    scopes = [ui],
    needs_project,
    example(title = "Bob at frame 3", params = r#"{"elementId": "scene:\"Intro\"/element:\"Bob\"", "frame": 3}"#),
    gui = "Timeline and Exposure Sheet: click a cell (M5 C5.1, C5.2)",
)]
pub(crate) fn xsheet_current_cell_set(cx: &mut Cx, p: CurrentCellParams) -> Result<CurrentCellResult, CommandError> {
    let id = element_id(cx, &p.element_id, "/elementId")?;
    let frame = u32::try_from(p.frame).ok().filter(|f| (1..=100_000).contains(f)).ok_or_else(|| {
        err(
            "cell.frameOutOfRange",
            format!("frame {} is outside 1 to 100000", p.frame),
            json!({ "pointer": "/frame", "allowed": { "minimum": 1, "maximum": 100_000 } }),
        )
    })?;
    super::element_read::put_current_element(cx, &id)?;
    super::timeline::frame_source(cx, "command");
    super::timeline::put_frame(cx, frame);
    Ok(CurrentCellResult { element_id: id.as_str().to_owned(), frame })
}
