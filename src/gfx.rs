mod mesh_renderer;
mod retina;
mod skybox_renderer;

use glam::{Mat4, Quat, Vec3, Vec4, vec3};
use sdl3::{
    self, Sdl,
    gpu::{
        Buffer, BufferBinding, BufferRegion, BufferUsageFlags, ColorTargetDescription,
        ColorTargetInfo, CommandBuffer, CompareOp, CullMode, DepthStencilState,
        DepthStencilTargetInfo, Device, FillMode, FrontFace, GraphicsPipeline,
        GraphicsPipelineTargetInfo, IndexElementSize, LoadOp, PrimitiveType, RasterizerState,
        SampleCount, Shader, ShaderFormat, ShaderStage, StoreOp, Texture, TextureCreateInfo,
        TextureFormat, TextureType, TextureUsage, TransferBuffer, TransferBufferLocation,
        VertexAttribute, VertexBufferDescription, VertexElementFormat, VertexInputRate,
        VertexInputState,
    },
    pixels::Color,
    video::Window,
};
use std::ffi::CStr;

use retina::Retina;

use crate::gfx::skybox_renderer::SkyboxRenderer;

const WINDOW_WIDTH: u32 = 1920;
const WINDOW_HEIGHT: u32 = 1080;
const MAX_SCREEN_WIDTH: u32 = 1920;
const MAX_SCREEN_HEIGHT: u32 = 1080;
const RETINA_WIDTH: f32 = 320.0;
const RETINA_HEIGHT: f32 = 240.0;
const RETINA_TO_SCREEN_SCALE: f32 = 4.0;

const MESH_VBUF_SIZE_MB: u32 = 2;

pub struct State {
    window: Window,
    device: Device,
    retina: Retina,
    skybox_renderer: SkyboxRenderer,
    mesh_pipeline: GraphicsPipeline,
    mesh_vbuf: Buffer,
    mesh_ibuf: Buffer,
    dbuf: Texture<'static>,
    tbuf1: TransferBuffer,
    tbuf2: TransferBuffer,
    mesh_data_sbuf: Buffer,
    mesh_buf_entries: Vec<MeshBufferEntry>,
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
        let swapchain_texture_format = device.get_swapchain_texture_format(&window);

        let retina = Retina::new(&device, RETINA_WIDTH, RETINA_HEIGHT, RETINA_TO_SCREEN_SCALE);
        let skybox_renderer = SkyboxRenderer::new(&device, Retina::TEXTURE_FORMAT);

        let mesh_pipeline = Self::new_mesh_render_pipeline(&device, swapchain_texture_format);

        // resource creation
        let mesh_vbuf = device
            .create_buffer()
            .with_usage(BufferUsageFlags::VERTEX)
            .with_size(MESH_VBUF_SIZE_MB * 1_024 * 1_024)
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
                    .with_width(MAX_SCREEN_WIDTH)
                    .with_height(MAX_SCREEN_HEIGHT)
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
        let mesh_data_sbuf = device
            .create_buffer()
            .with_usage(BufferUsageFlags::GRAPHICS_STORAGE_READ)
            .with_size(1_024 * 1_024)
            .build()
            .unwrap();

