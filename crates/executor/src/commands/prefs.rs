//! `prefs.*` (01 §7, §12.1 rule 15; M4 C4.1): preferences by key over the catalogue of [`crate::prefs`].
//!
//! `prefs.set` is atomic: every value is validated before anything is staged, so one invalid value changes nothing.
//! Writes are deferred effects accepted with the call (a dry run, a failed batch or a cancelled request writes no
//! settings file), and later steps of the same call read the staged values (`Cx::pref`). Preference changes are not
//! undoable and do not modify the project (01 §7.1). An agent cannot set or reset the `agent.*` keys, which control
//! its own access (01 §7.3 *Agent access*, 11 §15.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tt_registry::{CommandError, codes, command};

use crate::Origin;
use crate::cx::{Cx, Effect};
use crate::prefs::{self, catalogue};

/// `prefs.list` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListParams {
    /// Only keys starting with this prefix.
    #[serde(default)]
    #[schemars(length(max = 256))]
    prefix: Option<String>,
}

/// One preference as `prefs.list` reports it (01 §12.2).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrefEntry {
    /// Key (`<category>.<group>.<name>`).
    key: String,
    /// `bool`, `int`, `number`, `string`, `enum`, `colour`, `path` or `object`.
    #[serde(rename = "type")]
    ty: String,
    /// Default value.
    default: Value,
    /// Current value.
    value: Value,
    /// Minimum (numbers).
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<f64>,
    /// Maximum (numbers).
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<f64>,
    /// Allowed values (enums; `interface.theme` includes installed user themes).
    #[serde(skip_serializing_if = "Option::is_none")]
    values: Option<Vec<String>>,
    /// When a change takes effect.
    applies: String,
    /// What the preference does.
    description: String,
}

fn entry(cx: &Cx, d: &prefs::PrefDef) -> PrefEntry {
    let mut values = d.values.clone();
    if d.user_themes
        && let Some(v) = values.as_mut()
    {
        v.extend(cx.session().env.settings.theme_ids());
    }
    PrefEntry {
        key: d.key.clone(),
        ty: d.ty.as_str().to_owned(),
        default: d.default.clone(),
        value: cx.pref(&d.key).unwrap_or_else(|| d.default.clone()),
        min: d.min,
        max: d.max,
        values,
        applies: d.applies.as_str().to_owned(),
        description: d.description.clone(),
    }
}

#[command(
    id = "prefs.list",
    kind = query,
    label = "List preferences",
    scopes = [read],
    example(title = "The new-project defaults", params = r#"{"prefix": "project.defaults."}"#),
    gui = "Preferences window (every category; M4 C4.1)",
)]
pub(crate) fn prefs_list(cx: &mut Cx, p: ListParams) -> Result<Vec<PrefEntry>, CommandError> {
    if let Some(e) = &catalogue().error {
        return Err(CommandError::internal(format!("the preference catalogue did not load: {e}")));
    }
    let prefix = p.prefix.unwrap_or_default();
    Ok(catalogue().prefs.iter().filter(|d| d.key.starts_with(&prefix)).map(|d| entry(cx, d)).collect())
}

/// `prefs.get` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetParams {
    /// Keys to read.
    #[schemars(length(min = 1, max = 1000))]
    keys: Vec<String>,
}

/// `prefs.get` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct GetResult {
    /// Key → current value.
    values: Map<String, Value>,
}

#[command(
    id = "prefs.get",
    kind = query,
    label = "Get preferences",
    scopes = [read],
    example(title = "Theme and autosave interval", params = r#"{"keys": ["interface.theme", "files.autosave.intervalMinutes"]}"#),
    gui = "Preferences window (M4 C4.1)",
)]
pub(crate) fn prefs_get(cx: &mut Cx, p: GetParams) -> Result<GetResult, CommandError> {
    let mut values = Map::new();
    for k in &p.keys {
        let v = cx.pref(k).ok_or_else(|| prefs::unknown(k))?;
        values.insert(k.clone(), v);
    }
    Ok(GetResult { values })
}

