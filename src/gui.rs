//! # Gui
//!
//! Creates a new [`Window`] and renders the simulation in it using the [`wgpu`] graphics API.
//!
//! The render loop receives render job [`Command`]s from the [`Tui`] thread,
//! and sends [`Signal`]s in response, to continue or terminate the [`Tui`] thread.

use crate::{
    Command, Signal, View,
    config::{Config, Point},
    geometry::{
        line::{BufferAction, LineInstance, LineInstancesTracker},
        stock::{Stock, StockInstance},
        tools::ToolInstance,
        uniforms::Uniforms,
    },
};
use std::{
    mem::size_of,
    sync::{Arc, mpsc::Sender},
};
use wgpu::{BindGroupLayoutEntry, CurrentSurfaceTexture, util::DeviceExt};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle},
    window::{Window, WindowId},
};

const MAX_TRAVELS: Point = Point {
    x: 500.0,
    y: 250.0,
    z: 250.0,
};

/// Maximum number of [`LineInstance`]s allowed to be used in the [`Graphics::lines_buffer`].
const MAX_INSTANCES: u64 = 1_000_000;

/// Represents the current state of the [`Gui`](crate::gui), owned by the **main thread**.
pub struct Gui {
    /// Sender half of the channel for [`Signal`] to [`Tui`].
    signal: Sender<Signal>,
    /// [`Config`] for tool start position and stock dimensions.
    config: Config,
    /// Currently processing [`Command`] received from [`Tui`].
    current_command: Option<Command>,
    /// Active GPU graphics state. [`None`] before window creation.
    graphics: Option<Graphics>,
    /// Stores any errors that occur during [`Graphics::render`] call.
    error: Option<anyhow::Error>,
    /// [`winit`] event loop that can receive user events in form of [`Command`]s.
    /// Consumed on [`Gui::run`] call.
    event_loop: Option<EventLoop<Command>>,
    /// Flag to make sure that the first redraw request is always fulfilled.
    first: bool,
    /// Flag to check if the [`Gui`] already sent a [`Signal::Proceed`] to the [`Tui`],
    /// and received a corresponding [`Command::Render`].
    render_received: bool,
    /// Single step through code blocks.
    /// This is to be in sync with [`Tui::single`] and is used to bypass [`LineInstancesTracker`]
    /// render check. While this is `true`, each [`Graphics::update`] call will be followed by a
    /// [`Graphics::render`] call, irrespective of the return value of [`Graphics::update`].
    single: bool,
}

impl Gui {
    /// Constructs a new [`Gui`],
    /// initializing the [`EventLoop`] ready to receive [`Command`]s and send [`Signal`]s.
    ///
    /// The event loop is configured to block and wait until a new (user or OS) event arrives.
    ///
    /// # Errors
    /// Returns [`EventLoopError`] on failure to build the event loop.
    pub fn build(signal: Sender<Signal>, config: Config) -> Result<Self, EventLoopError> {
        let event_loop = EventLoop::<Command>::with_user_event().build()?;
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

        Ok(Self {
            signal,
            config,
            current_command: None,
            graphics: None,
            error: None,
            event_loop: Some(event_loop),
            first: true,
            render_received: false,
            single: false,
        })
    }

    /// Returns an [`EventLoopProxy`] for sending [`Command`]s to the [`Gui`] from other threads.
    pub fn create_proxy(&self) -> EventLoopProxy<Command> {
        self.event_loop.as_ref().expect("Run method will consume self, therefore eventloop will always be present if the user has a Gui struct.").create_proxy()
    }

    /// Starts the [`Gui`] by running the [`EventLoop`].
    ///
    /// While exiting, checks if the [`Tui`] is still running, using [`Gui::current_command`],
    /// and sends [`Signal::Stop`] to signal a stop, else checks for any error in [`Command::Stop`].
    ///
    /// # Errors
    /// Returns any error in [`Command::Stop`] from [`Tui`] or [`EventLoopError`],
    /// prioritizing [`Tui`] error.
    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = self.event_loop.take().unwrap();
        let res = event_loop.run_app(&mut self);

        // prioritize tui thread error
        // check if the tui thread is still running, if so, tell it to stop
        match self.current_command {
            // the tui thread signalled main thread to stop because of an error in tui thread
            Some(Command::Stop(Some(e))) => self.error = Some(e),
            Some(Command::Stop(None)) => (),
            // tui thread still running, stop it
            _ => self.signal.send(Signal::Stop).unwrap(),
        };

