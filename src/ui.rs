use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line as TextLine, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph},
};

use crate::{
    app::{App, Interrupt, View},
    describe::Describe,
    machine::{CircularDirection, FeedMode, Motion, Plane, Positioning, Unit},
};

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    // Then cut the middle vertical piece into three width-wise pieces
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1] // Return the middle chunk
}

pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(92), Constraint::Percentage(8)])
        .split(frame.area());

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(chunks[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(20),
            Constraint::Percentage(10),
        ])
        .split(top_chunks[1]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(10), Constraint::Percentage(90)])
        .split(chunks[1]);

    let preview = get_preview(app);
    let machine = get_machine(app);
    let active = get_active(app);
    let title = get_title();
    let keys = get_keys(app);

    frame.render_widget(preview, right_chunks[0]);
    frame.render_widget(machine, right_chunks[1]);
    frame.render_widget(active, right_chunks[2]);
    frame.render_widget(title, bottom_chunks[0]);
    frame.render_widget(keys, bottom_chunks[1]);

    render_main(app, frame, top_chunks[0]);

    // deal with error before quitting
    if let Some(err) = &app.error {
        let popup = Paragraph::new(err.describe().desc().to_string()).block(
            Block::default()
                .title(err.describe().title().to_string())
                .borders(Borders::NONE)
                .style(Style::default().bg(Color::DarkGray)),
        );

        let area = centered_rect(60, 25, frame.area());
        frame.render_widget(popup, area);
    }
}

/// Returns a styled [`Paragraph`] with **program title**.
fn get_title<'a>() -> Paragraph<'a> {
    Paragraph::new("GSim-RS")
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::RIGHT)
                .style(Style::default()),
        )
        .centered()
}

/// Returns a styled [`Paragraph`] with **possible keys inputs**.
fn get_keys(app: &App) -> Paragraph<'_> {
    let mut keys = vec![
        Span::styled("Q", Style::default().fg(Color::Yellow)),
        ": quit / ".into(),
        Span::styled("v", Style::default().fg(Color::Yellow)),
        ": toggle view / ".into(),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        ": toggle single".into(),
    ];

    if app.single && app.interrupt.is_none() {
        keys.push(" / ".into());
        keys.push(Span::styled("n", Style::default().fg(Color::Yellow)));
        keys.push(": next block".into());
    }

    Paragraph::new(TextLine::from(keys))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT)
                .title(TextLine::styled("Commands", Style::default().fg(Color::Yellow)).centered())
                .style(Style::default()),
        )
        .centered()
}

/// Returns a styled [`Paragraph`] with **loaded source**.
/// One line of context is also provided in the preview.
fn get_preview(app: &App) -> Paragraph<'_> {
    let mut lines = vec![];

    let mut current = if app.current > 0 { app.current - 1 } else { 0 };

    while let Some(line) = app.interpreter.get_line(current) {
        if current == app.current {
            lines.push(TextLine::styled(
                line,
                Style::default().bg(Color::White).fg(Color::Black),
            ))
        } else {
            lines.push(TextLine::from(line))
        }

        current += 1;
    }

    Paragraph::new(Text::from(lines))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .padding(Padding::horizontal(2))
                .borders(Borders::TOP | Borders::LEFT)
                .title(TextLine::styled("Preview", Style::default().fg(Color::Yellow)).centered())
                .style(Style::default()),
        )
}

/// Returns a styled [`Paragraph`] with **active options**.
fn get_active(app: &App) -> Paragraph<'_> {
    let mut active = vec![];
    let style = Style::default()
        .fg(Color::LightYellow)
        .add_modifier(Modifier::BOLD);

    if let Some(interrupt) = &app.interrupt {
        active.push(match interrupt {
            Interrupt::Start => Span::styled("START INTERRUPT", style),

            Interrupt::Stop => Span::styled("STOP INTERRUPT", style),

            Interrupt::OptionalStop => Span::styled("OPTIONAL STOP INTERRUPT", style),

            Interrupt::End => Span::styled("END INTERRUPT", style),
        });
        active.push(Span::from(" | "));
    }

    active.push(match app.view {
        View::Top => Span::styled("TOP", style),
        View::Isometric => Span::styled("ISOMETRIC", style),
    });

    if app.single {
        active.push(Span::from(" | "));
        active.push(Span::styled("SINGLE", style));
    }

    Paragraph::new(TextLine::from(active))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT)
                .title(TextLine::styled("Active", Style::default().fg(Color::Yellow)).centered())
                .style(Style::default()),
        )
        .centered()
}

