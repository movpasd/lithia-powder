mod anim;
mod gfx;
mod mesh;
mod world;

use std::{f32::consts::TAU, time::Instant};

use glam::{IVec3, Quat, Vec2, Vec3, Vec3Swizzles, Vec4Swizzles, vec3};

use anim::Anim;

fn main() {
    let sdl = sdl3::init().unwrap();

    // cube definition
    const CUBE_COUNT: usize = 3;
    const CUBE_SIDE_LENGTH: f32 = 0.67;
    let floor_mesh = mesh::floor();
    let floor_pose = gfx::Pose::default();
    let cube_meshes: Vec<_> = (0..CUBE_COUNT)
        .map(|_| {
            let cube = mesh::colorful_cube();
            cube.map_positions(|v| v.with_xyz(v.xyz() * CUBE_SIDE_LENGTH))
        })
        .collect();
    let mut cube_poses: Vec<gfx::Pose> = [gfx::Pose::default(); CUBE_COUNT].into();

    let cube_anims: Vec<_> = (0..CUBE_COUNT)
        .map(|i| {
            let length_secs = 1.5;
            let wait_secs = 0.2;
            let total_secs = 2.0 * (length_secs + wait_secs);
            let distance = 4.0;
            let height = 3.0;
            let flip_count = 1.0;
            let twist_count = 0.5;

            let angle = i as f32 * (TAU / (CUBE_COUNT as f32));
            let shift = i as f32 * total_secs / (CUBE_COUNT as f32);

            let ground_offset = Vec3::Z * 0.5 * CUBE_SIDE_LENGTH;
            let somersault = somersault_anim(
                distance * Vec3::X.rotate_z(angle) + ground_offset,
                Vec3::ZERO + ground_offset,
                height,
                length_secs,
                flip_count,
                twist_count,
            );
            somersault
                .then_pause(wait_secs)
                .then(&somersault.reversed())
                .then_pause(wait_secs)
                .loop_shifted(shift)
        })
        .collect();

    // chunk definition
    let chunk = world::Chunk::from_fn(|IVec3 { x, y, z }| {
        if z <= x && z <= y && z <= 32 - x && z <= 32 - y {
            world::Block::Sand
        } else {
            world::Block::Air
        }
    });
    let chunk_mesh = chunk.to_mesh();
    let chunk_pose = gfx::Pose {
        position: vec3(20.0, 0.0, 0.0),
        rotation: Quat::IDENTITY,
    };

    // GPU resources and declaration
    let mut gfx_state = gfx::State::new(&sdl);
    println!(
        "GPU device name: {}",
        gfx_state.get_gpu_model_name().to_str().unwrap()
    );

    // window and event stuff
    let mouse_subsystem = sdl.mouse();
    mouse_subsystem.set_relative_mouse_mode(gfx_state.window(), true);

    // game data
    let mut player = Player::new_at_spawn();
    fn updated_eyeball(player: &Player, gfx_state: &gfx::State) -> gfx::Eyeball {
        let aspect_ratio = {
            let (width, height) = gfx_state.get_retina_size();
            width / height
        };
        player.eyeball(70_f32.to_radians(), aspect_ratio)
    }
    let mut eyeball: gfx::Eyeball;

    const SUNLIGHT_PERIOD: f32 = 120.0;
    let sunlight_anim = sunlight_anim(SUNLIGHT_PERIOD);
    let mut sunlight: gfx::Sunlight;

    // mesh data upload
    {
        let floor_mesh_container = [floor_mesh];
        let chunk_mesh_container = [chunk_mesh];
        let meshes = floor_mesh_container
            .iter()
            .chain(&cube_meshes)
            .chain(&chunk_mesh_container);
        gfx_state.update_meshes(meshes);
    }

    // event loop
    let start_time = Instant::now();
    let mut event_pump = sdl.event_pump().unwrap();
    'main_loop: loop {
        let elapsed_time_secs = (Instant::now() - start_time).as_secs_f32();

        // event handling
        for event in event_pump.poll_iter() {
            use sdl3::keyboard::Keycode;
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
                sdl3::event::Event::MouseMotion { xrel, yrel, .. } => {
                    player.nudge_look(xrel, yrel);
                }
                _ => {}
            }
        }

        // raw input handling
        {
            use sdl3::keyboard::Scancode;

            const DT: f32 = 1.0 / 60.0;

            let keyboard_state = event_pump.keyboard_state();
            if keyboard_state.is_scancode_pressed(Scancode::W) {
                player.move_forward(DT)
            }
            if keyboard_state.is_scancode_pressed(Scancode::S) {
                player.move_backward(DT);
            }
            if keyboard_state.is_scancode_pressed(Scancode::A) {
                player.strafe_left(DT);
            }
            if keyboard_state.is_scancode_pressed(Scancode::D) {
                player.strafe_right(DT);
            }
            if keyboard_state.is_scancode_pressed(Scancode::Space) {
                player.fly_up(DT);
            }
            if keyboard_state.is_scancode_pressed(Scancode::LShift) {
                player.fly_down(DT);
            }
        }

        // logic
        eyeball = updated_eyeball(&player, &gfx_state);
        sunlight = sunlight_anim.sample_looped(elapsed_time_secs);
        for (pose, anim) in cube_poses.iter_mut().zip(&cube_anims) {
            *pose = anim.sample_looped(elapsed_time_secs);
        }

        // render
        {
            let floor_pose_container = [floor_pose];
            let chunk_pose_container = [chunk_pose];
            let poses = floor_pose_container
                .iter()
                .chain(&cube_poses)
                .chain(&chunk_pose_container);
            gfx_state.render(&eyeball, poses, &sunlight);
        }

        std::thread::sleep(std::time::Duration::from_millis(1_000 / 60));
    }
}

