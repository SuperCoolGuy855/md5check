use crate::Setting as SettingStorage;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Padding, Paragraph};

fn boolean_str_color(x: bool) -> Span<'static> {
    if x {
        Span::from("true".to_string()).fg(Color::LightGreen)
    } else {
        Span::from("false".to_string()).fg(Color::LightRed)
    }
}
pub struct Setting<'a> {
    settings: &'a SettingStorage,
}

impl<'a> Setting<'a> {
    pub fn new(settings: &'a SettingStorage) -> Self {
        Self {
            settings,
        }
    }
}

impl Widget for Setting<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let setting_block = Block::bordered()
            .title("Settings")
            .padding(Padding::uniform(1));

        let setting_lines = vec![
            Line::from(vec![
                "Parallel: ".into(),
                boolean_str_color(self.settings.parallel),
            ]),
            Line::from(vec!["Sort: ".into(), boolean_str_color(self.settings.sort)]),
            format!(
                "Block size: {} ({})",
                self.settings.block_size,
                size::Size::from_bytes(self.settings.block_size)
            )
            .into(),
        ];

        let tooltip_lines = vec![
            "Press <p> to toggle".into(),
            "      <s>          ".into(),
            "Press ← to decrease, → to increase".into(),
            "Press <Ctrl> for 1 MiB, <Shift> for 1 GiB".into(),
        ];

        let [setting_area, tooltip_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Fill(1)])
                .areas(setting_block.inner(area));

        Paragraph::new(setting_lines).render(setting_area, buf);

        Paragraph::new(tooltip_lines).render(tooltip_area, buf);

        setting_block.render(area, buf);
    }
}
