use crate::ui::vert_center;
use crate::Status as StatusStorage;
use parking_lot::RwLock;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Gauge, Padding, Paragraph};
use std::sync::Arc;

pub struct Status {
    running: bool,
    entered_empty: bool,
    hash_status: Arc<RwLock<StatusStorage>>,
    total_hash: usize,
}

impl Status {
    pub fn new(
        status: Arc<RwLock<StatusStorage>>,
        running: bool,
        total_hash: usize,
        entered_empty: bool,
    ) -> Self {
        Self {
            running,
            hash_status: status,
            total_hash,
            entered_empty,
        }
    }

    fn render_running(self, area: Rect, buf: &mut Buffer) {
        let status = { self.hash_status.read().clone() };

        let [stat_area, progress_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        let colored_hash = if status.file_hash == status.expected_hash {
            Span::styled(status.file_hash, Style::default().fg(Color::LightGreen))
        } else {
            Span::styled(status.file_hash, Style::default().fg(Color::LightRed))
        };

        let status_line = vec![
            format!("File name: {}", status.filename).into(),
            Line::from(vec!["File hash: ".into(), colored_hash]),
            format!("Expected hash: {}", status.expected_hash).into(),
            format!("Correct: {}", status.correct_num).into(),
            format!("Incorrect: {}", status.incorrect_num).into(),
            format!("Error: {}", status.error_num).into(),
        ];

        Paragraph::new(status_line).render(stat_area, buf);

        Gauge::default()
            .use_unicode(true)
            .ratio((status.correct_num + status.incorrect_num) as f64 / self.total_hash as f64)
            .render(progress_area, buf);
    }

    fn render_stopped(self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            "Hasher not running".bold().into(),
            Line::from(vec![
                "Press <Enter> ".into(),
                if self.entered_empty {
                    Span::from("after selecting").bold().fg(Color::LightRed)
                } else {
                    "after selecting".into()
                },
                " a hash list to run".into(),
            ]),
        ];

        let vert_cen_area = vert_center(area, lines.len() as u16);

        Paragraph::new(lines).centered().render(vert_cen_area, buf);
    }
}

impl Widget for Status {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let status_block = Block::bordered()
            .title("Status")
            .padding(Padding::uniform(1));

        let inner_area = status_block.inner(area);

        if self.running {
            self.render_running(inner_area, buf);
        } else {
            self.render_stopped(inner_area, buf);
        }

        status_block.render(area, buf);
    }
}
