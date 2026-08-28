use std::{f32::consts::TAU, ffi::CStr, time::Instant};

use glam::{vec3, Mat4, Quat, Vec3, Vec4};
use sdl3::{
    self,
    gpu::{
        Buffer, BufferBinding, BufferRegion, BufferUsageFlags, ColorTargetDescription,
        ColorTargetInfo, CompareOp, CullMode, DepthStencilState, DepthStencilTargetInfo, Device,
        FillMode, FrontFace, GraphicsPipeline, GraphicsPipelineTargetInfo, IndexElementSize,
        LoadOp, RasterizerState, SampleCount, Shader, ShaderFormat, ShaderStage, StoreOp, Texture,
        TextureCreateInfo, TextureFormat, TextureType, TextureUsage, TransferBuffer,
        TransferBufferLocation, VertexAttribute, VertexBufferDescription, VertexElementFormat,
        VertexInputRate, VertexInputState,
    },
    keyboard::Keycode,
    pixels::Color,
    video::Window,
};

const EMPTY_C_STRING: &CStr = c"";

fn main() {
    let sdl = sdl3::init().unwrap();

    // geometry data
    let meshes: Vec<meshobj::Mesh<Vec4>> = vec![
        meshobj::colorful_cube(),
        meshobj::colorful_cube(),
        meshobj::colorful_cube(),
    ];
    let mut poses: Vec<anim::Pose> = vec![
        anim::Pose::default(),
        anim::Pose {
            pos: vec3(-3.0, 0.0, 0.0),
            rot: Quat::default(),
        },
        anim::Pose {
            pos: vec3(-1.5, 3.0, 0.0),
            rot: Quat::default(),
        },
    ];
    assert!(meshes.len() == poses.len());

    // camera data
    let mut camera = Camera {
        position: Vec3::ZERO,
        facing: vec3(1.0, 0.0, 0.0),
        fov: 70.0f32.to_radians(),
        aspect_ratio: 1920.0 / 1080.0,
    };

    // window setup
    let video_sys = sdl.video().unwrap();
    let window = video_sys
        .window("lithia-powder", 980, 640)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    // GPU setup
    let mut device = sdl3::gpu::Device::new(ShaderFormat::SPIRV, true).unwrap();
    device = device.with_window(&window).unwrap();
    unsafe {
        let properties = sdl3::sys::gpu::SDL_GetGPUDeviceProperties(device.raw());
        let property_value = CStr::from_ptr(sdl3::sys::properties::SDL_GetStringProperty(
            properties,
            sdl3::sys::gpu::SDL_PROP_GPU_DEVICE_NAME_STRING,
            EMPTY_C_STRING.as_ptr(),
        ));
        println!("{property_value:?}");
    }

    // GPU declarations
    let mut gpu_resources = prepare_gpu_resources(&device);
    let pipeline = prepare_render_pipeline(&device, &window);

    // mesh data upload
    let mesh_buf_entries = upload_mesh_data(
        &device,
        &meshes,
        &gpu_resources.tbuf1,
        &gpu_resources.tbuf2,
        &gpu_resources.mesh_vbuf,
        &gpu_resources.mesh_ibuf,
    );

    // event loop
    let start_time = Instant::now();
    let mut event_pump = sdl.event_pump().unwrap();
    'main_loop: loop {
        let elapsed_time_secs = (Instant::now() - start_time).as_secs_f32();

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
            // camera stuff
            {
                const ORBIT_PERIOD: f32 = 60.0;
                const ORBIT_BIRDSEYE_DISTANCE: f32 = 4.5;
                const ORBIT_HEIGHT: f32 = 1.2;
                const ORBIT_PHASE_INIT: f32 = -TAU / 9.0;
                const CAMERA_LOOK_AT: Vec3 = vec3(0.0, 0.0, 0.8);

                let orbit_phase = ORBIT_PHASE_INIT + TAU * elapsed_time_secs / ORBIT_PERIOD;
                let position = vec3(
                    ORBIT_BIRDSEYE_DISTANCE * orbit_phase.cos(),
                    ORBIT_BIRDSEYE_DISTANCE * orbit_phase.sin(),
                    ORBIT_HEIGHT,
                );
                let facing = (CAMERA_LOOK_AT - position).normalize();

                let (width, height) = window.size();
                let aspect_ratio = width as f32 / height as f32;

                camera.position = position;
                camera.facing = facing;
                camera.aspect_ratio = aspect_ratio;
            }

            // cube animation
            poses[0] = anim::pose(elapsed_time_secs);
        }

        // render
        {
            let mut cbuf = device.acquire_command_buffer().unwrap();

            let color_target_info = {
                // need to grab screen texture and convert it to a color target _first_,
                // because .wait_and_acquire_swapchain_texture() takes cbuf as &mut (for
                // seemingly no reason nor safety improvement)
                let screen_texture = cbuf.wait_and_acquire_swapchain_texture(&window).unwrap();
                ColorTargetInfo::default()
                    .with_texture(&screen_texture)
                    .with_load_op(LoadOp::CLEAR)
                    .with_clear_color(Color::RGB(127, 127, 127))
            };

            let render_pass = device
                .begin_render_pass(
                    &cbuf,
                    &[color_target_info],
                    Some(
                        &DepthStencilTargetInfo::new()
                            .with_texture(&mut gpu_resources.dbuf)
                            .with_clear_depth(1.0)
                            .with_load_op(LoadOp::CLEAR)
                            .with_store_op(StoreOp::DONT_CARE)
                            .with_stencil_load_op(LoadOp::DONT_CARE)
                            .with_stencil_store_op(StoreOp::DONT_CARE)
                            .with_cycle(true),
                    ),
                )
                .unwrap();
            {
                render_pass.bind_graphics_pipeline(&pipeline);
                render_pass.bind_vertex_buffers(
                    0,
                    &[BufferBinding::new().with_buffer(&gpu_resources.mesh_vbuf)],
                );
                render_pass.bind_index_buffer(
                    &BufferBinding::new().with_buffer(&gpu_resources.mesh_ibuf),
                    IndexElementSize::_32BIT,
                );

                for (
                    &MeshBufferEntry {
                        first_index: ibuf_offset,
                        num_indices: ibuf_count,
                        vertex_offset: vbuf_offset,
                    },
                    pose,
                ) in itertools::izip![mesh_buf_entries.iter(), poses.iter()]
                {
                    let u_camera = {
                        const VUNIF_SIZE: usize =
                            size_of::<Vec4>() + size_of::<Mat4>() + size_of::<Mat4>();
                        let mut buf = [0u8; VUNIF_SIZE];
                        buf[0..16]
                            .copy_from_slice(bytemuck::bytes_of(&camera.position.extend(1.0)));
                        buf[16..80].copy_from_slice(bytemuck::bytes_of(&camera.view()));
                        buf[80..144].copy_from_slice(bytemuck::bytes_of(
                            &(camera.perspective() * camera.view()),
                        ));
                        buf
                    };
                    cbuf.push_vertex_uniform_data(0, &u_camera);
                    let pose_transform = Mat4::from_rotation_translation(pose.rot, pose.pos);
                    cbuf.push_vertex_uniform_data(2, &pose_transform);

                    cbuf.push_fragment_uniform_data(0, &u_camera);

                    render_pass.draw_indexed_primitives(ibuf_count, 1, ibuf_offset, vbuf_offset, 0);
                }
            }
            device.end_render_pass(render_pass);

            cbuf.submit().unwrap();
        }

        std::thread::sleep(std::time::Duration::from_millis(1_000 / 60))
    }
}

