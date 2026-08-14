use std::f64::consts::PI;

use sdl3::{
    self,
    gpu::{BufferRegion, BufferUsageFlags, ShaderFormat, TransferBufferLocation},
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

    const VERTEX_FLOAT_COUNT: usize = (2 + 3) * 3;
    const VERTEX_BUF_SIZE: u32 = (size_of::<f32>() * VERTEX_FLOAT_COUNT) as u32;
    let vertex_buffer = device
        .create_buffer()
        .with_usage(BufferUsageFlags::VERTEX)
        .with_size(VERTEX_BUF_SIZE)
        .build()
        .unwrap();

    // upload data to vertex buffer
    {
        #[rustfmt::skip]
        let vertex_data: [f32; VERTEX_FLOAT_COUNT] = [
        //   x     y    r    g    b
             0.5,  0.0, 1.0, 0.0, 0.0,
             0.5, -0.5, 0.0, 1.0, 0.0,
            -0.5, -0.5, 0.0, 0.0, 1.0,
        ];
        let transfer_buffer = device
            .create_transfer_buffer()
            .with_size(VERTEX_BUF_SIZE)
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
                    .with_size(VERTEX_BUF_SIZE),
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

    let mut canvas = window.into_canvas();
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
