use glam::{Affine3, Vec3, Vec4, vec3, vec4};

/// (make sure your positions have w=1.0 and normals have w=0.0)
#[derive(Debug, Clone)]
pub struct Vertex<D> {
    pub position: Vec3,
    pub normal: Vec3,
    pub data: D,
}
impl<D> Vertex<D> {
    pub fn map_data<D2, F>(self, mut f: F) -> Vertex<D2>
    where
        F: FnMut(D) -> D2,
    {
        Vertex {
            position: self.position,
            normal: self.normal,
            data: f(self.data),
        }
    }
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
    pub fn transform_affine(&mut self, a: Affine3) {
        self.vertexes.iter_mut().for_each(|v| {
            v.position = a.transform_point3(v.position);
            v.normal = a.transform_vector3(v.normal);
        });
    }
    pub fn transform_scale(&mut self, s: f32) {
        self.vertexes.iter_mut().for_each(|v| {
            v.position *= s;
        });
    }
    pub fn map_data<D2, F>(self, mut f: F) -> Mesh<D2>
    where
        F: FnMut(D) -> D2,
    {
        Mesh {
            vertexes: self
                .vertexes
                .into_iter()
                .map(|v| Vertex {
                    position: v.position,
                    normal: v.normal,
                    data: f(v.data),
                })
                .collect(),
            indexes: self.indexes.clone(),
        }
    }
    pub fn map_vertexes<D2, F>(self, f: F) -> Mesh<D2>
    where
        F: FnMut(Vertex<D>) -> Vertex<D2>,
    {
        Mesh {
            vertexes: self.vertexes.into_iter().map(f).collect(),
            indexes: self.indexes.clone(),
        }
    }
    pub fn map_positions<F>(self, mut f: F) -> Mesh<D>
    where
        F: FnMut(Vec3) -> Vec3,
    {
        Mesh {
            vertexes: self
                .vertexes
                .into_iter()
                .map(|v| Vertex {
                    position: f(v.position),
                    normal: v.normal,
                    data: v.data,
                })
                .collect(),
            indexes: self.indexes,
        }
    }
}

/// a unit square centred occupying x=0..1, y=0..1, z=0, facing up (+z)
pub fn unit_square() -> Mesh<()> {
    let vertex_positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ];
    let vertex_normals = [
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];
    let vertexes: Vec<Vertex<_>> = itertools::izip!(vertex_positions, vertex_normals)
        .map(|(pos_arr, norm_arr)| Vertex {
            position: Vec3::from_array(pos_arr),
            normal: Vec3::from_array(norm_arr),
            data: (),
        })
        .collect();

    let indexes = vec![0, 1, 3, 3, 2, 0];

    Mesh { vertexes, indexes }
}

/// a colourful unit cube, centred on the origin (data is given as a RGBA Vec4)
pub fn colorful_cube() -> Mesh<Vec4> {
    use std::f32::consts::{FRAC_PI_2, PI};

    let vertex_colors = [
        [0.0, 1.0, 1.0, 1.0],
        [0.75, 0.25, 1.0, 1.0],
        [1.0, 0.5, 0.5, 1.0],
        [0.75, 1.0, 0.25, 1.0],
    ];

    let mut plus_z_face = {
        let mut i = 0;
        unit_square()
            .map_data(|_| {
                i += 1;
                Vec4::from_array(vertex_colors[i - 1])
            })
            .map_positions(|pos| pos + vec3(-0.5, -0.5, 0.0))
    };
    plus_z_face.transform_affine(Affine3::from_translation(vec3(0.0, 0.0, 0.5)));

    // relative to the +Z face
    let transformations = [
        Affine3::IDENTITY,
        Affine3::from_axis_angle(vec3(0.0, 1.0, 0.0), PI),
        Affine3::from_axis_angle(vec3(0.0, 1.0, 0.0), FRAC_PI_2),
        Affine3::from_axis_angle(vec3(0.0, 1.0, 0.0), -FRAC_PI_2),
        Affine3::from_axis_angle(vec3(1.0, 0.0, 0.0), FRAC_PI_2),
        Affine3::from_axis_angle(vec3(1.0, 0.0, 0.0), -FRAC_PI_2),
    ];

    let mut cube = Mesh::new_empty();
    for transform in transformations {
        let mut next_face = plus_z_face.clone();
        next_face.transform_affine(transform);

        cube.append(&mut next_face);
    }

    cube
}

pub fn floor() -> Mesh<Vec4> {
    let mut mesh = Mesh::new_empty();
    itertools::iproduct!(-10..10, -10..10).for_each(|(i, j)| {
        const LIGHT_COLOUR: Vec4 = vec4(180.0 / 255.0, 175.0 / 255.0, 165.0 / 255.0, 1.0);
        const DARK_COLOUR: Vec4 = vec4(145.0 / 255.0, 140.0 / 255.0, 125.0 / 255.0, 1.0);
        let colour = if (i + j) % 2 == 0 {
            LIGHT_COLOUR
        } else {
            DARK_COLOUR
        };
        let mut square = unit_square()
            .map_data(|_| colour)
            .map_positions(|pos| pos + vec3(i as f32, j as f32, 0.0));
        mesh.append(&mut square)
    });
    mesh
}
