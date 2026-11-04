//! One node at one frame (05 §1.5, §2.2, §2.3, §8.3): static values, animated channels, effective values and `M_local`.

use std::collections::BTreeMap;

use tt_model::document::Document;
use tt_model::entities::channel::{Channel, CurveType};
use tt_model::entities::element::{ApplyPivotTo, Element, ElementType, PivotSet, PositionLoop};
use tt_model::entities::scene::TreeNode;

use crate::function::{self, KeyView};
use crate::matrix::Mat4;
use crate::tcb::{self, Path, Velocity};
use crate::timing::Bar;

/// A 1-D channel as the evaluator reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    /// Curve type.
    pub curve: CurveType,
    /// Keys in local frames.
    pub keys: Vec<KeyView>,
}

impl Curve {
    fn of(c: &Channel) -> Option<Self> {
        let keys = c.keys().as_ref()?;
        Some(Self {
            curve: c.curve_type().unwrap_or(CurveType::Interpolation),
            keys: keys.iter().map(KeyView::from).collect(),
        })
    }
}

/// The static values of a node (05 §1.5, §2.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Statics {
    /// Offset.
    pub position: [f64; 3],
    /// Rotation (camera: roll).
    pub rotation: f64,
    /// Skew.
    pub skew: f64,
    /// Scale.
    pub scale: [f64; 2],
    /// Element rotation pivot.
    pub rotation_pivot: [f64; 2],
    /// Element scale pivot.
    pub scale_pivot: [f64; 2],
    /// Peg or skeleton pivot (with *use child drawing pivot* applied).
    pub pivot: [f64; 2],
    /// Which pivot rule applies.
    pub pivots: PivotSet,
    /// Element: *use rotation pivot*.
    pub rotate_about_pivot: bool,
    /// Peg, skeleton: *apply pivot to*.
    pub apply_pivot_to: ApplyPivotTo,
    /// Constant Z of the whole node.
    pub lock_depth: bool,
    /// Orient to path.
    pub orient_to_path: bool,
}

impl Statics {
    /// The rotation and scale pivots for an animated pivot `ap` (05 §2.3).
    #[must_use]
    pub fn pivots(&self, ap: [f64; 2]) -> ([f64; 2], [f64; 2]) {
        const ZERO: [f64; 2] = [0.0, 0.0];
        let plus = |a: [f64; 2]| [a[0] + ap[0], a[1] + ap[1]];
        match self.pivots {
            PivotSet::Camera => (ZERO, ZERO),
            PivotSet::Element => {
                let cs = plus(self.scale_pivot);
                (if self.rotate_about_pivot { plus(self.rotation_pivot) } else { cs }, cs)
            }
            _ => {
                let p = plus(self.pivot);
                let rotation = matches!(self.apply_pivot_to, ApplyPivotTo::Rotation | ApplyPivotTo::Both);
                let scaling = matches!(self.apply_pivot_to, ApplyPivotTo::Scaling | ApplyPivotTo::Both);
                (if rotation { p } else { ZERO }, if scaling { p } else { ZERO })
            }
        }
    }
}

/// The animated values of a node at a frame (defaults: 0, scale 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animated {
    /// Motion path position.
    pub position: [f64; 3],
    /// Rotation.
    pub rotation: f64,
    /// Scale.
    pub scale: [f64; 2],
    /// Skew.
    pub skew: f64,
    /// Pivot offset.
    pub pivot: [f64; 2],
    /// V(f).
    pub velocity: f64,
    /// Orient-to-path angle.
    pub orient_angle: f64,
}

/// A node's evaluation at one scene frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalEval {
    /// Local frame (before the cycle mapping).
    pub local_frame: f64,
    /// Static values.
    pub statics: Statics,
    /// Animated values.
    pub animated: Animated,
    /// Effective position.
    pub position: [f64; 3],
    /// Effective rotation, with the orient angle.
    pub rotation: f64,
    /// Effective scale.
    pub scale: [f64; 2],
    /// Effective skew, clamped to ±89°.
    pub skew: f64,
    /// Rotation pivot in use.
    pub rotation_pivot: [f64; 2],
    /// Scale pivot in use.
    pub scale_pivot: [f64; 2],
    /// M_local.
    pub matrix: Mat4,
}

