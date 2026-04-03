use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, View};

pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(92), Constraint::Percentage(8)])
        .split(frame.area());

    // split bottom into 3 sections:
    // name of app
    // available commands
    // gcode groups active

    let title = get_title();
    // print the present state

    let keys = get_keys(app);

    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(10), Constraint::Percentage(90)])
        .split(chunks[1]);

    frame.render_widget(title, footer_chunks[0]);
    frame.render_widget(keys, footer_chunks[1]);
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
fn get_keys<'a>(app: &App) -> Paragraph<'a> {
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
        }

        false => {
            keys.push(" / ".into());
            keys.push(Span::styled("s", Style::default().fg(Color::Yellow)));
            keys.push(": single block".into());
        }
    };

    Paragraph::new(Line::from(keys))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default()),
        )
        .centered()
}
