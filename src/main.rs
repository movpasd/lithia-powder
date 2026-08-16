#![allow(dead_code)]

use glam::{vec3, Mat4, Vec4};
use sdl3::{
    self,
    gpu::{
        BufferBinding, BufferRegion, BufferUsageFlags, ColorTargetDescription, ColorTargetInfo,
        CompareOp, CullMode, DepthStencilState, FillMode, FrontFace, GraphicsPipeline,
        GraphicsPipelineTargetInfo, IndexElementSize, RasterizerState, Shader, ShaderFormat,
        ShaderStage, TransferBufferLocation, VertexAttribute, VertexBufferDescription,
        VertexElementFormat, VertexInputState,
    },
    keyboard::Keycode,
    pixels::Color,
    sys::gpu::SDL_GPULoadOp,
};

fn main() {
    let sdl = sdl3::init().unwrap();

    let video_sys = sdl.video().unwrap();
    let window = video_sys
        .window("lithia-powder", 980, 640)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    let mut device = sdl3::gpu::Device::new(ShaderFormat::SPIRV, false).unwrap();
    device = device.with_window(&window).unwrap();

    const VERTEX_F32_SIZE: u32 = 3 + 3;
    const VERTEX_SIZE: u32 = size_of::<f32>() as u32 * VERTEX_F32_SIZE;
    const VERTEX_COUNT: u32 = 4;
    #[rustfmt::skip]
    let vertex_data: [f32; (VERTEX_F32_SIZE * VERTEX_COUNT) as usize] = [
    //     x     y     z     r     g     b
        -0.5, -0.5,  0.0, 1.00, 0.00, 0.00,
         0.5, -0.5,  0.0, 0.25, 0.75, 0.00,
         0.5,  0.5,  0.0, 0.00, 0.50, 0.50,
        -0.5,  0.5,  0.0, 0.25, 0.00, 0.75,
    ];
    let vertex_buffer = device
        .create_buffer()
        .with_usage(BufferUsageFlags::VERTEX)
        .with_size(VERTEX_SIZE * VERTEX_COUNT)
        .build()
        .unwrap();

    const INDEX_SIZE: u32 = size_of::<u32>() as u32;
    const INDEX_COUNT: u32 = 6;
    #[rustfmt::skip]
    let index_data: [u32; INDEX_COUNT as usize] = [
        0, 1, 2,
        2, 3, 0,
    ];
    let index_buffer = device
        .create_buffer()
        .with_usage(BufferUsageFlags::INDEX)
        .with_size(INDEX_SIZE * INDEX_COUNT)
        .build()
        .unwrap();

    // upload data to geometry buffers
    {
        let vertex_transfer_buf = device
            .create_transfer_buffer()
            .with_size(vertex_buffer.len())
            .build()
            .unwrap();
        vertex_transfer_buf
            .map(&device, false)
            .mem_mut()
            .copy_from_slice(bytemuck::bytes_of(&vertex_data));

        let index_transfer_buf = device
            .create_transfer_buffer()
            .with_size(index_buffer.len())
            .build()
            .unwrap();
        index_transfer_buf
            .map(&device, false)
            .mem_mut()
            .copy_from_slice(bytemuck::bytes_of(&index_data));

        let data_upload = device.acquire_command_buffer().unwrap();
        {
            let copy_pass = device.begin_copy_pass(&data_upload).unwrap();
            copy_pass.upload_to_gpu_buffer(
                TransferBufferLocation::new().with_transfer_buffer(&vertex_transfer_buf),
                BufferRegion::new()
                    .with_buffer(&vertex_buffer)
                    .with_size(vertex_buffer.len()),
                false,
            );
            copy_pass.upload_to_gpu_buffer(
                TransferBufferLocation::new().with_transfer_buffer(&index_transfer_buf),
                BufferRegion::new()
                    .with_buffer(&index_buffer)
                    .with_size(index_buffer.len()),
                false,
            );
            device.end_copy_pass(copy_pass);
        }
        let data_upload_fence = data_upload.submit_and_acquire_fence(&device).unwrap();
        while !data_upload_fence.query(&device) {}
    }

    // load and render shaders
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
            .build()
            .unwrap();
    }

    // set up rendering pipeline
    let pipeline: GraphicsPipeline;
    let texture_format = device.get_swapchain_texture_format(&window);
    {
        use sdl3::gpu::{PrimitiveType, VertexElementFormat, VertexInputRate};

        pipeline = device
            .create_graphics_pipeline()
            .with_vertex_shader(&vertex_shader)
            .with_fragment_shader(&fragment_shader)
            .with_vertex_input_state(
                VertexInputState::new()
                    .with_vertex_buffer_descriptions(&[VertexBufferDescription::new()
                        .with_slot(0)
                        .with_pitch(VERTEX_SIZE)
                        .with_input_rate(VertexInputRate::Vertex)])
                    .with_vertex_attributes(&[
                        VertexAttribute::new()
                            .with_buffer_slot(0)
                            // va_position
                            .with_location(0)
                            .with_offset(0)
                            .with_format(VertexElementFormat::Float3),
                        VertexAttribute::new()
                            .with_buffer_slot(0)
                            // va_color
                            .with_location(1)
                            .with_offset(3 * size_of::<f32>() as u32)
                            .with_format(VertexElementFormat::Float3),
                    ]),
            )
            .with_primitive_type(PrimitiveType::TriangleList)
            .with_rasterizer_state(
                RasterizerState::new()
                    .with_fill_mode(FillMode::Fill)
                    .with_cull_mode(CullMode::None)
                    .with_front_face(FrontFace::CounterClockwise),
            )
            .with_depth_stencil_state(
                DepthStencilState::new()
                    .with_enable_depth_test(true)
                    .with_compare_op(CompareOp::Greater),
            )
            .with_target_info(
                GraphicsPipelineTargetInfo::new().with_color_target_descriptions(&[
                    ColorTargetDescription::new().with_format(texture_format), // think not required: .with_blend_state(?)
                ]),
            )
            .build()
            .unwrap();
    }

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
        let projection;
        {
            // camera stuff
            // SDL_GPU uses DirectX-like convention
            use glam::camera::rh::{proj::directx::perspective, view::look_at_mat4};

            let (width, height) = window.size();
            let persp = perspective(
                70.0f32.to_radians(),
                width as f32 / height as f32,
                0.1,
                200.0,
            );
            let look = look_at_mat4(
                vec3(3.0, 2.0, 1.0),
                vec3(0.0, 0.0, 0.0),
                vec3(0.0, 0.0, 1.0),
            );
            projection = persp * look;
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
                cbuf.push_vertex_uniform_data(0, &projection);
                render_pass.draw_indexed_primitives(index_buffer.len(), 1, 0, 0, 0);
            }
            device.end_render_pass(render_pass);

            let fence = cbuf.submit_and_acquire_fence(&device).unwrap();
            while !fence.query(&device) {}
        }

        std::thread::sleep(std::time::Duration::from_millis(1_000 / 60))
    }

    println!("bye bye!");
}

