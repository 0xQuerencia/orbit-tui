use std::{
    collections::HashSet,
    env, fs,
    path::PathBuf,
    process::Command,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::metrics::{Metrics, ProcessRow, SocketRow};

pub const TABS: [&str; 6] = ["Dashboard", "Processes", "Ports", "Connections", "Git", "Todo"];

#[derive(Clone, Debug)]
pub struct Todo {
    pub done: bool,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct GitInfo {
    pub inside: bool,
    pub root: String,
    pub branch: String,
    pub changed: Vec<String>,
    pub recent: Vec<String>,
}

impl GitInfo {
    pub fn read() -> Self {
        fn git(args: &[&str]) -> Option<String> {
            let output = Command::new("git").args(args).output().ok()?;
            output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        }
        let Some(root) = git(&["rev-parse", "--show-toplevel"]) else { return Self::default() };
        let branch = git(&["branch", "--show-current"]).unwrap_or_default();
        let changed = git(&["status", "--short", "-uno"]).unwrap_or_default()
            .lines().map(str::to_owned).collect();
        let recent = git(&["log", "-5", "--oneline"]).unwrap_or_default()
            .lines().map(str::to_owned).collect();
        Self { inside: true, root, branch, changed, recent }
    }
}

pub struct App {
    pub metrics: Metrics,
    pub tab: usize,
    pub selected: usize,
    pub help: bool,
    pub quit: bool,
    pub searching: bool,
    pub search: String,
    pub adding: Option<String>,
    pub confirm_kill: Option<(u32, String)>,
    pub status: String,
    pub theme: usize,
    pub pinned: HashSet<u32>,
    pub todos: Vec<Todo>,
    pub git: GitInfo,
}

impl App {
    pub fn new() -> Self {
        Self {
            metrics: Metrics::new(),
            tab: 0,
            selected: 0,
            help: false,
            quit: false,
            searching: false,
            search: String::new(),
            adding: None,
            confirm_kill: None,
            status: String::new(),
            theme: 0,
            pinned: HashSet::new(),
            todos: load_todos(),
            git: GitInfo::read(),
        }
    }

    pub fn tick(&mut self) {
        self.metrics.refresh();
        if self.metrics.tick % 30 == 0 && (self.tab == 0 || self.tab == 4) {
            self.git = GitInfo::read();
        }
        self.clamp_selection();
    }

    pub fn filtered_processes(&self) -> Vec<&ProcessRow> {
        let query = self.search.to_lowercase();
        let mut rows: Vec<_> = self.metrics.processes.iter()
            .filter(|p| query.is_empty() || p.name.to_lowercase().contains(&query) || p.pid.to_string().contains(&query))
            .collect();
        rows.sort_by(|a, b| {
            let a_pin = self.pinned.contains(&a.pid);
            let b_pin = self.pinned.contains(&b.pid);
            b_pin.cmp(&a_pin).then_with(|| b.cpu.total_cmp(&a.cpu))
        });
        rows
    }

    pub fn filtered_sockets(&self) -> Vec<&SocketRow> {
        let query = self.search.to_lowercase();
        self.metrics.sockets.iter()
            .filter(|s| self.tab == 3 || s.state == "LISTEN")
            .filter(|s| {
                query.is_empty() || s.local.to_lowercase().contains(&query) ||
                s.remote.to_lowercase().contains(&query) ||
                s.process.to_lowercase().contains(&query) ||
                s.pid.map(|p| p.to_string().contains(&query)).unwrap_or(false)
            })
            .collect()
    }

    fn list_len(&self) -> usize {
        match self.tab {
            1 => self.filtered_processes().len(),
            2 | 3 => self.filtered_sockets().len(),
            5 => self.todos.len(),
            _ => 0,
        }
    }

    fn clamp_selection(&mut self) {
        self.selected = self.selected.min(self.list_len().saturating_sub(1));
    }

    fn switch_tab(&mut self, tab: usize) {
        self.tab = tab % TABS.len();
        self.selected = 0;
        self.search.clear();
        self.searching = false;
        self.status.clear();
        if self.tab == 4 { self.git = GitInfo::read(); }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }

        if self.confirm_kill.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.kill_confirmed(),
                KeyCode::Char('n') | KeyCode::Esc => self.confirm_kill = None,
                _ => {}
            }
            return;
        }

        if let Some(ref mut input) = self.adding {
            match key.code {
                KeyCode::Enter => {
                    let value = input.trim().to_owned();
                    self.adding = None;
                    if !value.is_empty() {
                        self.todos.push(Todo { done: false, text: value });
                        self.save();
                    }
                }
                KeyCode::Esc => self.adding = None,
                KeyCode::Backspace => { input.pop(); }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if input.chars().count() < 200 { input.push(c); }
                }
                _ => {}
            }
            return;
        }

        if self.searching {
            match key.code {
                KeyCode::Enter => self.searching = false,
                KeyCode::Esc => { self.searching = false; self.search.clear(); self.selected = 0; },
                KeyCode::Backspace => { self.search.pop(); self.selected = 0; }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if self.search.chars().count() < 100 { self.search.push(c); }
                    self.selected = 0;
                }
                _ => {}
            }
            return;
        }

        if self.help {
            self.help = false;
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('/') if matches!(self.tab, 1..=3) => { self.searching = true; self.selected = 0; },
            KeyCode::Char(c @ '1'..='6') => self.switch_tab(c as usize - '1' as usize),
            KeyCode::Tab | KeyCode::Right => self.switch_tab(self.tab + 1),
            KeyCode::BackTab | KeyCode::Left => self.switch_tab((self.tab + TABS.len() - 1) % TABS.len()),
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = self.selected.saturating_add(1).min(self.list_len().saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('r') => {
                self.tick();
                self.git = GitInfo::read();
                self.status = "Refreshed".to_owned();
            }
            KeyCode::Char('w') => {
                self.theme = (self.theme + 1) % 2;
                self.status = if self.theme == 0 { "Theme: Orbit".to_owned() } else { "Theme: Catppuccin".to_owned() };
            }
            KeyCode::Char('p') if self.tab == 1 => {
                if let Some(pid) = self.filtered_processes().get(self.selected).map(|p| p.pid) {
                    if !self.pinned.insert(pid) { self.pinned.remove(&pid); }
                    self.status = format!("Toggled pin for PID {pid}");
                    self.selected = 0;
                }
            }
            KeyCode::Char('a') if self.tab == 0 || self.tab == 5 => self.adding = Some(String::new()),
            KeyCode::Char('x') if self.tab == 5 => self.toggle_todo(),
            KeyCode::Char('d') if self.tab == 5 => self.delete_todo(),
            KeyCode::Enter => self.handle_enter(),
            _ => {}
        }
        self.clamp_selection();
    }

    fn handle_enter(&mut self) {
        if self.tab == 5 {
            self.toggle_todo();
            return;
        }
        let target = match self.tab {
            1 => self.filtered_processes().get(self.selected)
                .map(|p| (p.pid, p.name.clone())),
            2 => self.filtered_sockets().get(self.selected)
                .and_then(|s| s.pid.map(|pid| (pid, s.process.clone()))),
            _ => None,
        };
        if let Some((pid, name)) = target {
            if pid <= 1 || pid == std::process::id() {
                self.status = format!("Refusing to signal protected PID {pid}");
            } else {
                self.confirm_kill = Some((pid, name));
            }
        } else if self.tab == 2 {
            self.status = "No accessible process owner for this socket".to_owned();
        }
    }

    fn kill_confirmed(&mut self) {
        let Some((pid, _)) = self.confirm_kill.take() else { return };
        if pid <= 1 || pid == std::process::id() { return; }
        // The user must confirm before a SIGTERM is sent.
        let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        self.status = if result == 0 {
            format!("Sent SIGTERM to PID {pid}")
        } else {
            format!("PID {pid}: {}", std::io::Error::last_os_error())
        };
    }

    fn toggle_todo(&mut self) {
        if let Some(todo) = self.todos.get_mut(self.selected) {
            todo.done = !todo.done;
            self.save();
        }
    }

    fn delete_todo(&mut self) {
        if self.selected < self.todos.len() {
            self.todos.remove(self.selected);
            self.clamp_selection();
            self.save();
        }
    }

    fn save(&mut self) {
        match save_todos(&self.todos) {
            Ok(()) => self.status = "Todos saved".to_owned(),
            Err(e) => self.status = format!("Todo save failed: {e}"),
        }
    }
}

