//! # Gui
//!
//! Creates a new [`Window`] and renders the simulation in it using the [`wgpu`] graphics API.
//!
//! Drives G-code interpretation with each new frame draw.
//! Handles **both** mouse and keyboard input from the user.
//!
//! Sends [`Signal`]s to the [`Tui`](crate::tui) thread to change the tui frontend,
//! reflecting the new active state.

use crate::{
    config::Config,
    defaults,
    geometry::view::View,
    interpreter::{Interpreter, InterpreterError, Interrupt},
    machine::MotionSummary,
    renderer::Graphics,
    signal::Signal,
    speed::Speed,
};
use std::{
    ops::Neg,
    sync::{Arc, Mutex},
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

/// Current state of the [`Gui`](crate::gui), owned by the **main thread**.
pub struct Gui {
    /// [`Config`] for tool start position, stock dimensions and collection of tool configs
    /// available.
    config: Config,

    /// Active GPU graphics state. [`None`] before window creation.
    graphics: Option<Graphics>,

    /// Stores any errors that occur during [`Graphics::render`], [`Self::resumed`] or [`Self::execute`].
    ///
    /// Exit behaviour differs depending on where the error originated from:
    /// - [`Graphics::render`] and [`Self::resumed`] errors
    ///   cause the gui to exit immediately and close the window, without waiting on the tui.
    /// - [`Self::execute`] errors keep the window alive and wait on the tui to send
    ///   an event before exiting.
    ///
    /// This exit behaviour is implemented because it lets the user observe the toolpath
    /// in case the reason for error was the actual G-code and not the simulation.
    error: Option<anyhow::Error>,

    /// [`winit`] event loop for receiving exit event from the [`Tui`](crate::tui).
    /// Consumed on [`Gui::run`] call.
    event_loop: Option<EventLoop<()>>,

    /// An [`Arc`][`Mutex`] that can be altered to send [`Signal`]s to the [`Tui`](crate::tui).
    signal: Arc<Mutex<Signal>>,

    /// Parsed source loaded [`Interpreter`], ready for iteration.
    interpreter: Interpreter,

    /// [`None`] if the program is running.
    interrupt: Option<Interrupt>,

    /// Single step through code blocks.
    ///
    /// This is to be in sync with [`Tui::single`](crate::tui::Tui::single)
    /// and is used to determine when to wait for user input for executing next block(s).
    single: bool,

    /// Checked while [`Self::single`] is `true` and means that a new block must be executed
    /// and drawn completely. The loop then waits for next user input.
    ///
    /// Reset to `false` when a new block is executed and simulated,
    /// or when [`Self::single`] is reset to `false`.
    next_requested: bool,

    /// Simulation speed.
    speed: Speed,

    /// Current selected [`View`].
    /// [`None`] if the user has interacted with simulation window with their mouse.
    view: Option<View>,

    /// Set to `true` when [`Self::view`] is **centered** and **zoomed-in** the most without overflow.
    fit: bool,

    /// Flag to track mouse events received while **left-mouse-button** is pressed.
    /// This is set and unset on receiving an appropriate [`WindowEvent::MouseInput`] event.
    ///
    /// While this is set to `true` any mouse movement is recorded and used for **panning**.
    left_mouse_pressed: bool,

    /// Flag to track mouse events received while **right-mouse-button** is pressed.
    /// This is set and unset on receiving an appropriate [`WindowEvent::MouseInput`] event.
    ///
    /// While this is set to `true` any mouse movement is recorded and used for
    /// **rotation around Z axis** of the window.
    right_mouse_pressed: bool,

    /// Flag to track mouse events received while **middle-mouse-button** is pressed.
    /// This is set and unset on receiving an appropriate [`WindowEvent::MouseInput`] event.
    ///
    /// While this is set to `true` any mouse movement is recorded and used for
    /// **rotation around X and Y axes** of the window.
    middle_mouse_pressed: bool,
}

impl Gui {
    /// Constructs a new [`Gui`],
    /// initializing the [`EventLoop`] ready to begin G-code execution and send [`Signal`]s.
    ///
    /// The event loop is configured to block and wait until a new (user or OS) event arrives.
    pub fn new(config: Config, signal: Arc<Mutex<Signal>>, interpreter: Interpreter) -> Self {
        let event_loop = EventLoop::builder()
            .build()
            .expect("constructing on the main thread");

        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

        Self {
            config,
            graphics: None,
            error: None,
            signal,
            interpreter,
            interrupt: Some(Interrupt::Start),
            event_loop: Some(event_loop),
            single: defaults::SINGLE,
            next_requested: false,
            speed: Speed::default(),
            view: Some(View::default()),
            fit: true,
            left_mouse_pressed: false,
            right_mouse_pressed: false,
            middle_mouse_pressed: false,
        }
    }

    /// Returns an [`EventLoopProxy`] for sending events to the [`Gui`] from other threads.
    ///
    /// Any event received from this is assumed to be an **exit event**.
    pub fn create_proxy(&self) -> EventLoopProxy<()> {
        self.event_loop.as_ref().expect("run method will consume self, therefore eventloop will always be present if the user has a gui struct.").create_proxy()
    }

    /// Starts the [`Gui`] by running the [`EventLoop`].
    ///
    /// Always sends a [`Signal::Stop`] to the [`Tui`](crate::tui) while exiting.
    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = self.event_loop.take().unwrap();
        let res = event_loop.run_app(&mut self);

        self.send_signal(Signal::Stop);

        if let Some(e) = self.error {
            Err(e)
        } else {
            res.map_err(|e| e.into())
        }
    }

    /// Obtains the lock for [`Self::signal`] and updates it to match the new provided `signal`.
    fn send_signal(&self, new_signal: Signal) {
        if let Ok(mut signal) = self.signal.lock() {
            *signal = new_signal
        }
    }

    /// Requests redraw for [`Self::graphics`] window.
    fn redraw(&mut self) {
        self.graphics.as_mut().unwrap().window.request_redraw()
    }

    /// Reloads internals of [`Gui`] to begin rendering again from the first block.
    ///
    /// Sends [`Signal::Pause`] with an [`Interrupt::Start`] to the [`Tui`](crate::tui).
    /// After this, the program will require user input to resume.
    ///
    /// Removes any drawn toolpaths and resets the stock.
    fn reload(&mut self) {
        self.interrupt = Some(Interrupt::Start);
        self.interpreter.reload();
        self.send_signal(Signal::Pause {
            interrupt: Interrupt::Start,
            machine: *self.interpreter.machine(),
            index: 0,
        });

        if let Some(graphics) = self.graphics.as_mut() {
            graphics.clear();
            graphics.window.request_redraw();
        }
    }

    /// Updates [`Self::graphics`] and [`execute`](Self::execute)s the next block if the previous block has finished rendering.
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
            if self.single && !self.next_requested {
                // in single mode and no next block was requested,
                // therefore this redraw request was not related to the simulation
                // as a block is always drawn to completion before checking for new ones
                // only seed new block on next user request
                return Ok(false);
            }

            // exhausted, execute new block and seed the line tracker
            // in single mode, we reach here if user requested next block
            match self.execute() {
                Ok(Some(motion)) => self.graphics.as_mut().unwrap().lines_tracker.add(motion),

                Ok(None) => (), // just try again on next redraw

                Err(error) => {
                    // since the line has already been executed,
                    // we have to provide the index of previous block, that caused this error
                    let index = self.interpreter.source().index().saturating_sub(1);

                    self.send_signal(Signal::Error {
                        error,
                        machine: *self.interpreter.machine(),
                        index,
                    });

                    return Err(error.into());
                }
            }

            if self.single {
                // if single mode is on, then this must be reset here so that next block is not triggered
                self.next_requested = false;
                // always render when lines have exhausted on single mode
                // to make sure that the block is fully rendered
                return Ok(true);
            }
        }

        // only pause the redraw simulation loop if single block is detected,
        // and no next block is requsted,
        // the simulation loop can now only resume with user input
        // since we have already returned in that scenario, we can safely call redraw
        self.redraw();

        Ok(render)
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
                // change tool, if config not found use the default one
                let tool_config = self
                    .config
                    .tools
                    .iter()
                    .find(|tool| tool.number == machine.tool())
                    .unwrap_or(&self.config.default_tool);

                self.graphics.as_mut().unwrap().set_tool(*tool_config);

                (
                    block.motion,
                    Signal::Run {
                        summary: block.clone(),
                        machine,
                        index: current,
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
                        index: current,
                    },
                )
            }

            (current, machine, Some(block)) => (
                block.motion,
                Signal::Run {
                    summary: block.clone(),
                    machine,
                    index: current,
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
                        index: current,
                    },
                )
            }
        };

        self.send_signal(signal);

        Ok(motion)
    }

    // some means key event was recorded
    /// Handles a **keypress**.
    ///
    /// Returns:
    /// - `None`: The event was not of kind **press** or the key was not supported.
    /// - `Some(true)`: The event was handled and a new frame needs to be rendered.
    /// - `Some(false)`: The event was handled but a new frame does not need to be rendered.
    ///
    /// On handling the event, sends appropriate [`Signal`] to the [`Tui`](crate::tui).
    fn handle_key_input(&mut self, key: KeyEvent, event_loop: &ActiveEventLoop) -> Option<bool> {
        if key.state.is_pressed() {
            return None; // only consider release events
        }

        let graphics = self.graphics.as_mut().unwrap();

        match key.logical_key {
            Key::Named(NamedKey::Enter) if self.interrupt.is_some() => {
                match self.interrupt {
                    Some(Interrupt::End) => self.reload(),
                    _ => {
                        self.interrupt = None;
                        self.redraw(); // resume simulation
                    }
                };

                Some(false)
            }

            Key::Named(NamedKey::Space) => {
                self.single = !self.single;
                self.next_requested = false;

                self.send_signal(Signal::SetSingle(self.single));

                if !self.single {
                    self.redraw(); // resume simulation
                }

                Some(false)
            }

            Key::Character(character) => match character.as_str() {
                "f" if !self.fit => {
                    graphics.fit_view();

                    self.fit = true;
                    self.send_signal(Signal::SetFit(true));
                    Some(true)
                }

                "n" if self.single => {
                    self.redraw(); // resume simulation
                    self.next_requested = true;
                    Some(true)
                }

                "p" => {
                    let toolpath = !graphics.toolpath;
                    graphics.toolpath = toolpath;
                    self.send_signal(Signal::SetToolpathVisibility(toolpath));
                    Some(true)
                }

                "q" => {
                    event_loop.exit();
                    Some(false)
                }

                "s" => {
                    let stock = !graphics.stock;
                    graphics.stock = stock;
                    self.send_signal(Signal::SetStockVisibility(stock));
                    Some(true)
                }

                "t" => {
                    let tool = !graphics.tool;
                    graphics.tool = tool;
                    self.send_signal(Signal::SetToolVisibility(tool));
                    Some(true)
                }

                "v" => {
                    let new_view = match self.view {
                        Some(View::Isometric) => View::Top,
                        Some(View::Top) => View::Isometric,
                        None => View::default(),
                    };

                    self.view = Some(new_view);
                    graphics.set_view(new_view);
                    self.send_signal(Signal::SetView(self.view));
                    Some(true)
                }

                "+" if self.speed.inc() => {
                    graphics.speed = self.speed;
                    self.send_signal(Signal::SetSpeed(self.speed));
                    Some(false)
                }

                "-" if self.speed.dec() => {
                    graphics.speed = self.speed;
                    self.send_signal(Signal::SetSpeed(self.speed));
                    Some(false)
                }

                _ => None,
            },

            _ => None,
        }
    }
}

