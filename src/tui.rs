//! # Tui
//!
//! Cooks up a **terminal user interface** using [`RataTui`](ratatui),
//! and houses its render loop and event handling.
//!
//! The `Tui` is drawn to [`Stdout`] and uses [`Crossterm`](CrosstermBackend) as its backend.
//!
//! The render loop draws endlessly to achieve the [`TARGET_FPS`],
//! and only terminates on reading [`Signal::Error`] or [`Signal::Stop`] from the [`Gui`].
//! Before drawing each frame, [`Tui::signal`] is refreshed to fetch new [`Signal`] from [`Gui`].
//!
//! Listens for user input,
//! and sends [`Command`]s to the [`Gui`] thread on receiving corresponding user input.

use crate::machine::Plane;
#[allow(unused_imports)]
use crate::{
    Command, Gui, Interrupt, SINGLE, STOCK, Signal, Speed, TOOL, TOOLPATH, View,
    config::Unit,
    machine::{CircularDirection, FeedMode, Motion, Positioning},
    source::Source,
};
use ratatui::{
    Frame, Terminal,
    crossterm::{
        event::{self, Event, KeyCode, KeyEvent, poll},
        execute, terminal,
    },
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Backend, CrosstermBackend},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};
use std::{
    error::Error,
    io::Stdout,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use winit::event_loop::EventLoopProxy;

/// Frames to draw per second. This also decides how often [`Tui::signal`] should be refreshed.
pub const TARGET_FPS: u64 = 30;

// approx, due to int division truncation
const TIME_BETWEEN_FRAMES: Duration = Duration::from_millis(1_000 / TARGET_FPS);

/// Maximum number of [`Block`]s from [`Source`] visible ahead of the current block.
const MAX_PREVIEW_AHEAD: usize = 10;

/// Hex encoded default background color.
const BG: Color = Color::from_u32(0x001e1e1e);

/// Default [`Theme`] for the [`Tui`].
const THEME: Theme = Theme {
    root: Style::new().bg(BG).fg(Color::White),
    title: Style::new().fg(Color::Red).bg(BG).bold(),
    block_title: Style::new().fg(Color::LightGreen).bold(),
    interrupt: Style::new().fg(Color::Black).bg(Color::White).bold(),
    summary: Style::new().fg(Color::Blue).bg(BG).bold(),
    machine: Style::new().fg(Color::LightBlue).bg(BG).bold(),
    active_mode: Style::new().fg(Color::LightBlue).bg(BG).bold(),
    inactive_mode: Style::new().fg(Color::Gray).bg(BG),
    key: Style::new().fg(Color::Black).bg(Color::DarkGray).bold(),
    key_desc: Style::new().fg(Color::DarkGray).bg(Color::Black),
    alarm: Style::new().fg(Color::Black).bg(Color::Rgb(200, 200, 200)),
};

/// Current state of the [`Tui`](crate::tui).
pub struct Tui {
    /// Event proxy for sending [`Command`]s to [`Gui`].
    proxy: EventLoopProxy<Command>,

    /// Current selected [`View`].
    view: View,

    /// Copy of source for previewing.
    source: Source,

    /// Single step through code blocks.
    single: bool,

    /// Tool visibility flag.
    tool: bool,

    /// Toolpath visibility flag.
    toolpath: bool,

    /// Stock visibility flag.
    stock: bool,

    /// Simulation speed.
    speed: Speed,

    /// An [`Arc`][`Mutex`] that can be altered by the [`Gui`] to send [`Signal`]s.
    signal: Arc<Mutex<Signal>>,
}

impl Tui {
    /// Constructs a new [`Tui`] and sets up all the flags to their predefined constants.
    ///
    /// The [`Self::view`] is set to [`View::default`],
    /// and [`Self::speed`] to [`Speed::default`].
    pub fn new(proxy: EventLoopProxy<Command>, source: Source, signal: Arc<Mutex<Signal>>) -> Self {
        Self {
            proxy,
            view: View::default(),
            source,
            single: SINGLE,
            tool: TOOL,
            toolpath: TOOLPATH,
            stock: STOCK,
            speed: Speed::default(),
            signal,
        }
    }

    /// Starts up the [`Tui`] by drawing frames at [`TARGET_FPS`] and communicates any user input to
    /// the [`Gui`].
    ///
    /// The [`Tui`] thread cannot terminate the program now, just by returning an `Error`.
    /// A [`Command::Stop`], with an optional [`Error`](anyhow::Error),
    /// must be sent to the main thread running the [`Gui`],
    /// to tell it to exit the program.
    ///
    /// Therefore, to report any error from this function,
    /// it must be sent to the main [`Gui`] thread.
    pub fn run(mut self) {
        // on failure to prepare terminal, tell main thread to stop and stop current thread
        let mut terminal = match prepare_terminal() {
            Ok(t) => t,
            Err(e) => return self.proxy.send_event(Command::Stop(Some(e))).unwrap(),
        };

        let mut res = self.start_loop(&mut terminal);

        // prioritize terminal error
        if let Err(e) = restore_terminal(terminal) {
            res = Err(e)
        };

        match self.refresh_signal() {
            Signal::Stop => (), // main thread already signalled to stop
            _ => {
                let _ = self.proxy.send_event(Command::Stop(res.err()));
            }
        }
    }

    /// Starts the [`Tui`] by drawing to the `terminal` at [`TARGET_FPS`] in a loop and waits for user input.
    ///
    /// Before drawing each new frame, [`Self::signal`] is refreshed to get potential new
    /// [`Signal`] from [`Gui`].
    fn start_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()>
    where
        anyhow::Error: From<B::Error>,
    {
        let mut time_tracker = Instant::now();
        let mut signal = self.refresh_signal();

        loop {
            if time_tracker.elapsed() > TIME_BETWEEN_FRAMES {
                signal = self.refresh_signal();
                terminal.draw(|frame| self.draw(frame, &signal))?;
                time_tracker -= TIME_BETWEEN_FRAMES;
                // time_tracker = Instant::now(); // not performant
            }

            match &signal {
                Signal::Run { .. } => {
                    if let Some(key) = poll_key_press()? {
                        if !self.handle_common_keys(key) {
                            match key.code {
                                KeyCode::Char('Q') => return Ok(()),

                                KeyCode::Char('n') if self.single => {
                                    self.proxy.send_event(Command::Next).unwrap()
                                }

                                _ => (),
                            }
                        }
                    }
                }

                Signal::Pause { .. } => {
                    if let Some(key) = poll_key_press()? {
                        if !self.handle_common_keys(key) {
                            match key.code {
                                KeyCode::Enter => {
                                    self.proxy.send_event(Command::ClearInterrupt).unwrap()
                                }

                                KeyCode::Char('Q') => return Ok(()),

                                _ => (),
                            }
                        }
                    }
                }

                Signal::Error { error, .. } => {
                    if let Some(key) = poll_key_press()?
                        && (key.code == KeyCode::Enter
                            || key.code == KeyCode::Char('Q')
                            || key.code == KeyCode::Esc)
                    {
                        return Err((*error).into());
                    }
                }

                Signal::Stop => return Ok(()),
            }
        }
    }

    /// Obtains the lock for [`Self::signal`], copies the [`Signal`] and returns it.
    ///
    /// This [`Signal`] may or may not be different from the one used for previous frame.
    fn refresh_signal(&mut self) -> Signal {
        self.signal.lock().unwrap().clone()
    }

    /// Handles common key inputs. Returns `true` if the event was handled.
    fn handle_common_keys(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('v') => {
                match self.view {
                    View::Isometric => self.view = View::Top,
                    View::Top => self.view = View::Isometric,
                };
                self.proxy.send_event(Command::SetView(self.view)).unwrap();
            }

            KeyCode::Char('1') => {
                self.single = !self.single;
                self.proxy
                    .send_event(Command::SetSingle(self.single))
                    .unwrap()
            }

            KeyCode::Char('t') => {
                self.tool = !self.tool;
                self.proxy
                    .send_event(Command::SetToolVisibility(self.tool))
                    .unwrap()
            }

            KeyCode::Char('p') => {
                self.toolpath = !self.toolpath;
                self.proxy
                    .send_event(Command::SetToolpathVisibility(self.toolpath))
                    .unwrap()
            }

            KeyCode::Char('s') => {
                self.stock = !self.stock;
                self.proxy
                    .send_event(Command::SetStockVisibility(self.stock))
                    .unwrap()
            }

            KeyCode::Char('+') if self.speed.inc() => self
                .proxy
                .send_event(Command::SetSpeed(self.speed))
                .unwrap(),

            KeyCode::Char('-') if self.speed.dec() => self
                .proxy
                .send_event(Command::SetSpeed(self.speed))
                .unwrap(),

            _ => return false,
        };

        true
    }

    /// Prepares individual sections of the terminal screen,
    /// and draws the current state of [`Tui`] in the said sections.
    fn draw(&self, frame: &mut Frame, signal: &Signal) {
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(100),
                Constraint::Min(6),
                Constraint::Min(5),
                Constraint::Min(6),
            ])
            .split(frame.area());

        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main_chunks[0]);

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Percentage(100)])
            .split(top_chunks[0]);

        frame.render_widget(self.title_text(), left_chunks[0]);
        frame.render_widget(self.summary_widget(signal), left_chunks[1]);
        frame.render_widget(self.preview_widget(signal), top_chunks[1]);
        frame.render_widget(self.machine_widget(signal), main_chunks[1]);
        frame.render_widget(self.modes_widget(), main_chunks[2]);
        frame.render_widget(self.keys_widget(signal), main_chunks[3]);

        // present error, if any
        if let Signal::Error { error, .. } = signal {
            let mut error_lines = vec![error.to_string(), "\n".to_string()];
            let mut source = error.source();

            while let Some(cause) = source {
                error_lines.push(format!("caused by:\n{cause}"));
                source = cause.source();
            }

            let popup = Paragraph::new(error_lines.join("\n"))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .padding(Padding::symmetric(2, 1))
                        .title(Line::styled(" Alarm ", THEME.title).centered()) // use program title
                        .style(THEME.alarm),
                )
                .centered();

            let area = get_centered(75, 40, frame.area());
            frame.render_widget(Clear, area);
            frame.render_widget(popup, area);
        }
    }

    /// Returns program title for display at top.
    fn title_text(&self) -> Paragraph<'_> {
        Paragraph::new("GSim-rs")
            .style(THEME.title)
            .block(Block::default().padding(Padding::symmetric(1, 1)))
            .centered()
    }

    /// Returns the user-facing summary of the provided `signal`.
    fn summary_widget(&self, signal: &Signal) -> Paragraph<'_> {
        let lines = match signal {
            Signal::Run { summary, .. } => {
                let gcodes_len = summary.gcodes.len();
                let mcode_some = summary.mcode.is_some();
                let codes_len = summary.codes.len();

                // preallocate based on lengths
                let mut lines = Vec::with_capacity(
                    if gcodes_len > 0 { gcodes_len + 2 } else { 0 }
                        + if mcode_some { 3 } else { 0 }
                        + if codes_len > 0 { codes_len + 1 } else { 0 },
                );

                if gcodes_len > 0 {
                    lines.push(Line::styled(
                        if gcodes_len > 1 { "GCODES:" } else { "GCODE:" },
                        THEME.summary,
                    ));
                    for gcode in &summary.gcodes {
                        lines.push(Line::styled(gcode.to_string(), THEME.root));
                    }
                    lines.push(Line::default());
                };

                if let Some(mcode) = summary.mcode {
                    lines.push(Line::styled("MCODE:", THEME.summary));
                    lines.push(Line::styled(mcode.to_string(), THEME.root));
                    lines.push(Line::default());
                }

                if codes_len > 0 {
                    lines.push(Line::styled(
                        if codes_len > 1 {
                            "Other CODES:"
                        } else {
                            "Other CODE:"
                        },
                        THEME.summary,
                    ));
                    for code in &summary.codes {
                        lines.push(Line::styled(code.to_string(), THEME.root));
                    }
                };

                lines
            }

            Signal::Pause { interrupt, .. } => vec![
                Line::from(vec![
                    Span::styled(interrupt.to_string(), THEME.interrupt),
                    Span::styled(" detected.", THEME.root),
                ]),
                Line::default(),
                Line::from(vec![
                    Span::styled("Press ", THEME.root),
                    Span::styled("Enter", THEME.interrupt),
                    Span::styled(" to remove the interrupt.", THEME.root),
                ]),
            ],

            Signal::Error { .. } | Signal::Stop => vec![],
        };

        Paragraph::new(lines)
            .block(
                Block::default()
                    .padding(Padding::symmetric(2, 1))
                    .borders(Borders::TOP)
                    .title(Line::styled("Block Summary", THEME.block_title).centered())
                    .style(THEME.root),
            )
            .centered()
    }

    /// Returns a preview of [`Self::source`] based on the current block of execution.
    ///
    /// One line of context is also provided in the preview.
    /// Does not highlight first line if the start interrupt is detected.
    fn preview_widget(&self, signal: &Signal) -> Paragraph<'_> {
        let mut start_interrupt = false;

        let current = match signal {
            Signal::Pause {
                interrupt: Interrupt::Start,
                current,
                ..
            } => {
                start_interrupt = true;
                *current
            }

            Signal::Run { current, .. }
            | Signal::Error { current, .. }
            | Signal::Pause { current, .. } => *current,

            Signal::Stop => return Paragraph::default(),
        };

        const CONTEXT_COUNT: usize = 1; // num of lines to show before current line
        // max num of lines in total. one context, one current and rest look ahead
        const COUNT: usize = MAX_PREVIEW_AHEAD + CONTEXT_COUNT + 1;

        // preallocate for max num of lines
        let mut lines = Vec::with_capacity(COUNT);

        for i in 0..COUNT {
            let i = if current < CONTEXT_COUNT {
                i + current
            } else {
                i + current - CONTEXT_COUNT
            };

            match self.source.get(i) {
                Some(line) => {
                    // do not highlight current line if Interrupt::Start is detected
                    if i == current && !start_interrupt {
                        lines.push(Line::styled(
                            line,
                            Style::default().bg(Color::White).fg(Color::Black),
                        ))
                    } else {
                        lines.push(Line::from(line))
                    }
                }
                None => break,
            }
        }

        Paragraph::new(lines).block(
            Block::default()
                .padding(Padding::symmetric(3, 1))
                .borders(Borders::TOP | Borders::LEFT)
                .title(Line::styled("Preview", THEME.block_title).centered())
                .style(THEME.root),
        )
    }

    /// Returns a widget for showing the state of [`Machine`](crate::machine) from
    /// the provided `signal`.
    fn machine_widget(&self, signal: &Signal) -> Paragraph<'_> {
        let machine = match signal {
            Signal::Run { machine, .. }
            | Signal::Pause { machine, .. }
            | Signal::Error { machine, .. } => machine,

            Signal::Stop => return Paragraph::default(),
        };

        let pos = machine.pos();
        let unit = Span::styled(
            match machine.units() {
                Unit::Imperial => " in",
                Unit::Metric => " mm",
            },
            THEME.root,
        );

        let mut line1 = vec![
            Span::styled("X", THEME.machine),
            Span::styled(": ", THEME.root),
            Span::styled(pos.x.to_string(), THEME.root.bold()),
            unit.clone(),
            Span::styled(" | ", THEME.root),
            Span::styled("Y", THEME.machine),
            Span::styled(": ", THEME.root),
            Span::styled(pos.y.to_string(), THEME.root.bold()),
            unit.clone(),
            Span::styled(" | ", THEME.root),
            Span::styled("Z", THEME.machine),
            Span::styled(": ", THEME.root),
            Span::styled(pos.z.to_string(), THEME.root.bold()),
            unit.clone(),
            Span::styled(" | ", THEME.root),
            Span::styled("T", THEME.machine),
            Span::styled(": ", THEME.root),
            Span::styled(machine.tool().to_string(), THEME.root.bold()),
        ];

        // append feed if available
        if let Some(feed) = *machine.feed() {
            line1.extend(vec![
                Span::styled(" | ", THEME.root),
                Span::styled("F", THEME.machine),
                Span::styled(": ", THEME.root),
                Span::styled(feed.to_string(), THEME.root.bold()),
                unit,
                Span::styled(
                    match machine.feed_mode() {
                        FeedMode::PerMinute => "/min",
                        FeedMode::PerRev => "/rev",
                    },
                    THEME.root,
                ),
            ]);
        }

        let line2 = vec![
            Span::styled(
                match machine.motion() {
                    Motion::Rapid => "RAPID",
                    Motion::Feed => "FEED",
                    Motion::Arc(CircularDirection::Clockwise) => "CLOCKWISE",
                    Motion::Arc(CircularDirection::CounterClockwise) => "ANTICLOCKWISE",
                },
                THEME.machine,
            ),
            Span::styled(" | ", THEME.root),
            Span::styled(
                match machine.plane() {
                    Plane::XY => "XY",
                    Plane::XZ => "XZ",
                    Plane::YZ => "YZ",
                },
                THEME.machine,
            ),
            Span::styled(" | ", THEME.root),
            Span::styled(
                match machine.code_units() {
                    Unit::Imperial => "IMPERIAL",
                    Unit::Metric => "METRIC",
                },
                THEME.machine,
            ),
            Span::styled(" | ", THEME.root),
            Span::styled(
                match machine.positioning() {
                    Positioning::Absolute => "ABSOLUTE",
                    Positioning::Incremental => "INCREMENTAL",
                },
                THEME.machine,
            ),
        ];

        Paragraph::new(vec![line1.into(), line2.into()])
            .block(
                Block::default()
                    .padding(Padding::symmetric(3, 1))
                    .borders(Borders::TOP)
                    .title(Line::styled("Machine", THEME.block_title).centered())
                    .style(THEME.root),
            )
            .centered()
    }

    /// Returns the active simulation modes.
    ///
    /// Also shows [`Self::speed`].
    fn modes_widget(&self) -> Paragraph<'_> {
        // preallocate for a few modes
        let modes = vec![
            Span::styled(self.view.to_string(), THEME.active_mode),
            Span::styled(" | ", THEME.root),
            Span::styled(
                "SINGLE",
                if self.single {
                    THEME.active_mode
                } else {
                    THEME.inactive_mode
                },
            ),
            Span::styled(" | ", THEME.root),
            Span::styled(
                "TOOL",
                if self.tool {
                    THEME.active_mode
                } else {
                    THEME.inactive_mode
                },
            ),
            Span::styled(" | ", THEME.root),
            Span::styled(
                "TOOLPATH",
                if self.toolpath {
                    THEME.active_mode
                } else {
                    THEME.inactive_mode
                },
            ),
            Span::styled(" | ", THEME.root),
            Span::styled(
                "STOCK",
                if self.stock {
                    THEME.active_mode
                } else {
                    THEME.inactive_mode
                },
            ),
        ];

        let speed = vec![
            Span::styled("SPEED", THEME.active_mode),
            Span::styled(": ", THEME.root),
            Span::styled(self.speed.to_string(), THEME.root),
        ];

        Paragraph::new(Text::from(vec![Line::from(modes), Line::from(speed)]))
            .block(
                Block::default()
                    .padding(Padding::symmetric(3, 1))
                    .borders(Borders::TOP)
                    .title(Line::styled("Modes", THEME.block_title).centered())
                    .style(THEME.root),
            )
            .centered()
    }

    /// Returns every **possible key input** based on the current state.
    ///
    /// ## Reference
    /// [Github](https://github.com/ratatui/ratatui/blob/main/examples/apps/demo2/src/app.rs)
    fn keys_widget(&self, signal: &Signal) -> Paragraph<'_> {
        let mut spans1 = vec![
            Span::styled("  Q  ", THEME.key),
            Span::styled(" Quit ", THEME.key_desc),
            Span::styled("  v  ", THEME.key),
            Span::styled(" Switch View ", THEME.key_desc),
            Span::styled("  +  ", THEME.key),
            Span::styled(" Speed Up ", THEME.key_desc),
            Span::styled("  -  ", THEME.key),
            Span::styled(" Slow Down ", THEME.key_desc),
        ];

        // only add "n" key if no interrupt and single block is on
        match signal {
            Signal::Pause { .. } => (),

            _ if self.single => {
                spans1.push(Span::styled("  n  ", THEME.key));
                spans1.push(Span::styled(" Next Block ", THEME.key_desc));
            }

            _ => (),
        }

        let spans2 = vec![
            Span::styled("  1  ", THEME.key),
            Span::styled(" Toggle Single ", THEME.key_desc),
            Span::styled("  t  ", THEME.key),
            Span::styled(" Toggle Tool ", THEME.key_desc),
            Span::styled("  p  ", THEME.key),
            Span::styled(" Toggle Toolpath ", THEME.key_desc),
            Span::styled("  s  ", THEME.key),
            Span::styled(" Toggle Stock ", THEME.key_desc),
        ];

        Paragraph::new(Text::from(vec![
            Line::from(spans1),
            Line::default(),
            Line::from(spans2),
        ]))
        .block(
            Block::default()
                .padding(Padding::symmetric(3, 1))
                .borders(Borders::TOP)
                .title(Line::styled("Commands", THEME.block_title).centered())
                .style(THEME.root),
        )
        .centered()
    }
}

