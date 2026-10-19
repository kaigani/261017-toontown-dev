// A load grant (ADR-0013 §16; C12-R1-01) cannot be forged: only the guarded load transaction creates one.
fn main() {
    let _grant = tt_executor::LoadGrant { _private: () };
}
