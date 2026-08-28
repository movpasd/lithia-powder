mod gfx;
mod meshobj;

use std::{f32::consts::TAU, time::Instant};

use glam::{vec3, Quat, Vec3, Vec4};
use sdl3::keyboard::Keycode;

fn main() {
    let sdl = sdl3::init().unwrap();

    // GPU resources and declaration
    let mut gfx_state = gfx::State::new(&sdl);
    println!(
        "GPU device name: {}",
        gfx_state.get_gpu_model_name().to_str().unwrap()
    );

    // geometry data
    let meshes: Vec<meshobj::Mesh<Vec4>> = vec![
        meshobj::colorful_cube(),
        meshobj::colorful_cube(),
        meshobj::colorful_cube(),
    ];
    let mut poses: Vec<gfx::Pose> = vec![
        gfx::Pose::default(),
        gfx::Pose {
            position: vec3(-3.0, 0.0, 0.0),
            rotation: Quat::default(),
        },
        gfx::Pose {
            position: vec3(-1.5, 3.0, 0.0),
            rotation: Quat::default(),
        },
    ];

    // camera data
    let mut camera = gfx::Camera {
        position: Vec3::ZERO,
        facing: vec3(1.0, 0.0, 0.0),
        fov: 70.0f32.to_radians(),
        aspect_ratio: 1920.0 / 1080.0,
    };

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

                let (width, height) = gfx_state.get_window_size();
                let aspect_ratio = width / height;

                camera.position = position;
                camera.facing = facing;
                camera.aspect_ratio = aspect_ratio;
            }

            // cube animation
            poses[0] = anim::pose(elapsed_time_secs);
        }

        // render
        gfx_state.render(&camera, &poses);

        std::thread::sleep(std::time::Duration::from_millis(1_000 / 60))
    }
}

mod anim {
    use std::{
        f32::consts::TAU,
        ops::{Add, Mul},
    };

    use glam::{vec3, Quat};

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

    pub fn pose(t: f32) -> super::gfx::Pose {
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

        super::gfx::Pose {
            position: pos,
            rotation: rot,
        }
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
