//! The exclusive mutation capability, proven at compile time (C1.2; plan §3.2): outside the executor no code can
//! construct, clone or obtain a mutation context, stage changes through a shared context (enablement checks), keep a
//! context in background work (jobs), reach the executor's session, or write to a document or entity. The pass case
//! shows what other crates can do: read immutable snapshots.
#![allow(missing_docs, reason = "test")]

#[test]
fn exclusive_mutation_capability() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
    t.pass("tests/ui-pass/*.rs");
}
