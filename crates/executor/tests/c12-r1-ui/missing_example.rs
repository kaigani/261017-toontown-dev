#[tt_registry::command(id="app.state.get",kind=query,label="Missing example")]
fn invalid(_cx: &mut (), _params: serde_json::Value) -> Result<serde_json::Value, tt_registry::CommandError> {
    Ok(serde_json::Value::Null)
}

fn main() {}