impl ApplicationHandler for Gui {
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
    /// Handles mouse and keyboard key presses.
    /// Does **not** handle [`WindowEvent::CursorMoved`].
    /// See [`Self::device_event`] implementation for mouse movement handling.
    ///
    /// On receiving [`WindowEvent::RedrawRequested`], if no [`Self::error`] and [`Self::interrupt`]
    /// are detected, [`updates`](Self::update) the simulation by either: executing a new G-code block
    /// or continuing to render a block already in process. During this,
    /// appropriate [`Signal`]s are sent to the [`Tui`](crate::tui), to reflect the changes.
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

        // force render is used to make sure that every toolpath frame is rendered.
        // this is necessary as this is used for controlling the speed
        let force_render = match event {
            WindowEvent::Resized(size) => match self.graphics.as_mut() {
                Some(graphics) => {
                    graphics.resize(size);
                    true
                }
                None => return,
            },

            WindowEvent::CloseRequested | WindowEvent::Destroyed => return event_loop.exit(),

            WindowEvent::RedrawRequested if self.interrupt.is_none() => {
                match self.update() {
                    Ok(true) => true, // render

                    Ok(false) => return,

                    Err(e) => {
                        // do not exit on interpreter error, wait on the tui
                        self.error = Some(e);
                        false
                    }
                }
            }

            WindowEvent::RedrawRequested => true, // just render

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.left_mouse_pressed = state.is_pressed();
                false
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => {
                self.right_mouse_pressed = state.is_pressed();
                false
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Middle,
                ..
            } => {
                self.middle_mouse_pressed = state.is_pressed();
                false
            }

            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(_, y) => {
                    self.graphics.as_mut().unwrap().zoom(y);
                    false
                }
                MouseScrollDelta::PixelDelta(_) => return, // does not support touchpads yet
            },

