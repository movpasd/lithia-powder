mod mesh_renderer;
mod retina;
mod skybox_renderer;

use glam::{Mat4, Vec3, Vec4, vec3};
use sdl3::{
    self, Sdl,
    gpu::{
        ColorTargetInfo, DepthStencilTargetInfo, Device, LoadOp, SampleCount, ShaderFormat,
        StoreOp, Texture, TextureCreateInfo, TextureFormat, TextureType, TextureUsage,
    },
    pixels::Color,
    video::Window,
};
use std::ffi::CStr;

use crate::mesh::Mesh;
use mesh_renderer::MeshRenderer;
pub use mesh_renderer::Pose;
use retina::Retina;
use skybox_renderer::SkyboxRenderer;

const WINDOW_WIDTH: u32 = 1920;
const WINDOW_HEIGHT: u32 = 1080;
const MAX_SCREEN_WIDTH: u32 = 1920;
const MAX_SCREEN_HEIGHT: u32 = 1080;
const RETINA_WIDTH: f32 = 320.0;
const RETINA_HEIGHT: f32 = 240.0;
const RETINA_TO_SCREEN_SCALE: f32 = 4.0;

pub struct State {
    window: Window,
    device: Device,
    retina: Retina,
    skybox_renderer: SkyboxRenderer,
    mesh_renderer: MeshRenderer,
    dbuf: Texture<'static>,
}
impl State {
    pub fn new(sdl: &Sdl) -> State {
        let video_sys = sdl.video().unwrap();
        let window = video_sys
            .window("lithia-powder", WINDOW_WIDTH, WINDOW_HEIGHT)
            .position_centered()
            .borderless()
            .build()
            .unwrap();

        let mut device = sdl3::gpu::Device::new(ShaderFormat::SPIRV, true).unwrap();
        device = device.with_window(&window).unwrap();

        let retina = Retina::new(&device, RETINA_WIDTH, RETINA_HEIGHT, RETINA_TO_SCREEN_SCALE);
        let skybox_renderer = SkyboxRenderer::new(&device, Retina::TEXTURE_FORMAT);
        let mesh_renderer = MeshRenderer::new(&device, Retina::TEXTURE_FORMAT);

        let dbuf = device
            .create_texture(
                TextureCreateInfo::new()
                    .with_type(TextureType::_2D)
                    .with_format(TextureFormat::D16Unorm)
                    .with_usage(TextureUsage::DEPTH_STENCIL_TARGET)
                    .with_width(MAX_SCREEN_WIDTH)
                    .with_height(MAX_SCREEN_HEIGHT)
                    .with_layer_count_or_depth(1)
                    .with_num_levels(1)
                    .with_sample_count(SampleCount::NoMultiSampling),
            )
            .unwrap();

        // resource creation
        State {
            window,
            device,
            retina,
            skybox_renderer,
            mesh_renderer,
            dbuf,
        }
    }

    pub fn get_retina_size(&self) -> (f32, f32) {
        (self.retina.width(), self.retina.height())
    }
    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn render(
        &mut self,
        eyeball: &Eyeball,
        poses: &[Pose],
        sunlight: &Sunlight,
        meshes_update: Option<&[Mesh<Vec4>]>,
    ) {
        let mut command_buffer = self.device.acquire_command_buffer().unwrap();

        self.retina.prepare();
        self.skybox_renderer
            .prepare(&self.device, &command_buffer, eyeball);
        self.mesh_renderer
            .prepare(&self.device, &command_buffer, meshes_update, Some(poses));

        // uniforms
        let u_eyeball = UEyeball::from_eyeball(eyeball);
        let u_lamp = ULamp {
            from_direction: sunlight.from_direction.extend(0.0),
        };

        // render skybox to retina
        {
            let skybox_render_pass = self
                .device
                .begin_render_pass(
                    &command_buffer,
                    &[ColorTargetInfo::default()
                        .with_texture(&self.retina.surface())
                        // technically, I think LoadOp::DONT_CARE should be OK since we
                        // should be writing on the whole retina surface, but just in
                        // case let's clear
                        .with_load_op(LoadOp::CLEAR)
                        .with_clear_color(Color::RGB(127, 127, 127))
                        .with_store_op(StoreOp::STORE)],
                    None,
                )
                .unwrap();
            self.device
                .set_viewport(&skybox_render_pass, self.retina.viewport());
            self.skybox_renderer
                .render(&skybox_render_pass, &command_buffer, &u_lamp);
            self.device.end_render_pass(skybox_render_pass);
        }

        // render meshes to retina
        {
            let mesh_render_pass = self
                .device
                .begin_render_pass(
                    &command_buffer,
                    &[ColorTargetInfo::default()
                        .with_texture(&self.retina.surface())
                        .with_load_op(LoadOp::LOAD)],
                    Some(
                        &DepthStencilTargetInfo::new()
                            .with_texture(&mut self.dbuf)
                            .with_clear_depth(1.0)
                            .with_load_op(LoadOp::CLEAR)
                            .with_store_op(StoreOp::DONT_CARE)
                            .with_stencil_load_op(LoadOp::DONT_CARE)
                            .with_stencil_store_op(StoreOp::DONT_CARE)
                            .with_cycle(true),
                    ),
                )
                .unwrap();
            self.device
                .set_viewport(&mesh_render_pass, self.retina.viewport());
            self.mesh_renderer
                .render(&command_buffer, &mesh_render_pass, &u_eyeball, &u_lamp);
            self.device.end_render_pass(mesh_render_pass);
        }

        // blit off-screen surface to screen
        {
            let swapchain_texture = command_buffer
                .wait_and_acquire_swapchain_texture(&self.window)
                .unwrap();
            let retina_target = self.retina.prepare_target(&swapchain_texture);
            self.retina.render(&command_buffer, retina_target);
        }

        command_buffer.submit().unwrap();
    }

    pub fn get_gpu_model_name(&self) -> &CStr {
        unsafe {
            let properties = sdl3::sys::gpu::SDL_GetGPUDeviceProperties(self.device.raw());
            let property_value = CStr::from_ptr(sdl3::sys::properties::SDL_GetStringProperty(
                properties,
                sdl3::sys::gpu::SDL_PROP_GPU_DEVICE_NAME_STRING,
                c"".as_ptr(),
            ));
            property_value
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Eyeball {
    pub position: Vec3,
    pub facing: Vec3,
    pub fov: f32,
    pub aspect_ratio: f32,
}
impl Eyeball {
    fn perspective(&self) -> Mat4 {
        // nb: SDL_GPU uses DirectX-like convention
        glam::camera::rh::proj::directx::perspective(self.fov, self.aspect_ratio, 0.1, 200.0)
    }
    fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_to_mat4(self.position, self.facing, vec3(0.0, 0.0, 1.0))
    }
}
impl Default for Eyeball {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            facing: Vec3::X,
            fov: 45_f32.to_radians(),
            aspect_ratio: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct UEyeball {
    world_position: Vec4,
    view: Mat4,
    view_perspective: Mat4,
}
impl UEyeball {
    fn from_eyeball(eyeball: &Eyeball) -> Self {
        let perspective = eyeball.perspective();
        let view = eyeball.view();
        Self {
            world_position: eyeball.position.extend(1.0),
            view,
            view_perspective: perspective * view,
        }
    }
}
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct ULamp {
    from_direction: Vec4,
}

#[derive(Debug, Clone, Copy)]
pub struct Sunlight {
    pub from_direction: Vec3,
}
