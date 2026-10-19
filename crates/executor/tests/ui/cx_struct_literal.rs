// A mutation context cannot be built by hand: its fields are private.
fn main() {
    let _cx = tt_executor::Cx {};
}
