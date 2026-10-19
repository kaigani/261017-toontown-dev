// A mutation context lent to a handler cannot be duplicated or conjured.
fn keep(cx: &mut tt_executor::Cx) -> tt_executor::Cx {
    cx.clone()
}

fn conjure() -> tt_executor::Cx {
    Default::default()
}

fn main() {
    let _ = (keep, conjure);
}
