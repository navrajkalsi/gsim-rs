use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph},
};

use crate::app::{App, View};

pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(92), Constraint::Percentage(8)])
        .split(frame.area());

    // in case of animating, make the ui just get the main info,
    // and call the ui reccursively to fulfil the entire move before moving back to app
    //
    // split bottom into 3 sections:
    // name of app
    // available commands
    // gcode groups active

    let title = get_title();
    // print the present state

    let keys = get_keys(app);

    let main = get_main(app);

    let lines = get_preview(app);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(chunks[0]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(10), Constraint::Percentage(90)])
        .split(chunks[1]);

    frame.render_widget(main, top_chunks[0]);
    frame.render_widget(lines, top_chunks[1]);

    frame.render_widget(title, bottom_chunks[0]);
    frame.render_widget(keys, bottom_chunks[1]);
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
        ": quit".into(),
    ];

    let views = match app.view {
        View::Text => vec![
            " / ".into(),
            Span::styled("p", Style::default().fg(Color::Yellow)),
            ": top plane view".into(),
            " / ".into(),
            Span::styled("i", Style::default().fg(Color::Yellow)),
            ": isometric view".into(),
        ],
        View::Plane => vec![
            " / ".into(),
            Span::styled("t", Style::default().fg(Color::Yellow)),
            ": text view".into(),
            " / ".into(),
            Span::styled("i", Style::default().fg(Color::Yellow)),
            ": isometric view".into(),
        ],
        View::Isometric => vec![
            " / ".into(),
            Span::styled("t", Style::default().fg(Color::Yellow)),
            ": text view".into(),
            " / ".into(),
            Span::styled("p", Style::default().fg(Color::Yellow)),
            ": top plane view".into(),
        ],
    };

    for view in views {
        keys.push(view);
    }

    match app.single {
        true => {
            keys.push(" / ".into());
            keys.push(Span::styled("n", Style::default().fg(Color::Yellow)));
            keys.push(": next block".into());
            keys.push(" / ".into());
            keys.push(Span::styled("s", Style::default().fg(Color::Yellow)));
            keys.push(": single block off".into());
        }

        false => {
            keys.push(" / ".into());
            keys.push(Span::styled("s", Style::default().fg(Color::Yellow)));
            keys.push(": single block on".into());
        }
    };

    Paragraph::new(Line::from(keys))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .title(Line::styled("Commands", Style::default().fg(Color::Yellow)).centered())
                .style(Style::default()),
        )
        .centered()
}

/// Returns a styled [`Paragraph`] with **loaded source**.
fn get_preview(app: &App) -> Paragraph<'_> {
    let mut lines = vec![];

    let mut current = if app.current > 0 { app.current - 1 } else { 0 };

    while let Some(line) = app.preview.get(current) {
        if current == app.current {
            lines.push(Line::styled(
                line.as_str(),
                Style::default().bg(Color::White).fg(Color::Black),
            ))
        } else {
            lines.push(Line::from(line.as_str()))
        }

        current += 1;
    }

    Paragraph::new(Text::from(lines))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .padding(Padding::horizontal(2))
                .borders(Borders::TOP | Borders::LEFT)
                .title(Line::styled("Preview", Style::default().fg(Color::Yellow)).centered())
                .style(Style::default()),
        )
}

/// Returns a styled [`Paragraph`] with **main section**.
fn get_main(app: &App) -> Paragraph<'_> {
    Paragraph::new("")
        .style(Style::default())
        .block(Block::default())
}
