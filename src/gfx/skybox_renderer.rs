use glam::{Vec4, vec4};
use sdl3::gpu::{
    Buffer, BufferUsageFlags, ColorTargetDescription, CompareOp, CullMode, DepthStencilState,
    Device, FillMode, FrontFace, GraphicsPipeline, GraphicsPipelineTargetInfo, PrimitiveType,
    RasterizerState, Shader, ShaderFormat, ShaderStage, TextureFormat, TransferBuffer,
    VertexAttribute, VertexBufferDescription, VertexElementFormat, VertexInputRate,
    VertexInputState,
};

use super::Eyeball;

pub struct SkyboxRenderer {
    pub tbuf: TransferBuffer,
    pub vbuf: Buffer,
    pub ibuf: Buffer,
    pub pipeline: GraphicsPipeline,
}
impl SkyboxRenderer {
    pub fn new(device: &Device, texture_format: TextureFormat) -> Self {
        let tbuf = device
            .create_transfer_buffer()
            .with_size(1024)
            .build()
            .unwrap();
        let vbuf = device
            .create_buffer()
            .with_usage(BufferUsageFlags::VERTEX)
            .with_size(1_024)
            .build()
            .unwrap();
        let ibuf = device
            .create_buffer()
            .with_usage(BufferUsageFlags::INDEX)
            .with_size(1_024)
            .build()
            .unwrap();
        let pipeline = {
            // load and compile shaders
            let vertex_shader: Shader;
            let fragment_shader: Shader;
            {
                use shaderc::ShaderKind;

                let compiler = shaderc::Compiler::new().unwrap();

                let vertex_source = include_str!("shaders/skybox.vert.glsl");
                let vertex_ir = compiler
                    .compile_into_spirv(
                        vertex_source,
                        ShaderKind::Vertex,
                        "shaders/skybox.vert.glsl",
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
                    .with_uniform_buffers(0)
                    .with_storage_buffers(0)
                    .build()
                    .unwrap();

                let fragment_source = include_str!("shaders/skybox.frag.glsl");
                let fragment_ir = compiler
                    .compile_into_spirv(
                        fragment_source,
                        ShaderKind::Fragment,
                        "shaders/skybox.frag.glsl",
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
                    .with_uniform_buffers(1)
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
                            .with_pitch(size_of::<GpuSkyboxVertex>() as u32)
                            .with_input_rate(VertexInputRate::Vertex)])
                        .with_vertex_attributes(&GpuSkyboxVertex::get_attributes(0)),
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
                        .with_compare_op(CompareOp::Invalid)
                        .with_enable_depth_test(false)
                        .with_enable_depth_write(false),
                )
                .with_target_info(
                    GraphicsPipelineTargetInfo::new()
                        .with_color_target_descriptions(&[
                            ColorTargetDescription::new().with_format(texture_format)
                        ])
                        .with_has_depth_stencil_target(false),
                )
                .build()
                .unwrap()
        };
        SkyboxRenderer {
            tbuf,
            vbuf,
            ibuf,
            pipeline,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
/// aligned vertex data for the vertex shader
pub struct GpuSkyboxVertex {
    ndc_position: Vec4,
    world_normal: Vec4,
}
impl GpuSkyboxVertex {
    pub fn get_attributes(buffer_slot: u32) -> Vec<VertexAttribute> {
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
        ]
    }

    pub fn calculate_data(eyeball: &Eyeball) -> ([Self; 4], [u32; 6]) {
        let ndc_positions = [
            vec4(-1.0, -1.0, 1.0, 1.0),
            vec4(1.0, -1.0, 1.0, 1.0),
            vec4(-1.0, 1.0, 1.0, 1.0),
            vec4(1.0, 1.0, 1.0, 1.0),
        ];
        let inv_view = eyeball.view().transpose();
        let inv_persp = eyeball.perspective().inverse();
        let world_normals =
            ndc_positions.map(|ndc_pos| (inv_view * inv_persp * ndc_pos).with_w(0.0));

        let skybox_vertexes: [Self; 4] = std::array::from_fn(|i| Self {
            ndc_position: ndc_positions[i],
            world_normal: world_normals[i],
        });

        (skybox_vertexes, [0, 1, 3, 3, 2, 0])
    }
}
