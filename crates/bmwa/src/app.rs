use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::{ListState, TableState};
use sgbd::Sgbd;

use crate::cable::{self, Port};
use crate::diag::{Command, Found, FaultMemory, Handle, ScanState};
use crate::index::{EcuFile, Update};

const CABLE_RESCAN: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Ecus,
    Faults,
    Live,
    Adapter,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Ecus, Tab::Faults, Tab::Live, Tab::Adapter];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Ecus => "ECU files",
            Tab::Faults => "Fault memory",
            Tab::Live => "Live data",
            Tab::Adapter => "Adapter",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Files,
    Items,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Items {
    Jobs,
    Tables,
}

pub struct Loaded {
    pub name: String,
    pub sgbd: Result<Sgbd, String>,
}

pub struct App {
    pub ecu_dir: PathBuf,
    pub files: Vec<EcuFile>,
    pub visible: Vec<usize>,
    pub indexed: usize,
    index_rx: Receiver<Update>,

    pub tab: Tab,
    pub focus: Focus,
    pub items: Items,
    pub filter: String,
    pub editing_filter: bool,

    pub file_state: ListState,
    pub item_state: ListState,
    pub table_state: TableState,
    pub detail_scroll: u16,
    pub loaded: Option<Loaded>,

    pub ports: Vec<Port>,
    last_scan: Instant,
    /// Tab follows the cable (switches to Fault memory) until the user picks a tab.
    tab_auto: bool,
    had_ftdi: bool,

    pub worker: Handle,
    pub memory: FaultMemory,
    pub fault_list: ListState,
    /// First fault card shown for the selected module. Reset when the module changes.
    pub fault_scroll: usize,
    pub confirm_clear: bool,
    pub quit: bool,
}

impl App {
    pub fn new(ecu_dir: PathBuf, files: Vec<EcuFile>, index_rx: Receiver<Update>, worker: Handle) -> Self {
        let ports = cable::scan();
        let had_ftdi = ports.iter().any(Port::is_ftdi);
        let mut app = Self {
            ecu_dir,
            visible: (0..files.len()).collect(),
            files,
            indexed: 0,
            index_rx,
            tab: Tab::Ecus,
            focus: Focus::Files,
            items: Items::Jobs,
            filter: String::new(),
            editing_filter: false,
            file_state: ListState::default(),
            item_state: ListState::default(),
            table_state: TableState::default(),
            detail_scroll: 0,
            loaded: None,
            ports,
            last_scan: Instant::now(),
            tab_auto: true,
            had_ftdi,
            worker,
            memory: FaultMemory::default(),
            fault_list: ListState::default(),
            fault_scroll: 0,
            confirm_clear: false,
            quit: false,
        };
        if !app.visible.is_empty() {
            app.file_state.select(Some(0));
            app.load_selected();
        }
        app
    }

    /// Background work between frames: index updates, cable rescans, worker events.
    pub fn tick(&mut self) {
        let mut changed = false;
        while let Ok(u) = self.index_rx.try_recv() {
            let f = &mut self.files[u.index];
            f.haystack = format!("{} {}", f.name, u.comment).to_lowercase();
            f.kind = u.kind;
            f.comment = u.comment;
            self.indexed += 1;
            changed = true;
        }
        if changed && !self.filter.is_empty() {
            self.refilter();
        }
        if self.last_scan.elapsed() >= CABLE_RESCAN {
            self.ports = cable::scan();
            self.last_scan = Instant::now();
        }
        let ftdi = self.ports.iter().any(Port::is_ftdi);
        if ftdi && !self.had_ftdi && self.tab_auto {
            self.tab = Tab::Faults;
        }
        self.had_ftdi = ftdi;

        while let Ok(ev) = self.worker.rx.try_recv() {
            self.memory.apply(ev);
        }
        self.fix_module_selection();
        self.ensure_faults();
    }

    pub fn selected_file(&self) -> Option<&EcuFile> {
        self.file_state.selected().and_then(|i| self.visible.get(i)).map(|&i| &self.files[i])
    }

    pub fn selected_module(&self) -> Option<&Found> {
        self.fault_list.selected().and_then(|i| self.memory.found.get(i))
    }

    pub fn sgbd(&self) -> Option<&Sgbd> {
        self.loaded.as_ref().and_then(|l| l.sgbd.as_ref().ok())
    }

    pub fn item_count(&self) -> usize {
        self.sgbd().map_or(0, |s| match self.items {
            Items::Jobs => s.jobs.len(),
            Items::Tables => s.tables.len(),
        })
    }

    fn refilter(&mut self) {
        let keep = self.selected_file().map(|f| f.name.clone());
        let needle = self.filter.to_lowercase();
        self.visible = (0..self.files.len())
            .filter(|&i| needle.split_whitespace().all(|w| self.files[i].haystack.contains(w)))
            .collect();
        let pos = keep.and_then(|n| self.visible.iter().position(|&i| self.files[i].name == n));
        self.file_state.select(pos.or((!self.visible.is_empty()).then_some(0)));
        let now = self.selected_file().map(|f| f.name.as_str());
        if now != self.loaded.as_ref().map(|l| l.name.as_str()) {
            self.load_selected();
        }
    }

    fn load_selected(&mut self) {
        self.loaded = self.selected_file().map(|f| Loaded {
            name: f.name.clone(),
            sgbd: Sgbd::open(&f.path).map_err(|e| e.to_string()),
        });
        self.reset_items();
    }

    fn reset_items(&mut self) {
        self.item_state.select((self.item_count() > 0).then_some(0));
        self.reset_detail();
    }

