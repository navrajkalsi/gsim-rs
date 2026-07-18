//! # Tui
//!
//! Cooks up a **terminal user interface** using [`RataTui`](ratatui),
//! and houses its render loop and event handling.
//!
//! The `Tui` is drawn to [`Stdout`] and uses [`Crossterm`](CrosstermBackend) as its backend.
//!
//! The render loop is driven by [`Signal`]s from the [`Gui`] thread,
//! which receive [`Command`]s in response from the [`Tui`] thread,
//! communicating user input and state changes.

use ratatui::{
    Frame, Terminal,
    crossterm::{
        event::{self, Event, KeyCode, poll},
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
    fmt::Display,
    io::Stdout,
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};
use winit::event_loop::EventLoopProxy;

#[allow(unused_imports)]
use crate::{
    Command, SINGLE, Signal, TOOL, View,
    cli::Cli,
    config::{Config, Unit},
    gui::Gui,
    interpreter::InterpreterError,
    interpreter::{BlockSummary, Interpreter},
    lexer::Lexer,
    machine::Machine,
    machine::{CircularDirection, FeedMode, Motion, Positioning},
    parser::Plane,
    parser::{CodeBlock, MCode, Parser},
    source::Source,
};
use crate::{STOCK, TOOLPATH};

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

/// Represents the types of program cycle interruptions.
/// These interruptions need user input to be removed and resume cycle.
enum Interrupt {
    /// Confirm program start or restart.
    Start,
    /// M00 program stop detected.
    Stop,
    /// M01 optional program stop detected.
    OptionalStop,
    /// M30 program end detected.
    End,
}

impl Display for Interrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Interrupt::Start => "START INTERRUPT",
            Interrupt::Stop => "STOP INTERRUPT",
            Interrupt::OptionalStop => "OPTIONAL STOP INTERRUPT",
            Interrupt::End => "END INTERRUPT",
        };

        write!(f, "{string}")
    }
}

/// Represents the current state of the [`Tui`](crate::tui).
pub struct Tui {
    /// Receiver end of the channel for [`Signal`] from [`Gui`].
    signal: Receiver<Signal>,
    /// Event proxy for sending [`Command`]s to [`Gui`].
    proxy: EventLoopProxy<Command>,
    /// Current selected [`View`].
    view: View,
    /// Single step through code blocks.
    single: bool,
    /// Tool visibility flag.
    tool: bool,
    /// Toolpath visibility flag.
    toolpath: bool,
    /// Stock visibility flag.
    stock: bool,
    /// Parsed source loaded [`Interpreter`], ready for iteration.
    interpreter: Interpreter,
    /// Index of current block being executed for preview.
    current: usize,
    /// Total number of summaries stored till [`MCode::Stop`] block.
    /// This is only set to [`Some`] once all the blocks have been interpreted once.
    total: Option<usize>,
    /// [`None`] if the program is running.
    interrupt: Option<Interrupt>,
    /// Summaries for all executed blocks.
    /// These stay in memory for the whole life of the program,
    /// making looping for the second times more efficient.
    summaries: Vec<BlockSummary>,
    /// Previously received [`Signal`] from [`Gui`].
    last_signal: Option<Signal>,
    /// Error that is a direct result of [`executing`](Interpreter::execute) a [`CodeBlock`].
    /// This is stored for rendering error to the Tui before exiting to the shell.
    error: Option<InterpreterError>,
}

impl Tui {
    /// Constructs a new [`Tui`] and loads the [`Source`] either from file at input path or `stdin`.
    ///
    /// The [`Tui::view`] is set to [`View::default`],
    /// [`Tui::single`] block execution is set to `false`,
    /// and [`Tui::interrupt`] to [`Interrupt::Start`].
    ///
    /// # Errors
    /// Returns [`Error`](anyhow::Error) on failure to read [`Source`] file or build the [`Machine`].
    pub fn build(
        signal: Receiver<Signal>,
        cli: Cli,
        config: Config,
        proxy: EventLoopProxy<Command>,
    ) -> anyhow::Result<Self> {
        let src = match &cli.source {
            Some(path) => Source::from_file(path),
            None => Source::from_stdin(),
        }?;

        // this may be larger than the total number of summaries at the end,
        // as summaries are only stored till the first M30 detect.
        let len = src.len(); // for preallocation

        Ok(Self {
            signal,
            proxy,
            view: View::default(),
            single: SINGLE,
            tool: TOOL,
            toolpath: TOOLPATH,
            stock: STOCK,
            interpreter: Interpreter::new(
                Parser::new(Lexer::new(src)),
                Machine::new(config.units, config.zero_pos, config.start_pos),
            ),
            current: 0,
            total: None,
            interrupt: Some(Interrupt::Start),
            summaries: Vec::with_capacity(len),
            last_signal: None,
            error: None,
        })
    }

