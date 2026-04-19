use std::sync::{Arc, mpsc::Sender};

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle},
    window::{Icon, Window, WindowId},
};

use wgpu::CurrentSurfaceTexture;

use crate::{Command, Signal, interpreter::BlockSummary, parser::Point};

struct Graphics {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    // needs to be arc to make sure surface gets static lifetime
    window: Arc<Window>,
    configured: bool,
}

impl Graphics {
    async fn build(handle: OwnedDisplayHandle, window: Arc<Window>) -> anyhow::Result<Self> {
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
                label: Some("GSim"),
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
            .unwrap_or(surface_caps.formats.get(0).expect("At least one format must be present, as the adapter is created to be compatible with the surface").clone());

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2, // reasonable default in docs
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        Ok(Self {
            device,
            queue,
            surface,
            config,
            window,
            configured: false,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.configured = true
        }
    }

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
                self.surface.configure(&self.device, &self.config);
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
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost surface, could recreate the resources here")
            }
        };

        // texture cannot be used directly, therefore we need to create a view into it
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GSim"),
            });

        let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GSim"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
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
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        drop(render_pass);

        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();

        Ok(())
    }
}

struct Gui {
    max_travels: Option<Point>,
    signal: Sender<Signal>,
    last_command: Option<Command>,
    graphics: Option<Graphics>,
    // for passing render errors out of the loop
    error: Option<anyhow::Error>,
}

impl Gui {
    fn new(signal: Sender<Signal>) -> Self {
        Self {
            signal,
            max_travels: None,
            last_command: None,
            graphics: None,
            error: None,
        }
    }
}

impl ApplicationHandler<Command> for Gui {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // repeat resumed call
        if self.graphics.is_some() {
            return;
        }

        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_active(false)
                    .with_decorations(false)
                    .with_visible(true)
                    .with_window_icon(Some(
                        Icon::from_rgba(vec![0, 0, 0, 0], 1, 1)
                            .expect("Could not create icon for window"),
                    ))
                    .with_title("GSim"),
            )
            .expect("Could not create a new window");

        self.graphics = Some(
            pollster::block_on(Graphics::build(
                event_loop.owned_display_handle(),
                Arc::new(window),
            ))
            .expect("Could not initialize GPU resources"),
        );
    }

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

        match event {
            WindowEvent::Resized(size) => graphics.resize(size.width, size.height),
            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::RedrawRequested => match graphics.render() {
                Ok(_) => (),
                Err(e) => {
                    self.error = Some(e);
                    event_loop.exit();
                }
            },
            _ => (),
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Command) {
        // tui thread sends Start after initializing once, and then Render commands
        match &event {
            Command::Start(point) => self.max_travels = Some(*point),
            Command::Render(block) => {
                let graphics = self.graphics.as_mut().expect("App has been started");
                graphics.window.request_redraw();
            }

            Command::Stop(_) => event_loop.exit(),
        }

        self.last_command = Some(event);
    }
}

pub fn init_gui() -> (EventLoop<Command>, EventLoopProxy<Command>) {
    let event_loop = EventLoop::<Command>::with_user_event().build().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    let proxy = event_loop.create_proxy();

    (event_loop, proxy)
}

pub fn run_gui(event_loop: EventLoop<Command>, signal: Sender<Signal>) -> anyhow::Result<()> {
    let mut gui = Gui::new(signal);
    let res = event_loop.run_app(&mut gui);

    // check if the tui thread is still running, if so, tell it to stop
    match gui.last_command {
        // the tui thread signalled main thread to stop
        Some(Command::Stop(_)) => (),
        // tui thread still running, stop it
        _ => gui.signal.send(Signal::Stop).unwrap(),
    };

    if let Some(e) = gui.error {
        Err(e)
    } else {
        res.map_err(|e| e.into())
    }
}
