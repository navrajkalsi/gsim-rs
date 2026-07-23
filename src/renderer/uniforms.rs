use crate::{config::Point, geometry::uniforms::Uniforms};
use wgpu::{BindGroupLayoutEntry, util::DeviceExt};
use winit::dpi::PhysicalSize;

const MAX_TRAVELS: Point = Point {
    x: 500.0,
    y: 250.0,
    z: 250.0,
};

// static data to be passed to the shader, that is common to vertices
pub fn setup_uniforms(
    window_size: PhysicalSize<u32>,
    device: &wgpu::Device,
) -> (
    Uniforms,
    wgpu::Buffer,
    wgpu::BindGroupLayout,
    wgpu::BindGroup,
) {
    let uniforms = Uniforms::new(window_size, MAX_TRAVELS);

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniforms Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Uniforms Bind Group Layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Uniforms Bind Group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });

    (uniforms, uniform_buffer, bind_group_layout, bind_group)
}
