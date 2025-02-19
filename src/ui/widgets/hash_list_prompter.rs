use std::path::Path;
use color_eyre::Report;
use crate::ui::vert_center;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};

pub struct HashListPrompt<'a> {
    selected_list: &'a Path,
    error: &'a Option<Report>,
}

impl<'a> HashListPrompt<'a> {
    pub fn new(selected_list: &'a Path, error: &'a Option<Report>) -> Self {
        Self {
            selected_list,
            error,
        }
    }
}

impl Widget for HashListPrompt<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let hash_list_block = Block::bordered()
            .title("MD5 hash list file")
            .title_bottom(
                Line::from(vec![
                    "Press <n> to select file, or <v> to get ".into(),
                    "absolute".bold(),
                    " path from clipboard".into(),
                ])
                .centered(),
            )
            .padding(Padding::uniform(1));

        let path_str = self.selected_list.to_string_lossy();

        let mut lines = vec![Line::from(vec![
            "MD5 hash list: ".into(),
            if path_str != "" {
                path_str.into()
            } else {
                "Not selected yet".bold()
            },
        ])];

        if let Some(e) = &self.error {
            lines.push("".into());
            lines.push(Line::from(vec![
                Span::from("Error: ").fg(Color::LightRed),
                e.to_string().into(),
            ]));
        }

        let vert_center_area = vert_center(hash_list_block.inner(area), lines.len() as u16);

        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .render(vert_center_area, buf);

        hash_list_block.render(area, buf);
    }
}
