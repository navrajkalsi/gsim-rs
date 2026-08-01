//! # Gui
//!
//! Creates a new [`Window`] and renders the simulation in it using the [`wgpu`] graphics API.
//!
//! Drives G-code interpretation with each new frame draw.
//!
//! The render loop receives user input as [`Command`]s from the [`Tui`](crate::tui) thread,
//! and sends [`Signal`]s to the [`Tui`](crate::tui) thread to change the tui frontend,
//! reflecting new active state.

use crate::{
    Command, Interrupt, SINGLE, Signal,
    config::Config,
    interpreter::{Interpreter, InterpreterError},
    machine::MotionSummary,
    renderer::Graphics,
};
use std::sync::{Arc, Mutex};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

/// Current state of the [`Gui`](crate::gui), owned by the **main thread**.
pub struct Gui {
    /// [`Config`] for tool start position, stock dimensions and collection of tool configs
    /// available.
    config: Config,

    /// Active GPU graphics state. [`None`] before window creation.
    graphics: Option<Graphics>,

    /// Latest [`Command`] received from the [`Tui`](crate::tui).
    /// Required for checking tui thread status during termination.
    command: Option<Command>,

    /// Stores any errors that occur during [`Graphics::render`], [`Self::resumed`] or [`Self::execute`].
    ///
    /// Exit behaviour differs depending on where the error originated from:
    /// - [`Graphics::render`] and [`Self::resumed`] errors
    ///   cause the gui to exit immediately and close the window, without waiting on the tui.
    /// - [`Self::execute`] errors keep the window alive and wait on the tui to send
    ///   [`Command::Stop`] before exiting.
    ///
    /// This exit behaviour is implemented because it lets the user observe the toolpath
    /// in case the reason for error was the actual G-code and not the simulation.
    error: Option<anyhow::Error>,

    /// [`winit`] event loop that can receive user events in form of [`Command`]s.
    /// Consumed on [`Gui::run`] call.
    event_loop: Option<EventLoop<Command>>,

    /// An [`Arc`][`Mutex`] that can be altered to send [`Signal`]s to the [`Tui`](crate::tui).
    signal: Arc<Mutex<Signal>>,

    /// Parsed source loaded [`Interpreter`], ready for iteration.
    interpreter: Interpreter,

    /// [`None`] if the program is running.
    interrupt: Option<Interrupt>,

    /// Single step through code blocks.
    ///
    /// This is to be in sync with [`Tui::single`](crate::tui::Tui::single)
    /// and is used determine when to wait for [`Command::Next`] from the tui thread.
    single: bool,
}

impl Gui {
    /// Constructs a new [`Gui`],
    /// initializing the [`EventLoop`] ready to receive [`Command`]s and send [`Signal`]s.
    ///
    /// The event loop is configured to block and wait until a new (user or OS) event arrives.
    pub fn new(config: Config, signal: Arc<Mutex<Signal>>, interpreter: Interpreter) -> Self {
        let event_loop = EventLoop::<Command>::with_user_event()
            .build()
            .expect("constructing on the main thread");

        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

        Self {
            config,
            graphics: None,
            command: None,
            error: None,
            signal,
            interpreter,
            interrupt: Some(Interrupt::Start),
            event_loop: Some(event_loop),
            single: SINGLE,
        }
    }

    /// Returns an [`EventLoopProxy`] for sending [`Command`]s to the [`Gui`] from other threads.
    pub fn create_proxy(&self) -> EventLoopProxy<Command> {
        self.event_loop.as_ref().expect("run method will consume self, therefore eventloop will always be present if the user has a gui struct.").create_proxy()
    }

    /// Starts the [`Gui`] by running the [`EventLoop`].
    ///
    /// While exiting,
    /// checks if the [`Tui`](crate::tui) is still running using [`Self::command`],
    /// and sends [`Signal::Stop`] to signal a stop, else checks for any error in [`Command::Stop`].
    ///
    /// # Errors
    /// Returns any error in [`Self::command`] from the [`Tui`](crate::tui) or
    /// stored [`Self::error`], prioritizing the [`Tui`](crate::tui) error.
    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = self.event_loop.take().unwrap();
        let res = event_loop.run_app(&mut self);

