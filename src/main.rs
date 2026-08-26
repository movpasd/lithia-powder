#![allow(dead_code)]

use std::{f32::consts::TAU, time::Instant};

use glam::{vec3, Mat4, Vec3, Vec4};
use sdl3::{
    self,
    gpu::{
        BufferBinding, BufferRegion, BufferUsageFlags, ColorTargetDescription, ColorTargetInfo,
        CompareOp, CullMode, DepthStencilState, Device, FillMode, FrontFace, GraphicsPipeline,
        GraphicsPipelineTargetInfo, IndexElementSize, RasterizerState, Shader, ShaderFormat,
        ShaderStage, TransferBufferLocation, VertexAttribute, VertexBufferDescription,
        VertexElementFormat, VertexInputRate, VertexInputState,
    },
    keyboard::Keycode,
    pixels::Color,
    sys::gpu::SDL_GPULoadOp,
    video::Window,
};

fn main() {
    let sdl = sdl3::init().unwrap();

    // window setup
    let video_sys = sdl.video().unwrap();
    let window = video_sys
        .window("lithia-powder", 980, 640)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    // GPU setup
    let mut device = sdl3::gpu::Device::new(ShaderFormat::SPIRV, false).unwrap();
    device = device.with_window(&window).unwrap();

    // logic data
    let meshes: Vec<Mesh> = vec![
        cube_mesh(),
        {
            let mut c = cube_mesh();
            c.transform(Mat4::from_translation(vec3(3.0, 0.0, 0.0)));
            c
        },
        {
            let mut c = cube_mesh();
            c.transform(Mat4::from_translation(vec3(1.5, 3.0, 0.0)));
            c
        },
    ];
    let mut poses: Vec<anim::Pose> = vec![
        anim::Pose::default(),
        anim::Pose::default(),
        anim::Pose::default(),
    ];
    assert!(meshes.len() == poses.len());

    let mut camera_pos: Vec3;
    let mut view: Mat4;
    let mut persp: Mat4;

    // upload data to GPU geometry buffers
    let vertex_buffer = device
        .create_buffer()
        .with_usage(BufferUsageFlags::VERTEX)
        .with_size(1_024 * 1_024)
        .build()
        .unwrap();
    let index_buffer = device
        .create_buffer()
        .with_usage(BufferUsageFlags::INDEX)
        .with_size(1_024 * 1_024)
        .build()
        .unwrap();
    let vbuf_entries: Vec<(u32, u32)>; // (offset in bytes, size in bytes)
    let ibuf_entries: Vec<(u32, u32)>; // idem
    (vbuf_entries, ibuf_entries) = {
        // accumulate data into local byte array, keeping track of entries
        let mut vbuf_data: Vec<u8> = vec![];
        let mut ibuf_data: Vec<u8> = vec![];
        let mut vbuf_entries = vec![];
        let mut ibuf_entries = vec![];
        let mut next_vbuf_offset: u32 = 0;
        let mut next_ibuf_offset: u32 = 0;
        for mesh in &meshes {
            let vbytes = bytemuck::cast_slice::<_, u8>(&mesh.vertexes);
            let vsize = vbytes.len() as u32;
            vbuf_entries.push((next_vbuf_offset, vsize));
            vbuf_data.extend_from_slice(vbytes);
            next_vbuf_offset += vsize;

            let ibytes = bytemuck::cast_slice::<_, u8>(&mesh.indexes);
            let isize = ibytes.len() as u32;
            ibuf_entries.push((next_ibuf_offset, vsize));
            ibuf_data.extend_from_slice(ibytes);
            next_ibuf_offset += isize;
        }
        {
            let vertex_transfer_buf = device
                .create_transfer_buffer()
                .with_size(vertex_buffer.len())
                .build()
                .unwrap();
            let index_transfer_buf = device
                .create_transfer_buffer()
                .with_size(index_buffer.len())
                .build()
                .unwrap();

            vertex_transfer_buf.map(&device, true).mem_mut()[0..vbuf_data.len()]
                .copy_from_slice(&vbuf_data);
            index_transfer_buf.map(&device, true).mem_mut()[0..ibuf_data.len()]
                .copy_from_slice(&ibuf_data);

            let data_upload = device.acquire_command_buffer().unwrap();
            {
                let copy_pass = device.begin_copy_pass(&data_upload).unwrap();
                copy_pass.upload_to_gpu_buffer(
                    TransferBufferLocation::new().with_transfer_buffer(&vertex_transfer_buf),
                    BufferRegion::new()
                        .with_buffer(&vertex_buffer)
                        .with_size(vertex_buffer.len()),
                    true,
                );
                copy_pass.upload_to_gpu_buffer(
                    TransferBufferLocation::new().with_transfer_buffer(&index_transfer_buf),
                    BufferRegion::new()
                        .with_buffer(&index_buffer)
                        .with_size(index_buffer.len()),
                    true,
                );
                device.end_copy_pass(copy_pass);
            }
            let data_upload_fence = data_upload.submit_and_acquire_fence(&device).unwrap();
            while !data_upload_fence.query(&device) {}
        }

        (vbuf_entries, ibuf_entries)
    };

    // set up rendering pipeline
    let pipeline = prepare_render_pipeline(&device, &window);

    // event loop
    let start_time = Instant::now();
    let mut event_pump = sdl.event_pump().unwrap();
    'main_loop: loop {
        for event in event_pump.poll_iter() {
            match event {
                sdl3::event::Event::Quit { .. }
                | sdl3::event::Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                }
                | sdl3::event::Event::KeyDown {
                    keycode: Some(Keycode::Q),
                    ..
                } => {
                    break 'main_loop;
                }
                _ => {}
            }
        }

        // logic
        {
            let elapsed_time_secs = (Instant::now() - start_time).as_secs_f32();

            // camera stuff
            {
                // SDL_GPU uses DirectX-like convention
                use glam::camera::rh::{proj::directx::perspective, view::look_at_mat4};

                let (width, height) = window.size();
                persp = perspective(
                    70.0f32.to_radians(),
                    width as f32 / height as f32,
                    0.1,
                    200.0,
                );
                let orbit_period = 60.0;
                let orbit_distance = 1.67;
                let orbit_angle = -TAU / 9.0 + TAU * elapsed_time_secs / orbit_period;

                camera_pos = vec3(
                    orbit_distance * orbit_angle.cos(),
                    orbit_distance * orbit_angle.sin(),
                    2.0,
                );
                view = look_at_mat4(camera_pos, vec3(0.0, 0.0, 0.8), vec3(0.0, 0.0, 1.0));
            }

            // cube animation
            poses[0] = anim::pose(elapsed_time_secs);
        }

        // render
        {
            let mut cbuf = device.acquire_command_buffer().unwrap();

            let screen_texture = cbuf.wait_and_acquire_swapchain_texture(&window).unwrap();
            let color_target_info = ColorTargetInfo::default()
                .with_texture(&screen_texture)
                .with_load_op(SDL_GPULoadOp::CLEAR)
                .with_clear_color(Color::RGB(127, 127, 127));

            let render_pass = device
                .begin_render_pass(&cbuf, &[color_target_info], None)
                .unwrap();
            {
                render_pass.bind_graphics_pipeline(&pipeline);
                render_pass
                    .bind_vertex_buffers(0, &[BufferBinding::new().with_buffer(&vertex_buffer)]);
                render_pass.bind_index_buffer(
                    &BufferBinding::new().with_buffer(&index_buffer),
                    IndexElementSize::_32BIT,
                );

                let vunif_transforms_data = [
                    view,
                    persp,
                    Mat4::from_translation(poses[0].pos),
                    Mat4::from_quat(poses[0].rot),
                ];
                let funif_camera_data = [camera_pos.x, camera_pos.y, camera_pos.z, 1.0];
                cbuf.push_vertex_uniform_data(0, &vunif_transforms_data);
                cbuf.push_fragment_uniform_data(0, &funif_camera_data);

                render_pass.draw_indexed_primitives(index_buffer.len(), 1, 0, 0, 0);
            }
            device.end_render_pass(render_pass);

            let fence = cbuf.submit_and_acquire_fence(&device).unwrap();
            while !fence.query(&device) {}
        }

        std::thread::sleep(std::time::Duration::from_millis(1_000 / 60))
    }
}

