//! voxel map and mesh generation utilities

use glam::{UVec3, Vec4, uvec3, vec4};

use super::mesh::Mesh;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Block {
    // reserve 0 for None
    Air = 1,
    Sand = 2,
}
impl Block {
    fn color(&self) -> Vec4 {
        match self {
            Block::Air => vec4(0.0, 0.0, 0.0, 0.0),
            Block::Sand => vec4(244.0 / 255.0, 164.0 / 255.0, 96.0 / 255.0, 1.0),
        }
    }
    fn is_solid(&self) -> bool {
        match self {
            Block::Air => false,
            Block::Sand => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Chunk {
    blocks: Box<[Block; 32 * 32 * 32]>,
}
impl Chunk {
    pub const BLOCK_SIZE: f32 = 0.5;

    pub fn new_air() -> Self {
        Self {
            blocks: Box::new([Block::Air; 32 * 32 * 32]),
        }
    }
    pub fn from_fn<F>(f: F) -> Self
    where
        F: Fn(UVec3) -> Block,
    {
        Self {
            blocks: Box::new(std::array::from_fn(|idx| f(Self::idx_to_cell_id(idx)))),
        }
    }
    pub fn get_block(&self, cell_id: UVec3) -> Block {
        self.blocks[Self::cell_id_to_idx(cell_id)]
    }
    pub fn try_get_block(&self, cell_id: UVec3) -> Option<Block> {
        self.blocks.get(Self::cell_id_to_idx(cell_id)).copied()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter()
    }
    pub fn iter_indexed(&self) -> impl Iterator<Item = (UVec3, &Block)> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(idx, b)| (Self::idx_to_cell_id(idx), b))
    }
}
impl Chunk {
    fn cell_id_to_idx(cell_id: UVec3) -> usize {
        (cell_id.x * 32 * 32 + cell_id.y * 32 + cell_id.z) as usize
    }
    fn idx_to_cell_id(idx: usize) -> UVec3 {
        let idx_u32 = idx as u32;
        let x = idx_u32 / (32 * 32);
        let x_rem = idx_u32 % (32 * 32);
        let y = x_rem / 32;
        let y_rem = x_rem % 32;
        let z = y_rem;

        uvec3(x, y, z)
    }
}

impl Chunk {
    fn to_mesh(&self) -> Mesh<Vec4> {
        chunk_mesh_building::chunk_to_mesh(self)
    }
}
mod chunk_mesh_building {
    use std::iter::zip;

    use glam::{IVec3, Mat4, UVec3, Vec3, Vec4, ivec3, uvec3};

    use super::{Block, Chunk};
    use crate::mesh::{self, Mesh};

    const FACE_CELL_DELTAS: [IVec3; 6] = [
        ivec3(1, 0, 0),
        ivec3(-1, 0, 0),
        ivec3(0, 1, 0),
        ivec3(0, -1, 0),
        ivec3(0, 0, 1),
        ivec3(0, 0, -1),
    ];
    /// rotations to get from the +Z face to the corresponding face, represented as an
    /// axis and an angle
    const FACE_ROTATIONS: [(Vec3, f32); 6] = [
        (Vec3::Y, 90_f32.to_degrees()),
        (Vec3::Y, -90_f32.to_degrees()),
        (Vec3::X, -90_f32.to_degrees()),
        (Vec3::X, 90_f32.to_degrees()),
        (Vec3::X, 0_f32.to_degrees()),
        (Vec3::X, 180_f32.to_degrees()),
    ];

    pub fn chunk_to_mesh(chunk: &Chunk) -> Mesh<Vec4> {
        let mut mesh = Mesh::<Vec4>::new_empty();
        for (cell_id, block) in chunk.iter_indexed() {
            'faces_loop: for (cell_delta, (rot_axis, rot_angle)) in
                zip(FACE_CELL_DELTAS, FACE_ROTATIONS)
            {
                let cull_face = !block.is_solid()
                    || chunk
                        .try_get_block(cell_id.wrapping_add(cell_delta.as_uvec3()))
                        .map(|neighbour| neighbour.is_solid())
                        .unwrap_or(false);
                if cull_face {
                    continue 'faces_loop;
                }

                let mut face = mesh::unit_square().map_data(|_| block.color());
                face.transform(
                    Mat4::from_axis_angle(rot_axis, rot_angle)
                        * Mat4::from_translation(Vec3::Z * 0.5),
                );
                face.transform(Mat4::from_translation(cell_id.as_vec3() + 0.5));
                face.transform(Mat4::from_scale(Vec3::ONE * Chunk::BLOCK_SIZE));
                mesh.append(&mut face);
            }
        }
        mesh
    }
}
