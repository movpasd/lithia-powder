//! voxel map and mesh generation utilities

use glam::{IVec3, Vec4, ivec3, vec4};

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
        F: Fn(IVec3) -> Block,
    {
        Self {
            blocks: Box::new(std::array::from_fn(|idx| f(Self::idx_to_cell_loc(idx)))),
        }
    }
    pub fn get_block(&self, cell_loc: IVec3) -> Block {
        self.blocks[Self::cell_loc_to_idx(cell_loc)]
    }
    pub fn try_get_block(&self, cell_loc: IVec3) -> Option<Block> {
        if cell_loc.cmplt(IVec3::ZERO).any() || cell_loc.cmpgt(IVec3::ONE * 31).any() {
            None
        } else {
            self.blocks.get(Self::cell_loc_to_idx(cell_loc)).copied()
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter()
    }
    pub fn iter_indexed(&self) -> impl Iterator<Item = (IVec3, &Block)> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(idx, b)| (Self::idx_to_cell_loc(idx), b))
    }
}
impl Chunk {
    fn cell_loc_to_idx(cell_loc: IVec3) -> usize {
        (cell_loc.x * 32 * 32 + cell_loc.y * 32 + cell_loc.z) as usize
    }
    fn idx_to_cell_loc(idx: usize) -> IVec3 {
        let idx_u32 = idx as i32;
        let x = idx_u32 / (32 * 32);
        let x_rem = idx_u32 % (32 * 32);
        let y = x_rem / 32;
        let y_rem = x_rem % 32;
        let z = y_rem;

        ivec3(x, y, z)
    }
}

impl Chunk {
    pub fn to_mesh(&self) -> Mesh<Vec4> {
        chunk_mesh_building::chunk_to_mesh(self)
    }
}
mod chunk_mesh_building {
    use std::iter::zip;

    use glam::{Affine3, IVec3, Vec3, Vec4, ivec3, vec3};

    use super::Chunk;
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
        (Vec3::Y, 90_f32.to_radians()),
        (Vec3::Y, -90_f32.to_radians()),
        (Vec3::X, -90_f32.to_radians()),
        (Vec3::X, 90_f32.to_radians()),
        (Vec3::X, 0_f32.to_radians()),
        (Vec3::X, 180_f32.to_radians()),
    ];

    pub fn chunk_to_mesh(chunk: &Chunk) -> Mesh<Vec4> {
        let mut mesh = Mesh::<Vec4>::new_empty();
        for (cell_loc, block) in chunk.iter_indexed() {
            'faces_loop: for (cell_delta, (rot_axis, rot_angle)) in
                zip(FACE_CELL_DELTAS, FACE_ROTATIONS)
            {
                let cull_face = !block.is_solid()
                    || chunk
                        .try_get_block(cell_loc + cell_delta)
                        .map(|neighbour| neighbour.is_solid())
                        .unwrap_or(false);
                if cull_face {
                    continue 'faces_loop;
                }

                let mut face = mesh::unit_square().map_data(|_| block.color());
                face.transform_affine(
                    Affine3::from_axis_angle(rot_axis, rot_angle)
                        * Affine3::from_translation(vec3(-0.5, -0.5, 0.5)),
                );
                face.transform_affine(Affine3::from_translation(cell_loc.as_vec3() + 0.5));
                face.transform_scale(Chunk::BLOCK_SIZE);
                mesh.append(&mut face);
            }
        }
        mesh
    }
}
