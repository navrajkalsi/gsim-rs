//! # Gui
//!
//! Creates a new [`Window`] and renders the simulation in it using the [`wgpu`] graphics API.
//!
//! The render loop receives render job [`Command`]s from the [`Tui`] thread,
//! and sends [`Signal`]s in response, to continue or terminate the [`Tui`] thread.

use crate::{Command, Signal, config::Config, renderer::Graphics};
use std::sync::{Arc, mpsc::Sender};
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

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
                            let _ = self.signal.send(Signal::Proceed);
                            // exhausted, request new render command
                            self.render_received = false;
                        } else {
                            graphics.window.request_redraw();
                            // still more instances in the tracker
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

            Command::SetToolVisibility(tool) => {
                graphics.tool = *tool;
                graphics.window.request_redraw();
            }

            Command::SetToolpathVisibility(toolpath) => {
                graphics.toolpath = *toolpath;
                graphics.window.request_redraw();
            }

            Command::SetStockVisibility(stock) => {
                graphics.stock = *stock;
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
