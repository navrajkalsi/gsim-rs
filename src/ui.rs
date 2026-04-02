use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;

pub fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(90), Constraint::Percentage(10)])
        .split(frame.area());

    // split bottom into 3 sections:
    // name of app
    // available commands
    // gcode groups active

    let title_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default());

    let title = Paragraph::new(Text::styled("GSim-RS", Style::default().fg(Color::Green)))
        .block(title_block);

    // print the present state

    let keys = {
        Span::styled("(Q): quit", Style::default().fg(Color::Red))
        // Span::styled( "(t): text view", Style::default().fg(Color::Red)),
        // Span::styled( "(p): top plane view", Style::default().fg(Color::Red)),
        // Span::styled( "(i): isometric view", Style::default().fg(Color::Red)),
        // Span::styled( "(s): single block", Style::default().fg(Color::Red)),
    };

    let keys = Paragraph::new(Line::from(keys)).block(Block::default().borders(Borders::ALL));

    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(10), Constraint::Percentage(90)])
        .split(chunks[1]);

    frame.render_widget(title, footer_chunks[0]);
    frame.render_widget(keys, footer_chunks[1]);
}
