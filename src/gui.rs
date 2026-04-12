use std::sync::{
    Arc,
    mpsc::{Receiver, Sender, TryRecvError},
};

use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::Signal;

#[derive(Debug)]
pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    window: Arc<Window>,
}

impl State {
    async fn build(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        println!("size: {size:?}");

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        // for web compatible_surface is explicitly required
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo, // cap display rate to display's frame rate
            alpha_mode: surface_caps.alpha_modes[0],
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;
        }
    }

    fn update(&mut self) {}

    fn render(&mut self) -> anyhow::Result<()> {
        // does not loop, just queues in a new redraw request
        self.window.request_redraw();

        // We can't render unless the surface is configured
        if !self.is_surface_configured {
            return Ok(());
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => {
                self.surface.configure(&self.device, &self.config);
                surface_texture
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                // Skip this frame
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                // You could recreate the devices and all resources
                // created with it here, but we'll just bail
                anyhow::bail!("Lost device");
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {}
        }
    }
}

pub struct Renderer {
    /// Receive rendering jobs from the [`Ratatui`](ratatui) thread.
    pub job: Receiver<Signal>,
    /// Proceed and receive another job from the [`Ratatui`](ratatui) thread or stop it.
    pub proceed: Sender<bool>,
    pub state: Option<State>,
    pub last_signal: Signal,
}

impl Renderer {
    pub fn new(job: Receiver<Signal>, proceed: Sender<bool>) -> Self {
        Self {
            job,
            proceed,
            state: None,
            last_signal: Signal::Start,
        }
    }
}

impl ApplicationHandler<()> for Renderer {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        eprintln!("resumed");
        let mut window_attributes = Window::default_attributes().with_title("GSim");

        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Could not create a new window."),
        );

        // block and run the futures
        self.state = Some(pollster::block_on(State::build(window)).unwrap());
        eprintln!("{:?}", self.state.as_ref().unwrap());
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };

        match self.job.try_recv() {
            Ok(job) => {
                let ret = match &job {
                    Signal::Start => todo!(),
                    Signal::Render(block) => {
                        eprintln!("{block:?}");
                        self.proceed.send(true).unwrap();
                        false
                    }
                    Signal::Stop(_) => true,
                };
                self.last_signal = job;
                if ret {
                    return event_loop.exit();
                }
            }
            Err(e) => match e {
                TryRecvError::Empty => (),
                TryRecvError::Disconnected => return event_loop.exit(),
            },
        };

        eprintln!("event: {event:?}");
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            // only deal with resize logic, redrawing event will be emitted later automatically
            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height);
            }
            WindowEvent::RedrawRequested => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(e) => {
                        // TODO Log the error and exit gracefully
                        eprintln!("{e}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed()),
            _ => {}
        }
    }
}

pub fn run_gui(job: Receiver<Signal>, proceed: Sender<bool>) -> anyhow::Result<()> {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut renderer = Renderer::new(job, proceed);
    eprintln!("event loop start");
    let res = event_loop.run_app(&mut renderer);

    // check if the tui thread is still running,
    // if so, tell it to stop
    match renderer.last_signal {
        // the tui thread signalled main thread to stop
        Signal::Stop(_) => (),
        // tui thread still running, stop it
        Signal::Start | Signal::Render { .. } => renderer.proceed.send(false).unwrap(),
    };

    res.map_err(|e| e.into())
}