/// Everything about a node the evaluator needs, read once per call.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeData {
    /// Element type.
    pub kind: ElementType,
    /// The bar.
    pub bar: Bar,
    /// Static values; `None` for nodes that pass the transform through (effects, parameter rows, bones in C3.2).
    pub statics: Option<Statics>,
    /// The motion path.
    pub path: Option<Path>,
    /// The velocity channel.
    pub velocity: Velocity,
    /// The other 1-D channels by id.
    pub curves: BTreeMap<String, Curve>,
    /// Position looping mode.
    pub position_loop: PositionLoop,
}

impl NodeData {
    /// Work units reading a node costs.
    #[must_use]
    pub fn cost(e: &Element) -> u64 {
        let mut n: u64 = 16;
        for c in e.channels().iter().flat_map(BTreeMap::values) {
            n = n.saturating_add(u64::try_from(c.keys().as_ref().map_or(0, Vec::len)).unwrap_or(u64::MAX));
            n = n.saturating_add(Path::cost(c.points().as_ref().map_or(0, Vec::len)));
        }
        n
    }

    /// Reads a node. `tree` is its scene's element tree (for *use child drawing pivot*, 05 §2.3).
    #[must_use]
    pub fn read(doc: &Document, tree: &[TreeNode], e: &Element, scene_length: u64) -> Self {
        let statics = e.transform().as_ref().and_then(|t| {
            let set = PivotSet::of(*e.element_type())?;
            let p2 = |p: &Option<tt_model::value::Point2>| p.map_or([0.0, 0.0], |q| [q.x, q.y]);
            let mut pivot = p2(&t.pivot);
            if t.use_child_drawing_pivot == Some(true)
                && let Some(n) = crate::hierarchy::find(tree, e.id())
                && let [only] = n.children.as_slice()
                && let Some(c) = doc.element(&only.id).filter(|c| *c.element_type() == ElementType::Drawing)
                && let Some(ct) = c.transform()
            {
                let sp = p2(&ct.scale_pivot);
                pivot = [sp[0] + ct.offset.x, sp[1] + ct.offset.y];
            }
            Some(Statics {
                position: [t.offset.x, t.offset.y, t.offset.z],
                rotation: t.rotation,
                skew: t.skew,
                scale: [t.scale.x, t.scale.y],
                rotation_pivot: p2(&t.rotation_pivot),
                scale_pivot: p2(&t.scale_pivot),
                pivot,
                pivots: set,
                rotate_about_pivot: t.rotate_about_pivot == Some(true),
                apply_pivot_to: t.apply_pivot_to.unwrap_or(ApplyPivotTo::Rotation),
                lock_depth: t.lock_depth,
                orient_to_path: t.orient_to_path,
            })
        });
        let lock_depth = statics.as_ref().is_some_and(|s| s.lock_depth);
        let mut path = None;
        let mut velocity = Velocity::default();
        let mut curves = BTreeMap::new();
        for (name, c) in e.channels().iter().flat_map(BTreeMap::iter) {
            if let Some(points) = c.points() {
                path = Some(Path::new(points.iter().map(tcb::Point::from).collect(), lock_depth));
            } else if name == "velocity" {
                if let Some(v) = Curve::of(c) {
                    velocity = Velocity { curve: v.curve, keys: v.keys };
                }
            } else if let Some(v) = Curve::of(c) {
                curves.insert(name.clone(), v);
            }
        }
        Self {
            kind: *e.element_type(),
            bar: Bar::of(e, scene_length),
            statics,
            path,
            velocity,
            curves,
            position_loop: *e.position_loop(),
        }
    }

    /// V at a cycle frame (the node's arc length, 0 without a path).
    #[must_use]
    pub fn velocity_at(&self, f: f64) -> f64 {
        self.path.as_ref().map_or(0.0, |p| self.velocity.value(p, f))
    }

    /// A 1-D channel at a cycle frame, or `None` when it has no keys.
    #[must_use]
    pub fn curve_at(&self, channel: &str, f: f64) -> Option<f64> {
        let c = self.curves.get(channel)?;
        let progress = |x: f64| self.velocity_at(x);
        let v: Option<&dyn Fn(f64) -> f64> = if self.path.is_some() { Some(&progress) } else { None };
        function::evaluate(c.curve, &c.keys, f, v)
    }

