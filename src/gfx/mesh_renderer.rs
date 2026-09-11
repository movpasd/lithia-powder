use glam::{Mat4, Vec4};
use sdl3::gpu::{
    Buffer, BufferBinding, BufferRegion, BufferUsageFlags, ColorTargetDescription, CommandBuffer,
    CompareOp, CullMode, DepthStencilState, Device, FillMode, FrontFace, GraphicsPipeline,
    GraphicsPipelineTargetInfo, IndexElementSize, PrimitiveType, RasterizerState, RenderPass,
    Shader, ShaderFormat, ShaderStage, TextureFormat, TransferBuffer, TransferBufferLocation,
    VertexAttribute, VertexBufferDescription, VertexElementFormat, VertexInputRate,
    VertexInputState,
};

use crate::mesh::{self, Mesh};

use crate::geom::Pose;

pub struct MeshRenderer {
    pipeline: GraphicsPipeline,
    vbuf: Buffer,
    ibuf: Buffer,
    tbuf1: TransferBuffer,
    tbuf2: TransferBuffer,
    mesh_data_sbuf: Buffer,
    mesh_buf_entries: Vec<MeshBufferEntry>,
}
impl MeshRenderer {
    const MAX_VERTEXES: u32 = 512 * 1_024;
    const MAX_INDEXES: u32 = 512 * 1_024;
    const MAX_MESHES: u32 = 512;

