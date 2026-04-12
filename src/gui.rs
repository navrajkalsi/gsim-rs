use std::sync::mpsc::{Receiver, Sender};

use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{Proceed, Signal, error::GSimError};

pub struct Renderer {
    /// Receive rendering jobs from the [`Ratatui`](ratatui) thread.
    pub job: Receiver<Signal>,
    /// Proceed and receive another job from the [`Ratatui`](ratatui) thread.
    pub proceed: Sender<Proceed>,
}

impl Renderer {
    pub fn new(job: Receiver<Signal>, proceed: Sender<Proceed>) -> Self {
        Self { job, proceed }
    }
}

impl ApplicationHandler<()> for Renderer {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        eprintln!("resumed");
        let mut window_attributes = Window::default_attributes();

        let window = event_loop.create_window(window_attributes).unwrap();
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => eprintln!("resize {size:?}"),
            WindowEvent::RedrawRequested => eprintln!("redraw"),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => match (code, state.is_pressed()) {
                (KeyCode::Escape, true) => event_loop.exit(),
                _ => {}
            },
            _ => {}
        }
    }
}

pub fn run_gui(job: Receiver<Signal>, proceed: Sender<Proceed>) -> Result<(), GSimError> {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut renderer = Renderer::new(job, proceed);
    eprintln!("hi");
    event_loop.run_app(&mut renderer).map_err(|err| err.into())
}
