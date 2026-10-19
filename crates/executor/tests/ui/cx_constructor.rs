// The executor's constructor of a mutation context is not public.
fn main() {
    let _cx = tt_executor::Cx::new();
}
