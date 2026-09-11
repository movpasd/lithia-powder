//! Fundamental geometry utilities

use glam::{Mat4, Quat, Vec3};

/// represents a rigid body configuration state
#[derive(Debug, Clone, Copy, Default)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}
impl Pose {
    pub fn to_transform(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.position)
    }
}
