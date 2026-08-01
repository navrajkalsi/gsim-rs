//! # Renderer
//!
//! Manages GPU resources and renders the simulation using the [`wgpu`] graphics API.

mod line;
mod stock;
mod tool;
mod uniforms;

use crate::{
    STOCK, Speed, TOOL, TOOLPATH, View,
    config::{Config, ToolConfig},
    geometry::{
        line::{BufferAction, LineInstance, LinesTracker},
        stock::{StockInstance, StockTracker},
        tools::ToolInstance,
        uniforms::Uniforms,
    },
    points::Point,
};
use std::sync::Arc;
use wgpu::CurrentSurfaceTexture;
use winit::{dpi::PhysicalSize, event_loop::OwnedDisplayHandle, window::Window};

/// Maximum number of [`LineInstance`]s allowed to be used in the [`Graphics::lines_instance_buffer`].
const MAX_INSTANCES: u64 = 1_000_000;

/// GPU state for toothpath simulation.
pub struct Graphics {
    /// Logical connection to a GPU.
    device: wgpu::Device,
    /// Command queue for [`Self::device`].
    queue: wgpu::Queue,
    /// Rendering surface created from a [`Window`].
    /// Since the surface holds a reference to the [`Window`] it was created from,
    /// the window is kept alive as long as the surface.
    surface: wgpu::Surface<'static>,
    /// Description of [`Self::surface`].
    surface_config: wgpu::SurfaceConfiguration,

    /// Depth texture configured to [`Self::surface`] size.
    depth_texture: wgpu::Texture,
    /// View for [`Self::depth_texture`] to be used in the render pass.
    depth_texture_view: wgpu::TextureView,
    /// Multisample anti-aliasing texture configured to [`Self::surface`] size.
    msaa_texture: wgpu::Texture,
    /// View for [`Self::msaa_texture`] to be used in the render pass.
    msaa_texture_view: wgpu::TextureView,

    /// Pipeline for rendering [`LineInstance`]s.
    lines_pipeline: wgpu::RenderPipeline,
    /// GPU buffer for storing all vertices unique to a [`LineInstance`].
    lines_vertex_buffer: wgpu::Buffer,
    /// GPU buffer configured to hold [`MAX_INSTANCES`] number of [`LineInstance`]s.
    lines_instance_buffer: wgpu::Buffer,
    /// GPU buffer for storing indices of [`Self::lines_vertex_buffer`],
    /// allowing reuse of vertices without duplication.
    lines_index_buffer: wgpu::Buffer,
    /// Total number of active [`LineInstance`]s in [`Self::lines_instance_buffer`].
    lines_count: u32,
    /// Memory offset to write next toolpath [`LineInstance`] to in [`Self::lines_instance_buffer`].
    lines_offset: u64,

    /// Pipeline for rendering the [`ToolInstance`].
    tool_pipeline: wgpu::RenderPipeline,
    /// GPU buffer configured to hold a **single** [`ToolInstance`].
    tool_instance_buffer: wgpu::Buffer,

    /// Pipeline for rendering [`StockInstance`]s.
    stock_pipeline: wgpu::RenderPipeline,
    /// GPU buffer for storing all vertices unique to a [`StockInstance`].
    stock_vertex_buffer: wgpu::Buffer,
    /// GPU buffer for storing all unique [`StockInstance`]s.
    stock_instance_buffer: wgpu::Buffer,
    /// GPU buffer for storing indices of [`Self::stock_vertex_buffer`],
    /// allowing reuse of vertices without duplication.
    stock_index_buffer: wgpu::Buffer,
    /// Total number of [`StockInstance`]s in [`Self::stock_instance_buffer`].
    stock_count: u32,

    /// Tracks total [`LineInstance`]s drawn and left to be drawn from the latest simulation move.
    pub lines_tracker: LinesTracker,

    /// Tracks state changes of [`StockInstance`]s during cutting moves.
    stock_tracker: StockTracker,

    /// Constant data shared across all the pipelines.
    uniforms: Uniforms,
    /// Read-only buffer containing [`Self::uniforms`].
    uniform_buffer: wgpu::Buffer,
    /// GPU bind group, with entry bound to [`Self::uniform_buffer`].
    uniform_bind_group: wgpu::BindGroup,

    /// Surface configured flag.
    /// Configured on the first [`Graphics::resize`] call.
    configured: bool,

