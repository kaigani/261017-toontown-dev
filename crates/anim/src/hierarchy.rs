//! The Timeline hierarchy of a scene (04 §1.2, §9; 10 §4.3 `tree`): reads and edits of the scene tree as pure
//! functions over [`TreeNode`] lists. Every element of a scene appears exactly once; siblings are listed top to
//! bottom as displayed in the Timeline. Work is proportional to the number of nodes.

use std::collections::BTreeMap;

use tt_model::Id;
use tt_model::entities::scene::TreeNode;

/// One element in Timeline order (pre-order: parents before children).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The element.
    pub id: Id,
    /// Its parent, `None` at the root level.
    pub parent: Option<Id>,
    /// Depth (0 at the root level).
    pub depth: usize,
    /// Index among its siblings (0 = top).
    pub index: usize,
}

/// Every element in Timeline order with its parent, depth and sibling index.
#[must_use]
pub fn placed(tree: &[TreeNode]) -> Vec<Placed> {
    fn walk(nodes: &[TreeNode], parent: Option<&Id>, depth: usize, out: &mut Vec<Placed>) {
        for (index, n) in nodes.iter().enumerate() {
            out.push(Placed { id: n.id.clone(), parent: parent.cloned(), depth, index });
            walk(&n.children, Some(&n.id), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(tree, None, 0, &mut out);
    out
}

/// `child → parent` for every element.
#[must_use]
pub fn parents(tree: &[TreeNode]) -> BTreeMap<Id, Option<Id>> {
    placed(tree).into_iter().map(|p| (p.id, p.parent)).collect()
}

/// The node of an element.
#[must_use]
pub fn find<'a>(tree: &'a [TreeNode], id: &Id) -> Option<&'a TreeNode> {
    for n in tree {
        if &n.id == id {
            return Some(n);
        }
        if let Some(f) = find(&n.children, id) {
            return Some(f);
        }
    }
    None
}

/// The sibling list containing an element's level: the children of `parent` (the root list for `None`).
#[must_use]
pub fn level<'a>(tree: &'a [TreeNode], parent: Option<&Id>) -> Option<&'a [TreeNode]> {
    match parent {
        None => Some(tree),
        Some(p) => find(tree, p).map(|n| n.children.as_slice()),
    }
}

fn level_mut<'a>(tree: &'a mut Vec<TreeNode>, parent: Option<&Id>) -> Option<&'a mut Vec<TreeNode>> {
    match parent {
        None => Some(tree),
        Some(p) => {
            for n in tree.iter_mut() {
                if &n.id == p {
                    return Some(&mut n.children);
                }
                if let Some(l) = level_mut(&mut n.children, Some(p)) {
                    return Some(l);
                }
            }
            None
        }
    }
}

/// The IDs of one level, top to bottom.
#[must_use]
pub fn level_ids(tree: &[TreeNode], parent: Option<&Id>) -> Vec<Id> {
    level(tree, parent).map(|l| l.iter().map(|n| n.id.clone()).collect()).unwrap_or_default()
}

/// Whether `id` is `ancestor` or one of its descendants.
#[must_use]
pub fn is_within(tree: &[TreeNode], id: &Id, ancestor: &Id) -> bool {
    let ps = parents(tree);
    let mut cur = Some(id.clone());
    while let Some(c) = cur {
        if &c == ancestor {
            return true;
        }
        cur = ps.get(&c).cloned().flatten();
    }
    false
}

/// Every descendant of an element in Timeline order (not the element itself).
#[must_use]
pub fn descendants(tree: &[TreeNode], id: &Id) -> Vec<Id> {
    find(tree, id).map(|n| placed(&n.children).into_iter().map(|p| p.id).collect()).unwrap_or_default()
}

/// Detaches an element's subtree; returns it (with its children) or `None` when absent.
pub fn detach(tree: &mut Vec<TreeNode>, id: &Id) -> Option<TreeNode> {
    if let Some(i) = tree.iter().position(|n| &n.id == id) {
        return Some(tree.remove(i));
    }
    for n in tree.iter_mut() {
        if let Some(d) = detach(&mut n.children, id) {
            return Some(d);
        }
    }
    None
}

/// Inserts a node into the children of `parent` (root when `None`) at `index` (clamped; `None` = bottom).
/// Returns `false` when the parent is not in the tree.
pub fn attach(tree: &mut Vec<TreeNode>, parent: Option<&Id>, index: Option<usize>, node: TreeNode) -> bool {
    let Some(l) = level_mut(tree, parent) else { return false };
    let i = index.unwrap_or(l.len()).min(l.len());
    l.insert(i, node);
    true
}

/// Removes elements, re-attaching each removed element's children to its parent at its position, keeping their
/// order (04 §8.3).
#[must_use]
pub fn without(tree: &[TreeNode], gone: &dyn Fn(&Id) -> bool) -> Vec<TreeNode> {
    let mut out = Vec::new();
    for n in tree {
        let children = without(&n.children, gone);
        if gone(&n.id) {
            out.extend(children);
        } else {
            out.push(TreeNode { id: n.id.clone(), children });
        }
    }
    out
}

/// Replaces the order of one level with `order` (a permutation of the level's IDs); `false` when it is not one.
pub fn reorder(tree: &mut Vec<TreeNode>, parent: Option<&Id>, order: &[Id]) -> bool {
    let Some(l) = level_mut(tree, parent) else { return false };
    let mut have: Vec<&Id> = l.iter().map(|n| &n.id).collect();
    let mut want: Vec<&Id> = order.iter().collect();
    have.sort();
    want.sort();
    if have != want {
        return false;
    }
    let mut taken: Vec<Option<TreeNode>> = l.drain(..).map(Some).collect();
    for id in order {
        if let Some(n) = taken.iter_mut().find(|n| n.as_ref().is_some_and(|n| &n.id == id)).and_then(Option::take) {
            l.push(n);
        }
    }
    true
}

/// Number of nodes.
#[must_use]
pub fn count(tree: &[TreeNode]) -> usize {
    tree.iter().map(|n| 1 + count(&n.children)).sum()
}