        if let Some(e) = self.error {
            Err(e)
        } else {
            res.map_err(|e| e.into())
        }
    }
}

impl ApplicationHandler<Command> for Gui {
    /// On the first call,
    /// creates [`Window`] and builds [`Graphics`] by blocking till completion.
    ///
    /// On failure to create either of the two,
    /// stores the error in [`Gui::error`] and [`exit`](ActiveEventLoop::exit)s the event loop.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return; // repeat resumed call
        }

        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_active(false)
                .with_decorations(false)
                .with_visible(true)
                .with_title("GSim"),
        ) {
            Ok(w) => w,
            Err(e) => {
                self.error = Some(e.into());
                return event_loop.exit();
            }
        };

        let graphics = match pollster::block_on(Graphics::build(
            event_loop.owned_display_handle(),
            Arc::new(window),
            self.config.clone(),
        )) {
            Ok(g) => g,
            Err(e) => {
                self.error = Some(e);
                return event_loop.exit();
            }
        };

        self.graphics = Some(graphics);
    }

    /// Handles [`WindowEvent`]s sent by the OS.
    ///
    /// Ignores any event if [`Graphics`] has not yet been initialized.
    ///
    /// On receiving [`WindowEvent::RedrawRequested`], updates simulation state, and:
    /// - Sends [`Signal::Proceed`] to [`Tui`],
    ///   if this redraw completely fulfils the last received [`Command::Render`].
    /// - Requests another redraw to fulfil the last received [`Command::Render`].
    ///
    /// If [`Graphics::render`] fails, stores the error and exits the event loop.
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let graphics = match self.graphics.as_mut() {
            Some(g) => g,
            None => return,
        };

        // after this match the frame is rendered every time
        // return when frame is not to be rendered
        match event {
            WindowEvent::Resized(size) => return graphics.resize(size),

            WindowEvent::CloseRequested | WindowEvent::Destroyed => return event_loop.exit(),

            WindowEvent::RedrawRequested if self.first => {
                self.first = false;
                debug_assert!(!self.render_received);
                // this may fail if the program is processing blocks quickly
                // and the tui receives quit signal from user and exits the loop,
                // the gui will then exit on next command execution.
                let _ = self.signal.send(Signal::Proceed);
            }

            WindowEvent::RedrawRequested if self.render_received => {
                // render command received, update graphcis state
                match graphics.update(self.single) {
                    Ok((proceed, render)) => {
                        if proceed {
                            let _ = self.signal.send(Signal::Proceed); // exhausted, request new render command
                            self.render_received = false;
                        } else {
                            graphics.window.request_redraw(); // still more instances in the tracker
                        };

                        if !render && !self.single {
                            return;
                        }
                    }
                    Err(e) => {
                        self.error = Some(e); // buffer overflow
                        return event_loop.exit();
                    }
                };
            }

            // this was not triggered by a Command::Render
            // do not update the graphics state, just render
            WindowEvent::RedrawRequested => (),

            _ => return,
        };

        if let Err(e) = graphics.render() {
            self.error = Some(e); // render error
            event_loop.exit()
        }
    }

    /// Handles [`Command`]s sent from the [`Tui`] thread.
    ///
    /// Each command alters [`Graphics`] state or exits the loop, for [`Command::Stop`].
    /// Latest command is always stored at the end.
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Command) {
        let graphics = self.graphics.as_mut().expect("App has been started");

        match &event {
            Command::Render(summary) => {
                debug_assert!(!self.render_received);
                self.render_received = true;

                graphics.lines_tracker.add(*summary);
                graphics.window.request_redraw();
            }

            Command::SetView(view) => {
                graphics.set_view(*view);
                graphics.window.request_redraw();
            }

            Command::SetSingle(single) => {
                self.single = *single;
            }

            Command::SetTool(tool) => {
                graphics.set_tool(*tool);
                graphics.window.request_redraw();
            }

            Command::Clear => {
                graphics.clear();
                graphics.window.request_redraw();
            }

            Command::Stop(_) => {
                event_loop.exit();
            }
        }

        self.current_command = Some(event);
    }
}

