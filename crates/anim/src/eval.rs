//! The evaluator of one call (05 §2.4, §2.9): nodes are read once, world matrices are memoised per scene frame, and
//! all work is charged to the caller's meter so a long range is refused or cancelled, never blocks (ADR-0019).

use std::collections::BTreeMap;

use tt_model::Id;
use tt_model::document::Document;
use tt_model::entities::element::{CameraBlock, FovMode};

use crate::camera::CameraView;
use crate::hierarchy;
use crate::matrix::Mat4;
use crate::node::{LocalEval, NodeData};

/// Work units of evaluating one node at one frame (the channels of §1.5, the matrix product).
pub const NODE_FRAME_COST: u64 = 16;

/// Why an evaluation stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum EvalError<E> {
    /// The meter refused (budget or cancellation).
    Meter(E),
    /// No such node.
    NotFound(Id),
    /// The result left the finite range (extreme values through a deep hierarchy).
    NonFinite(Id),
}

impl<E> From<E> for EvalError<E> {
    fn from(e: E) -> Self {
        Self::Meter(e)
    }
}

/// One evaluation session over a document snapshot.
pub struct Evaluator<'d, E> {
    doc: &'d Document,
    spend: Box<dyn FnMut(u64) -> Result<(), E> + 'd>,
    nodes: BTreeMap<Id, NodeData>,
    parents: BTreeMap<Id, BTreeMap<Id, Option<Id>>>,
    memo_frame: f64,
    memo: BTreeMap<Id, Mat4>,
}

impl<'d, E> Evaluator<'d, E> {
    /// A session charging `spend`.
    pub fn new(doc: &'d Document, spend: impl FnMut(u64) -> Result<(), E> + 'd) -> Self {
        Self {
            doc,
            spend: Box::new(spend),
            nodes: BTreeMap::new(),
            parents: BTreeMap::new(),
            memo_frame: f64::NAN,
            memo: BTreeMap::new(),
        }
    }

    /// Charges the meter directly (for the caller's own per-result work).
    pub fn charge(&mut self, n: u64) -> Result<(), E> {
        (self.spend)(n)
    }

    /// A node's data, read once.
    pub fn node(&mut self, id: &Id) -> Result<&NodeData, EvalError<E>> {
        if !self.nodes.contains_key(id) {
            let e = self.doc.element(id).ok_or_else(|| EvalError::NotFound(id.clone()))?;
            let scene =
                self.doc.scene_of(id).and_then(|s| self.doc.scene(s)).ok_or_else(|| EvalError::NotFound(id.clone()))?;
            (self.spend)(NodeData::cost(e))?;
            let data = NodeData::read(self.doc, scene.tree(), e, self.doc.scene_length(scene));
            self.nodes.insert(id.clone(), data);
        }
        self.nodes.get(id).ok_or_else(|| EvalError::NotFound(id.clone()))
    }

    /// The parent of a node.
    fn parent(&mut self, id: &Id) -> Result<Option<Id>, EvalError<E>> {
        let scene = self.doc.scene_of(id).cloned().ok_or_else(|| EvalError::NotFound(id.clone()))?;
        if !self.parents.contains_key(&scene) {
            let sc = self.doc.scene(&scene).ok_or_else(|| EvalError::NotFound(id.clone()))?;
            (self.spend)(u64::try_from(hierarchy::count(sc.tree())).unwrap_or(u64::MAX))?;
            self.parents.insert(scene.clone(), hierarchy::parents(sc.tree()));
        }
        Ok(self.parents.get(&scene).and_then(|m| m.get(id)).cloned().flatten())
    }

    /// The node at a scene frame.
    pub fn local(&mut self, id: &Id, frame: f64) -> Result<LocalEval, EvalError<E>> {
        self.node(id)?;
        (self.spend)(NODE_FRAME_COST)?;
        let data = self.nodes.get(id).ok_or_else(|| EvalError::NotFound(id.clone()))?;
        let out = data.evaluate(frame, &mut *self.spend)?;
        if !out.matrix.is_finite() {
            return Err(EvalError::NonFinite(id.clone()));
        }
        Ok(out)
    }

    /// W(node, F) = W(parent, F) · M_local(node, f_node(F)) (05 §2.4), iteratively from the topmost uncached ancestor.
    pub fn world(&mut self, id: &Id, frame: f64) -> Result<Mat4, EvalError<E>> {
        if self.memo_frame != frame {
            self.memo.clear();
            self.memo_frame = frame;
        }
        let mut chain = vec![id.clone()];
        let mut base = Mat4::IDENTITY;
        while let Some(top) = chain.last().cloned() {
            if let Some(m) = self.memo.get(&top) {
                base = *m;
                chain.pop();
                break;
            }
            match self.parent(&top)? {
                Some(p) => chain.push(p),
                None => break,
            }
        }
        while let Some(n) = chain.pop() {
            let l = self.local(&n, frame)?;
            base = base.mul(&l.matrix);
            if !base.is_finite() {
                return Err(EvalError::NonFinite(n));
            }
            self.memo.insert(n, base);
        }
        Ok(base)
    }

    /// The parent's world matrix (identity at the top level).
    pub fn parent_world(&mut self, id: &Id, frame: f64) -> Result<Mat4, EvalError<E>> {
        match self.parent(id)? {
            Some(p) => self.world(&p, frame),
            None => Ok(Mat4::IDENTITY),
        }
    }

    /// The effective field of view of a camera (05 §2.9): the static value, or the `fov` channel in dynamic mode.
    pub fn fov(&mut self, id: &Id, block: &CameraBlock, frame: f64) -> Result<f64, EvalError<E>> {
        if block.fov_mode == FovMode::Static {
            return Ok(block.fov);
        }
        let data = self.node(id)?;
        let (fc, _) = data.bar.cycle_frame(data.bar.local(frame));
        Ok(data.curve_at("fov", fc).unwrap_or(block.fov).clamp(1.0, 179.0))
    }

    /// A camera's view at a scene frame.
    pub fn camera(&mut self, id: &Id, frame: f64) -> Result<CameraView, EvalError<E>> {
        let doc = self.doc;
        let e = doc.element(id).ok_or_else(|| EvalError::NotFound(id.clone()))?;
        let block = e.camera().as_ref().ok_or_else(|| EvalError::NotFound(id.clone()))?;
        let world = self.world(id, frame)?;
        let fov = self.fov(id, block, frame)?;
        let p = doc.project();
        Ok(CameraView::new(&world, fov, f64::from(*p.camera_width()), f64::from(*p.camera_height())))
    }
}
