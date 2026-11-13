//! Independent field-for-field generic-tool parity and selection override probes.
mod support;

use serde_json::json;

#[test]
fn inspection_wrappers_return_every_command_field() -> Result<(), Box<dyn std::error::Error>> {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).map_err(|e| format!("{e:?}"))?, None, false).is_ok());
    let (mut client, _) = support::Client::connect(&h, support::opts(&dir), json!({}));
    for (tool, command, args) in [
        ("describe_frame", "inspect.frame.describe", json!({"frame":3,"detail":"full"})),
        ("hit_test", "inspect.hitTest", json!({"space":"image","point":{"x":960,"y":540},"frame":1})),
        (
            "convert_coordinates",
            "inspect.coordinates.convert",
            json!({"from":"image","to":"scene","points":[{"x":17.25,"y":103.5}],"frame":3}),
        ),
        ("render_contact_sheet", "inspect.contactSheet.render", json!({"frames":[1,3],"cellWidth":32,"labels":true})),
    ] {
        let wrapped = client.call(tool, args.clone())["structuredContent"].clone();
        let direct = client.run(command, args);
        assert_eq!(wrapped["status"], "ok", "{wrapped}");
        assert_eq!(wrapped["result"], direct["result"], "{tool}");
    }
    client.close()?;
    Ok(())
}

#[test]
fn render_frame_accepts_exclude_override() -> Result<(), Box<dyn std::error::Error>> {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).map_err(|e| format!("{e:?}"))?, None, false).is_ok());
    let (mut client, _) = support::Client::connect(&h, support::opts(&dir), json!({}));
    let answer = client.call("render_frame", json!({"width":64,"exclude":["scene:\"Intro\"/element:\"Bob\""]}));
    assert!(
        h.invoke(tt_executor::Invocation::user(
            "element.visibility.set",
            json!({"elementIds":["scene:\"Intro\"/element:\"Bob\""],"scope":"scene","visible":false})
        ))
        .is_ok()
    );
    let expected = client.run("render.frame.run", json!({"width":64,"composition":"camera"}));
    client.close()?;
    assert_eq!(
        answer["structuredContent"]["status"], "ok",
        "11 section 7.5 requires include/exclude overrides: {answer}"
    );
    assert_eq!(
        answer["structuredContent"]["result"]["sha256"], expected["result"]["sha256"],
        "exclude must actually omit Bob"
    );
    Ok(())
}

#[test]
fn render_frame_reports_render_ms() -> Result<(), Box<dyn std::error::Error>> {
    let dir = support::tmp();
    let h = support::executor(&dir.join("user"));
    assert!(h.open_document(tt_model::fixture::sample_document(1).map_err(|e| format!("{e:?}"))?, None, false).is_ok());
    let (mut client, _) = support::Client::connect(&h, support::opts(&dir), json!({}));
    let answer = client.call("render_frame", json!({"width":64}));
    client.close()?;
    assert!(
        answer["structuredContent"]["result"]["renderMs"].is_number(),
        "11 section 7.5 render_frame output: {answer}"
    );
    Ok(())
}