struct GpuResources {
    mesh_vbuf: Buffer,
    mesh_ibuf: Buffer,
    dbuf: Texture<'static>,
    tbuf1: TransferBuffer,
    tbuf2: TransferBuffer,
}

fn prepare_gpu_resources(device: &Device) -> GpuResources {
    let mesh_vbuf = device
        .create_buffer()
        .with_usage(BufferUsageFlags::VERTEX)
        .with_size(1_024 * 1_024)
        .build()
        .unwrap();
    let mesh_ibuf = device
        .create_buffer()
        .with_usage(BufferUsageFlags::INDEX)
        .with_size(1_024 * 1_024)
        .build()
        .unwrap();
    let dbuf = device
        .create_texture(
            TextureCreateInfo::new()
                .with_type(TextureType::_2D)
                .with_format(TextureFormat::D16Unorm)
                .with_usage(TextureUsage::DEPTH_STENCIL_TARGET)
                .with_width(1920)
                .with_height(1080)
                .with_layer_count_or_depth(1)
                .with_num_levels(1)
                .with_sample_count(SampleCount::NoMultiSampling),
        )
        .unwrap();
    let tbuf1 = device
        .create_transfer_buffer()
        .with_size(mesh_vbuf.len())
        .build()
        .unwrap();
    let tbuf2 = device
        .create_transfer_buffer()
        .with_size(mesh_ibuf.len())
        .build()
        .unwrap();
    GpuResources {
        mesh_vbuf,
        mesh_ibuf,
        dbuf,
        tbuf1,
        tbuf2,
    }
}

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
            .with_uniform_buffers(3)
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
                ShaderStage::Fragment,
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
                    .with_pitch(size_of::<GpuVertex>() as u32)
                    .with_input_rate(VertexInputRate::Vertex)])
                .with_vertex_attributes(&GpuVertex::get_attributes(0)),
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
                .with_compare_op(CompareOp::Less)
                .with_enable_depth_test(true)
                .with_enable_depth_write(true),
        )
        .with_target_info(
            GraphicsPipelineTargetInfo::new()
                .with_color_target_descriptions(&[
                    ColorTargetDescription::new().with_format(texture_format)
                ])
                .with_has_depth_stencil_target(true)
                .with_depth_stencil_format(TextureFormat::D16Unorm),
        )
        .build()
        .unwrap()
}