fn todo_path() -> PathBuf {
    let base = env::var_os("XDG_STATE_HOME").map(PathBuf::from).or_else(|| {
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
    }).unwrap_or_else(env::temp_dir);
    base.join("orbit").join("todos.tsv")
}

fn load_todos() -> Vec<Todo> {
    let Ok(raw) = fs::read_to_string(todo_path()) else { return Vec::new() };
    raw.lines().filter_map(|line| {
        let (flag, text) = line.split_once('\t')?;
        (!text.is_empty()).then(|| Todo { done: flag == "1", text: text.to_owned() })
    }).collect()
}

fn save_todos(todos: &[Todo]) -> std::io::Result<()> {
    let path = todo_path();
    if let Some(dir) = path.parent() { fs::create_dir_all(dir)?; }
    let data: String = todos.iter().map(|todo| {
        format!("{}\t{}\n", if todo.done { 1 } else { 0 }, todo.text.replace(['\n', '\r', '\t'], " "))
    }).collect();
    let temp = path.with_extension("tmp");
    fs::write(&temp, data)?;
    fs::rename(temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_names_match_reference() {
        assert_eq!(TABS.len(), 6);
        assert_eq!(TABS[0], "Dashboard");
        assert_eq!(TABS[5], "Todo");
    }
}
