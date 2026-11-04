//! Cameras and projection (05 §2.6). The camera is rigid: only translation and Z rotation are taken from its world
//! matrix; scale and skew inherited from ancestors are ignored.

use crate::matrix::Mat4;
use crate::num;

/// The near plane, in fields (05 §2.6): points at this depth or closer are not drawn.
pub const NEAR_PLANE: f64 = 0.1;

/// A camera at one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraView {
    /// World position.
    pub position: [f64; 3],
    /// Roll, degrees.
    pub roll: f64,
    /// View matrix (world → camera space).
    pub view: Mat4,
    /// Effective horizontal field of view, degrees.
    pub fov: f64,
    /// Output width in pixels.
    pub width: f64,
    /// Output height in pixels.
    pub height: f64,
}

/// A projected point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projected {
    /// Depth d = −q.z.
    pub depth: f64,
    /// Output pixel, `None` at or behind the near plane.
    pub pixel: Option<[f64; 2]>,
}

impl CameraView {
    /// The view of a camera with world matrix `world`.
    #[must_use]
    pub fn new(world: &Mat4, fov: f64, width: f64, height: f64) -> Self {
        let d = world.decompose();
        let (c, s) = (num::cos_deg(d.rotation), num::sin_deg(d.rotation));
        let p = d.position;
        // R(−θ) · T(−p).
        let view = Mat4([
            c,
            s,
            0.0,
            -(c * p[0] + s * p[1]),
            -s,
            c,
            0.0,
            -(-s * p[0] + c * p[1]),
            0.0,
            0.0,
            1.0,
            -p[2],
            0.0,
            0.0,
            0.0,
            1.0,
        ]);
        Self { position: p, roll: d.rotation, view, fov, width, height }
    }

    /// Projects a world point (05 §2.6).
    #[must_use]
    pub fn project(&self, p: [f64; 3]) -> Projected {
        let q = self.view.apply(p);
        let depth = -q[2];
        if depth <= NEAR_PLANE {
            return Projected { depth, pixel: None };
        }
        let h = num::tan_deg(self.fov / 2.0);
        let xn = q[0] / (depth * h);
        let yn = q[1] / (depth * h * self.height / self.width);
        Projected { depth, pixel: Some([(xn + 1.0) / 2.0 * self.width, (1.0 - yn) / 2.0 * self.height]) }
    }

    /// The world point of an output pixel at depth `depth` in front of the camera (the inverse of [`Self::project`]).
    #[must_use]
    pub fn unproject(&self, pixel: [f64; 2], depth: f64) -> Option<[f64; 3]> {
        let h = num::tan_deg(self.fov / 2.0);
        let xn = pixel[0] / self.width * 2.0 - 1.0;
        let yn = 1.0 - pixel[1] / self.height * 2.0;
        let q = [xn * depth * h, yn * depth * h * self.height / self.width, -depth];
        Some(self.view.inverse()?.apply(q))
    }

    /// The camera frame's corners on the table plane Z = 0, top-left first, clockwise; `None` when the plane is not
    /// in front of the near plane (SI-100).
    #[must_use]
    pub fn frame_corners_at_z0(&self) -> Option<[[f64; 3]; 4]> {
        let depth = self.position[2];
        if depth <= NEAR_PLANE {
            return None;
        }
        let w = self.width;
        let h = self.height;
        Some([
            self.unproject([0.0, 0.0], depth)?,
            self.unproject([w, 0.0], depth)?,
            self.unproject([w, h], depth)?,
            self.unproject([0.0, h], depth)?,
        ])
    }
}
