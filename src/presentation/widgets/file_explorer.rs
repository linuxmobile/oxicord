use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, StatefulWidget, Widget},
};
use std::{
    cmp::Ordering,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub enum FileExplorerAction {
    SelectFile(PathBuf),
    Close,
    None,
}

#[derive(Debug, Clone)]
struct FileEntry {
    path: PathBuf,
    is_dir: bool,
    name: String,
}

pub struct FileExplorerComponent {
    current_dir: PathBuf,
    entries: Vec<FileEntry>,
    state: ListState,
    show_hidden: bool,
}

impl FileExplorerComponent {
    pub fn new() -> Self {
        let root = env::var("HOME").map_or_else(|_| PathBuf::from("/"), PathBuf::from);
        let mut component = Self {
            current_dir: root.clone(),
            entries: Vec::new(),
            state: ListState::default(),
            show_hidden: false,
        };
        component.load_entries(&root);
        component
    }

    pub fn load_entries(&mut self, path: &Path) {
        let selected_name = self.selected_entry().map(|e| e.name.clone());

        self.entries.clear();

        if let Some(parent) = path.parent() {
            self.entries.push(FileEntry {
                path: parent.to_path_buf(),
                is_dir: true,
                name: "..".to_string(),
            });
        }

        if let Ok(read_dir) = fs::read_dir(path) {
            let mut entries: Vec<FileEntry> = read_dir
                .filter_map(Result::ok)
                .filter(|entry| {
                    if self.show_hidden {
                        return true;
                    }
                    !entry
                        .path()
                        .file_name()
                        .map(|s| s.to_string_lossy().starts_with('.'))
                        .unwrap_or(false)
                })
                .map(|entry| {
                    let path = entry.path();
                    let is_dir = path.is_dir();
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    FileEntry { path, is_dir, name }
                })
                .collect();

            entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => a.name.cmp(&b.name),
            });

            self.entries.extend(entries);
        }

        if let Some(name) = selected_name {
            if let Some(idx) = self.entries.iter().position(|e| e.name == name) {
                self.state.select(Some(idx));
            } else {
                self.state.select(Some(0));
            }
        } else if self.entries.is_empty() {
            self.state.select(None);
        } else {
            self.state.select(Some(0));
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        let path = self.current_dir.clone();
        self.load_entries(&path);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FileExplorerAction {
        match key.code {
            KeyCode::Esc => FileExplorerAction::Close,
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
                FileExplorerAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
                FileExplorerAction::None
            }
            KeyCode::Char('h') => {
                let parent = self
                    .current_dir
                    .parent()
                    .map(PathBuf::from)
                    .unwrap_or(self.current_dir.clone());
                self.current_dir = parent;
                let path = self.current_dir.clone();
                self.load_entries(&path);
                FileExplorerAction::None
            }
            KeyCode::Char('l') => {
                if let Some(selected) = self.selected_entry() {
                    if selected.name == ".." {
                        FileExplorerAction::None
                    } else if selected.is_dir {
                        self.current_dir = selected.path.clone();
                        let path = self.current_dir.clone();
                        self.load_entries(&path);
                        FileExplorerAction::None
                    } else {
                        FileExplorerAction::SelectFile(selected.path.clone())
                    }
                } else {
                    FileExplorerAction::None
                }
            }
            KeyCode::Enter => {
                if let Some(selected) = self.selected_entry() {
                    if selected.name == ".." {
                        let parent = self
                            .current_dir
                            .parent()
                            .map(PathBuf::from)
                            .unwrap_or(self.current_dir.clone());
                        self.current_dir = parent;
                        let path = self.current_dir.clone();
                        self.load_entries(&path);
                        FileExplorerAction::None
                    } else if selected.is_dir {
                        self.current_dir = selected.path.clone();
                        let path = self.current_dir.clone();
                        self.load_entries(&path);
                        FileExplorerAction::None
                    } else {
                        FileExplorerAction::SelectFile(selected.path.clone())
                    }
                } else {
                    FileExplorerAction::None
                }
            }
            _ => FileExplorerAction::None,
        }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.entries.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.entries.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn selected_entry(&self) -> Option<&FileEntry> {
        self.state.selected().and_then(|i| self.entries.get(i))
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let title = if self.show_hidden {
            " File Explorer (H) - Select Attachment "
        } else {
            " File Explorer - Select Attachment "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_style(Style::default().add_modifier(Modifier::BOLD));

        let inner_area = block.inner(area);

        let layout =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner_area);
        let list_area = layout[0];
        let footer_area = layout[1];

        block.render(area, buf);

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|entry| {
                let icon = if entry.is_dir { "  " } else { "  " };
                let content = format!("{}{}", icon, entry.name);
                ListItem::new(content).style(Style::default().fg(if entry.is_dir {
                    Color::Blue
                } else {
                    Color::White
                }))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        StatefulWidget::render(list, list_area, buf, &mut self.state);

        let footer = Line::from(vec![
            Span::raw(" [Esc] Cancel | "),
            Span::raw("[Enter] Select/Enter "),
        ])
        .style(Style::default().fg(Color::Gray));

        Widget::render(footer, footer_area, buf);
    }
}

impl Default for FileExplorerComponent {
    fn default() -> Self {
        Self::new()
    }
}
