use glam::{vec3, Mat4, Vec4};

/// (make sure your positions have w=1.0 and normals have w=0.0)
#[derive(Debug, Clone)]
pub struct Vertex<D> {
    pub position: Vec4,
    pub normal: Vec4,
    pub data: D,
}

/// (do not store more than u32::MAX)
#[derive(Debug, Clone)]
pub struct Mesh<D> {
    pub vertexes: Vec<Vertex<D>>,
    pub indexes: Vec<u32>,
}
impl<D> Mesh<D> {
    pub fn new_empty() -> Self {
        Mesh {
            vertexes: vec![],
            indexes: vec![],
        }
    }
    pub fn append(&mut self, other: &mut Self) {
        other
            .indexes
            .iter_mut()
            .for_each(|i| *i += self.vertexes.len() as u32);
        self.vertexes.append(&mut other.vertexes);
        self.indexes.append(&mut other.indexes);
    }
    pub fn transform(&mut self, m: Mat4) {
        self.vertexes.iter_mut().for_each(|v| {
            v.position = m * v.position;
            v.normal = m * v.normal
        });
    }
}

pub fn colorful_cube() -> Mesh<Vec4> {
    use std::f32::consts::{FRAC_PI_2, PI};

    let mut plus_z_face = {
        let vertex_positions = [
            [-0.5, -0.5, 0.0, 1.0],
            [0.5, -0.5, 0.0, 1.0],
            [-0.5, 0.5, 0.0, 1.0],
            [0.5, 0.5, 0.0, 1.0],
        ];
        let vertex_colors = [
            [0.0, 1.0, 1.0, 1.0],
            [0.75, 0.25, 1.0, 1.0],
            [1.0, 0.5, 0.5, 1.0],
            [0.75, 1.0, 0.25, 1.0],
        ];
        let vertex_normals = [
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        let vertexes: Vec<Vertex<_>> =
            itertools::izip!(vertex_positions, vertex_colors, vertex_normals)
                .map(|(pos_arr, col_arr, norm_arr)| Vertex {
                    position: Vec4::from_array(pos_arr),
                    normal: Vec4::from_array(norm_arr),
                    data: Vec4::from_array(col_arr),
                })
                .collect();

        let indexes = vec![0, 1, 3, 3, 2, 0];

        Mesh { vertexes, indexes }
    };
    plus_z_face.transform(Mat4::from_translation(vec3(0.0, 0.0, 0.5)));

    // relative to the +Z face
    let transformations = [
        Mat4::IDENTITY,
        Mat4::from_axis_angle(vec3(0.0, 1.0, 0.0), PI),
        Mat4::from_axis_angle(vec3(0.0, 1.0, 0.0), FRAC_PI_2),
        Mat4::from_axis_angle(vec3(0.0, 1.0, 0.0), -FRAC_PI_2),
        Mat4::from_axis_angle(vec3(1.0, 0.0, 0.0), FRAC_PI_2),
        Mat4::from_axis_angle(vec3(1.0, 0.0, 0.0), -FRAC_PI_2),
    ];

    let mut cube = Mesh::new_empty();
    for transform in transformations {
        let mut next_face = plus_z_face.clone();
        next_face.transform(transform);

        cube.append(&mut next_face);
    }

    cube
}