    /// The animated position at a local frame, with the location on the curve; `positionLoop: offset` adds the first
    /// cycle's displacement per cycle (05 §8.4).
    #[must_use]
    pub fn position_at(&self, local: f64) -> tcb::Travel {
        let Some(path) = &self.path else { return tcb::Travel { position: [0.0; 3], at: None } };
        let lock = self.statics.as_ref().is_some_and(|s| s.lock_depth);
        let (fc, k) = self.bar.cycle_frame(local);
        let mut t = tcb::travel(path, &self.velocity, lock, fc);
        if k > 0.0
            && self.position_loop == PositionLoop::Offset
            && let Some(l) = self.bar.cycle
        {
            let a = tcb::travel(path, &self.velocity, lock, 1.0).position;
            let b = tcb::travel(path, &self.velocity, lock, l).position;
            for i in 0..3 {
                t.position[i] += k * (b[i] - a[i]);
            }
        }
        t
    }

    /// Evaluates the node at scene frame `frame` (05 §1.5, §2.2). `spend` is charged for the orient-to-path search.
    pub fn evaluate<E>(&self, frame: f64, spend: &mut dyn FnMut(u64) -> Result<(), E>) -> Result<LocalEval, E> {
        let local = self.bar.local(frame);
        let (fc, _) = self.bar.cycle_frame(local);
        let travel = self.position_at(local);
        let at = |c: &str, default: f64| self.curve_at(c, fc).unwrap_or(default);
        let statics = self.statics.unwrap_or(Statics {
            position: [0.0; 3],
            rotation: 0.0,
            skew: 0.0,
            scale: [1.0, 1.0],
            rotation_pivot: [0.0, 0.0],
            scale_pivot: [0.0, 0.0],
            pivot: [0.0, 0.0],
            pivots: PivotSet::Camera,
            rotate_about_pivot: false,
            apply_pivot_to: ApplyPivotTo::Rotation,
            lock_depth: false,
            orient_to_path: false,
        });
        let orient_angle = match &self.path {
            Some(p) if statics.orient_to_path => tcb::orient_angle(p, travel.at, fc, spend)?,
            _ => 0.0,
        };
        let animated = Animated {
            position: travel.position,
            rotation: at("rotation", 0.0),
            scale: [at("scale.x", 1.0), at("scale.y", 1.0)],
            skew: at("skew", 0.0),
            pivot: [at("pivot.x", 0.0), at("pivot.y", 0.0)],
            velocity: self.velocity_at(fc),
            orient_angle,
        };
        let position = [
            statics.position[0] + animated.position[0],
            statics.position[1] + animated.position[1],
            statics.position[2] + animated.position[2],
        ];
        let rotation = statics.rotation + animated.rotation + orient_angle;
        let scale = [statics.scale[0] * animated.scale[0], statics.scale[1] * animated.scale[1]];
        let skew = (statics.skew + animated.skew).clamp(-89.0, 89.0);
        let (cr, cs) = statics.pivots(animated.pivot);
        let matrix =
            if self.statics.is_some() { local_matrix(position, rotation, skew, scale, cr, cs) } else { Mat4::IDENTITY };
        Ok(LocalEval {
            local_frame: local,
            statics,
            animated,
            position,
            rotation,
            scale,
            skew,
            rotation_pivot: cr,
            scale_pivot: cs,
            matrix,
        })
    }
}

/// `M_local = T(P) · T(cr) · R(θ) · T(−cr) · T(cs) · K(κ) · S(sx, sy) · T(−cs)` (05 §2.2), composed left to right.
#[must_use]
pub fn local_matrix(p: [f64; 3], theta: f64, kappa: f64, s: [f64; 2], cr: [f64; 2], cs: [f64; 2]) -> Mat4 {
    Mat4::translate(p[0], p[1], p[2])
        .mul(&Mat4::translate(cr[0], cr[1], 0.0))
        .mul(&Mat4::rotate(theta))
        .mul(&Mat4::translate(-cr[0], -cr[1], 0.0))
        .mul(&Mat4::translate(cs[0], cs[1], 0.0))
        .mul(&Mat4::skew(kappa))
        .mul(&Mat4::scale(s[0], s[1]))
        .mul(&Mat4::translate(-cs[0], -cs[1], 0.0))
}
