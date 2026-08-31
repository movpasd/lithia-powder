use glam::{vec3, vec4, Mat4, Quat, Vec3, Vec4};
use sdl3::{
    self,
    gpu::{
        Buffer, BufferBinding, BufferRegion, BufferUsageFlags, ColorTargetDescription,
        ColorTargetInfo, CompareOp, CullMode, DepthStencilState, DepthStencilTargetInfo, Device,
        FillMode, FrontFace, GraphicsPipeline, GraphicsPipelineTargetInfo, IndexElementSize,
        LoadOp, PrimitiveType, RasterizerState, SampleCount, Shader, ShaderFormat, ShaderStage,
        StoreOp, Texture, TextureCreateInfo, TextureFormat, TextureType, TextureUsage,
        TransferBuffer, TransferBufferLocation, VertexAttribute, VertexBufferDescription,
        VertexElementFormat, VertexInputRate, VertexInputState,
    },
    pixels::Color,
    video::Window,
    Sdl,
};
use std::ffi::CStr;

pub struct State {
    window: Window,
    device: Device,
    pipeline: GraphicsPipeline,
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
            .window("lithia-powder", 980, 640)
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let mut device = sdl3::gpu::Device::new(ShaderFormat::SPIRV, true).unwrap();
        device = device.with_window(&window).unwrap();

        let pipeline = Self::new_render_pipeline(&device, &window);

        // resource creation
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
        let mesh_data_sbuf = device
            .create_buffer()
            .with_usage(BufferUsageFlags::GRAPHICS_STORAGE_READ)
            .with_size(1_024 * 1_024)
            .build()
            .unwrap();

        State {
            window,
            device,
            pipeline,
            mesh_vbuf,
            mesh_ibuf,
            dbuf,
            tbuf1,
            tbuf2,
            mesh_data_sbuf,
            mesh_buf_entries: vec![],
        }
    }

    pub fn get_window_size(&self) -> (f32, f32) {
        let (uwidth, uheight) = self.window.size();
        (uwidth as f32, uheight as f32)
    }

    fn new_render_pipeline(device: &Device, window: &Window) -> GraphicsPipeline {
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

        let color_target_info = {
            // need to grab screen texture and convert it to a color target _first_,
            // because .wait_and_acquire_swapchain_texture() takes cbuf as &mut (for
            // seemingly no reason nor safety improvement)
            let screen_texture = cbuf
                .wait_and_acquire_swapchain_texture(&self.window)
                .unwrap();
            ColorTargetInfo::default()
                .with_texture(&screen_texture)
                .with_load_op(LoadOp::CLEAR)
                .with_clear_color(Color::RGB(127, 127, 127))
        };

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

        // render pass
        {
            let render_pass = self
                .device
                .begin_render_pass(
                    &cbuf,
                    &[color_target_info],
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
