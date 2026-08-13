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
    Interrupt, SINGLE, Signal, Speed, View,
    config::Config,
    interpreter::{Interpreter, InterpreterError},
    machine::MotionSummary,
    renderer::Graphics,
};
use std::{
    ops::Neg,
    sync::{Arc, Mutex},
};
use winit::{
    application::ApplicationHandler,
    event::{
        DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent,
    },
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey, SmolStr},
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
    ///   [`Command::Stop`] before exiting.
    ///
    /// This exit behaviour is implemented because it lets the user observe the toolpath
    /// in case the reason for error was the actual G-code and not the simulation.
    error: Option<anyhow::Error>,

    /// [`winit`] event loop that can receive user events in form of [`Command`]s.
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
    /// and is used determine when to wait for [`Command::Next`] from the tui thread.
    single: bool,

    // user requested to execute next block while in single block.
    // reset to false on block fulfilling and turning off single block.
    next_requested: bool,

    /// Simulation speed.
    speed: Speed,

    /// Flag to set when the user has orbited the [`Graphics::window`] with their mouse,
    /// to signal that the current view setting is now to be forfeited.
    ///
    /// This is set to `false` when [`Command::SetView`] is received,
    /// that resets the view to a predefined viewing angle.
    view: Option<View>,

    /// Is [`Self::view`] zoomed in the most without overflow.
    fit: bool,

    /// Flag to track mouse events received while **left-mouse-button** is pressed.
    /// This is set and unset on receiving an appropriate [`WindowEvent::MouseInput`] event.
    ///
    /// While this is set to `true` the toolpath is stopped,
    /// as new projection is calculated during this state.
    left_mouse_pressed: bool,

    /// Flag to track mouse events received while **right-mouse-button** is pressed.
    /// This is set and unset on receiving an appropriate [`WindowEvent::MouseInput`] event.
    ///
    /// While this is set to `true` the toolpath is stopped,
    /// as new projection is calculated during this state.
    right_mouse_pressed: bool,

    /// Flag to track mouse events received while **middle-mouse-button** is pressed.
    /// This is set and unset on receiving an appropriate [`WindowEvent::MouseInput`] event.
    ///
    /// While this is set to `true` the toolpath is stopped,
    /// as new projection is calculated during this state.
    middle_mouse_pressed: bool,
}

impl Gui {
    /// Constructs a new [`Gui`],
    /// initializing the [`EventLoop`] ready to receive [`Command`]s and send [`Signal`]s.
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
            single: SINGLE,
            next_requested: false,
            speed: Speed::default(),
            view: Some(View::default()),
            fit: true,
            left_mouse_pressed: false,
            right_mouse_pressed: false,
            middle_mouse_pressed: false,
        }
    }

    /// Returns an [`EventLoopProxy`] for sending [`Command`]s to the [`Gui`] from other threads.
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
            index: 0,
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
            // in single mode and no next block was requested,
            // therefore this redraw request was not related to the simulation
            // as a block is always drawn to completion before checking for new ones
            // only seed new block on user request
            if self.single && !self.next_requested {
                return Ok(false);
            }

            // not on single block
            // exhausted, execute new block and seed the line tracker
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
                        index: current,
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
        // since we have already returned in this scenario, we can safely call redraw
        //
        // redraw for newly added motion, draining previous motion, or execute new block if motion was None
        // loops back to this func
        //
        // skips looping when proceed command is detected on single mode, as that must require
        // user input for single mode to work
        self.redraw();

        // always render when lines have exhausted on single mode
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
    fn handle_key_input(&mut self, key: KeyEvent) -> Option<bool> {
        if key.state.is_pressed() {
            return None; // only consider release events
        }

        let graphics = self.graphics.as_mut().expect("app has been started");

        match key.logical_key {
            Key::Named(NamedKey::Enter) => {
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
                    let new_toolpath_visibility = !graphics.toolpath;
                    graphics.toolpath = new_toolpath_visibility;
                    self.send_signal(Signal::SetToolpathVisibility(new_toolpath_visibility));
                    Some(true)
                }

                "s" => {
                    let new_stock_visibility = !graphics.stock;
                    graphics.stock = new_stock_visibility;
                    self.send_signal(Signal::SetStockVisibility(new_stock_visibility));
                    Some(true)
                }

                "t" => {
                    let new_tool_visibility = !graphics.tool;
                    graphics.tool = new_tool_visibility;
                    self.send_signal(Signal::SetToolVisibility(new_tool_visibility));
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
                if self.fit {
                    self.fit = false;
                    self.send_signal(Signal::SetFit(false));
                }

                self.left_mouse_pressed = state.is_pressed();

                false
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => {
                if self.view.is_some() {
                    // first interaction for oribiting after a preset view
                    // forfeit current view
                    self.view = None;
                    self.send_signal(Signal::SetView(None));
                }

                self.right_mouse_pressed = state.is_pressed();

                false
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Middle,
                ..
            } => {
                if self.view.is_some() {
                    // first interaction for oribiting after a preset view
                    // forfeit current view
                    self.view = None;
                    self.send_signal(Signal::SetView(None));
                }

                self.middle_mouse_pressed = state.is_pressed();

                false
            }

            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(_, y) => {
                    if self.fit {
                        self.fit = false;
                        self.send_signal(Signal::SetFit(false));
                    }

                    self.graphics.as_mut().unwrap().zoom(y);
                    false
                }
                MouseScrollDelta::PixelDelta(_) => return, // does not support touchpads yet
            },

            WindowEvent::KeyboardInput { event, .. } => match self.handle_key_input(event) {
                Some(render) => render,
                None => return,
            },

            _ => return,
        };

        if let Err(e) = self.graphics.as_mut().unwrap().render(force_render) {
            self.error = Some(e); // exit on render error
            event_loop.exit()
        }
    }

    // only handles mouse movement
    // but all mouse button presses are handled in windowevent
    // so we do not have to deal with raw button inputs
    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        let graphics = match self.graphics.as_mut() {
            Some(g) => g,
            None => return,
        };

        // handle pointer movement here as this is raw data
        match event {
            // prioritize orbiting
            DeviceEvent::MouseMotion { delta } if self.middle_mouse_pressed => {
                let delta = [delta.0 as f32, delta.1.neg() as f32, 0.0];
                graphics.orbit(delta) // orbiting around x and y
            }

            DeviceEvent::MouseMotion { delta } if self.right_mouse_pressed => {
                let delta = [0.0, 0.0, delta.1 as f32];
                graphics.orbit(delta) // orbiting around z
            }

            DeviceEvent::MouseMotion { delta } if self.left_mouse_pressed => {
                let delta = [delta.0 as f32, delta.1.neg() as f32];
                graphics.pan(delta) // panning
            }

            _ => return,
        };

        if let Err(e) = graphics.render(false) {
            self.error = Some(e); // exit on render error
            event_loop.exit()
        }
    }

    /// Handles [`Command`]s sent from the [`Tui`](crate::tui) thread.
    ///
    /// Each command alters [`Graphics`] state and draws a new frame or,
    /// in case of [`Command::Stop`], exits the loop.
    /// Latest command is always stored at the end.
    fn user_event(&mut self, event_loop: &ActiveEventLoop, _event: ()) {
        // do not record tui exit event,
        // just exit and report any errors
        event_loop.exit();
    }
}