    /// [`StockInstance`]s visibility flag.
    pub stock: bool,
    /// [`LineInstance`]s visibility flag.
    pub toolpath: bool,
    /// [`ToolInstance`] visibility flag.
    pub tool: bool,

    pub speed: Speed,

    tool_config: ToolConfig,

    // speed just batches up frames
    skipped_frames: u8,

    /// [`Arc`] keeps the [`Window`] valid for as long as [`Self::surface`] needs,
    /// and lets us use `'static` lifetime with the surface.
    pub window: Arc<Window>,
}

impl Graphics {
    /// Constructs a new [`Graphics`] by initializing all GPU resources, including:
    /// - [`Uniforms`] buffer and bind group, to pass constant data to the pipelines.
    /// - [`LineInstance`] buffers and pipeline.
    /// - [`ToolInstance`] buffer and pipeline. Creates a [`ToolInstance`],
    ///   with the tool at [`Config::start_pos`], and writes it to [`Self::tool_instance_buffer`].
    /// - [`StockInstance`] buffers and pipeline. Creates a stock corresponding to
    ///   [`Config::stock`], and writes it to [`Self::stock_instance_buffer`].
    ///
    // TODO sets up default tool
    ///
    /// Returns [`Error`](anyhow::Error) on failure to create any of the GPU resources.
    pub async fn build(
        handle: OwnedDisplayHandle,
        window: Arc<Window>,
        config: Config,
    ) -> anyhow::Result<Self> {
        let window_size = window.inner_size();

        // create entry point to the api
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::empty(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: Some(Box::new(handle)),
        });

        // a platform specific window to draw into
        let surface = instance.create_surface(window.clone())?;

        // handle to a physical gpu
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;

        // logical connection to a gpu and its command queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Device"),
                // required_features: wgpu::Features::POLYGON_MODE_LINE,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await?;

        // capabilities of a surface when used with a particular adapter(gpu)
        let surface_caps = surface.get_capabilities(&adapter);

        // try to use srgb or fallback
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(*surface_caps.formats.first().expect("At least one format must be present, as the adapter is created to be compatible with the surface"));

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window_size.width.max(1),
            height: window_size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2, // reasonable default in docs
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        let (depth_texture, depth_texture_view) = depth_texture(&device, window_size);

        let (msaa_texture, msaa_texture_view) = msaa_texture(&device, window_size, surface_format);

        let uniforms = Uniforms::new(window_size, config.stock, config.default_tool);

        let (uniform_buffer, uniform_bind_group_layout, uniform_bind_group) =
            uniforms::setup_uniforms(uniforms, &device);

        let (lines_pipeline, lines_vertex_buffer, lines_instance_buffer, lines_index_buffer) =
            line::setup_pipeline(&device, &uniform_bind_group_layout, surface_format);

        // show tool
        let tool = ToolInstance::at_point(config.start_pos);
        let (tool_pipeline, tool_instance_buffer) =
            tool::setup_pipeline(&device, &uniform_bind_group_layout, surface_format);
        queue.write_buffer(&tool_instance_buffer, 0, bytemuck::cast_slice(&[tool]));

        let stock_tracker = StockTracker::new(config.stock); // create new stock from config stock body
        let (stock_pipeline, stock_vertex_buffer, stock_instance_buffer, stock_index_buffer) =
            stock::setup_pipeline(
                &device,
                &uniform_bind_group_layout,
                surface_format,
                &stock_tracker,
            );
        queue.write_buffer(
            &stock_instance_buffer,
            0,
            bytemuck::cast_slice(stock_tracker.instances().1),
        );
        queue.submit([]);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,

            depth_texture,
            depth_texture_view,
            msaa_texture,
            msaa_texture_view,

            lines_pipeline,
            lines_vertex_buffer,
            lines_instance_buffer,
            lines_index_buffer,
            lines_count: 0,
            lines_offset: 0,

            tool_pipeline,
            tool_instance_buffer,

            stock_pipeline,
            stock_vertex_buffer,
            stock_instance_buffer,
            stock_index_buffer,
            stock_count: stock_tracker.total_count as u32,

            lines_tracker: LinesTracker::new(stock_tracker.voxel_edge),

            stock_tracker,

            uniforms,
            uniform_buffer,
            uniform_bind_group,

            configured: false,

            stock: STOCK,
            toolpath: TOOLPATH,
            tool: TOOL,