struct MeshBufferEntry {
    first_index: u32,
    num_indices: u32,
    vertex_offset: i32,
}

/// runs a copy pass to upload the data from `meshes` to mesh_vbuf and mesh_ibuf using
/// the given transfer buffers, returning location information for each mesh
fn upload_mesh_data(
    device: &Device,
    meshes: &[meshobj::Mesh<Vec4>],
    tbuf1: &TransferBuffer,
    tbuf2: &TransferBuffer,
    mesh_vbuf: &Buffer,
    mesh_ibuf: &Buffer,
) -> Vec<MeshBufferEntry> {
    // accumulate data into local byte array, keeping track of entries
    let mut vbuf_data: Vec<u8> = vec![];
    let mut ibuf_data: Vec<u8> = vec![];
    let mut buffer_entries = vec![];
    let mut next_first_index: u32 = 0;
    let mut next_vertex_offset: i32 = 0;
    for mesh in meshes {
        let gpu_vertexes: Vec<_> = mesh
            .vertexes
            .iter()
            .map(GpuVertex::from_mesh_vertex)
            .collect();

        let vbytes = bytemuck::cast_slice::<_, u8>(&gpu_vertexes);
        vbuf_data.extend_from_slice(vbytes);
        let ibytes = bytemuck::cast_slice::<_, u8>(&mesh.indexes);
        ibuf_data.extend_from_slice(ibytes);

        let mesh_index_count = mesh.indexes.len() as u32;
        let mesh_vertex_count = mesh.vertexes.len() as i32;
        let entry = MeshBufferEntry {
            first_index: next_first_index,
            num_indices: mesh_index_count,
            vertex_offset: next_vertex_offset,
        };
        buffer_entries.push(entry);
        next_first_index += mesh_index_count;
        next_vertex_offset += mesh_vertex_count;
    }
    {
        tbuf1.map(device, true).mem_mut()[0..vbuf_data.len()].copy_from_slice(&vbuf_data);
        tbuf2.map(device, true).mem_mut()[0..ibuf_data.len()].copy_from_slice(&ibuf_data);

        let data_upload = device.acquire_command_buffer().unwrap();
        {
            let copy_pass = device.begin_copy_pass(&data_upload).unwrap();
            copy_pass.upload_to_gpu_buffer(
                TransferBufferLocation::new().with_transfer_buffer(tbuf1),
                BufferRegion::new()
                    .with_buffer(mesh_vbuf)
                    .with_size(mesh_vbuf.len()),
                true,
            );
            copy_pass.upload_to_gpu_buffer(
                TransferBufferLocation::new().with_transfer_buffer(tbuf2),
                BufferRegion::new()
                    .with_buffer(mesh_ibuf)
                    .with_size(mesh_ibuf.len()),
                true,
            );
            device.end_copy_pass(copy_pass);
        }
        let data_upload_fence = data_upload.submit_and_acquire_fence(device).unwrap();
        while !data_upload_fence.query(device) {}
    }

    buffer_entries
}

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
/// aligned vertex data for the vertex shader
struct GpuVertex {
    model_position: Vec4,
    model_normal: Vec4,
    color: Vec4,
}
impl GpuVertex {
    fn get_attributes(buffer_slot: u32) -> Vec<VertexAttribute> {
        vec![
            VertexAttribute::new()
                .with_buffer_slot(buffer_slot)
                .with_location(0)
                .with_offset(0)
                .with_format(VertexElementFormat::Float4),
            VertexAttribute::new()
                .with_buffer_slot(buffer_slot)
                .with_location(1)
                .with_offset(16)
                .with_format(VertexElementFormat::Float4),
            VertexAttribute::new()
                .with_buffer_slot(buffer_slot)
                .with_location(2)
                .with_offset(32)
                .with_format(VertexElementFormat::Float4),
        ]
    }