fn somersault_anim(
    start_position: Vec3,
    end_position: Vec3,
    bounce_height: f32,
    length_secs: f32,
    flip_count: f32,
    twist_count: f32,
) -> Anim<gfx::Pose> {
    Anim::<f32>::parabola()
        .map_indexed(move |t, s| {
            // the "baseline" is the straight line between start_position to end_position
            let baseline = start_position + (end_position - start_position) * t;
            let bounce_displacement = Vec3::Z * s * bounce_height;
            let position = baseline + bounce_displacement;

            let facing = (end_position - start_position).xy().normalize();
            let facing_rotation = Quat::from_rotation_arc_2d(Vec2::X, facing);

            let twist_rotation = Quat::from_rotation_x(TAU * t * twist_count);
            let flip_rotation = Quat::from_rotation_y(TAU * t * flip_count);

            let rotation = facing_rotation * flip_rotation * twist_rotation;

            gfx::Pose { position, rotation }
        })
        .stretched(length_secs)
}

fn sunlight_anim(period: f32) -> Anim<gfx::Sunlight> {
    Anim::<Vec2>::circle()
        .map(move |xy| {
            let from_direction = xy.extend(0.0).rotate_towards(Vec3::Z, 30_f32.to_radians());
            gfx::Sunlight { from_direction }
        })
        .stretched(period)
}

#[derive(Debug, Clone)]
struct Player {
    position: Vec3,
    azimuth: f32,
    pitch: f32,
}
impl Player {
    const EYE_HEIGHT: f32 = 1.7;

    fn new_at_spawn() -> Self {
        Self {
            position: vec3(0.0, 0.0, 2.0),
            azimuth: 0.0,
            pitch: 0.0,
        }
    }
    fn eyeball(&self, fov: f32, aspect_ratio: f32) -> gfx::Eyeball {
        gfx::Eyeball {
            position: self.position + Vec3::Z * Self::EYE_HEIGHT,
            facing: self.facing(),
            fov,
            aspect_ratio,
        }
    }
    fn nudge_look(&mut self, dx: f32, dy: f32) {
        const SPEED_RAD_PER_PIXEL: f32 = 0.2_f32.to_radians();
        const EPSILON: f32 = 1e-5;

        self.azimuth -= dx * SPEED_RAD_PER_PIXEL;

        self.pitch -= (dy * SPEED_RAD_PER_PIXEL) % TAU;
        self.pitch = self.pitch.clamp(-TAU / 4.0 + EPSILON, TAU / 4.0 - EPSILON);
    }

    // unit vectors
    fn facing(&self) -> Vec3 {
        Vec3::X.rotate_y(-self.pitch).rotate_z(self.azimuth)
    }
    fn bearing(&self) -> Vec3 {
        Vec3::X.rotate_z(self.azimuth)
    }
    fn bearing_left(&self) -> Vec3 {
        Vec3::Y.rotate_z(self.azimuth)
    }

    // speeds in metres per second
    const HORIZONTAL_SPEED: f32 = 3.0;
    const VERTICAL_SPEEED: f32 = 3.0;

    fn move_forward(&mut self, dt: f32) {
        self.position += Self::HORIZONTAL_SPEED * dt * self.bearing();
    }
    fn move_backward(&mut self, dt: f32) {
        self.position -= Self::HORIZONTAL_SPEED * dt * self.bearing();
    }
    fn strafe_left(&mut self, dt: f32) {
        self.position += Self::HORIZONTAL_SPEED * dt * self.bearing_left();
    }
    fn strafe_right(&mut self, dt: f32) {
        self.position -= Self::HORIZONTAL_SPEED * dt * self.bearing_left();
    }
    fn fly_up(&mut self, dt: f32) {
        self.position += Self::VERTICAL_SPEEED * dt * Vec3::Z;
    }
    fn fly_down(&mut self, dt: f32) {
        self.position -= Self::VERTICAL_SPEEED * dt * Vec3::Z;
    }
}