            speed: Speed::default(),
            tool_config: config.default_tool,

            skipped_frames: 0,

            window,
        })
    }

    /// Reconfigures [`Self::surface`], [`Self::depth_texture`] and [`Self::msaa_texture`],
    /// updates & rewrites [`Self::uniforms`] to use the new provided size.
    pub fn resize(&mut self, mut new_size: PhysicalSize<u32>) {
        new_size.width = new_size.width.max(1);
        new_size.height = new_size.height.max(1);

        self.surface_config.width = new_size.width;
        self.surface_config.height = new_size.height;
        self.surface.configure(&self.device, &self.surface_config);

        (self.depth_texture, self.depth_texture_view) = depth_texture(&self.device, new_size);

        (self.msaa_texture, self.msaa_texture_view) =
            msaa_texture(&self.device, new_size, self.surface_config.format);

        self.uniforms.resize(new_size);
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
        self.configured = true;
    }

    /// Uploads the next [`LineInstance`] from [`Self::lines_tracker`] to
    /// [`Self::lines_instance_buffer`], depending on the returned [`BufferAction`].
    ///
    /// - [`BufferAction::Overwrite`]:
    ///   Overwrites the new line instance over the last instance in the buffer, extending it.
    /// - [`BufferAction::Add`]: Appends the new line instance individually to the buffer.
    ///
    /// Also, depending on the `render` flags of [`BufferAction`],
    /// updates the position of [`ToolInstance`] in [`Self::tool_instance_buffer`]
    /// to the new line instance end point.
    /// Although a `force_render_tool` flag can be provided to make sure the [`ToolInstance`] is
    /// updated to the new position.
    ///
    /// On success returns a tuple with two `bool`s:
    /// - whether the simulation should proceed to next command.
    /// - whether a redraw is required.
    ///
    /// On failure, returns [`anyhow::Error`] if [`Self::lines_instance_buffer`] overflows on adding the new
    /// [`LineInstance`].
    pub fn update(&mut self, force_render_tool: bool) -> anyhow::Result<(bool, bool)> {
        let mut new_pos = None;

        // if None, signal has already been sent to retrieve a command from previous block exhaustion
        let (proceed, render) = match self.lines_tracker.next() {
            // update tool if we are going to request redraw
            Some(BufferAction::Overwrite { instance, render }) => {
                new_pos = Some(instance.end);
                self.overwrite_instance(instance);
                (false, render)
            }
            Some(BufferAction::Add { instance, render }) => {
                new_pos = Some(instance.end);
                self.add_instance(instance)?;
                (false, render)
            }
            None => (true, false),
        };

        // if new instance is available, update tool and stock
        // skip if not going to call graphics render
        if let Some(pos) = new_pos
            && (render || force_render_tool)
        {
            let pos = Point::from_array(pos);
            self.queue.write_buffer(
                &self.tool_instance_buffer,
                0,
                bytemuck::cast_slice(&[ToolInstance::at_point(pos)]),
            );

            // only reconsturct instances if there was a change
            if self.stock_tracker.cut(
                crate::config::ToolConfig {
                    number: 1,
                    diameter: 20.0,
                    length: 125.0,
                },
                pos,
            ) {
                let (index, instances) = self.stock_tracker.instances(); // is guarraunteed to be rendered
                //
                let offset = index * size_of::<StockInstance>();

                self.queue.write_buffer(
                    &self.stock_instance_buffer,
                    offset as u64,
                    bytemuck::cast_slice(instances),
                );
            }
        }

        if render {
            if self.skipped_frames >= self.speed.numeric() {
                self.skipped_frames = 0;
                Ok((proceed, true))
            } else {
                self.skipped_frames += 1;
                Ok((proceed, false))
            }
        } else {
            Ok((proceed, render))
        }
    }

    /// Overwrites the provided [`LineInstance`] over the last instance inside [`Self::lines_instance_buffer`].
    ///
    /// [`Self::lines_instance_buffer`] cannot overflow on this call, because there is no instance addition
    /// to the buffer.
    fn overwrite_instance(&mut self, instance: LineInstance) {
        self.queue.write_buffer(
            &self.lines_instance_buffer,
            self.lines_offset - bytemuck::cast_slice::<LineInstance, u8>(&[instance]).len() as u64,
            bytemuck::cast_slice(&[instance]),
        );
    }

    /// Appends the provided [`LineInstance`] to [`Self::lines_instance_buffer`].
    ///
    /// On failure, returns [`anyhow::Error`] if [`Self::lines_instance_buffer`] overflows on adding the new
    /// [`LineInstance`].
    fn add_instance(&mut self, instance: LineInstance) -> anyhow::Result<()> {
        self.queue.write_buffer(
            &self.lines_instance_buffer,
            self.lines_offset,
            bytemuck::cast_slice(&[instance]),
        );

        if self.lines_count + 1 > MAX_INSTANCES as u32 {
            anyhow::bail!("lines instance buffer overflow");
        }

        self.lines_offset += bytemuck::cast_slice::<LineInstance, u8>(&[instance]).len() as u64;
        self.lines_count += 1;

        Ok(())
    }

    /// Clears all the toolpath [`LineInstance`]s from [`Self::lines_instance_buffer`].
    pub fn clear(&mut self) {
        self.lines_count = 0;
        self.lines_offset = 0;
        self.lines_tracker.reset();
        self.stock_tracker.reset();

        // reupload the full stock
        self.queue.write_buffer(
            &self.stock_instance_buffer,
            0,
            bytemuck::cast_slice(self.stock_tracker.instances().1),
        );
    }

    /// Renders a new frame to the [`Self::surface`],
    /// drawing the toolpath, tool and stock,
    /// by rendering all [`Self::lines_instance_buffer`], [`Self::tool_instance_buffer`]
    /// and [`Self::stock_instance_buffer`] after checking for [`Self::toolpath`],
    /// [`Self::tool`] and [`Self::stock`] flags respectively.
    ///
    /// # Errors
    /// Returns [`anyhow::Error`] indicating that the surface is lost.
    pub fn render(&mut self) -> anyhow::Result<()> {
        if !self.configured {
            return Ok(());
        }

        // surface texture to render to
        let surface_texture = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                // texture out of date with respect to the surface, need reconfiguration
                // still got the texture though
                self.surface.configure(&self.device, &self.surface_config);
                surface_texture
            }
            CurrentSurfaceTexture::Timeout
            | CurrentSurfaceTexture::Occluded
            | CurrentSurfaceTexture::Validation => {
                // skip frame
                return Ok(());
            }
            CurrentSurfaceTexture::Outdated => {
                // texture out of date with respect to the surface, need reconfiguration
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost surface, could recreate the resources here")
            }
        };

        // texture cannot be used directly, therefore we need to create a view into it
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GSim"),
            });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                // view: &self.msaa_view,
                view: &surface_view,
                depth_slice: None,
                // resolve_target: Some(&surface_view),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.01,
                        g: 0.01,
                        b: 0.01,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        // stock
        if self.stock {
            render_pass.set_pipeline(&self.stock_pipeline);
            render_pass.set_vertex_buffer(0, self.stock_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.stock_instance_buffer.slice(..));
            render_pass
                .set_index_buffer(self.stock_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..36, 0, 0..self.stock_count);
        }

        // tool
        if self.tool {
            render_pass.set_pipeline(&self.tool_pipeline);
            render_pass.set_vertex_buffer(0, self.tool_instance_buffer.slice(..));
            render_pass.draw(0..432, 0..1);
        }

        // lines
        if self.toolpath {
            render_pass.set_pipeline(&self.lines_pipeline);
            render_pass.set_vertex_buffer(0, self.lines_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.lines_instance_buffer.slice(..));
            render_pass
                .set_index_buffer(self.lines_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..self.lines_count);
        }

        drop(render_pass);

        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();

        Ok(())
    }

    /// Sets the active [`View`] in [`Self::uniforms`] and uploads the updated uniforms to
    /// [`Self::uniform_buffer`].
    pub fn set_view(&mut self, view: View) {
        if self.uniforms.view() == view {
            return;
        } else {
            self.uniforms.set_view(view);
        }

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
    }

    pub fn set_tool(&mut self, tool_config: ToolConfig) {
        self.tool_config = tool_config
    }
}

// make sure width and height are at least 1
fn depth_texture(
    device: &wgpu::Device,
    size: PhysicalSize<u32>,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    (texture, view)
}

// make sure width and height are at least 1
fn msaa_texture(
    device: &wgpu::Device,
    size: PhysicalSize<u32>,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("MSAA Texture"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    (texture, view)
}
