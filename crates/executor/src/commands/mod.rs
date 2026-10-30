//! Typed command declarations (11 §2; spec A) for the M1 namespaces — `app`, `project`, `edit`, `doc`, `agent`,
//! `inspect`, `checkpoint`, `job` — and the kernel-level `window` and `workspace` state commands (01 §5; readiness
//! bullet 4). Each handler is declared with `#[command]`; declarations whose implementation belongs to a later
//! checkpoint name it (`not_implemented = "…"`) and fail with `command.notImplemented` after validating their
//! parameters. Every other spec A entry has a generated declaration (`tt_registry::declared`, ADR-0013 §1).

pub(crate) mod agent;
pub(crate) mod app;
pub(crate) mod bundle;
pub(crate) mod checkpoint;
pub(crate) mod common;
pub(crate) mod derived;
pub(crate) mod doc;
pub(crate) mod doc_io;
pub(crate) mod doc_lint;
pub(crate) mod draw;
pub(crate) mod draw_stroke;
pub(crate) mod draw_text;
pub(crate) mod edit;
pub(crate) mod edit_ops;
pub(crate) mod inspect;
pub(crate) mod job;
pub(crate) mod project;
pub(crate) mod window;
pub(crate) mod workspace;

use tt_registry::Command;

/// Every typed declaration.
pub(crate) static ALL: &[&Command] = &[
    // app
    &app::APP_STATE_GET,
    &app::APP_CONTEXT_SET,
    &app::APP_CURRENT_ELEMENT_SET,
    &app::APP_CURRENT_ELEMENT_MOVE,
    &app::APP_FRAME_SET,
    &app::APP_FRAME_MOVE,
    &app::APP_MODE_SET,
    &app::APP_INFO_GET,
    &app::APP_QUIT,
    &app::APP_STATE_CHANGED,
    // project
    &project::PROJECT_CREATE,
    &project::PROJECT_OPEN,
    &project::PROJECT_CLOSE,
    &project::PROJECT_SAVE,
    &project::PROJECT_SAVE_AS,
    &project::PROJECT_INFO_GET,
    &project::PROJECT_PROPERTIES_GET,
    &project::PROJECT_PROPERTIES_SET,
    &project::PROJECT_PRESETS_LIST,
    &project::PROJECT_RECENT_LIST,
    &project::PROJECT_RECENT_REMOVE,
    &project::PROJECT_RECOVERY_LIST,
    &project::PROJECT_RECOVERY_OPEN,
    &project::PROJECT_RECOVERY_DELETE,
    &project::PROJECT_BACKUP_LIST,
    &project::PROJECT_BACKUP_OPEN,
    &project::PROJECT_AUTOSAVED,
    &project::PROJECT_CLOSED,
    &project::PROJECT_DIRTY_CHANGED,
    &project::PROJECT_OPENED,
    &project::PROJECT_SAVED,
    // edit
    &edit::EDIT_UNDO,
    &edit::EDIT_REDO,
    &edit::EDIT_HISTORY_GET,
    &edit::EDIT_HISTORY_MOVE,
    &edit::EDIT_HISTORY_CHANGED,
    &edit::EDIT_SELECTION_GET,
    &edit::EDIT_SELECTION_SET,
    &edit::EDIT_SELECT_ALL,
    &edit::EDIT_DESELECT_ALL,
    &edit::EDIT_SELECTION_CHANGED,
    &edit::EDIT_CLIPBOARD_GET,
    &edit::EDIT_CLIPBOARD_SET,
    &edit::EDIT_CLIPBOARD_CHANGED,
    &edit_ops::EDIT_CUT,
    &edit_ops::EDIT_COPY,
    &edit_ops::EDIT_PASTE,
    &edit_ops::EDIT_DELETE,
    &edit_ops::EDIT_CUT_SPECIAL,
    &edit_ops::EDIT_PASTE_SPECIAL,
    &edit_ops::EDIT_PASTE_AS_NEW_ELEMENT,
    // doc
    &doc::DOC_OBJECT_GET,
    &doc::DOC_OBJECT_FIND,
    &doc::DOC_OBJECT_RESOLVE,
    &doc::DOC_OBJECT_PATCH,
    &doc_lint::DOC_OBJECT_REFS,
    &doc::DOC_SCHEMA_LIST,
    &doc::DOC_SCHEMA_GET,
    &doc_lint::DOC_SCHEMA_VALIDATE,
    &doc::DOC_STATE_GET,
    &doc::DOC_DIFF,
    &doc_lint::DOC_VALIDATE,
    &doc_lint::DOC_REPAIR,
    &doc_io::DOC_LOAD_REPORT_GET,
    &bundle::DOC_EXPORT_JSON,
    &bundle::DOC_IMPORT_JSON,
    &doc_io::DOC_BLOB_GET,
    &doc_io::DOC_ASSET_LIST,
    &doc_io::DOC_ASSET_EMBED,
    &doc_io::DOC_ASSET_EXPORT,
    &doc_io::DOC_ASSET_RELINK,
    &doc_io::DOC_ASSET_REMOVE,
    &doc_io::DOC_PACKAGE_GET,
    &doc_io::DOC_PACKAGE_EXPORT,
    &doc::DOC_CHANGED,
    &doc_io::DOC_ASSET_STATUS_CHANGED,
    &doc_io::DOC_PACKAGE_CHANGED_ON_DISK,
    // draw (M2 C2.1)
    &draw::DRAW_DRAWING_GET,
    &draw::DRAW_CURRENT_GET,
    &draw::DRAW_PIVOT_SET,
    &draw::DRAW_OBJECT_DELETE,
    &draw::DRAW_GROUP_CREATE,
    &draw::DRAW_GROUP_DELETE,
    &draw_stroke::DRAW_STROKE_ADD,
    &draw_stroke::DRAW_STROKE_SET,
    &draw_stroke::DRAW_CONTENT_ADD,
    &draw_text::DRAW_TEXT_ADD,
    &draw_text::DRAW_TEXT_SET,
    &draw_text::DRAW_TEXT_FORMAT_SET,
    // agent
    &agent::AGENT_SERVER_SET,
    &agent::AGENT_SERVER_STATUS,
    &agent::AGENT_SESSION_GRANT,
    &agent::AGENT_SESSION_DISCONNECT,
    &agent::AGENT_SESSION_PAUSE,
    &agent::AGENT_SESSION_RESUME,
    &agent::AGENT_POLICY_DEFAULTS_SET,
    &agent::AGENT_TOKEN_REGENERATE,
    &agent::AGENT_TYPED_TOOLS_SET,
    &agent::AGENT_CONFIRMATION_RESPOND,
    &agent::AGENT_ACTIVITY_LIST,
    &agent::AGENT_ACTIVITY_EXPORT,
    &agent::AGENT_ACTIVITY_PERSIST_SET,
    &agent::AGENT_SESSION_CONNECTED,
    &agent::AGENT_SESSION_DISCONNECTED,
    &agent::AGENT_CONFIRMATION_REQUESTED,
    // inspect
    &inspect::INSPECT_FRAME_DESCRIBE,
    &inspect::INSPECT_HIT_TEST,
    &inspect::INSPECT_COORDINATES_CONVERT,
    &inspect::INSPECT_CONTACT_SHEET_RENDER,
    &inspect::INSPECT_UI_TREE,
    &inspect::INSPECT_SCREENSHOT,
    // checkpoint
    &checkpoint::CHECKPOINT_CREATE,
    &checkpoint::CHECKPOINT_LIST,
    &checkpoint::CHECKPOINT_RESTORE,
    &checkpoint::CHECKPOINT_DELETE,
    // job
    &job::JOB_LIST,
    &job::JOB_WAIT,
    &job::JOB_CANCEL,
    &job::JOB_PROGRESS,
    &job::JOB_FINISHED,
    // window
    &window::WINDOW_FULL_SCREEN_SET,
    &window::WINDOW_PANEL_LIST,
    &window::WINDOW_PANEL_SET,
    &window::WINDOW_PANEL_MOVE,
    &window::WINDOW_FLOATING_PANELS_SET,
    &window::WINDOW_SIDE_PANELS_EXTENDED_SET,
    &window::WINDOW_TOOLBAR_LIST,
    &window::WINDOW_TOOLBAR_SET,
    &window::WINDOW_TOOLBAR_ITEMS_SET,
    &window::WINDOW_TOOLBAR_ITEMS_RESET,
    &window::WINDOW_GEOMETRY_GET,
    &window::WINDOW_GEOMETRY_SET,
    &window::WINDOW_MINIMIZED_SET,
    &window::WINDOW_RAISE_ALL_RUN,
    // workspace
    &workspace::WORKSPACE_LIST,
    &workspace::WORKSPACE_GET,
    &workspace::WORKSPACE_SELECT,
    &workspace::WORKSPACE_SAVE,
    &workspace::WORKSPACE_CREATE,
    &workspace::WORKSPACE_RENAME,
    &workspace::WORKSPACE_DELETE,
    &workspace::WORKSPACE_RESET,
    &workspace::WORKSPACE_LAYOUT_SET,
    &workspace::WORKSPACE_EXPORT,
    &workspace::WORKSPACE_IMPORT,
    &workspace::WORKSPACE_CHANGED,
];