    fn from_mesh_vertex(v: &meshobj::Vertex<Vec4>) -> Self {
        Self {
            model_position: v.position,
            color: v.data,
            model_normal: v.normal,
        }
    }
}

#[derive(Debug, Clone)]
struct Camera {
    position: Vec3,
    facing: Vec3,
    fov: f32,
    aspect_ratio: f32,
}
impl Camera {
    fn perspective(&self) -> Mat4 {
        // nb: SDL_GPU uses DirectX-like convention
        glam::camera::rh::proj::directx::perspective(self.fov, self.aspect_ratio, 0.1, 200.0)
    }
    fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_to_mat4(self.position, self.facing, vec3(0.0, 0.0, 1.0))
    }
}

mod meshobj {
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

#[allow(dead_code)]
mod dbgutil {
    use sdl3::sys::gpu::{
        SDL_DownloadFromGPUTexture, SDL_GPUTextureRegion, SDL_GPUTextureTransferInfo,
    };

    pub fn download_texture_content<T: std::fmt::Debug + std::marker::Copy>(
        device: &sdl3::gpu::Device,
        texture: &sdl3::gpu::Texture,
    ) -> Vec<T> {
        let download_buffer = device
            .create_transfer_buffer()
            .with_size(1_024 * 1_024)
            .build()
            .unwrap();
        let vertex_data_download = device.acquire_command_buffer().unwrap();
        {
            let copy_pass = device.begin_copy_pass(&vertex_data_download).unwrap();
            unsafe {
                SDL_DownloadFromGPUTexture(
                    copy_pass.raw(),
                    &SDL_GPUTextureRegion {
                        texture: texture.raw(),
                        mip_level: 0,
                        layer: 0,
                        x: 0,
                        y: 0,
                        z: 0,
                        w: 0,
                        h: 0,
                        d: 0,
                    },
                    &SDL_GPUTextureTransferInfo {
                        transfer_buffer: download_buffer.raw(),
                        offset: 0,
                        pixels_per_row: 0,
                        rows_per_layer: 0,
                    },
                );
            }
            device.end_copy_pass(copy_pass);
        }
        let vertex_data_download_fence = vertex_data_download
            .submit_and_acquire_fence(device)
            .unwrap();
        while !vertex_data_download_fence.query(device) {}

        let content = download_buffer.map::<T>(device, false).mem().to_owned();
        content
    }
}
