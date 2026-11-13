//! Independent seeded hierarchy/order checks over the saved fixture model.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "review tests")]
use std::collections::{BTreeMap, BTreeSet};
use tt_anim::hierarchy as h;
use tt_model::{Id, entities::scene::TreeNode};
fn walk(nodes: &[TreeNode], parent: Option<Id>, out: &mut Vec<(Id, Option<Id>)>) {
    for n in nodes {
        out.push((n.id.clone(), parent.clone()));
        walk(&n.children, Some(n.id.clone()), out);
    }
}
fn next(seed: &mut u64, n: usize) -> usize {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed as usize % n
}
#[test]
fn permutations_and_deletion_reattachment_4096() {
    let doc = tt_model::fixture::sample_document(1).unwrap();
    let scene = doc.scene(&doc.project().scenes().ids()[0]).unwrap();
    let tree = scene.tree().clone();
    let mut original = vec![];
    walk(&tree, None, &mut original);
    let parents: BTreeMap<_, _> = original.iter().cloned().collect();
    let mut seed = 0xc31a11;
    for _ in 0..4096 {
        let mut perm: Vec<_> = tree.iter().map(|n| n.id.clone()).collect();
        for i in (1..perm.len()).rev() {
            let j = next(&mut seed, i + 1);
            perm.swap(i, j);
        }
        let mut changed = tree.clone();
        assert!(h::reorder(&mut changed, None, &perm));
        assert_eq!(changed.iter().map(|n| n.id.clone()).collect::<Vec<_>>(), perm);
        for n in &changed {
            assert_eq!(Some(n), tree.iter().find(|old| old.id == n.id));
        }
        let gone: BTreeSet<_> = original.iter().filter(|_| next(&mut seed, 3) == 0).map(|(id, _)| id.clone()).collect();
        let after = h::without(&tree, &|id| gone.contains(id));
        let mut observed = vec![];
        walk(&after, None, &mut observed);
        let expected: Vec<_> = original
            .iter()
            .filter(|(id, _)| !gone.contains(id))
            .map(|(id, p)| {
                let mut p = p.clone();
                while p.as_ref().is_some_and(|id| gone.contains(id)) {
                    p = parents[p.as_ref().unwrap()].clone();
                }
                (id.clone(), p)
            })
            .collect();
        assert_eq!(observed, expected);
        assert_eq!(h::parents(&after), expected.into_iter().collect());
    }
}
#[test]
fn invalid_permutation_is_atomic_and_detach_attach_roundtrips() {
    let doc = tt_model::fixture::sample_document(2).unwrap();
    let mut tree = doc.scene(&doc.project().scenes().ids()[0]).unwrap().tree().clone();
    let original = tree.clone();
    let roots: Vec<_> = tree.iter().map(|n| n.id.clone()).collect();
    let mut bad = roots.clone();
    bad.push(roots[0].clone());
    assert!(!h::reorder(&mut tree, None, &bad));
    assert_eq!(tree, original);
    let node = h::detach(&mut tree, &roots[0]).unwrap();
    assert!(h::attach(&mut tree, None, Some(0), node));
    assert_eq!(tree, original);
    for q in h::placed(&tree) {
        let mut depth = 0;
        let mut p = q.parent.clone();
        while let Some(id) = p {
            depth += 1;
            p = h::parents(&tree)[&id].clone();
        }
        assert_eq!(q.depth, depth);
        assert!(h::is_within(&tree, &q.id, &q.id));
    }
}