/// GPU state for toothpath simulation.
pub struct Graphics {
    /// Logical connection to a GPU.
    device: wgpu::Device,
    /// Command queue for the `device`.
    queue: wgpu::Queue,
    /// Rendering surface created from a [`Window`].
    /// Since the surface holds a reference to the [`Window`] it was created from,
    /// the window is kept alive as long as the surface.
    surface: wgpu::Surface<'static>,
    /// Depth texture configured to [`Self::surface`] size.
    depth_texture: wgpu::Texture,
    /// View for [`Self::depth_texture`] to be used in the render pass.
    depth_view: wgpu::TextureView,

    msaa_texture: wgpu::Texture,
    msaa_view: wgpu::TextureView,

    /// Description of a [`Surface`](wgpu::Surface).
    surface_config: wgpu::SurfaceConfiguration,

    /// Pipeline for rendering [`LineInstance`].
    lines_pipeline: wgpu::RenderPipeline,

    /// Vertex buffer configured to hold [`MAX_INSTANCES`] number of [`LineInstance`]s.
    lines_vertex_buffer: wgpu::Buffer,
    lines_instance_buffer: wgpu::Buffer,
    lines_index_buffer: wgpu::Buffer,

    /// Total number of [`LineInstance`]s in [`Self::lines_buffer`].
    lines_count: u32,
    /// Memory offset to write next toolpath [`LineInstance`] to.
    lines_offset: u64,

    /// Pipeline for rendering the [`ToolInstance`].
    tool_pipeline: wgpu::RenderPipeline,
    /// Vertex buffer configured to hold a single [`ToolInstance`].
    tool_buffer: wgpu::Buffer,

    stock_pipeline: wgpu::RenderPipeline,
    stock_vertex_buffer: wgpu::Buffer,
    stock_instance_buffer: wgpu::Buffer,
    stock_index_buffer: wgpu::Buffer,
    stock_count: u32,

    /// Tracks total [`LineInstance`]s drawn and left to be drawn to
    /// fulfil the latest [`Command::Render`] from [`Tui`].
    lines_tracker: LineInstancesTracker,

    stock: Stock,

    /// Constant data shared across all the [`LineInstance`]s and [`ToolInstance`].
    uniforms: Uniforms,
    /// Read-only buffer containing [`Uniforms`].
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    /// Surface is configured on the first [`Graphics::resize`] call.
    configured: bool,
    /// [`Arc`] keeps the [`Window`] valid for as long as [`Self::surface`] needs,
    /// and lets us use `'static` lifetime with the surface.
    window: Arc<Window>,
}