// -- rendering --

fn prepare_render_pipeline(device: &Device, window: &Window) -> GraphicsPipeline {
    use sdl3::gpu::PrimitiveType;

    // load and compile shaders
    let vertex_shader: Shader;
    let fragment_shader: Shader;
    {
        use shaderc::ShaderKind;

        let compiler = shaderc::Compiler::new().unwrap();

        let vertex_source = include_str!("shaders/vertex.glsl");
        let vertex_ir = compiler
            .compile_into_spirv(
                vertex_source,
                ShaderKind::Vertex,
                "shaders/vertex.glsl",
                "main",
                None,
            )
            .unwrap();
        vertex_shader = device
            .create_shader()
            .with_code(
                ShaderFormat::SPIRV,
                vertex_ir.as_binary_u8(),
                ShaderStage::Vertex,
            )
            .with_uniform_buffers(1)
            .build()
            .unwrap();

        let fragment_source = include_str!("shaders/fragment.glsl");
        let fragment_ir = compiler
            .compile_into_spirv(
                fragment_source,
                ShaderKind::Fragment,
                "shaders/fragment.glsl",
                "main",
                None,
            )
            .unwrap();
        fragment_shader = device
            .create_shader()
            .with_code(
                ShaderFormat::SPIRV,
                fragment_ir.as_binary_u8(),
                ShaderStage::Vertex,
            )
            .with_uniform_buffers(1)
            .build()
            .unwrap();
    }

    let texture_format = device.get_swapchain_texture_format(window);

    device
        .create_graphics_pipeline()
        .with_vertex_shader(&vertex_shader)
        .with_fragment_shader(&fragment_shader)
        .with_vertex_input_state(
            VertexInputState::new()
                .with_vertex_buffer_descriptions(&[VertexBufferDescription::new()
                    .with_slot(0)
                    .with_pitch(size_of::<Vertex>() as u32)
                    .with_input_rate(VertexInputRate::Vertex)])
                .with_vertex_attributes(&Vertex::get_attributes(0, 0)),
        )
        .with_primitive_type(PrimitiveType::TriangleList)
        .with_rasterizer_state(
            RasterizerState::new()
                .with_fill_mode(FillMode::Fill)
                .with_cull_mode(CullMode::Back)
                .with_front_face(FrontFace::CounterClockwise),
        )
        .with_depth_stencil_state(
            DepthStencilState::new()
                .with_enable_depth_test(true)
                .with_compare_op(CompareOp::Less),
        )
        .with_target_info(
            GraphicsPipelineTargetInfo::new().with_color_target_descriptions(&[
                ColorTargetDescription::new().with_format(texture_format),
            ]),
        )
        .build()
        .unwrap()
}

