// The request channel of the document thread is private: a document cannot be sent past the load guard
// (C12-R1-01).
fn main() {
    let h = tt_executor::ExecutorHandle::spawn(Default::default()).unwrap();
    let _ = &h.tx;
}