impl Graphics {
    /// Constructs a new [`Graphics`] by initializing all GPU resources, including:
    /// - [`Uniforms`] buffer and bind group, to pass constant data to the [`ToolInstance`] and all
    ///   [`LineInstance`]s.
    /// - [`LineInstance`] buffer and pipeline.
    /// - [`ToolInstance`] buffer and pipeline. Creates a [`ToolInstance`],
    ///   with the tool at [`Config::start_pos`], and writes it to [`Self::tool_buffer`].
    ///
    /// Returns [`Error`](anyhow::Error) on failure to create any of the GPU resources.
    async fn build(
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
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2, // reasonable default in docs
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: surface_config.width.max(1),
                height: surface_config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let msaa_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA Texture"),
            size: wgpu::Extent3d {
                width: surface_config.width.max(1),
                height: surface_config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let msaa_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // ######## Uniforms ########
        //
        // static data to be passed to the shader, that is common to vertices
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

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniforms Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // ######## Line Vertex ########
        //
        // mini program that runs on the gpu
        let shader = device.create_shader_module(wgpu::include_wgsl!("line.wgsl"));

        let lines_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Lines Pipeline Layout"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });

        let lines_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lines Pipeline"),
            layout: Some(&lines_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    // @location of buffers is decided here
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
                depth_compare: Some(wgpu::CompareFunction::Less),
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
                    format: surface_config.format,
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
        let lines_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lines Vertex Buffer"),
            contents: bytemuck::cast_slice(&LineInstance::vertices()),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let lines_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lines Instance Buffer"),
            size: MAX_INSTANCES * size_of::<LineInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lines_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lines Index Buffer"),
            contents: bytemuck::cast_slice(&LineInstance::indices()),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ######## Tool Vertex ########
        //
        // do not present tool yet
        let shader = device.create_shader_module(wgpu::include_wgsl!("tool.wgsl"));

        let tool_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Tool"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let tool_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tool"),
            layout: Some(&tool_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[ToolInstance::buffer_layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
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
                count: 4,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let tool_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tool"),
            size: size_of::<ToolInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let tool = ToolInstance::at_point(config.start_pos);
        queue.write_buffer(&tool_buffer, 0, bytemuck::cast_slice(&[tool]));

        // ######## Stock Vertex ########
        //
        // mini program that runs on the gpu
        let shader = device.create_shader_module(wgpu::include_wgsl!("stock.wgsl"));

        let stock_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Stock Pipeline Layout"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });

        let stock_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Stock Pipeline"),
            layout: Some(&stock_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[
                    // @location of buffers is decided here
                    StockInstance::vertex_buffer_layout(),
                    StockInstance::instance_buffer_layout(),
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Front),
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
                count: 4,
                mask: !0, // use all
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let stock = Stock::new(config.stock); // create new stock from config stock body

        // this vertex buffer is constant and can be mapped at creation
        let stock_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Stock Vertex Buffer"),
            contents: bytemuck::cast_slice(&StockInstance::vertices(&stock)),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let stock_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Stock Instance Buffer"),
            size: stock.total_count as u64 * size_of::<StockInstance>() as u64, // will only need at max
            // full stock instances
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let stock_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Stock Index Buffer"),
            contents: bytemuck::cast_slice(&StockInstance::indices()),
            usage: wgpu::BufferUsages::INDEX,
        });

        queue.write_buffer(
            &stock_instance_buffer,
            0,
            bytemuck::cast_slice(stock.instances()),
        );
        queue.submit([]);

        Ok(Self {
            device,
            queue,
            surface,
            depth_texture,
            depth_view,

            msaa_texture,
            msaa_view,

            surface_config,
            lines_pipeline,

            lines_vertex_buffer,
            lines_instance_buffer,
            lines_index_buffer,

            lines_count: 0,
            lines_offset: 0,
            tool_pipeline,
            tool_buffer,

            stock_pipeline,
            stock_vertex_buffer,
            stock_instance_buffer,
            stock_index_buffer,
            stock_count: stock.total_count as u32,

            lines_tracker: LineInstancesTracker::new(),

            stock,

            uniforms,
            uniform_buffer,
            uniform_bind_group,
            configured: false,
            window,
        })
    }

    /// Reconfigures [`Self::surface`] and [`Self::depth_texture`], updates & rewrites [`Self::uniforms`] to use the new provided size.
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        let width = new_size.width.max(1);
        let height = new_size.height.max(1);

        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            self.configured = true;

            self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Depth Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            self.depth_view = self
                .depth_texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            self.msaa_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("MSAA Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });

            self.msaa_view = self
                .msaa_texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            self.uniforms.resize(new_size);
            self.queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&[self.uniforms]),
            );
        }
    }

    /// Uploads the next [`LineInstance`] from [`Self::lines_tracker`] to
    /// [`Self::lines_buffer`], depending on the returned [`BufferAction`].
    ///
    /// - [`BufferAction::Overwrite`]:
    ///   Overwrites the new line instance over the last instance in the buffer, extending it.
    /// - [`BufferAction::Add`]: Appends the new line instance individually to the buffer.
    ///
    /// Also, depending on the `render` flags of [`BufferAction`],
    /// updates the position of [`ToolInstance`] in [`Self::tool_buffer`]
    /// to the new line instance end point.
    /// Although a `force_render_tool` flag can be provided to make sure the [`ToolInstance`] is
    /// updated to the new position.
    ///
    /// On success returns a tuple with two `bool`s:
    /// - First `bool` is set to `true` on [`BufferAction::Exhausted`], indicating [`Gui`] to send
    ///   [`Signal::Proceed`] to the [`Tui`] and receive a new [`Command`].
    /// - Second `bool` is used to indicate [`Gui`] to call [`Graphics::render`].
    ///
    /// On failure, returns [`anyhow::Error`] if [`Self::lines_buffer`] overflows on adding the new
    /// [`LineInstance`].
    fn update(&mut self, force_render_tool: bool) -> anyhow::Result<(bool, bool)> {
        let mut new_pos = None;

        // if None, signal has already been sent to retrieve a command from previous block exhaustion
        let (proceed, render) = match self.lines_tracker.next() {
            // update tool if we are going to request redraw
            BufferAction::Overwrite { instance, render } => {
                new_pos = Some(instance.end);
                self.overwrite_instance(instance);
                (false, render)
            }
            BufferAction::Add { instance, render } => {
                new_pos = Some(instance.end);
                self.add_instance(instance)?;
                (false, render)
            }
            BufferAction::Exhausted => (true, false),
        };

        // if new instance is available, update tool and stock
        // skip if not going to call graphics render
        if let Some(pos) = new_pos {
            if render || force_render_tool {
                let pos = Point::from_array(pos);
                self.queue.write_buffer(
                    &self.tool_buffer,
                    0,
                    bytemuck::cast_slice(&[ToolInstance::at_point(pos)]),
                );

                // only reconsturct instances if there was a change
                if self.stock.cut(
                    crate::config::ToolConfig {
                        number: 1,
                        diameter: 25.0,
                        length: 125.0,
                    },
                    pos,
                ) {
                    let stock = self.stock.instances();
                    // is guarraunteed to be rendered
                    self.queue.write_buffer(
                        &self.stock_instance_buffer,
                        0,
                        bytemuck::cast_slice(stock),
                    );
                }
            }
        }

        Ok((proceed, render))
    }

    /// Overwrites the provided [`LineInstance`] over the last instance inside [`Self::lines_buffer`].
    ///
    /// [`Self::lines_buffer`] cannot overflow on this call, because there is no instance addition
    /// to the buffer.
    fn overwrite_instance(&mut self, instance: LineInstance) {
        self.queue.write_buffer(
            &self.lines_instance_buffer,
            self.lines_offset - bytemuck::cast_slice::<LineInstance, u8>(&[instance]).len() as u64,
            bytemuck::cast_slice(&[instance]),
        );
    }

    /// Appends the provided [`LineInstance`] to [`Self::lines_buffer`].
    ///
    /// On failure, returns [`anyhow::Error`] if [`Self::lines_buffer`] overflows on adding the new
    /// [`LineInstance`].
    fn add_instance(&mut self, instance: LineInstance) -> anyhow::Result<()> {
        self.queue.write_buffer(
            &self.lines_instance_buffer,
            self.lines_offset,
            bytemuck::cast_slice(&[instance]),
        );

        if self.lines_count + 1 > MAX_INSTANCES as u32 {
            anyhow::bail!("Lines vertex buffer overflow.");
        }

        self.lines_offset += bytemuck::cast_slice::<LineInstance, u8>(&[instance]).len() as u64;
        self.lines_count += 1;

        Ok(())
    }

    /// Clears all the toolpath [`LineInstance`]s from [`Self::lines_buffer`].
    fn clear(&mut self) {
        self.lines_count = 0;
        self.lines_offset = 0;
        self.lines_tracker.reset();
    }

    /// Renders a new frame to the [`Self::surface`], drawing the toolpath and tool
    /// by rendering both [`Self::lines_buffer`] and [`Self::tool_buffer`].
    ///
    /// # Errors
    /// Returns [`anyhow::Error`] indicating that the surface is lost.
    fn render(&mut self) -> anyhow::Result<()> {
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
                view: &self.msaa_view,
                depth_slice: None,
                resolve_target: Some(&surface_view),
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
                view: &self.depth_view,
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
        render_pass.set_pipeline(&self.stock_pipeline);
        render_pass.set_vertex_buffer(0, self.stock_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.stock_instance_buffer.slice(..));
        render_pass.set_index_buffer(self.stock_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..36, 0, 0..self.stock_count);

        // lines
        render_pass.set_pipeline(&self.lines_pipeline);
        render_pass.set_vertex_buffer(0, self.lines_vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.lines_instance_buffer.slice(..));
        render_pass.set_index_buffer(self.lines_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..6, 0, 0..self.lines_count);

        // tool
        render_pass.set_pipeline(&self.tool_pipeline);
        render_pass.set_vertex_buffer(0, self.tool_buffer.slice(..));
        render_pass.draw(0..432, 0..1);

        drop(render_pass);

        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();

        Ok(())
    }

    /// Sets the active [`View`] in [`Self::uniforms`] and uploads the updated uniforms to
    /// [`Self::uniform_buffer`].
    fn set_view(&mut self, view: View) {
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

    /// Sets the tool visibility in [`Self::uniforms`] and uploads the updated uniforms to
    /// [`Self::uniform_buffer`].
    fn set_tool(&mut self, tool: bool) {
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
    }
}
