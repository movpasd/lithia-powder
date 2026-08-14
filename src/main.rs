use std::f64::consts::PI;

use sdl3::{
    self,
    gpu::{
        BufferRegion, BufferUsageFlags, ColorTargetBlendState, ColorTargetDescription, CompareOp,
        CullMode, DepthStencilState, FillMode, FrontFace, GraphicsPipeline,
        GraphicsPipelineTargetInfo, RasterizerState, Shader, ShaderFormat, ShaderStage,
        TextureFormat, TransferBufferLocation, VertexAttribute, VertexBufferDescription,
        VertexInputState,
    },
    keyboard::Keycode,
    pixels::Color,
    sys::render::SDL_RendererLogicalPresentation,
};

fn main() {
    let sdl = sdl3::init().unwrap();

    let video_sys = sdl.video().unwrap();
    let window = video_sys
        .window("lithia-powder", 640, 480)
        .position_centered()
        .build()
        .unwrap();

    let mut device = sdl3::gpu::Device::new(ShaderFormat::SPIRV, false).unwrap();
    device = device.with_window(&window).unwrap();

    const VERTEX_F32_SIZE: usize = 2 + 3;
    const VERTEX_SIZE: usize = size_of::<f32>() * VERTEX_F32_SIZE;
    const VERTEX_COUNT: usize = 3;
    let vertex_buffer = device
        .create_buffer()
        .with_usage(BufferUsageFlags::VERTEX)
        .with_size((VERTEX_SIZE * VERTEX_COUNT) as u32)
        .build()
        .unwrap();

    // upload data to vertex buffer
    {
        #[rustfmt::skip]
        let vertex_data: [f32; VERTEX_F32_SIZE * VERTEX_COUNT] = [
        //   x     y    r    g    b
             0.5,  0.0, 1.0, 0.0, 0.0,
             -0.5, -0.5, 0.0, 1.0, 0.0,
             0.5, -0.5, 0.0, 0.0, 1.0,
        ];
        let transfer_buffer = device
            .create_transfer_buffer()
            .with_size(vertex_buffer.len())
            .build()
            .unwrap();
        transfer_buffer
            .map(&device, false)
            .mem_mut()
            .copy_from_slice(bytemuck::bytes_of(&vertex_data));

        let vertex_data_upload = device.acquire_command_buffer().unwrap();
        {
            let copy_pass = device.begin_copy_pass(&vertex_data_upload).unwrap();
            copy_pass.upload_to_gpu_buffer(
                TransferBufferLocation::new().with_transfer_buffer(&transfer_buffer),
                BufferRegion::new()
                    .with_buffer(&vertex_buffer)
                    .with_size(vertex_buffer.len()),
                false,
            );
            device.end_copy_pass(copy_pass);
        }
        let vertex_data_upload_fence = vertex_data_upload
            .submit_and_acquire_fence(&device)
            .unwrap();
        while !vertex_data_upload_fence.query(&device) {}

        // for testing
        println!(
            "{:?}",
            download_buffer_content::<f32>(&device, vertex_buffer)
        );
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
    {
        use sdl3::gpu::{PrimitiveType, VertexElementFormat, VertexInputRate};

        let texture_format = device.get_swapchain_texture_format(&window);

        pipeline = device
            .create_graphics_pipeline()
            .with_vertex_shader(&vertex_shader)
            .with_fragment_shader(&fragment_shader)
            .with_vertex_input_state(
                VertexInputState::new()
                    .with_vertex_buffer_descriptions(&[VertexBufferDescription::new()
                        .with_slot(0)
                        .with_pitch(VERTEX_SIZE as u32)
                        .with_input_rate(VertexInputRate::Vertex)])
                    .with_vertex_attributes(&[
                        VertexAttribute::new()
                            .with_buffer_slot(0)
                            // va_position
                            .with_location(0)
                            .with_offset(0)
                            .with_format(VertexElementFormat::Float2),
                        VertexAttribute::new()
                            .with_buffer_slot(0)
                            // va_color
                            .with_location(1)
                            .with_offset(2 * size_of::<f32>() as u32)
                            .with_format(VertexElementFormat::Float3),
                    ]),
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
                    .with_compare_op(CompareOp::Greater),
            )
            .with_target_info(
                GraphicsPipelineTargetInfo::new().with_color_target_descriptions(&[
                    ColorTargetDescription::new().with_format(texture_format), // think not required: .with_blend_state(?),
                ]),
            )
            .build()
            .unwrap();
    }

    // for 2D rendering
    let mut canvas = window.clone().into_canvas();
    canvas
        .set_logical_size(640, 480, SDL_RendererLogicalPresentation::LETTERBOX)
        .unwrap();

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

        let color = {
            let ticks = sdl3::timer::ticks() as f64 / 1000.0;
            let red = 0.5 * (1.0 + f64::sin(ticks));
            let green = 0.5 * (1.0 + f64::sin(ticks + PI * 2. / 3.));
            let blue = 0.5 * (1.0 + f64::sin(ticks + PI * 4. / 3.));
            Color::RGB(
                (red * u8::MAX as f64) as u8,
                (green * u8::MAX as f64) as u8,
                (blue * u8::MAX as f64) as u8,
            )
        };

        canvas.set_draw_color(color);
        canvas.clear();
        canvas.present();

        std::thread::sleep(std::time::Duration::from_millis(1_000 / 60))
    }

    println!("bye bye!");
}

/// Downloads the content of a buffer for debugging purposes
///
/// Runs a whole buffer pass and blocks.
fn download_buffer_content<T: std::fmt::Debug + std::marker::Copy>(
    device: &sdl3::gpu::Device,
    vertex_buffer: sdl3::gpu::Buffer,
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
