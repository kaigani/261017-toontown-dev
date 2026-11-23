//! M5 C5.1b (builder): the Timeline toolbar's solo mode is a toggle (04 line 604): its reference names the rows (or
//! leaves them to the default), the kernel completes `enabled` to the next state and checks the item while solo is on
//! (ruling O-10), and the completed reference runs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test")]

mod common;

use common::*;
use serde_json::{Map, Value, json};
use tt_executor::ExecutorHandle;

fn facts(h: &ExecutorHandle, command: &str, params: Value) -> tt_executor::host::Availability {
    let p: Map<String, Value> = params.as_object().cloned().unwrap_or_default();
    h.availability(vec![(command.to_owned(), p)]).unwrap().remove(0)
}

#[test]
fn solo_completes_to_its_next_state_and_shows_it() {
    let h = fixture();
    let bob = h.invoke(tt_executor::Invocation::user("element.list", json!({}))).result.unwrap()["elements"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for reference in [json!({}), json!({ "elementIds": [bob] })] {
        let off = facts(&h, "element.solo.set", reference.clone());
        assert!(off.enabled, "{reference}: {:?}", off.reason);
        assert_eq!(off.checked, Some(false), "{reference}");
        assert_eq!(off.params["enabled"], true, "{reference}: the next state");
        ok(&h, "element.solo.set", off.params.clone());
        let on = facts(&h, "element.solo.set", reference.clone());
        assert_eq!(on.checked, Some(true), "{reference}");
        assert_eq!(on.params["enabled"], false, "{reference}: the next state");
        ok(&h, "element.solo.set", on.params.clone());
        assert_eq!(facts(&h, "element.solo.set", reference.clone()).checked, Some(false), "{reference}: off again");
    }
}
