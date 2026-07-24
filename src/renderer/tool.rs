//! # Tool
//!
//! Sets up the GPU resources for rendering [`ToolInstance`]s.

use crate::geometry::tools::ToolInstance;

/// Creates a pipeline for drawing [`ToolInstance`]s.
///
/// The pipeline expects the **instance buffer** at slot `0` in the render pass.
///
/// Returns a tuple consisting of:
/// - Render pipeline.
/// - Instance buffer, for uploading at most a **single** [`ToolInstance`] to the GPU.
pub fn setup_pipeline(
    device: &wgpu::Device,
    uniform_bind_group_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::Buffer) {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/tool.wgsl"));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Tool Pipeline Layout"),
        bind_group_layouts: &[Some(uniform_bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Tool Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[ToolInstance::instance_buffer_layout()],
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
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0, // use all
            alpha_to_coverage_enabled: false,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Tool Instance Buffer"),
        size: size_of::<ToolInstance>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    (pipeline, instance_buffer)
}
