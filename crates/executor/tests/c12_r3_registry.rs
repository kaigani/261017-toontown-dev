//! Semantic schema cases from the owning specifications, not generator-produced examples.
#![allow(clippy::unwrap_used, clippy::panic, missing_docs, reason = "review regressions")]
use serde_json::{Value, json};

fn accepts(id: &str, result: bool, value: &Value) {
    let entry = tt_executor::registry().unwrap().get(id).unwrap();
    let schema = if result { entry.result_schema() } else { entry.params_schema() };
    let errors = tt_registry::schema_violations(&schema, value, 10).unwrap();
    assert!(errors.is_empty(), "{id}: specification-valid value rejected: {errors:?}");
}

fn example(section: &str, heading: &str) -> Value {
    let content = section.split(heading).nth(1).unwrap().split("```json").nth(1).unwrap().split("```").next().unwrap();
    serde_json::from_str(content).unwrap()
}

#[test]
fn path_result_accepts_exact_05_section_19_4_example() {
    let value = example(include_str!("../../../spec/05_scene_planning.md"), "**Path**");
    accepts("anim.path.get", true, &value);
}

#[test]
fn velocity_function_pins_accept_the_explicit_frame_value_shape() {
    let mut value = example(include_str!("../../../spec/05_scene_planning.md"), "**Function**");
    value["channel"] = json!("velocity");
    value["pins"] = json!([{"frame":1,"value":0.0},{"frame":24,"value":14.37}]);
    accepts("anim.function.get", true, &value);
}

#[test]
fn selected_point_anchor_is_the_integer_index_from_02() {
    accepts("draw.pointSelection.set", true, &json!({"selectedPoints":[{"strokeId":"stroke-r3","anchor":2}]}));
}

#[test]
fn xsheet_panel_result_preserves_boolean_fields_from_parameters() {
    accepts("xsheet.panel.set", true, &json!({"elementList":true,"lightTable":false,"toolbar":true}));
}

#[test]
fn envelope_point_results_use_time_and_gains_from_07() {
    for id in ["sound.envelope.set", "sound.envelope.point.move", "sound.envelope.point.remove"] {
        accepts(id, true, &json!({"points":[{"time":0.25,"left":0.5,"right":0.75}]}));
    }
}

#[test]
fn preference_listing_can_return_a_boolean_preference() {
    accepts(
        "prefs.list",
        true,
        &json!([{"key":"general.workspace.autoSave","type":"bool","default":true,"value":true,"applies":"live","description":"Save workspace automatically"}]),
    );
}

#[test]
fn binding_params_support_commands_other_than_undo() {
    accepts(
        "keymap.list",
        true,
        &json!([{"commandId":"app.frame.set","params":{"frame":7},"keys":"Primary+7","context":"global","isDefault":false}]),
    );
}

#[test]
fn polyline_options_result_is_the_option_object() {
    accepts("tool.polyline.options.set", true, &json!({"options":{"snap":true,"snapRadius":8}}));
}

#[test]
fn print_settings_accepts_a_defined_print_parameter() {
    accepts("print.settings.set", false, &json!({"values":{"copies":2}}));
}

#[test]
fn export_settings_returns_an_object_as_its_setter_accepts() {
    accepts("export.settings.get", true, &json!({"preset":"default","settings":{}}));
}

#[test]
fn light_table_set_returns_the_entries_that_list_defines() {
    accepts(
        "drawAid.staticLightTable.set",
        true,
        &json!({"entries":[{
            "entryId":"r3-entry","sourceId":"r3-drawing","sourceKind":"drawing","visible":true,"placement":"underlay"
        }]}),
    );
}

#[test]
fn pen_set_returns_a_model_pen_instead_of_a_scalar() {
    let parts = tt_model::fixture::sample_parts(0xc123_7e61).unwrap();
    accepts("tool.pen.set", true, &json!({"pen":parts.project.pens()[0]}));
}