    /// Starts [`Tui`] execution by executing each G-Code line and managing the terminal state.
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

        match self.last_signal {
            Some(Signal::Stop) => {} // main thread already signalled to stop
            _ => self.proxy.send_event(Command::Stop(res.err())).unwrap(),
        }
    }

    /// Checks for any updates from the [`Gui`] thread by trying to receive any [`Signal`]
    /// **without blocking** current thread.
    ///
    /// On success, optionally returns a [`Signal`], if received, else returns [`None`].
    ///
    /// # Errors
    /// Returns [`TryRecvError::Disconnected`] if the main thread had already terminated.
    fn check_signal(&mut self) -> Result<Option<Signal>, TryRecvError> {
        match self.signal.try_recv() {
            Ok(signal) => Ok(Some(signal)),
            Err(err) => match err {
                TryRecvError::Empty => Ok(None),
                TryRecvError::Disconnected => Err(err),
            },
        }
    }

    /// Reloads internals of [`Tui`] to begin rendering again from the first block.
    /// Also sends [`Command::Clear`] to clear any toolpath from [`Gui`] screen.
    fn reload(&mut self) {
        self.current = 0;
        self.interrupt = Some(Interrupt::Start);
        self.interpreter.reload();
        self.proxy.send_event(Command::Clear).unwrap();
    }

    /// Starts the [`Tui`] by drawing to the `terminal` in a loop and waits for user input.
    fn start_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> anyhow::Result<()>
    where
        anyhow::Error: From<B::Error>,
    {
        // always wait for the gui to trigger
        let mut proceed = false;

        // the main idea of this loop is that the event loop from main thread,
        // drives this loop with every proceed signal
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            self.last_signal = self.check_signal()?;

            match self.last_signal {
                Some(Signal::Proceed) => proceed = true,
                Some(Signal::Stop) => return Ok(()),
                None => (),
            };

            #[allow(clippy::collapsible_if)]
            if proceed && !self.single && self.interrupt.is_none() && self.error.is_none() {
                proceed = self.execute();
            } else if self.error.is_some() {
                // poll for enter event or continue and check if the main thread is still running
                if poll(Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()?
                        && key.kind != event::KeyEventKind::Release
                        && (key.code == KeyCode::Enter
                            || key.code == KeyCode::Char('Q')
                            || key.code == KeyCode::Esc)
                    {
                        return Err(self.error.take().unwrap().into());
                    }
                }
            } else if poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()?
                    && key.kind != event::KeyEventKind::Release
                {
                    match key.code {
                        KeyCode::Char('Q') => return Ok(()),

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

                        KeyCode::Char('n') if proceed && self.interrupt.is_none() => {
                            proceed = self.execute();
                        }

                        KeyCode::Enter => match self.interrupt {
                            Some(Interrupt::End) => self.reload(),
                            Some(Interrupt::Start) => {
                                self.interrupt = None;
                                proceed = self.execute();
                            }
                            Some(_) => self.interrupt = None,
                            None => {}
                        },
                        _ => {}
                    }
                }
            }
        }
    }

    /// Executes the next block, which can be done in two ways:
    /// - For the first pass, each [`CodeBlock`] is executed with [`Interpreter::execute`] and the
    ///   resulting [`BlockSummary`] is stored in [`Tui::summaries`].
    /// - For repeat passes, only stored [`BlockSummary`]s are queried and no actual interpretation
    ///   or parsing takes place.
    ///
    /// Returns `true` when no [`MotionSummary`](crate::machine::MotionSummary) was found in the
    /// latest [`BlockSummary`], and another block needs to interpreted.
    ///
    /// Returns `false` when a valid [`MotionSummary`](crate::machine::MotionSummary) was found and
    /// sent to the [`Gui`] thread using [`Command::Render`].
    fn execute(&mut self) -> bool {
        if self.interrupt.is_some() {
            return false;
        }

        // branch off on if the results are already stored
        let block = match self.total {
            Some(total) => {
                if self.current > total {
                    unreachable!("Current count will never exceed total count.")
                } else if self.current == total {
                    None // end
                } else {
                    Some(&self.summaries[self.current]) // send stored summary
                }
            }
            None => match self.interpreter.execute() {
                Ok(res) => {
                    if let Some(summary) = res {
                        self.summaries.push(summary); // this was a new block summary
                        self.summaries.last()
                    } else {
                        None // exhausted
                    }
                }
                Err(e) => {
                    self.error = Some(e);
                    return false;
                }
            },
        };

        match block {
            Some(summary) => {
                let proceed = if let Some(motion) = &summary.motion {
                    // this could fail if the window is closed and the next signal from gui will be
                    // signal::stop
                    let _ = self.proxy.send_event(Command::Render(*motion));
                    false
                } else {
                    match summary.mcode {
                        Some(MCode::Stop) => {
                            self.interrupt = Some(Interrupt::Stop);
                            false
                        }
                        Some(MCode::OptionalStop) => {
                            self.interrupt = Some(Interrupt::OptionalStop);
                            false
                        }
                        Some(MCode::End) => {
                            self.interrupt = Some(Interrupt::End);
                            false
                        }
                        Some(_) => true,
                        None => true,
                    }
                };

                self.current += 1;

                proceed
            }

            None => {
                self.total = Some(self.current);
                self.interrupt = Some(Interrupt::End);
                false // end of blocks
            }
        }
    }

    /// Prepares individual sections of the terminal screen,
    /// and draws the current state of [`Tui`] in the said sections.
    fn draw(&self, frame: &mut Frame) {
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
        frame.render_widget(self.summary_widget(), left_chunks[1]);
        frame.render_widget(self.preview_widget(), top_chunks[1]);
        frame.render_widget(self.machine_widget(), main_chunks[1]);
        frame.render_widget(self.modes_widget(), main_chunks[2]);
        frame.render_widget(self.keys_widget(), main_chunks[3]);

        // present error, if any
        if let Some(e) = &self.error {
            let mut error_lines = vec![e.to_string(), "\n".to_string()];
            let mut source = e.source();

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

    /// Generates a styled [`Paragraph`] with **program title**.
    fn title_text(&self) -> Paragraph<'_> {
        Paragraph::new("GSim-rs")
            .style(THEME.title)
            .block(Block::default().padding(Padding::symmetric(1, 1)))
            .centered()
    }

    /// Generates a styled [`Paragraph`] using the [`BlockSummary`] for current block.
    fn summary_widget(&self) -> Paragraph<'_> {
        let lines = if let Some(interrupt) = &self.interrupt {
            vec![
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
            ]
        } else {
            let summary = self
                .summaries
                .get(self.current.saturating_sub(1))
                .expect("App module has pushed the text descriptions for the current block.");

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

    /// Generates a styled [`Paragraph`] with loaded [`Source`].
    /// One line of context is also provided in the preview.
    fn preview_widget(&self) -> Paragraph<'_> {
        // preallocate for max look ahead plus current and context line.
        let mut lines = Vec::with_capacity(MAX_PREVIEW_AHEAD + 2);

        // although we are subtracting 2 this will only start
        let mut current = self.current.saturating_sub(2);

        while let Some(line) = self.interpreter.get_line(current)
            && current < self.current + MAX_PREVIEW_AHEAD
        {
            // do not highlight current line if Interrupt::Start is detected
            if current == self.current.saturating_sub(1)
                && !matches!(self.interrupt, Some(Interrupt::Start))
            {
                lines.push(Line::styled(
                    line,
                    Style::default().bg(Color::White).fg(Color::Black),
                ))
            } else {
                lines.push(Line::from(line))
            }

            current += 1;
        }

        Paragraph::new(lines).block(
            Block::default()
                .padding(Padding::symmetric(3, 1))
                .borders(Borders::TOP | Borders::LEFT)
                .title(Line::styled("Preview", THEME.block_title).centered())
                .style(THEME.root),
        )
    }

    /// Generates a styled [`Paragraph`] showing the state of [`Machine`].
    fn machine_widget(&self) -> Paragraph<'_> {
        let machine = self.interpreter.machine();
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
                    .title(Line::styled("Machine State", THEME.block_title).centered())
                    .style(THEME.root),
            )
            .centered()
    }

    /// Generates a styled [`Paragraph`] showing the active state of [`Tui`] & [`Gui`].
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

        Paragraph::new(Line::from(modes))
            .block(
                Block::default()
                    .padding(Padding::symmetric(3, 1))
                    .borders(Borders::TOP)
                    .title(Line::styled("Active Modes", THEME.block_title).centered())
                    .style(THEME.root),
            )
            .centered()
    }

    /// Generates a styled [`Paragraph`] with **possible keys inputs**.
    ///
    /// ## Reference
    /// [Github](https://github.com/ratatui/ratatui/blob/main/examples/apps/demo2/src/app.rs)
    fn keys_widget(&self) -> Paragraph<'_> {
        let mut spans1 = vec![
            Span::styled("  Q  ", THEME.key),
            Span::styled(" Quit ", THEME.key_desc),
            Span::styled("  v  ", THEME.key),
            Span::styled(" Switch View ", THEME.key_desc),
            Span::styled("  1  ", THEME.key),
            Span::styled(" Toggle Single ", THEME.key_desc),
        ];

        if self.single && self.interrupt.is_none() {
            spans1.push(Span::styled("  n  ", THEME.key));
            spans1.push(Span::styled(" Next Block ", THEME.key_desc));
        }

        let spans2 = vec![
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

/// Creates a centered [`Rect`] using up a supplied percentages in X and Y.
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

/// Represents styling for each section of the [`Tui`].
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
