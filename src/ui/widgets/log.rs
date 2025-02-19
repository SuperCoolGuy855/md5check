use crate::Message;
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Padding, Paragraph};

pub struct Log<'a> {
    messages: &'a [Message],
}

impl<'a> Log<'a> {
    pub fn new(messages: &'a [Message]) -> Self {
        Self { messages }
    }
}

impl Widget for Log<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // TODO: Scrolling
        let log_block = Block::bordered().padding(Padding::uniform(1)).title("Log");

        let logs = self
            .messages
            .iter()
            .rev()
            .map(|x| match x {
                Message::Incorrect(s) => Line::from(vec![
                    Span::from("Incorrect: ").style(Color::Yellow),
                    s.into(),
                ]),
                Message::Error(e) => Line::from(vec![
                    Span::from("Error: ").style(Color::LightRed),
                    e.to_string().into(),
                ]),
                Message::Completed(duration) => {
                    format!("Completed in {duration:?}! Please close with <Ctrl+c>")
                        .bold()
                        .into()
                }
            })
            .collect_vec();

        Paragraph::new(logs).block(log_block).render(area, buf);
    }
}
