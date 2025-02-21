use crate::Message;
use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Padding, Paragraph};
use std::cmp::min;

pub struct Log<'a> {
    messages: &'a [Message],
    scroll_offset: u16,
}

impl<'a> Log<'a> {
    pub fn new<T>(messages: &'a [Message], scroll_offset: T) -> Self
    where
        T: Into<u16>,
    {
        Self {
            messages,
            scroll_offset: scroll_offset.into(),
        }
    }
}

impl Widget for Log<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // TODO: Fix scroll, don't allow to scroll down to the void
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
                Message::Empty => "".into(),
            })
            .collect_vec();

        let maximum_offset = (logs.len() as u16).saturating_sub(log_block.inner(area).height);
        let scroll_offset = min(self.scroll_offset, maximum_offset);

        Paragraph::new(logs)
            .scroll((scroll_offset, 0))
            .block(log_block)
            .render(area, buf);
    }
}
