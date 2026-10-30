// The core that installs a document (`Core::install`) is private to the executor; other crates reach the live
// document only through the guarded `ExecutorHandle::open_document` (C12-R1-01).
use tt_executor::run::Core;

fn main() {
    let _ = std::mem::size_of::<Core>();
}
