use glam::{vec2, vec3, vec4, Mat4, Quat, Vec2, Vec3, Vec4};
use sdl3::{
    self,
    gpu::{
        BlitInfo, Buffer, BufferBinding, BufferRegion, BufferUsageFlags, ColorTargetDescription,
        ColorTargetInfo, CompareOp, CullMode, DepthStencilState, DepthStencilTargetInfo, Device,
        FillMode, Filter, FrontFace, GraphicsPipeline, GraphicsPipelineTargetInfo,
        IndexElementSize, LoadOp, PrimitiveType, RasterizerState, SampleCount, Shader,
        ShaderFormat, ShaderStage, StoreOp, Texture, TextureCreateInfo, TextureFormat, TextureType,
        TextureUsage, TransferBuffer, TransferBufferLocation, VertexAttribute,
        VertexBufferDescription, VertexElementFormat, VertexInputRate, VertexInputState, Viewport,
    },
    pixels::Color,
    video::Window,
    Sdl,
};
use std::ffi::CStr;

const WINDOW_WIDTH: u32 = 1920;
const WINDOW_HEIGHT: u32 = 1080;
const MAX_SCREEN_WIDTH: u32 = 1920;
const MAX_SCREEN_HEIGHT: u32 = 1080;
const RETINA_WIDTH: f32 = 240.0;
const RETINA_HEIGHT: f32 = 180.0;
const RETINA_TO_SCREEN_SCALE: f32 = 6.0;

pub struct State {
    window: Window,
    retina: Retina,
    device: Device,
    pipeline: GraphicsPipeline,
    off_screen_surface: Texture<'static>,
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

        let retina = Retina {
            width: RETINA_WIDTH,
            height: RETINA_HEIGHT,
        };

        let mut device = sdl3::gpu::Device::new(ShaderFormat::SPIRV, true).unwrap();
        device = device.with_window(&window).unwrap();
        let swapchain_texture_format = device.get_swapchain_texture_format(&window);

        let pipeline = Self::new_render_pipeline(&device, swapchain_texture_format);

