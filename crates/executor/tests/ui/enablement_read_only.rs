// Enablement checks receive the context as `&dyn Any`: it downcasts to `&Cx`, which cannot stage changes.
fn check(any: &dyn std::any::Any) -> Result<(), tt_registry::Disabled> {
    if let Some(cx) = any.downcast_ref::<tt_executor::Cx>() {
        let _ = cx.patch_objects(Vec::new());
    }
    Ok(())
}

fn main() {
    let _ = check;
}
