// The executor's session (live document, history, checkpoints) is private to the crate.
use tt_executor::session::Session;

fn main() {
    let _ = std::mem::size_of::<Session>();
}