        // resource creation
        let off_screen_surface = device
            .create_texture(
                TextureCreateInfo::new()
                    .with_type(TextureType::_2D)
                    .with_format(swapchain_texture_format)
                    .with_usage(TextureUsage::COLOR_TARGET | TextureUsage::SAMPLER)
                    // strictly speaking, these should be the max _retina_ sizes, not
                    // _screen_ sizes.
                    .with_width(MAX_SCREEN_WIDTH)
                    .with_height(MAX_SCREEN_HEIGHT)
                    .with_layer_count_or_depth(1)
                    .with_num_levels(1)
                    .with_sample_count(SampleCount::NoMultiSampling),
            )
            .unwrap();
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
            retina,
            device,
            pipeline,
            off_screen_surface,
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
        (self.retina.width, self.retina.height)
    }

    fn new_render_pipeline(device: &Device, texture_format: TextureFormat) -> GraphicsPipeline {
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

    /// starts a copy pass
    pub fn update_meshes<'a>(
        &mut self,
        meshes: impl IntoIterator<Item = &'a super::obmesh::Mesh<Vec4>>,
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
                .map(|mesh_vertex| GpuVertex::from_mesh_vertex(mesh_id as u32, mesh_vertex))
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

    pub fn render<'a>(&mut self, camera: &Camera, poses: impl IntoIterator<Item = &'a Pose>) {
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

        // prepare swapchain info; unfortunately, .wait_and_acquire_swapchain_texture()
        // takes &'a mut CommandBuffer, so we have to prepare anything that uses it
        // ahead of time so that the texture handle Texture<'a> can be dropped. this is
        // all safe because it happens inside of the cbuf lifetime.
        let mut blit: BlitInfo;
        let swapchain_width: f32;
        let swapchain_height: f32;
        {
            let swapchain_texture = cbuf
                .wait_and_acquire_swapchain_texture(&self.window)
                .unwrap();
            (swapchain_width, swapchain_height) = (
                swapchain_texture.width() as f32,
                swapchain_texture.height() as f32,
            );
            blit = BlitInfo::default().with_destination_texture(&swapchain_texture);
        }

        // render to off-screen surface
        {
            let render_pass = self
                .device
                .begin_render_pass(
                    &cbuf,
                    &[ColorTargetInfo::default()
                        .with_texture(&self.off_screen_surface)
                        .with_clear_color(Color::RGB(127, 127, 127))
                        .with_load_op(LoadOp::CLEAR)],
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
                self.device.set_viewport(
                    &render_pass,
                    Viewport::new(0.0, 0.0, self.retina.width, self.retina.height, 0.0, 1.0),
                );

                render_pass.bind_graphics_pipeline(&self.pipeline);
                render_pass
                    .bind_vertex_buffers(0, &[BufferBinding::new().with_buffer(&self.mesh_vbuf)]);
                render_pass.bind_index_buffer(
                    &BufferBinding::new().with_buffer(&self.mesh_ibuf),
                    IndexElementSize::_32BIT,
                );
                render_pass
                    .bind_vertex_storage_buffers(0, std::slice::from_ref(&self.mesh_data_sbuf));

                let u_camera = UCamera::from_camera(camera);
                let u_lamp = ULamp {
                    from_direction: vec4(-1.0, -2.0, 2.0, 0.0).normalize(),
                };
                cbuf.push_vertex_uniform_data(0, &u_camera);
                cbuf.push_vertex_uniform_data(1, &u_lamp);
                cbuf.push_fragment_uniform_data(0, &u_camera);
                cbuf.push_fragment_uniform_data(1, &u_lamp);

                for &MeshBufferEntry {
                    first_index: ibuf_offset,
                    num_indices: ibuf_count,
                    vertex_offset: vbuf_offset,
                } in self.mesh_buf_entries.iter()
                {
                    render_pass.draw_indexed_primitives(ibuf_count, 1, ibuf_offset, vbuf_offset, 0);
                }
            }
            self.device.end_render_pass(render_pass);
        }

        // blit off-screen surface to screen
        {
            let ((src_x, src_y, src_w, src_h), (dest_x, dest_y, dest_w, dest_h)) = {
                let ((clip_tl_retpx, clip_diag_retpx), (target_tl_scpx, target_diag_scpx)) =
                    self.retina.calc_screen_blit(
                        RETINA_TO_SCREEN_SCALE,
                        vec2(swapchain_width, swapchain_height),
                    );
                (
                    (
                        clip_tl_retpx.x as u32,
                        clip_tl_retpx.y as u32,
                        clip_diag_retpx.x as u32,
                        clip_diag_retpx.y as u32,
                    ),
                    (
                        target_tl_scpx.x as u32,
                        target_tl_scpx.y as u32,
                        target_diag_scpx.x as u32,
                        target_diag_scpx.y as u32,
                    ),
                )
            };

            blit = blit
                .with_source_texture(&self.off_screen_surface)
                .with_source_region(0, src_x, src_y, src_w, src_h)
                .with_source_mip(0)
                .with_destination_region(0, dest_x, dest_y, dest_w, dest_h)
                .with_destination_mip(0)
                .with_load_op(LoadOp::CLEAR)
                .with_clear_color(Color::RGB(0, 0, 0))
                .with_filter(Filter::Nearest)
                .with_cycle(false);
            cbuf.blit_texture(blit);
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
struct GpuVertex {
    model_position: Vec4,
    model_normal: Vec4,
    color: Vec4,
    mesh_id: u32,
    _pad: [u8; 12],
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
            VertexAttribute::new()
                .with_buffer_slot(buffer_slot)
                .with_location(3)
                .with_offset(48)
                .with_format(VertexElementFormat::Uint),
        ]
    }

    fn from_mesh_vertex(mesh_id: u32, mesh_vertex: &super::obmesh::Vertex<Vec4>) -> Self {
        Self {
            model_position: mesh_vertex.position,
            color: mesh_vertex.data,
            model_normal: mesh_vertex.normal,
            mesh_id,
            _pad: [0; _],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub position: Vec3,
    pub facing: Vec3,
    pub fov: f32,
    pub aspect_ratio: f32,
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
impl Default for Camera {
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
struct UCamera {
    world_position: Vec4,
    view: Mat4,
    view_perspective: Mat4,
}
impl UCamera {
    fn from_camera(camera: &Camera) -> Self {
        let perspective = camera.perspective();
        let view = camera.view();
        Self {
            world_position: camera.position.extend(1.0),
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

/// represents the surface which the camera sees, and provides utilities for sizing that
/// onto a screen
///
/// the logic herein supports pixel-perfect blitting, preferring to cut pixels off
/// rather than distort.
#[derive(Debug, Clone, Copy)]
struct Retina {
    width: f32,
    height: f32,
}
impl Retina {
    /// works out how to blit the image on the retina onto the screen, so the retina
    /// image is scaled and centred on the screen
    ///
    /// returns two rectangles.
    ///
    /// each rectangle is in the format (tl: Vec2, diag: Vec2); `tl` is the top-left
    /// of the rectangle; `diag` is the diagonal from the top-left to the bottom-right,
    /// therefore its two co-ordinates are the rectangle's width and height
    /// respectively. in other words (tl, size) == ([x, y], [w, h]).
    ///
    /// the first returned rectangle is in retpx (retina pixels) and represents the clip
    /// region to cut out of the retina; the second is in scpx (screen pixels) and
    /// represents the blit target region of the cut out region.
    ///
    /// retpx and scpx coordinates are measured in texel space, i.e.: origin in the
    /// top-left corner.
    ///
    /// if `scale` is an integer (i.e.: has fractional part == 0.0) and both sizes in
    /// `screen_size_scpx` are even, all returned co-ordinates should be integers.
    fn calc_screen_blit(
        &self,
        retina_to_screen_scale: f32,
        screen_diag_scpx: Vec2,
    ) -> ((Vec2, Vec2), (Vec2, Vec2)) {
        // 0. useful values
        let screen_centre_scpx = screen_diag_scpx / 2.0;
        let screen_tl_scpx = vec2(0.0, 0.0);
        let screen_br_scpx = screen_tl_scpx + screen_diag_scpx; // br = bottom-right

        let retina_diag_retpx = vec2(self.width, self.height);
        let retina_centre_retpx = retina_diag_retpx / 2.0;

        // 1. calculate scpx coordinates of the blit if there was no clipping (so if the scaled blit is too large )
        let unclipped_diag_scpx = retina_diag_retpx * retina_to_screen_scale;
        let unclipped_tl_scpx = screen_centre_scpx - unclipped_diag_scpx / 2.0;
        let unclipped_br_scpx = screen_centre_scpx + unclipped_diag_scpx / 2.0;

        // 2. constrain the rectangle to the size of the screen to get target
        let target_tl_scpx = unclipped_tl_scpx.clamp(screen_tl_scpx, screen_br_scpx);
        let target_br_scpx = unclipped_br_scpx.clamp(screen_tl_scpx, screen_br_scpx);

        // 3. convert the clip back to retina co-ords to work out clip region
        let target_diag_scpx = target_br_scpx - target_tl_scpx;
        let clip_diag_retpx = target_diag_scpx / retina_to_screen_scale;
        let clip_tl_retpx = retina_centre_retpx - clip_diag_retpx / 2.0;

        (
            (clip_tl_retpx, clip_diag_retpx),
            (target_tl_scpx, target_diag_scpx),
        )
    }
}