        State {
            window,
            device,
            retina,
            skybox_renderer,
            mesh_pipeline,
            mesh_vbuf,
            mesh_ibuf,
            dbuf,
            tbuf1,
            tbuf2,
            mesh_data_sbuf,
            mesh_buf_entries: vec![],
        }
    }

    pub fn get_retina_size(&self) -> (f32, f32) {
        (self.retina.width(), self.retina.height())
    }
    pub fn window(&self) -> &Window {
        &self.window
    }

    fn new_mesh_render_pipeline(
        device: &Device,
        texture_format: TextureFormat,
    ) -> GraphicsPipeline {
        // load and compile shaders
        let vertex_shader: Shader;
        let fragment_shader: Shader;
        {
            use shaderc::ShaderKind;

            let compiler = shaderc::Compiler::new().unwrap();

            let vertex_source = include_str!("shaders/mesh.vert.glsl");
            let vertex_ir = compiler
                .compile_into_spirv(
                    vertex_source,
                    ShaderKind::Vertex,
                    "shaders/mesh.vert.glsl",
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
                .with_uniform_buffers(2)
                .with_storage_buffers(1)
                .build()
                .unwrap();

            let fragment_source = include_str!("shaders/mesh.frag.glsl");
            let fragment_ir = compiler
                .compile_into_spirv(
                    fragment_source,
                    ShaderKind::Fragment,
                    "shaders/mesh.frag.glsl",
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
                .with_uniform_buffers(2)
                .build()
                .unwrap();
        }

        device
            .create_graphics_pipeline()
            .with_vertex_shader(&vertex_shader)
            .with_fragment_shader(&fragment_shader)
            .with_vertex_input_state(
                VertexInputState::new()
                    .with_vertex_buffer_descriptions(&[VertexBufferDescription::new()
                        .with_slot(0)
                        .with_pitch(size_of::<GpuMeshVertex>() as u32)
                        .with_input_rate(VertexInputRate::Vertex)])
                    .with_vertex_attributes(&GpuMeshVertex::get_attributes(0)),
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

    /// starts a copy pass
    pub fn update_meshes<'a>(
        &mut self,
        meshes: impl IntoIterator<Item = &'a super::mesh::Mesh<Vec4>>,
    ) {
        // accumulate data into local byte array, keeping track of entries
        let mut vbuf_data: Vec<u8> = vec![];
        let mut ibuf_data: Vec<u8> = vec![];
        let mut mesh_buf_entries = vec![];
        let mut next_first_index: u32 = 0;
        let mut next_vertex_offset: i32 = 0;
        for (mesh_id, mesh) in meshes.into_iter().enumerate() {
            let gpu_vertexes: Vec<_> = mesh
                .vertexes
                .iter()
                .map(|mesh_vertex| GpuMeshVertex::from_mesh_vertex(mesh_id as u32, mesh_vertex))
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
            mesh_buf_entries.push(entry);
            next_first_index += mesh_index_count;
            next_vertex_offset += mesh_vertex_count;
        }
        {
            self.tbuf1.map(&self.device, true).mem_mut()[0..vbuf_data.len()]
                .copy_from_slice(&vbuf_data);
            self.tbuf2.map(&self.device, true).mem_mut()[0..ibuf_data.len()]
                .copy_from_slice(&ibuf_data);

            let data_upload = self.device.acquire_command_buffer().unwrap();
            {
                let copy_pass = self.device.begin_copy_pass(&data_upload).unwrap();
                copy_pass.upload_to_gpu_buffer(
                    TransferBufferLocation::new().with_transfer_buffer(&self.tbuf1),
                    BufferRegion::new()
                        .with_buffer(&self.mesh_vbuf)
                        .with_size(self.mesh_vbuf.len()),
                    true,
                );
                copy_pass.upload_to_gpu_buffer(
                    TransferBufferLocation::new().with_transfer_buffer(&self.tbuf2),
                    BufferRegion::new()
                        .with_buffer(&self.mesh_ibuf)
                        .with_size(self.mesh_ibuf.len()),
                    true,
                );
                self.device.end_copy_pass(copy_pass);
            }
            data_upload.submit().unwrap();
        }

        self.mesh_buf_entries = mesh_buf_entries;
    }

    fn submit_skybox_update_pass(&self, eyeball: &Eyeball, cbuf: &CommandBuffer) {
        let copy_pass = self.device.begin_copy_pass(cbuf).unwrap();
        let (vbuf_data, ibuf_data) = skybox_renderer::GpuSkyboxVertex::calculate_data(eyeball);
        self.tbuf1.map(&self.device, true).mem_mut()[0] = vbuf_data;
        self.tbuf2.map(&self.device, true).mem_mut()[0] = ibuf_data;
        copy_pass.upload_to_gpu_buffer(
            TransferBufferLocation::new()
                .with_transfer_buffer(&self.tbuf1)
                .with_offset(0),
            BufferRegion::new()
                .with_buffer(&self.skybox_renderer.vbuf)
                .with_offset(0)
                .with_size(size_of_val(&vbuf_data) as u32),
            true,
        );
        copy_pass.upload_to_gpu_buffer(
            TransferBufferLocation::new()
                .with_transfer_buffer(&self.tbuf2)
                .with_offset(0),
            BufferRegion::new()
                .with_buffer(&self.skybox_renderer.ibuf)
                .with_offset(0)
                .with_size(size_of_val(&ibuf_data) as u32),
            true,
        );
        self.device.end_copy_pass(copy_pass);
    }

    pub fn render<'a>(
        &mut self,
        eyeball: &Eyeball,
        poses: impl IntoIterator<Item = &'a Pose>,
        sunlight: &Sunlight,
    ) {
        self.retina.prepare();

        let mut cbuf = self.device.acquire_command_buffer().unwrap();

        // upload pose data to storage buffer
        {
            let pose_transforms = {
                let mut pose_transforms = [Mat4::ZERO; _];
                for (i, pose) in poses.into_iter().enumerate() {
                    pose_transforms[i] = pose.transform();
                }
                pose_transforms
            };
            // ensure that tbuf1 has enough room for at least 1 SMeshData before copying it over
            assert!(self.tbuf1.len() >= size_of::<SMeshData>() as u32);
            self.tbuf1.map::<SMeshData>(&self.device, true).mem_mut()[0] =
                SMeshData { pose_transforms };

            let pose_upload_pass = self.device.begin_copy_pass(&cbuf).unwrap();
            pose_upload_pass.upload_to_gpu_buffer(
                TransferBufferLocation::new()
                    .with_transfer_buffer(&self.tbuf1)
                    .with_offset(0),
                BufferRegion::new()
                    .with_buffer(&self.mesh_data_sbuf)
                    .with_offset(0)
                    .with_size(size_of::<SMeshData>() as u32),
                true,
            );
            self.device.end_copy_pass(pose_upload_pass);
        }

        // upload skybox data
        self.submit_skybox_update_pass(eyeball, &cbuf);

        // data preparation used in multiple passes
        let u_lamp = ULamp {
            from_direction: sunlight.from_direction.extend(0.0),
        };

        // render skybox to retina
        {
            let skybox_render_pass = self
                .device
                .begin_render_pass(
                    &cbuf,
                    &[ColorTargetInfo::default()
                        .with_texture(&self.retina.surface())
                        // technically, I think LoadOp::DONT_CARE should be OK since we should be
                        // writing on the whole retina surface, but just in case let's clear ---
                        // this takes care of the Clear op for the proper mesh render pass
                        .with_load_op(LoadOp::CLEAR)
                        .with_clear_color(Color::RGB(127, 127, 127))
                        .with_store_op(StoreOp::STORE)],
                    None,
                )
                .unwrap();
            self.device
                .set_viewport(&skybox_render_pass, self.retina.viewport());

            skybox_render_pass.bind_graphics_pipeline(&self.skybox_renderer.pipeline);
            skybox_render_pass.bind_vertex_buffers(
                0,
                &[BufferBinding::new()
                    .with_buffer(&self.skybox_renderer.vbuf)
                    .with_offset(0)],
            );
            skybox_render_pass.bind_index_buffer(
                &BufferBinding::new()
                    .with_buffer(&self.skybox_renderer.ibuf)
                    .with_offset(0),
                IndexElementSize::_32BIT,
            );
            cbuf.push_fragment_uniform_data(0, &u_lamp);
            skybox_render_pass.draw_indexed_primitives(6, 1, 0, 0, 0);
            self.device.end_render_pass(skybox_render_pass);
        }

        // render meshes to retina
        {
            let mesh_render_pass = self
                .device
                .begin_render_pass(
                    &cbuf,
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
            {
                self.device
                    .set_viewport(&mesh_render_pass, self.retina.viewport());

                mesh_render_pass.bind_graphics_pipeline(&self.mesh_pipeline);
                mesh_render_pass
                    .bind_vertex_buffers(0, &[BufferBinding::new().with_buffer(&self.mesh_vbuf)]);
                mesh_render_pass.bind_index_buffer(
                    &BufferBinding::new().with_buffer(&self.mesh_ibuf),
                    IndexElementSize::_32BIT,
                );
                mesh_render_pass
                    .bind_vertex_storage_buffers(0, std::slice::from_ref(&self.mesh_data_sbuf));

                let u_eyeball = UEyeball::from_eyeball(eyeball);
                cbuf.push_vertex_uniform_data(0, &u_eyeball);
                cbuf.push_vertex_uniform_data(1, &u_lamp);
                cbuf.push_fragment_uniform_data(0, &u_eyeball);
                cbuf.push_fragment_uniform_data(1, &u_lamp);

                for &MeshBufferEntry {
                    first_index: ibuf_offset,
                    num_indices: ibuf_count,
                    vertex_offset: vbuf_offset,
                } in self.mesh_buf_entries.iter()
                {
                    mesh_render_pass.draw_indexed_primitives(
                        ibuf_count,
                        1,
                        ibuf_offset,
                        vbuf_offset,
                        0,
                    );
                }
            }
            self.device.end_render_pass(mesh_render_pass);
        }

        // blit off-screen surface to screen
        {
            let swapchain_texture = cbuf
                .wait_and_acquire_swapchain_texture(&self.window)
                .unwrap();
            let retina_target = self.retina.prepare_target(&swapchain_texture);
            self.retina.render(&cbuf, retina_target);
        }

        cbuf.submit().unwrap();
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

struct MeshBufferEntry {
    first_index: u32,
    num_indices: u32,
    vertex_offset: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
/// aligned vertex data for the vertex shader
struct GpuMeshVertex {
    model_position: Vec4,
    model_normal: Vec4,
    color: Vec4,
    mesh_id: u32,
    _pad: [u8; 12],
}
impl GpuMeshVertex {
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
            VertexAttribute::new()
                .with_buffer_slot(buffer_slot)
                .with_location(3)
                .with_offset(48)
                .with_format(VertexElementFormat::Uint),
        ]
    }

    fn from_mesh_vertex(mesh_id: u32, mesh_vertex: &super::mesh::Vertex<Vec4>) -> Self {
        Self {
            model_position: mesh_vertex.position.extend(1.0),
            color: mesh_vertex.data,
            model_normal: mesh_vertex.normal.extend(0.0),
            mesh_id,
            _pad: [0; _],
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
#[repr(C)]
struct SMeshData {
    pose_transforms: [Mat4; Self::MAX_MESHES as usize],
}
impl SMeshData {
    const MAX_MESHES: u32 = 1024;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}
impl Pose {
    fn transform(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.position)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Sunlight {
    pub from_direction: Vec3,
}