// -- utilities --

fn mat4_as_glsl(mat: Mat4) -> String {
    let (x, y, z, w) = (mat.x_axis, mat.y_axis, mat.z_axis, mat.w_axis);
    #[rustfmt::skip]
    return format!(
"mat4(
    vec4({:?}, {:?}, {:?}, {:?}),
    vec4({:?}, {:?}, {:?}, {:?}),
    vec4({:?}, {:?}, {:?}, {:?}),
    vec4({:?}, {:?}, {:?}, {:?})
)
",
        x.x, x.y, x.z, x.w,
        y.x, y.y, y.z, y.w,
        z.x, z.y, z.z, z.w,
        w.x, w.y, w.z, w.w,
    );
}

// -- vertex and mesh stuff --

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
struct Vertex {
    position: Vec4,
    color: Vec4,
    normal: Vec4,
}
impl Vertex {
    const ATTRIBUTE_COUNT: u32 = 3;

    fn get_attributes(buffer_slot: u32, first_location: u32) -> Vec<VertexAttribute> {
        vec![
            VertexAttribute::new()
                .with_buffer_slot(buffer_slot)
                .with_location(first_location)
                .with_offset(0)
                .with_format(VertexElementFormat::Float4),
            VertexAttribute::new()
                .with_buffer_slot(buffer_slot)
                .with_location(first_location + 1)
                .with_offset(16)
                .with_format(VertexElementFormat::Float4),
            VertexAttribute::new()
                .with_buffer_slot(buffer_slot)
                .with_location(first_location + 2)
                .with_offset(32)
                .with_format(VertexElementFormat::Float4),
        ]
    }
}

/// do not store more than u32::MAX
#[derive(Debug, Clone)]
struct Mesh {
    vertexes: Vec<Vertex>,
    indexes: Vec<u32>,
}
impl Mesh {
    fn new_empty() -> Mesh {
        Mesh {
            vertexes: vec![],
            indexes: vec![],
        }
    }
    fn len(&self) -> u32 {
        self.indexes.len() as u32
    }
    fn append(&mut self, other: &mut Mesh) {
        other
            .indexes
            .iter_mut()
            .for_each(|i| *i += self.vertexes.len() as u32);
        self.vertexes.append(&mut other.vertexes);
        self.indexes.append(&mut other.indexes);
    }
    fn transform(&mut self, m: Mat4) {
        self.vertexes.iter_mut().for_each(|v| {
            v.position = m * v.position;
            v.normal = m * v.normal
        });
    }
    fn vertexes_bytes_size(&self) -> u32 {
        size_of_val(self.vertexes.as_slice()) as u32
    }
    fn indexes_bytes_size(&self) -> u32 {
        size_of_val(self.indexes.as_slice()) as u32
    }
}

