//! Estimates of the memory values hold on the heap (M5 ruling E-73): what the undo history's memory bound and the
//! revision log's budget count, instead of a serialised length (a JSON value in memory held about 4.7 times its text in
//! the measurement of `crates/executor/tests/c52a_history_size.rs`).
//!
//! The estimates follow the layouts of the values: a `serde_json::Value` node's inline size counted by its container,
//! a string's capacity, an array's capacity in nodes, and an object as `serde_json` builds it with `preserve_order` (an
//! insertion-ordered map: a vector of entries, each a hash, a key and a value, and a hash table of entry indices). The
//! allocator's own bookkeeping is not counted (the counting allocator of the test measures requested bytes).

use std::mem::size_of;

use serde_json::{Map, Value};
use tt_model::change::PatchOp;
use tt_model::document::{Document, EntryCommit, EntryKey};

/// The capacity a vector reaches when it grows to `len` elements one at a time (doubling from 4); a vector built with
/// its final length has exactly `len`, so this is an upper estimate for those.
fn grown(len: usize) -> usize {
    if len == 0 { 0 } else { len.next_power_of_two().max(4) }
}

/// The heap an object holds besides its values' own heaps: the entry vector and the index table.
fn map_heap(m: &Map<String, Value>) -> usize {
    let n = m.len();
    if n == 0 {
        return 0;
    }
    // Entries: the hash, the key and the value inline.
    let entries = grown(n) * (size_of::<usize>() + size_of::<String>() + size_of::<Value>());
    // The table: buckets of an index each and a control byte, at most 7/8 full, a power of two (at least 4).
    let buckets = (n * 8 / 7 + 1).next_power_of_two().max(4);
    let table = buckets * (size_of::<usize>() + 1) + 16;
    entries + table
}

/// The heap a value holds (its own inline size excluded: its container counts it).
#[must_use]
pub fn value(v: &Value) -> usize {
    match v {
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
        Value::String(s) => s.capacity(),
        Value::Array(a) => a.capacity() * size_of::<Value>() + a.iter().map(value).sum::<usize>(),
        Value::Object(m) => map_heap(m) + m.iter().map(|(k, x)| k.capacity() + value(x)).sum::<usize>(),
    }
}

/// A value held behind an `Option` in a struct (its inline size is the struct's).
fn optional(v: Option<&Value>) -> usize {
    v.map_or(0, value)
}

/// The heap one patch operation holds (its inline size excluded).
#[must_use]
pub fn op(o: &PatchOp) -> usize {
    o.path.capacity()
        + o.from.as_ref().map_or(0, String::capacity)
        + optional(o.value.as_ref())
        + optional(o.old_value.as_ref())
}

/// The memory a list of patch operations holds: the vector and every operation.
#[must_use]
pub fn ops(list: &[PatchOp], capacity: usize) -> usize {
    capacity * size_of::<PatchOp>() + list.iter().map(op).sum::<usize>()
}

/// The memory one entry's commit holds (inline and heap: it is an element of its step's vector).
#[must_use]
pub fn commit(c: &EntryCommit) -> usize {
    size_of::<EntryCommit>() + ops(&c.undo, c.undo.capacity()) + ops(&c.redo, c.redo.capacity())
}

/// The memory an entry of `doc` would hold were it held alone: its JSON form's estimate (the typed entry's own layout
/// is close to its value tree), with the entry's key.
#[must_use]
pub fn entry(doc: &Document, key: &EntryKey) -> usize {
    size_of::<EntryKey>() + doc.entry_raw_json(key).map_or(0, |v| size_of::<Value>() + value(&v))
}