        // prioritize tui thread error
        // check if the tui thread is still running, if so, tell it to stop
        match self.command {
            // the tui thread signalled main thread to stop because of an error on tui thread
            Some(Command::Stop(Some(e))) => self.error = Some(e),

            // quit event from the user
            Some(Command::Stop(None)) => (),

            // tui thread still running, stop it
            _ => self.send_signal(Signal::Stop),
        };

        if let Some(e) = self.error {
            Err(e)
        } else {
            res.map_err(|e| e.into())
        }
    }

    /// Obtains the lock for [`Self::signal`] and updates it to match the new provided `signal`.
    fn send_signal(&self, signal: Signal) {
        *self.signal.lock().unwrap() = signal;
    }

    /// Requests redraw for [`Self::graphics`] window.
    fn request_redraw(&mut self) {
        self.graphics.as_mut().unwrap().window.request_redraw()
    }

    /// Reloads internals of [`Gui`] to begin rendering again from the first block.
    ///
    /// Sends [`Signal::Pause`] with an [`Interrupt::Start`] to the [`Tui`](crate::tui),
    /// so that the tui can wait for user-event to begin the simulation again.
    ///
    /// Removes any drawn toolpaths and resets the stock.
    fn reload(&mut self) {
        self.interrupt = Some(Interrupt::Start);
        self.interpreter.reload();
        self.send_signal(Signal::Pause {
            interrupt: Interrupt::Start,
            machine: *self.interpreter.machine(),
            current: 0,
        });

        if let Some(graphics) = self.graphics.as_mut() {
            graphics.clear();
            graphics.window.request_redraw();
        }
    }

    /// Updates [`Self::graphics`] and [`Self::execute`]s the next block if the previous block was finished rendering.
    ///
    /// Returns `true` if the simulation now needs to be rendered and `false` to skip this frame.
    fn update(&mut self) -> anyhow::Result<bool> {
        debug_assert!(self.interrupt.is_none());

        let (proceed, render) = if let Some(graphics) = self.graphics.as_mut() {
            graphics.update(self.single)?
        } else {
            return Ok(false);
        };

        if proceed {
            // exhausted, execute new block and seed the line tracker

            if self.single {
                match self.command {
                    Some(Command::Next) | Some(Command::SetSingle(_)) => (), // only proceed if commanded next or set single
                    _ => return Ok(true), // render as the command may have changed some static objects
                }
            };

            match self.execute() {
                Ok(Some(motion)) => self.graphics.as_mut().unwrap().lines_tracker.add(motion),

                Ok(None) => (), // just try again on next redraw

                Err(error) => {
                    // since the line has already been executed,
                    // we have to provide the index of previous block, that caused this error
                    let current = self.interpreter.source().index().saturating_sub(1);

                    self.send_signal(Signal::Error {
                        error,
                        machine: *self.interpreter.machine(),
                        current,
                    });

                    return Err(error.into());
                }
            }
        }

        // redraw for newly added motion, draining previous motion, or execute new block if motion was None
        if !self.single || !proceed {
            // loops back to this func
            //
            // skips looping when proceed command is detected on single mode, as that must require
            // user input for single mode to work
            self.request_redraw();
        }

        // always render when lines have exhausted on single mode
        Ok(render || (self.single && proceed))
    }

    /// Retrieves [`MotionSummary`] from [`Interpreter::execute`],
    /// and sends the appropriate [`Signal`] to the [`Tui`](crate::tui).
    ///
    /// If no summary is found, on exhaustion of blocks,
    /// [`Interrupt::End`] is activated and also sent to the tui.
    fn execute(&mut self) -> Result<Option<MotionSummary>, InterpreterError> {
        debug_assert!(self.interrupt.is_none());

        let (motion, signal) = match self.interpreter.execute()? {
            (current, machine, Some(block)) if block.is_tool_change() => {
                let tool_config = self
                    .config
                    .tools
                    .iter()
                    .find(|tool| tool.number == machine.tool())
                    .unwrap_or(&self.config.default_tool);

                self.graphics
                    .as_mut()
                    .expect(
                        "should only be reached after an update request, which requires graphics",
                    )
                    .set_tool(*tool_config);

                (
                    block.motion,
                    Signal::Run {
                        summary: block.clone(),
                        machine,
                        current,
                    },
                )
            }

            (current, machine, Some(block)) if block.is_interrupt() => {
                self.interrupt = block.as_ref().into();

                (
                    block.motion,
                    Signal::Pause {
                        interrupt: self
                            .interrupt
                            .expect("it has been checked that this block causes an interrupt"),
                        machine,
                        current,
                    },
                )
            }

            (current, machine, Some(block)) => (
                block.motion,
                Signal::Run {
                    summary: block.clone(),
                    machine,
                    current,
                },
            ),

            (current, machine, None) => {
                // exhausted
                self.interrupt = Some(Interrupt::End);
                (
                    None,
                    Signal::Pause {
                        interrupt: Interrupt::End,
                        machine,
                        current,
                    },
                )
            }
        };

        self.send_signal(signal);

        Ok(motion)
    }
}