/// `prefs.set` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    /// Key → value (01 §7.2). Every value must be valid, or nothing changes.
    values: Map<String, Value>,
}

/// `prefs.set` result.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetResult {
    /// Keys set.
    applied: Vec<String>,
    /// Keys that take effect only after a restart.
    restart_required: Vec<String>,
}

fn refuse_agent_access(cx: &Cx, keys: &[&String]) -> Result<(), CommandError> {
    if matches!(cx.origin(), Origin::Agent { .. })
        && let Some(k) = keys.iter().find(|k| prefs::agent_restricted(k))
    {
        return Err(CommandError::with_details(
            codes::PERMISSION_DENIED,
            format!("an agent cannot change `{k}`: agent access is the user's (01 §7.3)"),
            json!({ "missingScopes": ["app"], "key": k }),
        ));
    }
    Ok(())
}

fn source(cx: &Cx) -> &'static str {
    if matches!(cx.origin(), Origin::Agent { .. }) { "agent" } else { "user" }
}

/// Applies the session-side consequences of preference values (staged with the call, so a rollback restores them):
/// the main toolbar's visibility follows `interface.mainToolbar.visible`, the agent hub follows the agent keys.
fn apply_consequences(cx: &mut Cx, values: &Map<String, Value>) -> Result<(), CommandError> {
    if let Some(v) = values.get("interface.mainToolbar.visible").and_then(Value::as_bool) {
        cx.ui_mut().set_toolbar("main", v, None, None)?;
    }
    if let Some(v) = values.get("general.startup.behaviour").and_then(Value::as_str) {
        cx.ui_mut().shell_mut().start_screen_by_default = v == "startScreen";
    }
    if let Some(v) = values.get("agent.server.enabled").and_then(Value::as_bool) {
        cx.session_mut().agents.server_enabled = v;
    }
    if let Some(v) = values.get("agent.policy.defaults")
        && let Ok(limits) = serde_json::from_value::<crate::agents::Limits>(v.clone())
    {
        cx.session_mut().agents.limits = limits;
    }
    Ok(())
}

/// Two preference values are the same (numbers compare by value: `24` and `24.0` are one frame rate).
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < f64::EPSILON,
        _ => a == b,
    }
}

/// Stages validated preference values, their consequences and `prefs.changed` (shared by `prefs.set`, `prefs.reset`
/// and the commands that write a preference key, such as `ui.statusBar.set`). Only keys whose effective value changes
/// are staged and listed (C41-R1-10): a call that changes nothing writes nothing and emits nothing.
pub(crate) fn stage(cx: &mut Cx, values: Map<String, Value>) -> Result<(), CommandError> {
    let values: Map<String, Value> =
        values.into_iter().filter(|(k, v)| cx.pref(k).is_none_or(|now| !same(&now, v))).collect();
    if values.is_empty() {
        return Ok(());
    }
    apply_consequences(cx, &values)?;
    let keys: Vec<String> = values.keys().cloned().collect();
    let src = source(cx);
    cx.defer(Effect::Prefs(values));
    cx.emit("prefs.changed", json!({ "keys": keys, "source": src }));
    Ok(())
}

