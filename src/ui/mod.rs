pub mod widgets;

use crate::{
    hash::{hash_list_parser, prepare_hashing},
    Message, Setting, Status,
};

use color_eyre::eyre::{eyre, Context, Report, Result};
use crossbeam::channel;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use parking_lot::RwLock;
use ratatui::layout::Flex;
use ratatui::prelude::*;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget, DefaultTerminal, Frame};
use ratatui_explorer::{FileExplorer, Theme};
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
                let message_rx = self
                    .message_rx
                    .clone()
                    .expect("If self.running, then self.message_rx exists");
                let messages = message_rx.try_iter();
                self.messages.extend(messages);
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
        if event::poll(Duration::from_micros(12500))? {
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
                self.get_path_from_clipboard()?;
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

    fn get_path_from_clipboard(&mut self) -> Result<()> {
        let text = {
            let mut clipboard = arboard::Clipboard::new()?;
            match clipboard.get_text() {
                Ok(x) => x,
                Err(e) => {
                    self.error = Some(e.into());
                    return Ok(());
                }
            }
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

        Ok(())
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

        // Right window (Hash status)
        widgets::Status::new(
            self.hash_status.clone(),
            self.running,
            self.total_hash,
            self.entered_empty,
        )
        .render(right_area, buf);

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
