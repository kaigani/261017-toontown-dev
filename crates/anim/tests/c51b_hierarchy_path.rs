//! M5 C5.1b: `hierarchy::path` (the element's own levels, read by `element.hierarchy.select`'s precheck in every
//! availability pass) agrees with `hierarchy::placed` for every element of the saved fixture scenes: the parent, the
//! sibling index, the depth and the sibling list.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs, reason = "test assertions")]

use tt_anim::hierarchy as h;
use tt_model::Id;

#[test]
fn path_agrees_with_placed_for_every_element() {
    let mut checked = 0;
    for seed in 1..=4 {
        let doc = tt_model::fixture::sample_document(seed).unwrap();
        for scene_id in doc.project().scenes().ids() {
            let tree = doc.scene(scene_id).unwrap().tree();
            for q in h::placed(tree) {
                let path = h::path(tree, &q.id).unwrap();
                let (level, index) = *path.last().unwrap();
                assert_eq!(level.get(index).map(|n| &n.id), Some(&q.id));
                assert_eq!(index, q.index, "{}", q.id.as_str());
                assert_eq!(path.len() - 1, q.depth, "{}", q.id.as_str());
                let parent = path.len().checked_sub(2).map(|k| path[k].0[path[k].1].id.clone());
                assert_eq!(parent, q.parent, "{}", q.id.as_str());
                let siblings: Vec<Id> = level.iter().map(|n| n.id.clone()).collect();
                assert_eq!(siblings, h::level_ids(tree, q.parent.as_ref()));
                checked += 1;
            }
            assert!(h::path(tree, &Id::parse("el_01K7JJN800PRNF9AQQBWJQ0JDM").unwrap()).is_none());
        }
    }
    assert!(checked > 0);
}