#[command(
    id = "prefs.set",
    kind = ui,
    label = "Set preferences",
    scopes = [prefs],
    example(title = "Dark theme and a 10-minute autosave", params = r#"{"values": {"interface.theme": "dark", "files.autosave.intervalMinutes": 10}}"#),
    gui = "Preferences window: OK (M4 C4.1)",
)]
pub(crate) fn prefs_set(cx: &mut Cx, p: SetParams) -> Result<SetResult, CommandError> {
    if p.values.is_empty() {
        return Err(CommandError::with_details(
            codes::ARG_INVALID,
            "`values` names no preference",
            json!({ "pointer": "/values" }),
        ));
    }
    refuse_agent_access(cx, &p.values.keys().collect::<Vec<_>>())?;
    let themes = cx.session().env.settings.theme_ids();
    // Validate everything first: one invalid value changes nothing (01 §12.1 rule 15).
    let mut staged = Map::new();
    for (k, v) in &p.values {
        let value = prefs::validate(k, v, &themes).map_err(|e| {
            let mut e = e;
            if let Some(m) = e.details.as_object_mut() {
                m.insert("pointer".into(), json!(format!("/values/{}", k.replace('~', "~0").replace('/', "~1"))));
            }
            e
        })?;
        staged.insert(k.clone(), value);
    }
    let restart_required = staged
        .keys()
        .filter(|k| catalogue().get(k).is_some_and(|d| d.applies == prefs::Applies::Restart))
        .cloned()
        .collect();
    let applied = staged.keys().cloned().collect();
    stage(cx, staged)?;
    Ok(SetResult { applied, restart_required })
}

/// `prefs.reset` parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResetParams {
    /// Keys to reset.
    #[serde(default)]
    #[schemars(length(max = 1000))]
    keys: Option<Vec<String>>,
    /// Reset every key with this prefix (`""` = all).
    #[serde(default)]
    #[schemars(length(max = 256))]
    prefix: Option<String>,
}

/// `prefs.reset` result.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ResetResult {
    /// Keys reset to their defaults.
    reset: Vec<String>,
}

#[command(
    id = "prefs.reset",
    kind = ui,
    label = "Restore default preferences",
    scopes = [prefs],
    example(title = "Restore the Display category", params = r#"{"prefix": "display."}"#),
    example(title = "Restore one key", params = r#"{"keys": ["interface.theme"]}"#),
    gui = "Preferences window: Restore defaults (per category; M4 C4.1)",
)]
pub(crate) fn prefs_reset(cx: &mut Cx, p: ResetParams) -> Result<ResetResult, CommandError> {
    let mut keys: Vec<String> = Vec::new();
    match (&p.keys, &p.prefix) {
        (None, None) => {
            return Err(CommandError::with_details(
                codes::ARG_MISSING,
                "give `keys` or `prefix` (\"\" resets every preference)",
                json!({ "pointer": "/keys" }),
            ));
        }
        (k, prefix) => {
            for key in k.iter().flatten() {
                if catalogue().get(key).is_none() {
                    return Err(prefs::unknown(key));
                }
                keys.push(key.clone());
            }
            if let Some(prefix) = prefix {
                keys.extend(catalogue().prefs.iter().filter(|d| d.key.starts_with(prefix)).map(|d| d.key.clone()));
            }
        }
    }
    keys.sort();
    keys.dedup();
    // An agent resetting everything leaves its own access alone; naming an agent key is refused.
    if matches!(cx.origin(), Origin::Agent { .. }) {
        if let Some(k) = p.keys.iter().flatten().find(|k| prefs::agent_restricted(k)) {
            refuse_agent_access(cx, &[k])?;
        }
        keys.retain(|k| !prefs::agent_restricted(k));
    }
    let mut values = Map::new();
    for k in &keys {
        if let Some(d) = prefs::default_of(k) {
            values.insert(k.clone(), d);
        }
    }
    stage(cx, values)?;
    Ok(ResetResult { reset: keys })
}

/// `prefs.changed` payload.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ChangedPayload {
    /// Keys whose value was set.
    keys: Vec<String>,
    /// `user` or `agent`.
    source: String,
}

#[command(
    id = "prefs.changed",
    kind = event,
    label = "Preferences changed",
    scopes = [read],
    example(title = "Subscribe (events take no parameters)", params = "{}"),
    agent_only = "an event delivered to subscribers",
)]
pub(crate) fn prefs_changed(_cx: &mut Cx, _p: super::common::NoParams) -> Result<ChangedPayload, CommandError> {
    Err(super::common::event_not_invocable("prefs.changed"))
}
