mod animobj;
mod gfx;
mod meshobj;

use std::{f32::consts::TAU, time::Instant};

use glam::{vec3, Quat, Vec2, Vec3, Vec3Swizzles, Vec4};
use sdl3::keyboard::Keycode;

fn main() {
    let sdl = sdl3::init().unwrap();

    // cube definition
    const CUBE_COUNT: usize = 6;
    let meshes: Vec<meshobj::Mesh<Vec4>> =
        (0..CUBE_COUNT).map(|_| meshobj::colorful_cube()).collect();
    let mut poses: Vec<gfx::Pose> = [gfx::Pose::default(); CUBE_COUNT].into();

    let cube_anims: Vec<_> = (0..CUBE_COUNT)
        .map(|i| {
            let length_secs = 3.2;
            let wait_secs = 0.2;
            let total_secs = 2.0 * (length_secs + wait_secs);
            let distance = 8.0;
            let height = 9.0;
            let flip_count = 3.0;

            let angle = i as f32 * (TAU / (CUBE_COUNT as f32));
            let shift = i as f32 * total_secs / (CUBE_COUNT as f32);

            let somersault = somersault_anim(
                distance * Vec3::X.rotate_z(angle),
                Vec3::ZERO,
                height,
                length_secs,
                flip_count,
            );
            somersault
                .then_pause(wait_secs)
                .then(&somersault.reversed())
                .then_pause(wait_secs)
                .loop_shifted(shift)
        })
        .collect();

    // camera data
    let mut camera = gfx::Camera {
        position: Vec3::ZERO,
        facing: vec3(1.0, 0.0, 0.0),
        fov: 70.0f32.to_radians(),
        aspect_ratio: 1920.0 / 1080.0,
    };

    // GPU resources and declaration
    let mut gfx_state = gfx::State::new(&sdl);
    println!(
        "GPU device name: {}",
        gfx_state.get_gpu_model_name().to_str().unwrap()
    );

    // mesh data upload
    gfx_state.update_meshes(&meshes);

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
                const ORBIT_PERIOD: f32 = 40.0;
                const ORBIT_BIRDSEYE_DISTANCE: f32 = 2.0;
                const ORBIT_HEIGHT: f32 = 17.0;
                const ORBIT_PHASE_INIT: f32 = 0.0;
                const CAMERA_LOOK_AT: Vec3 = vec3(0.0, 0.0, 0.5);

                let orbit_phase = ORBIT_PHASE_INIT + TAU * elapsed_time_secs / ORBIT_PERIOD;
                let position = vec3(
                    ORBIT_BIRDSEYE_DISTANCE * orbit_phase.cos(),
                    ORBIT_BIRDSEYE_DISTANCE * orbit_phase.sin(),
                    ORBIT_HEIGHT,
                );
                let facing = (CAMERA_LOOK_AT - position).normalize();

                let (width, height) = gfx_state.get_window_size();
                let aspect_ratio = width / height;

                camera.position = position;
                camera.facing = facing;
                camera.aspect_ratio = aspect_ratio;
            }

            // cube animation
            for (pose, anim) in poses.iter_mut().zip(&cube_anims) {
                *pose = anim.sample_looped(elapsed_time_secs);
            }
        }

        // render
        gfx_state.render(&camera, &poses);

        std::thread::sleep(std::time::Duration::from_millis(1_000 / 60))
    }
}

fn somersault_anim(
    start_position: Vec3,
    end_position: Vec3,
    bounce_height: f32,
    length_secs: f32,
    flip_count: f32,
) -> animobj::Anim<gfx::Pose> {
    animobj::f32::parabola()
        .map_indexed(move |t, s| {
            // the "baseline" is the straight line between start_position to end_position
            let baseline = start_position + (end_position - start_position) * t;
            let bounce_displacement = Vec3::Z * s * bounce_height;
            let position = baseline + bounce_displacement;

            let facing = (end_position - start_position).xy().normalize();
            let facing_rotation = Quat::from_rotation_arc_2d(Vec2::X, facing);

            let flip_rotation = Quat::from_rotation_y(TAU * t * flip_count);

            let rotation = facing_rotation * flip_rotation;

            gfx::Pose { position, rotation }
        })
        .stretched(length_secs)
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