impl ApplicationHandler<Command> for Gui {
    /// On the first call,
    /// creates [`Window`] and builds [`Graphics`] by blocking till completion.
    ///
    /// Ignores any subsequent invocations.
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
    /// On receiving [`WindowEvent::RedrawRequested`], if no [`Self::error`] and [`Self::interrupt`]
    /// are detected, [`updates`](Self::update) the simulation by either: executing a new G-code block
    /// or continuing to render a block already in process. During this,
    /// appropriate [`Signal`]s are sent to the [`Tui`](crate::tui), in case any user input is required.
    ///
    /// Calls [`Graphics::render`] if a new frame is to be drawn.
    /// On failure to `render` stores the error and exits the event loop.
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.error.is_some() {
            return;
        }

        // after this match the frame is rendered every time
        // return when frame is not to be rendered
        match event {
            WindowEvent::Resized(size) => match self.graphics.as_mut() {
                Some(graphics) => graphics.resize(size),
                None => return,
            },

            WindowEvent::CloseRequested | WindowEvent::Destroyed => return event_loop.exit(),

            WindowEvent::RedrawRequested if self.interrupt.is_none() => {
                match self.update() {
                    Ok(true) => (), // render

                    Ok(false) => return,

                    Err(e) => self.error = Some(e), // do not exit on interpreter error,
                                                    // wait on the tui
                };
            }

            WindowEvent::RedrawRequested => (), // just render

            _ => return,
        };

        if let Err(e) = self.graphics.as_mut().unwrap().render() {
            self.error = Some(e); // exit on render error
            event_loop.exit()
        }
    }

    /// Handles [`Command`]s sent from the [`Tui`](crate::tui) thread.
    ///
    /// Each command alters [`Graphics`] state and draws a new frame or,
    /// in case of [`Command::Stop`], exits the loop.
    /// Latest command is always stored at the end.
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Command) {
        let graphics = self.graphics.as_mut().expect("app has been started");

        match &event {
            Command::SetView(view) => graphics.set_view(*view),

            Command::SetSingle(single) => self.single = *single,

            Command::SetToolVisibility(tool) => graphics.tool = *tool,

            Command::SetToolpathVisibility(toolpath) => graphics.toolpath = *toolpath,

            Command::SetStockVisibility(stock) => graphics.stock = *stock,

            Command::SetSpeed(speed) => graphics.speed = *speed,

            Command::ClearInterrupt => match self.interrupt {
                Some(Interrupt::End) => self.reload(),
                _ => self.interrupt = None,
            },

            Command::Next => (), // just redraw

            Command::Stop(_) => event_loop.exit(),
        }

        self.command = Some(event);

        self.request_redraw();
    }
}