/// Returns a styled [`Paragraph`] with the **current machine state**.
fn get_machine(app: &App) -> Paragraph<'_> {
    let machine = app.interpreter.machine();
    let unit = Span::from(match machine.units() {
        Unit::Imperial => "in",
        Unit::Metric => "mm",
    });

    let mut line1 = vec![
        Span::styled(
            "X",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        ),
        ": ".into(),
        machine.pos().x().to_string().into(),
        unit.clone(),
        " | ".into(),
        Span::styled(
            "Y",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        ),
        ": ".into(),
        machine.pos().y().to_string().into(),
        unit.clone(),
        " | ".into(),
        Span::styled(
            "Z",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        ),
        ": ".into(),
        machine.pos().z().to_string().into(),
        unit.clone(),
    ];
    // append feed if available
    if let Some(feed) = machine.feed().clone() {
        line1.extend(
            vec![
                " | ".into(),
                Span::styled(
                    "F",
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                ),
                ": ".into(),
                feed.to_string().into(),
                unit,
                Span::from(match machine.feed_mode() {
                    FeedMode::PerMinute => "/min",
                    FeedMode::PerRev => "/rev",
                }),
            ]
            .into_iter(),
        );
    }

    let line2 = vec![
        Span::styled(
            match machine.motion() {
                Motion::Rapid => "RAPID",
                Motion::Feed => "FEED",
                Motion::Arc(CircularDirection::Clockwise) => "CLOCKWISE",
                Motion::Arc(CircularDirection::CounterClockwise) => "ANTICLOCKWISE",
            },
            Style::default().fg(Color::Blue),
        ),
        " | ".into(),
        Span::styled(
            match machine.plane() {
                Plane::XY => "XY",
                Plane::XZ => "XZ",
                Plane::YZ => "YZ",
            },
            Style::default().fg(Color::Blue),
        ),
        " | ".into(),
        Span::styled(
            match machine.positioning() {
                Positioning::Absolute => "ABSOLUTE",
                Positioning::Incremental => "INCREMENTAL",
            },
            Style::default().fg(Color::Blue),
        ),
        " | ".into(),
        Span::styled(
            match machine.code_units() {
                Unit::Imperial => "IMPERIAL",
                Unit::Metric => "METRIC",
            },
            Style::default().fg(Color::Blue),
        ),
    ];

    Paragraph::new(Text::from(vec![line1.into(), line2.into()]))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::LEFT)
                .title(
                    TextLine::styled("Machine State", Style::default().fg(Color::Yellow))
                        .centered(),
                )
                .style(Style::default()),
        )
        .centered()
}

/// Generates and renders the main section of the app
fn render_main(app: &App, frame: &mut Frame, rect: Rect) {
    if let Some(interrupt) = &app.interrupt {
        let style = Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD);

        let mut interrupt = vec![match interrupt {
            Interrupt::Start => Span::styled("START", style),
            Interrupt::Stop => Span::styled("STOP", style),
            Interrupt::OptionalStop => Span::styled("OPTIONAL STOP", style),
            Interrupt::End => Span::styled("END", style),
        }];

        interrupt.push(" interrupt detected.".into());

        let command = vec![
            "Press ".into(),
            Span::styled("Enter", style),
            " to remove the interrupt.".into(),
        ];

        return frame.render_widget(
            Paragraph::new(Text::from(vec![interrupt.into(), command.into()]))
                .block(Block::default().style(Style::default()))
                .centered(),
            rect,
        );
    }

    render_text_view(app, frame, rect)
}

/// Generates and renders the text view of the app.
fn render_text_view(app: &App, frame: &mut Frame, rect: Rect) {
    let summary = app
        .summary
        .get(app.current.saturating_sub(1))
        .expect("App module has pushed the text descriptions for the current block.");

    let mut lines = vec![];

    if !summary.gcodes.is_empty() {
        lines.push(TextLine::styled(
            "GCODE(s):",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        for gcode in &summary.gcodes {
            lines.push(TextLine::styled(gcode, Style::default()));
        }
        lines.push(TextLine::from(""));
    };

    if let Some(mcode) = summary.mcode.clone() {
        lines.push(TextLine::styled(
            "MCODE:",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(TextLine::styled(mcode, Style::default()));
        lines.push(TextLine::from(""));
    }

    if !summary.codes.is_empty() {
        lines.push(TextLine::styled(
            "Other CODE(s):",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        for code in &summary.codes {
            lines.push(TextLine::styled(code, Style::default()));
        }
    };

    frame.render_widget(Paragraph::new(lines), rect);
}
