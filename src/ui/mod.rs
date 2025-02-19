pub mod widgets;

use crate::{
    hash::{hash_list_parser, prepare_hashing},
    Message, Setting, Status,
};

use color_eyre::eyre::{eyre, Context, Report, Result};
use crossbeam::channel;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use itertools::Itertools;
use parking_lot::RwLock;
use ratatui::layout::Flex;
use ratatui::prelude::*;
use ratatui::widgets::{Gauge, Padding, Wrap};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    text::Line,
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};
use ratatui_explorer::{FileExplorer, Theme};
use size::Size;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct App {
    hash_status: Arc<RwLock<Status>>,
    total_hash: usize,
    settings: Setting,
    file_explorer: FileExplorer,
    cwd: PathBuf,
    selected_list: PathBuf,
    selected_idx: usize,
    showing_explorer: bool,
    running: bool,
    message_rx: Option<channel::Receiver<Message>>,
    messages: Vec<Message>,
    entered_empty: bool,
    error: Option<Report>,
    exit: bool,
}

impl Default for App {
    fn default() -> Self {
        let cwd = std::env::current_dir().unwrap(); //TODO: Make this better
        let theme = Theme::default()
            .add_default_title()
            .with_title_bottom(|_| "Press <Enter> to select file. Press <c> to cancel".into());
        let mut file_explorer = FileExplorer::with_theme(theme).unwrap(); //TODO: Make this better
        file_explorer.set_cwd(&cwd).unwrap();

        Self {
            hash_status: Default::default(),
            settings: Default::default(),
            total_hash: 0,
            file_explorer,
            cwd,
            selected_list: PathBuf::new(),
            showing_explorer: false,
            selected_idx: 0,
            running: false,
            entered_empty: false,
            message_rx: None,
            messages: vec![],
            error: None,
            exit: false,
        }
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit {
            if self.running {
                if let Ok(mess) = self
                    .message_rx
                    .clone()
                    .expect("If self.running, then self.message_rx exists")
                    .try_recv()
                {
                    // TODO: Check if need to reset program
                    self.messages.push(mess);
                }
            }
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events().wrap_err("handle events failed")?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_micros(16667))? {
            let event = event::read()?;
            match event {
                Event::Key(key_event)
                    if key_event == KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL) =>
                {
                    self.exit();
                    Ok(())
                }
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press && !self.running => {
                    self.handle_key_event(key_event)
                        .wrap_err_with(|| format!("handling key event failed:\n{key_event:#?}"))
                }
                _ => Ok(()),
            }?;

            if self.showing_explorer {
                self.file_explorer.handle(&event)?;
            }
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        let block_size_adjustment = 1024
            * match key_event.modifiers {
                KeyModifiers::CONTROL => 1024,
                KeyModifiers::SHIFT => 1024 * 1024,
                _ => 1,
            };

        match key_event.code {
            KeyCode::Char('n') if !self.showing_explorer => self.showing_explorer = true,
            KeyCode::Char('v') if !self.showing_explorer => {
                let text = {
                    let mut clipboard = arboard::Clipboard::new()?;
                    clipboard.get_text()?
                };

                let path = PathBuf::from(text.trim_matches('"'));
                if !path.is_absolute() {
                    self.error = Some(eyre!("Path is not absolute: {path:?}"));
                } else if !path.is_file() {
                    self.error = Some(eyre!("Path is not file: {path:?}"));
                } else {
                    self.cwd = path
                        .parent()
                        .expect("Path is a file and is absolute (checked above) so has a parent")
                        .to_path_buf();
                    self.file_explorer.set_cwd(&self.cwd)?;
                    self.selected_list = path;
                    self.error = None;
                }
            }
            KeyCode::Char('c') if self.showing_explorer => {
                self.showing_explorer = false;
                self.file_explorer.set_cwd(&self.cwd)?;
                self.file_explorer.set_selected_idx(self.selected_idx);
            }
            KeyCode::Char('p') if !self.showing_explorer => {
                self.settings.parallel = !self.settings.parallel;
            }
            KeyCode::Char('s') if !self.showing_explorer => {
                self.settings.sort = !self.settings.sort;
            }
            KeyCode::Left if !self.showing_explorer => {
                self.settings.block_size = self
                    .settings
                    .block_size
                    .saturating_sub(block_size_adjustment);
                if self.settings.block_size < 1024 {
                    self.settings.block_size = 1024;
                }
            }
            KeyCode::Right if !self.showing_explorer => {
                self.settings.block_size = self
                    .settings
                    .block_size
                    .saturating_add(block_size_adjustment);
            }
            KeyCode::Enter if self.showing_explorer => {
                let current = self.file_explorer.current();
                if !current.is_dir() {
                    self.showing_explorer = false;
                    self.cwd = self.file_explorer.cwd().clone();
                    self.selected_list = current.path().clone();
                    self.selected_idx = self.file_explorer.selected_idx();
                    self.error = None;
                }
            }
            KeyCode::Enter if self.selected_list.to_string_lossy().is_empty() => {
                self.entered_empty = true
            }
            KeyCode::Enter if !self.selected_list.to_string_lossy().is_empty() => {
                self.pre_run();
            }
            _ => (),
        }

        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn pre_run(&mut self) {
        let res = hash_list_parser(&self.selected_list);
        let hash_list = match res {
            Ok(x) => x,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };

        if let Err(e) = std::env::set_current_dir(&self.cwd) {
            self.error = Some(e.into());
            return;
        }
        self.running = true;
        self.total_hash = hash_list.len();

        let status_clone = Arc::clone(&self.hash_status);
        let settings = self.settings;
        let (tx, rx) = channel::unbounded();
        self.message_rx = Some(rx);

        thread::spawn(move || prepare_hashing(hash_list, &settings, status_clone, tx));
    }
}

fn vert_center(area: Rect, height: u16) -> Rect {
    let [_, vert_centered_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .flex(Flex::Center)
    .areas(area);

    vert_centered_area
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [top_area, bottom_area] = Layout::new(
            Direction::Vertical,
            [Constraint::Percentage(50), Constraint::Fill(1)],
        )
        .areas(area);

        let [left_area, right_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(top_area);

        let status_block = Block::bordered()
            .title("Status")
            .padding(Padding::uniform(1));

        // Hash status window
        if self.running {
            let status = { self.hash_status.read().clone() };

            let [stat_area, progress_area] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(1)])
                    .areas(status_block.inner(right_area));

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
        } else {
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

            let vert_centered_area =
                vert_center(status_block.inner(right_area), lines.len() as u16);

            Paragraph::new(lines)
                .centered()
                .render(vert_centered_area, buf);
        }
        status_block.render(right_area, buf);
        
        // Bottom window (Navigator, prompter, log)
        if self.showing_explorer {
            self.file_explorer.widget().render(bottom_area, buf);
        } else if self.running {
            widgets::Log::new(&self.messages).render(bottom_area, buf);
        } else {
            widgets::HashListPrompt::new(&self.selected_list, &self.error).render(bottom_area, buf);
        }

        // Upper-left window (Setting)
        widgets::Setting::new(&self.settings).render(left_area, buf);
    }
}
