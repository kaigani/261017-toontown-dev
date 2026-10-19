// The request type of the document thread is private (C12-R1-01).
use tt_executor::Request;

fn main() {
    let _ = std::mem::size_of::<Request>();
}