/// Downloads the content of a buffer for debugging purposes
///
/// Runs a whole buffer pass and blocks.
fn download_buffer_content<T: std::fmt::Debug + std::marker::Copy>(
    device: &sdl3::gpu::Device,
    vertex_buffer: &sdl3::gpu::Buffer,
) -> Vec<T> {
    let download_buffer = device
        .create_transfer_buffer()
        .with_size(vertex_buffer.len())
        .build()
        .unwrap();
    let vertex_data_download = device.acquire_command_buffer().unwrap();
    {
        let copy_pass = device.begin_copy_pass(&vertex_data_download).unwrap();
        unsafe {
            use sdl3::sys::gpu::{
                SDL_DownloadFromGPUBuffer, SDL_GPUBufferRegion, SDL_GPUTransferBufferLocation,
            };
            SDL_DownloadFromGPUBuffer(
                copy_pass.raw(),
                &SDL_GPUBufferRegion {
                    buffer: vertex_buffer.raw(),
                    offset: 0,
                    size: vertex_buffer.len(),
                },
                &SDL_GPUTransferBufferLocation {
                    transfer_buffer: download_buffer.raw(),
                    offset: 0,
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

#[derive(Debug, Clone, PartialEq)]
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
        other.indexes.iter_mut().for_each(|i| *i += self.len());
        self.vertexes.append(&mut other.vertexes);
        self.indexes.append(&mut other.indexes);
    }

    fn transform(&mut self, m: Mat4) {
        self.vertexes.iter_mut().for_each(|v| {
            v.position = m * v.position;
            v.normal = m * v.normal
        });
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
            [1.0, 0.0, 0.0, 1.0],
            [0.25, 0.75, 0.0, 1.0],
            [0.0, 0.5, 0.5, 1.0],
            [0.25, 0.0, 0.75, 1.0],
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
        Mat4::from_axis_angle(vec3(1.0, 0.0, 0.0), PI),
        Mat4::from_axis_angle(vec3(1.0, 0.0, 0.0), FRAC_PI_2),
        Mat4::from_axis_angle(vec3(1.0, 0.0, 0.0), -FRAC_PI_2),
        Mat4::from_axis_angle(vec3(0.0, 1.0, 0.0), FRAC_PI_2),
        Mat4::from_axis_angle(vec3(0.0, 1.0, 0.0), FRAC_PI_2),
    ];

    let mut cube = Mesh::new_empty();
    for transform in transformations {
        let mut next_face = plus_z_face.clone();
        next_face.transform(transform);
        cube.append(&mut next_face);
    }

    cube
}