/// Prepares the terminal for use with [`Tui`],
/// by enabling raw mode and using alternate screen to preserve shell history.
fn prepare_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = std::io::stdout();

    terminal::enable_raw_mode()?;

    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        event::EnableMouseCapture
    )?;

    let backend = CrosstermBackend::new(stdout);

    Terminal::new(backend).map_err(|e| e.into())
}

/// Restores the terminal to its previous state and restores the shell history.
fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    terminal::disable_raw_mode()?;

    execute!(
        terminal.backend_mut(),
        terminal::LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;

    terminal.show_cursor()?;

    Ok(())
}

/// Creates a centered [`Rect`] using up supplied percentages in X and Y.
fn get_centered(x: u16, y: u16, rect: Rect) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - y) / 2),
            Constraint::Percentage(y),
            Constraint::Percentage((100 - y) / 2),
        ])
        .split(rect);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - x) / 2),
            Constraint::Percentage(x),
            Constraint::Percentage((100 - x) / 2),
        ])
        .split(chunks[1])[1] // return the middle chunk
}

/// Polls for an [`Event::Key`] of [`KeyEventKind::Press`](KeyEventKind::Press).
fn poll_key_press() -> Result<Option<KeyEvent>, std::io::Error> {
    if !poll(Duration::from_millis(100))? {
        return Ok(None);
    };

    match event::read()? {
        Event::Key(key) if key.is_press() => Ok(Some(key)),
        _ => Ok(None),
    }
}

/// Styling for each section of the [`Tui`].
struct Theme {
    root: Style,
    title: Style,
    block_title: Style,
    interrupt: Style,
    summary: Style,
    machine: Style,
    active_mode: Style,
    inactive_mode: Style,
    key: Style,
    key_desc: Style,
    alarm: Style,
}