    pub fn new(device: &Device, texture_format: TextureFormat) -> Self {
        let pipeline = {
            // load and compile shaders
            let vertex_shader: Shader;
            let fragment_shader: Shader;
            {
                let compiler = shaderc::Compiler::new().unwrap();

                let vert_spirv = shaders::vert_spirv(&compiler);
                vertex_shader = device
                    .create_shader()
                    .with_code(ShaderFormat::SPIRV, &vert_spirv, ShaderStage::Vertex)
                    .with_uniform_buffers(2)
                    .with_storage_buffers(1)
                    .build()
                    .unwrap();

                let frag_spirv = shaders::frag_spirv(&compiler);
                fragment_shader = device
                    .create_shader()
                    .with_code(ShaderFormat::SPIRV, &frag_spirv, ShaderStage::Fragment)
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
        };

        let vbuf = device
            .create_buffer()
            .with_usage(BufferUsageFlags::VERTEX)
            .with_size(Self::MAX_VERTEXES * size_of::<GpuMeshVertex>() as u32)
            .build()
            .unwrap();
        let ibuf = device
            .create_buffer()
            .with_usage(BufferUsageFlags::INDEX)
            .with_size(Self::MAX_INDEXES * size_of::<u32>() as u32)
            .build()
            .unwrap();
        let mesh_data_sbuf = device
            .create_buffer()
            .with_usage(BufferUsageFlags::GRAPHICS_STORAGE_READ)
            .with_size(size_of::<SMeshData>() as u32)
            .build()
            .unwrap();
        let tbuf1 = device
            .create_transfer_buffer()
            .with_size(vbuf.len())
            .build()
            .unwrap();
        let tbuf2 = device
            .create_transfer_buffer()
            .with_size(ibuf.len())
            .build()
            .unwrap();
        let mesh_buf_entries = vec![];

        Self {
            pipeline,
            vbuf,
            ibuf,
            tbuf1,
            tbuf2,
            mesh_data_sbuf,
            mesh_buf_entries,
        }
    }

    pub fn prepare(
        &mut self,
        device: &Device,
        command_buffer: &CommandBuffer,
        meshes_update: Option<&[Mesh<Vec4>]>,
        poses_update: Option<&[Pose]>,
    ) {
        if let Some(meshes) = meshes_update {
            self.reupload_meshes(device, command_buffer, meshes);
        }
        if let Some(poses) = poses_update {
            self.reupload_poses(device, command_buffer, poses);
        }
    }

    fn reupload_meshes(
        &mut self,
        device: &Device,
        command_buffer: &CommandBuffer,
        meshes: &[Mesh<Vec4>],
    ) {
        // accumulate data into local byte array, keeping track of entries
        let mut vbuf_data: Vec<u8> = vec![];
        let mut ibuf_data: Vec<u8> = vec![];
        let mut mesh_buf_entries = vec![];
        let mut next_first_index: u32 = 0;
        let mut next_vertex_offset: i32 = 0;
        for (mesh_id, mesh) in meshes.iter().enumerate() {
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
            self.tbuf1.map(device, true).mem_mut()[0..vbuf_data.len()].copy_from_slice(&vbuf_data);
            self.tbuf2.map(device, true).mem_mut()[0..ibuf_data.len()].copy_from_slice(&ibuf_data);

            let copy_pass = device.begin_copy_pass(command_buffer).unwrap();
            copy_pass.upload_to_gpu_buffer(
                TransferBufferLocation::new().with_transfer_buffer(&self.tbuf1),
                BufferRegion::new()
                    .with_buffer(&self.vbuf)
                    .with_size(self.vbuf.len()),
                true,
            );
            copy_pass.upload_to_gpu_buffer(
                TransferBufferLocation::new().with_transfer_buffer(&self.tbuf2),
                BufferRegion::new()
                    .with_buffer(&self.ibuf)
                    .with_size(self.ibuf.len()),
                true,
            );
            device.end_copy_pass(copy_pass);
        }

        self.mesh_buf_entries = mesh_buf_entries;
    }

    fn reupload_poses(&mut self, device: &Device, command_buffer: &CommandBuffer, poses: &[Pose]) {
        let pose_transforms = {
            let mut pose_transforms = [Mat4::ZERO; _];
            for (i, pose) in poses.iter().enumerate() {
                pose_transforms[i] = pose.to_transform();
            }
            pose_transforms
        };
        // ensure that tbuf1 has enough room for at least 1 SMeshData before copying it over
        assert!(self.tbuf1.len() >= size_of::<SMeshData>() as u32);
        self.tbuf1.map::<SMeshData>(device, true).mem_mut()[0] = SMeshData { pose_transforms };

        let pose_upload_pass = device.begin_copy_pass(command_buffer).unwrap();
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
        device.end_copy_pass(pose_upload_pass);
    }

    pub fn render(
        &self,
        command_buffer: &CommandBuffer,
        render_pass: &RenderPass,
        u_eyeball: &super::uniforms::UEyeball,
        u_lamp: &super::uniforms::ULamp,
    ) {
        command_buffer.push_vertex_uniform_data(0, u_eyeball);
        command_buffer.push_vertex_uniform_data(1, u_lamp);
        command_buffer.push_fragment_uniform_data(0, u_eyeball);
        command_buffer.push_fragment_uniform_data(1, u_lamp);

        render_pass.bind_graphics_pipeline(&self.pipeline);
        render_pass.bind_vertex_buffers(0, &[BufferBinding::new().with_buffer(&self.vbuf)]);
        render_pass.bind_index_buffer(
            &BufferBinding::new().with_buffer(&self.ibuf),
            IndexElementSize::_32BIT,
        );
        render_pass.bind_vertex_storage_buffers(0, std::slice::from_ref(&self.mesh_data_sbuf));

        for &MeshBufferEntry {
            first_index: ibuf_offset,
            num_indices: ibuf_count,
            vertex_offset: vbuf_offset,
        } in self.mesh_buf_entries.iter()
        {
            render_pass.draw_indexed_primitives(ibuf_count, 1, ibuf_offset, vbuf_offset, 0);
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

    fn from_mesh_vertex(mesh_id: u32, mesh_vertex: &mesh::Vertex<Vec4>) -> Self {
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
#[repr(C)]
struct SMeshData {
    pose_transforms: [Mat4; MeshRenderer::MAX_MESHES as usize],
}

/// interface between GLSL shaders and CPU data
mod shaders {
    use shaderc::{Compiler, ShaderKind};

    const VERT_PATH: &str = "shaders/mesh.vert.glsl";
    const VERT_SOURCE: &str = include_str!("shaders/mesh.vert.glsl");
    pub fn vert_spirv(compiler: &Compiler) -> Box<[u8]> {
        Box::from(
            compiler
                .compile_into_spirv(VERT_SOURCE, ShaderKind::Vertex, VERT_PATH, "main", None)
                .unwrap()
                .as_binary_u8(),
        )
    }

    const FRAG_PATH: &str = "shaders/mesh.frag.glsl";
    const FRAG_SOURCE: &str = include_str!("shaders/mesh.frag.glsl");
    pub fn frag_spirv(compiler: &Compiler) -> Box<[u8]> {
        Box::from(
            compiler
                .compile_into_spirv(FRAG_SOURCE, ShaderKind::Fragment, FRAG_PATH, "main", None)
                .unwrap()
                .as_binary_u8(),
        )
    }
}
