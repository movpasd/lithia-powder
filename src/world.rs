//! voxel map and mesh generation utilities

use glam::{IVec3, Vec3, Vec4, ivec3, vec4};

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
    /// sample relative to the bottom southwest corner of the chunk (low X, low Y, low
    /// Z), in world distance units
    pub fn try_sample_block(&self, coords: Vec3) -> Option<Block> {
        let cell_loc = (coords / Self::BLOCK_SIZE).floor().as_ivec3();
        self.try_get_block(cell_loc)
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

#[derive(Debug, Clone, Copy)]
pub struct ChunkVertexData {
    pub color: Vec4,
    pub corner_state: CornerState,
}
#[derive(Debug, Clone, Copy)]
pub enum CornerState {
    NoCorner,
    TwoFaces,
    ThreeFaces,
}
impl CornerState {
    pub fn corner_occlusion(&self) -> f32 {
        match self {
            CornerState::NoCorner => 0.0,
            CornerState::TwoFaces => 0.33,
            CornerState::ThreeFaces => 0.67,
        }
    }
}
impl Chunk {
    pub fn to_mesh(&self) -> Mesh<ChunkVertexData> {
        chunk_mesh_building::chunk_to_mesh(self)
    }
}
mod chunk_mesh_building {
    use glam::{Affine3, IVec3, Vec3, ivec3, vec3};

    use super::{Block, Chunk, ChunkVertexData, CornerState};
    use crate::mesh::{self, Mesh};

    // face order for face arrays: +X, -X, +Y, -Y, +Z, -Z

    const NEIGHBOUR_DELTAS: [IVec3; 6] = [
        ivec3(1, 0, 0),
        ivec3(-1, 0, 0),
        ivec3(0, 1, 0),
        ivec3(0, -1, 0),
        ivec3(0, 0, 1),
        ivec3(0, 0, -1),
    ];
    /// rotations to get from the +Z face to the corresponding face, represented as an
    /// axis and an angle
    const ROTATIONS: [(Vec3, f32); 6] = [
        (Vec3::Y, 90_f32.to_radians()),
        (Vec3::Y, -90_f32.to_radians()),
        (Vec3::X, -90_f32.to_radians()),
        (Vec3::X, 90_f32.to_radians()),
        (Vec3::X, 0_f32.to_radians()),
        (Vec3::X, 180_f32.to_radians()),
    ];
    const NORMALS: [Vec3; 6] = [
        vec3(1.0, 0.0, 0.0),
        vec3(-1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, -1.0, 0.0),
        vec3(0.0, 0.0, 1.0),
        vec3(0.0, 0.0, -1.0),
    ];

    pub fn chunk_to_mesh(chunk: &Chunk) -> Mesh<ChunkVertexData> {
        let mut mesh = Mesh::<ChunkVertexData>::new_empty();
        for (cell_loc, block) in chunk.iter_indexed() {
            'faces_loop: for (cell_delta, (rot_axis, rot_angle), face_normal) in
                itertools::izip!(NEIGHBOUR_DELTAS, ROTATIONS, NORMALS)
            {
                let cull_face =
                    !block.is_solid() || is_solid(chunk.try_get_block(cell_loc + cell_delta));
                if cull_face {
                    continue 'faces_loop;
                }

                // face without position data provided yet
                let positioned_face = {
                    let mut positioned_face = mesh::unit_square();
                    positioned_face.transform_affine(
                        Affine3::from_axis_angle(rot_axis, rot_angle)
                            * Affine3::from_translation(vec3(-0.5, -0.5, 0.5)),
                    );
                    positioned_face
                        .transform_affine(Affine3::from_translation(cell_loc.as_vec3() + 0.5));
                    positioned_face.transform_scale(Chunk::BLOCK_SIZE);
                    positioned_face
                };

                let mut face = positioned_face.map_vertexes(|v| {
                    let color = block.color();
                    let corner_state: CornerState = {
                        let block_centre = (cell_loc.as_vec3() + 0.5) * Chunk::BLOCK_SIZE;
                        // the diagonal pointing to this corner from its opposite
                        let corner_diagonal = 2.0 * (v.position - block_centre);

                        let is_left_solid = is_solid(chunk.try_sample_block(
                            block_centre
                                + corner_diagonal.rotate_axis(face_normal, 45_f32.to_radians()),
                        ));
                        let is_right_solid = is_solid(chunk.try_sample_block(
                            block_centre
                                + corner_diagonal.rotate_axis(face_normal, -45_f32.to_radians()),
                        ));
                        let is_cross_solid = is_solid(chunk.try_sample_block(
                            block_centre + corner_diagonal.rotate_axis(face_normal, 0.0),
                        ));

                        if is_left_solid && is_right_solid {
                            CornerState::ThreeFaces
                        } else if is_left_solid || is_right_solid || is_cross_solid {
                            CornerState::TwoFaces
                        } else {
                            CornerState::NoCorner
                        }
                    };
                    v.map_data(|_| ChunkVertexData {
                        color,
                        corner_state,
                    })
                });

                mesh.append(&mut face);
            }
        }

        /// utility function
        fn is_solid(block: Option<Block>) -> bool {
            block.map(|neighbour| neighbour.is_solid()).unwrap_or(false)
        }

        mesh
    }
}