fn cube_mesh() -> Mesh {
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
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
        ];
        let vertexes: Vec<Vertex> =
            itertools::izip!(vertex_positions, vertex_colors, vertex_normals)
                .map(|(pos_arr, col_arr, norm_arr)| Vertex {
                    position: Vec4::from_array(pos_arr),
                    color: Vec4::from_array(col_arr),
                    normal: Vec4::from_array(norm_arr),
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

// -- cube animation --

mod anim {
    use std::{
        f32::consts::TAU,
        ops::{Add, Mul},
    };

    use glam::{vec3, Quat, Vec3};

    const WAIT_TIME: f32 = 1.5;
    const MOVE_TIME: f32 = 0.5;
    const SPIN_TIME: f32 = 0.33;
    const SPIN_OVERLAP_TIME: f32 = 0.4;

    const ANIM_TIME: f32 = WAIT_TIME + MOVE_TIME + SPIN_TIME + MOVE_TIME;
    const Z_KEYFRAME_TIMES: [f32; 4] = [
        0.0,
        WAIT_TIME,
        WAIT_TIME + MOVE_TIME,
        WAIT_TIME + MOVE_TIME + SPIN_TIME,
    ];
    const ROT_KEYFRAME_TIMES: [f32; 4] = const {
        let mut rot_keyframe_times = Z_KEYFRAME_TIMES;
        rot_keyframe_times[2] -= SPIN_OVERLAP_TIME;
        rot_keyframe_times[3] += SPIN_OVERLAP_TIME;
        rot_keyframe_times
    };

    const H: f32 = 1.0;
    const THETA: f32 = TAU;

    #[derive(Debug, Default)]
    pub struct Pose {
        pub pos: Vec3,
        pub rot: Quat,
    }

    pub fn pose(t: f32) -> Pose {
        let t = t % ANIM_TIME;

        #[allow(unused_variables)]
        let z = if (Z_KEYFRAME_TIMES[0]..Z_KEYFRAME_TIMES[1]).contains(&t) {
            // wait
            let subt = t - Z_KEYFRAME_TIMES[0];
            let wlen = Z_KEYFRAME_TIMES[1] - Z_KEYFRAME_TIMES[0];
            0.0
        } else if (Z_KEYFRAME_TIMES[1]..Z_KEYFRAME_TIMES[2]).contains(&t) {
            // move up
            let subt = t - Z_KEYFRAME_TIMES[1];
            let wlen = Z_KEYFRAME_TIMES[2] - Z_KEYFRAME_TIMES[1];
            lerp(0.0, H, smooth(subt / wlen))
        } else if (Z_KEYFRAME_TIMES[2]..Z_KEYFRAME_TIMES[3]).contains(&t) {
            // spin
            let subt = t - Z_KEYFRAME_TIMES[2];
            let wlen = Z_KEYFRAME_TIMES[3] - Z_KEYFRAME_TIMES[2];
            H
        } else if (Z_KEYFRAME_TIMES[3]..ANIM_TIME).contains(&t) {
            // move down
            let subt = t - Z_KEYFRAME_TIMES[3];
            let wlen = ANIM_TIME - Z_KEYFRAME_TIMES[3];
            lerp(H, 0.0, smooth(subt / wlen))
        } else {
            unreachable!()
        };
        let pos = vec3(0.0, 0.0, z);

        #[allow(unused_variables)]
        let angle = if (ROT_KEYFRAME_TIMES[0]..ROT_KEYFRAME_TIMES[1]).contains(&t) {
            // wait
            let subt = t - ROT_KEYFRAME_TIMES[0];
            let wlen = ROT_KEYFRAME_TIMES[1] - ROT_KEYFRAME_TIMES[0];
            0.0
        } else if (ROT_KEYFRAME_TIMES[1]..ROT_KEYFRAME_TIMES[2]).contains(&t) {
            // move up
            let subt = t - ROT_KEYFRAME_TIMES[1];
            let wlen = ROT_KEYFRAME_TIMES[2] - ROT_KEYFRAME_TIMES[1];
            0.0
        } else if (ROT_KEYFRAME_TIMES[2]..ROT_KEYFRAME_TIMES[3]).contains(&t) {
            // spin
            let subt = t - ROT_KEYFRAME_TIMES[2];
            let wlen = ROT_KEYFRAME_TIMES[3] - ROT_KEYFRAME_TIMES[2];
            lerp(0.0, THETA, smooth(subt / wlen))
        } else if (ROT_KEYFRAME_TIMES[3]..ANIM_TIME).contains(&t) {
            // move down
            let subt = t - ROT_KEYFRAME_TIMES[3];
            let wlen = ANIM_TIME - ROT_KEYFRAME_TIMES[3];
            0.0
        } else {
            unreachable!()
        };
        let rot = Quat::from_rotation_x(angle);

        Pose { pos, rot }
    }

    fn lerp<T: Mul<f32, Output = T> + Add<T, Output = T>>(a: T, b: T, x: f32) -> T {
        if x < 0.0 {
            a
        } else if 1.0 <= x {
            b
        } else {
            a * (1.0 - x) + b * x
        }
    }

    fn smooth(x: f32) -> f32 {
        if x < 0.0 {
            0.0
        } else if 1.0 <= x {
            1.0
        } else {
            x * x * (3.0 - 2.0 * x)
        }
    }
}
