//! # Stock
//!
//! Sets up the GPU resources for rendering [`StockInstance`]s.

use crate::geometry::stock::{StockInstance, StockTracker};
use wgpu::util::DeviceExt;

/// Creates a pipeline for drawing [`StockInstance`]s.
///
/// The pipeline expects the following buffers in the render pass:
/// - **vertex buffer** at slot `0`.
/// - **instance buffer** at slot `1`.
///
/// **Back-face culling** is enabled,
/// which discards any triangles that are drawn in clockwise order.
///
/// This pipeline is also setup to accept an **intermediate** of size `4` bytes,
/// which can be changed during each render pass.
///
/// Returns a tuple consisting of:
/// - Render pipeline.
/// - Vertex buffer, used to expand a single instance into multiple unique vertices.
/// - Instance buffer, for uploading [`StockInstance`] to the GPU.
/// - Index buffer, used to reuse vertices from the vertex buffer without duplicating them.
pub fn setup_pipeline(
    device: &wgpu::Device,
    uniform_bind_group_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    stock_tracker: &StockTracker,
) -> (
    wgpu::RenderPipeline,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::Buffer,
) {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/stock.wgsl"));

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Stock Pipeline Layout"),
        bind_group_layouts: &[Some(uniform_bind_group_layout)],
        immediate_size: 4, // an f32
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Stock Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[
                // slot for the render pass is decided here
                StockInstance::vertex_buffer_layout(),
                StockInstance::instance_buffer_layout(),
            ],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back), // hide back faces
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Line,
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

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Stock Vertex Buffer"),
        contents: bytemuck::cast_slice(&StockInstance::vertices(stock_tracker)),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Stock Instance Buffer"),
        size: stock_tracker.total_count as u64 * size_of::<StockInstance>() as u64,
        // will only need at max full stock instances
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Stock Index Buffer"),
        contents: bytemuck::cast_slice(&StockInstance::indices()),
        usage: wgpu::BufferUsages::INDEX,
    });

    (pipeline, vertex_buffer, instance_buffer, index_buffer)
}
