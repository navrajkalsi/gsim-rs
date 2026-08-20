//! # Line
//!
//! Sets up the GPU resources for rendering [`LineInstance`]s.

use crate::geometry::line::LineInstance;
use wgpu::util::DeviceExt;

/// Creates a pipeline for drawing **anti-aliased** [`LineInstance`]s.
///
/// The pipeline expects the following buffers in the render pass:
/// - **vertex buffer** at slot `0`.
/// - **instance buffer** at slot `1`.
///
/// Returns a tuple consisting of:
/// - Render pipeline.
/// - Vertex buffer, used to expand a single instance into multiple unique vertices.
/// - Instance buffer, for uploading [`LineInstance`] to the GPU.
/// - Index buffer, used to reuse vertices from the vertex buffer without duplicating them.
pub fn setup_pipeline(
    device: &wgpu::Device,
    uniform_bind_group_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::Buffer,
) {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/line.wgsl"));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Lines Pipeline Layout"),
        bind_group_layouts: &[Some(uniform_bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Lines Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[
                // slot for the render pass is decided here
                LineInstance::vertex_buffer_layout(),
                LineInstance::instance_buffer_layout(),
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None, // render every triangle, irrespective of forward facing or not
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 4,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        // enable anti aliasing with alpha for color
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    // this vertex buffer is constant and can be mapped at creation
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Lines Vertex Buffer"),
        contents: bytemuck::cast_slice(&LineInstance::vertices()),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Lines Instance Buffer"),
        size: super::MAX_INSTANCES * size_of::<LineInstance>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Lines Index Buffer"),
        contents: bytemuck::cast_slice(&LineInstance::indices()),
        usage: wgpu::BufferUsages::INDEX,
    });

    (pipeline, vertex_buffer, instance_buffer, index_buffer)
}