    fn reset_detail(&mut self) {
        self.detail_scroll = 0;
        self.table_state = TableState::default().with_selected(Some(0));
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if self.editing_filter {
            match key.code {
                KeyCode::Esc => {
                    self.editing_filter = false;
                    self.filter.clear();
                    self.refilter();
                }
                KeyCode::Enter => self.editing_filter = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.refilter();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.refilter();
                }
                _ => self.on_move(key.code),
            }
            return;
        }
        if self.tab == Tab::Faults && self.confirm_clear {
            self.on_confirm_key(key.code);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char(c @ '1'..='4') => {
                self.tab = Tab::ALL[c as usize - '1' as usize];
                self.tab_auto = false;
            }
            _ if self.tab == Tab::Faults => self.on_faults_key(key.code),
            _ if self.tab != Tab::Ecus => {}
            KeyCode::Char('/') => {
                self.editing_filter = true;
                self.focus = Focus::Files;
            }
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refilter();
            }
            KeyCode::Char('t') => {
                self.items = match self.items {
                    Items::Jobs => Items::Tables,
                    Items::Tables => Items::Jobs,
                };
                self.reset_items();
            }
            KeyCode::Tab | KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                self.focus = match self.focus {
                    Focus::Files => Focus::Items,
                    _ => Focus::Detail,
                }
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.focus = match self.focus {
                    Focus::Detail => Focus::Items,
                    _ => Focus::Files,
                }
            }
            code => self.on_move(code),
        }
    }

    fn on_faults_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('s' | 'S') => self.start_scan(),
            KeyCode::Char('c' | 'C') => self.confirm_clear = self.selected_module().is_some(),
            KeyCode::Up => self.move_module(-1),
            KeyCode::Down => self.move_module(1),
            KeyCode::PageDown | KeyCode::Char('j') => self.scroll_faults(1),
            KeyCode::PageUp | KeyCode::Char('k') => self.scroll_faults(-1),
            _ => {}
        }
    }

    /// Fault cards stored for the selected module.
    pub fn selected_fault_count(&self) -> usize {
        self.selected_module().and_then(|m| self.memory.faults.get(&m.id)).map_or(0, Vec::len)
    }

    fn scroll_faults(&mut self, delta: isize) {
        let n = self.selected_fault_count();
        self.fault_scroll = if n == 0 { 0 } else { (self.fault_scroll as isize + delta).clamp(0, n as isize - 1) as usize };
    }

    /// Moves the module selection. Scroll goes back to the top when the module changes.
    fn select_module(&mut self, sel: Option<usize>) {
        if sel != self.fault_list.selected() {
            self.fault_list.select(sel);
            self.fault_scroll = 0;
        }
    }

    fn on_confirm_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y' | 'Y') => {
                self.confirm_clear = false;
                if let Some(id) = self.selected_module().map(|m| m.id.clone()) {
                    self.send(Command::ClearFaults(id));
                }
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => self.confirm_clear = false,
            _ => {}
        }
    }

    fn start_scan(&mut self) {
        if matches!(self.memory.scan, ScanState::Running { .. }) {
            return;
        }
        self.memory.begin_scan();
        self.select_module(None);
        self.send(Command::Scan);
    }

    fn move_module(&mut self, delta: isize) {
        let n = self.memory.found.len();
        if n == 0 {
            return;
        }
        let cur = self.fault_list.selected().unwrap_or(0) as isize;
        self.select_module(Some((cur + delta).clamp(0, n as isize - 1) as usize));
    }

    fn fix_module_selection(&mut self) {
        let n = self.memory.found.len();
        let sel = (n > 0).then(|| self.fault_list.selected().filter(|&i| i < n).unwrap_or(0));
        self.select_module(sel);
    }

    /// Asks the worker for the selected module's faults once, unless already cached.
    fn ensure_faults(&mut self) {
        let Some(id) = self.selected_module().map(|m| m.id.clone()) else {
            return;
        };
        if self.memory.needs_read(&id) {
            self.memory.requested.insert(id.clone());
            self.send(Command::ReadFaults(id));
        }
    }

    fn send(&self, cmd: Command) {
        // A dead worker shows up as no events; nothing else to do here.
        let _ = self.worker.tx.send(cmd);
    }

    fn on_move(&mut self, code: KeyCode) {
        let delta: isize = match code {
            KeyCode::Up | KeyCode::Char('k') => -1,
            KeyCode::Down | KeyCode::Char('j') => 1,
            KeyCode::PageUp => -20,
            KeyCode::PageDown => 20,
            KeyCode::Home | KeyCode::Char('g') => isize::MIN / 2,
            KeyCode::End | KeyCode::Char('G') => isize::MAX / 2,
            _ => return,
        };
        let step = |cur: Option<usize>, len: usize| -> Option<usize> {
            (len > 0).then(|| (cur.unwrap_or(0) as isize).saturating_add(delta).clamp(0, len as isize - 1) as usize)
        };
        match self.focus {
            Focus::Files => {
                let next = step(self.file_state.selected(), self.visible.len());
                if next != self.file_state.selected() {
                    self.file_state.select(next);
                    self.load_selected();
                }
            }
            Focus::Items => {
                let next = step(self.item_state.selected(), self.item_count());
                if next != self.item_state.selected() {
                    self.item_state.select(next);
                    self.reset_detail();
                }
            }
            Focus::Detail => match self.items {
                Items::Jobs => {
                    self.detail_scroll = (self.detail_scroll as isize).saturating_add(delta).clamp(0, u16::MAX as isize) as u16;
                }
                Items::Tables => {
                    let rows = self.sgbd().zip(self.item_state.selected()).map_or(0, |(s, i)| s.tables[i].rows.len());
                    let next = step(self.table_state.selected(), rows);
                    self.table_state.select(next);
                }
            },
        }
    }
}