            WindowEvent::KeyboardInput { event, .. } => {
                match self.handle_key_input(event, event_loop) {
                    Some(render) => render,
                    None => return,
                }
            }

            _ => return,
        };

        if let Err(e) = self.graphics.as_mut().unwrap().render(force_render) {
            self.error = Some(e); // exit on render error
            event_loop.exit()
        }
    }

    /// Handles mouse movement.
    ///
    /// Does **not** handle any key presses.
    /// See [`Self::window_event`] implementation for mouse and keyboard key input handling.
    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.graphics.is_none() {
            return;
        }

        // handle pointer movement here as this is raw data
        match event {
            // prioritize orbiting
            DeviceEvent::MouseMotion { delta } if self.middle_mouse_pressed => {
                if self.view.is_some() {
                    // first interaction for oribiting after a preset view
                    // forfeit current view
                    self.view = None;
                    self.send_signal(Signal::SetView(None));
                }

                let delta = [delta.0 as f32, delta.1.neg() as f32, 0.0];
                self.graphics.as_mut().unwrap().orbit(delta) // orbiting around x and y
            }

            DeviceEvent::MouseMotion { delta } if self.right_mouse_pressed => {
                if self.view.is_some() {
                    // first interaction for oribiting after a preset view
                    // forfeit current view
                    self.view = None;
                    self.send_signal(Signal::SetView(None));
                }

                let delta = [0.0, 0.0, delta.1 as f32];
                self.graphics.as_mut().unwrap().orbit(delta) // orbiting around z
            }

            DeviceEvent::MouseMotion { delta } if self.left_mouse_pressed => {
                if self.fit {
                    self.fit = false;
                    self.send_signal(Signal::SetFit(false));
                }

                let delta = [delta.0 as f32, delta.1.neg() as f32];
                self.graphics.as_mut().unwrap().pan(delta) // panning
            }

            _ => return,
        };

        if let Err(e) = self.graphics.as_mut().unwrap().render(false) {
            self.error = Some(e); // exit on render error
            event_loop.exit()
        }
    }

    /// Exits the `event_loop` on receiving the first event from user.
    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        // do not record tui exit event,
        // just exit and report any errors
        event_loop.exit();
    }
}
