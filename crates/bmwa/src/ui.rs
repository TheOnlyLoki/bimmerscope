use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Cell, Clear, Gauge, List, ListItem, Paragraph, Row, Table, Wrap};
use sgbd::{Kind, Param, Sgbd};

use crate::app::{App, Focus, Items, Tab};
use crate::diag::{self, Fault, ScanState};
use crate::modules;

const M_LIGHT: Color = Color::Rgb(0x00, 0x9A, 0xDA);
const M_DARK: Color = Color::Rgb(0x2B, 0x3E, 0x8C);
const M_RED: Color = Color::Rgb(0xE2, 0x27, 0x18);
const ACCENT: Color = M_LIGHT;
const DIM: Color = Color::DarkGray;
const OK: Color = Color::Green;
const WARN: Color = Color::Yellow;

pub fn draw(f: &mut Frame, app: &mut App) {
    let [header, body, footer] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).areas(f.area());
    draw_header(f, header, app);
    match app.tab {
        Tab::Ecus => draw_ecus(f, body, app),
        Tab::Faults => draw_faults(f, body, app),
        Tab::Live => draw_planned(f, body, "Live data", LIVE_TODO),
        Tab::Adapter => draw_adapter(f, body, app),
    }
    draw_footer(f, footer, app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled("▐█", Style::new().fg(M_LIGHT)),
        Span::styled("█", Style::new().fg(M_DARK)),
        Span::styled("█▌", Style::new().fg(M_RED)),
        Span::styled(" BMW ANALYZER ", Style::new().bold()),
    ];
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, tab.title());
        spans.push(if *tab == app.tab {
            Span::styled(label, Style::new().fg(Color::Black).bg(ACCENT).bold())
        } else {
            Span::styled(label, Style::new().fg(DIM))
        });
    }
    f.render_widget(Line::from(spans), area);

    let cable = match app.ports.iter().find(|p| p.is_ftdi()) {
        Some(p) if p.accessible => Span::styled(format!("● {} ", p.dev.display()), Style::new().fg(OK)),
        Some(p) => Span::styled(format!("● {} no access ", p.dev.display()), Style::new().fg(WARN)),
        None => Span::styled("○ no cable ", Style::new().fg(DIM)),
    };
    f.render_widget(Line::from(cable).right_aligned(), area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let keys: &[(&str, &str)] = match (app.tab, app.editing_filter) {
        (Tab::Ecus, true) => &[("type", "filter"), ("↑↓", "move"), ("enter", "done"), ("esc", "clear")],
        (Tab::Ecus, false) => &[
            ("/", "search"),
            ("tab/←→", "pane"),
            ("↑↓ pgup/pgdn g/G", "move"),
            ("t", "jobs/tables"),
            ("1-4", "tabs"),
            ("q", "quit"),
        ],
        (Tab::Faults, _) => &[("S", "scan"), ("↑↓", "module"), ("C", "clear faults"), ("1-4", "tabs"), ("q", "quit")],
        _ => &[("1-4", "tabs"), ("q", "quit")],
    };
    let mut spans = Vec::new();
    for (k, v) in keys {
        spans.push(Span::styled(format!(" {k} "), Style::new().fg(Color::Black).bg(DIM)));
        spans.push(Span::raw(format!(" {v}  ")));
    }
    f.render_widget(Line::from(spans), area);
}

fn pane(title: impl Into<Line<'static>>, focused: bool) -> Block<'static> {
    let border = if focused { ACCENT } else { DIM };
    Block::bordered().border_type(BorderType::Rounded).border_style(Style::new().fg(border)).title(title)
}

fn highlight() -> Style {
    Style::new().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)
}

fn draw_ecus(f: &mut Frame, area: Rect, app: &mut App) {
    let [files, middle, detail] =
        Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(25), Constraint::Fill(1)]).areas(area);
    draw_files(f, files, app);
    let [info, items] = Layout::vertical([Constraint::Length(10), Constraint::Fill(1)]).areas(middle);
    draw_info(f, info, app);
    draw_items(f, items, app);
    draw_detail(f, detail, app);
}

fn draw_files(f: &mut Frame, area: Rect, app: &mut App) {
    let [search, list] = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

    let cursor = if app.editing_filter { "▏" } else { "" };
    let text = if app.filter.is_empty() && !app.editing_filter {
        Line::styled("press / to search names and descriptions", Style::new().fg(DIM))
    } else {
        Line::from(format!("{}{cursor}", app.filter))
    };
    f.render_widget(Paragraph::new(text).block(pane(" Search ", app.editing_filter)), search);

    let width = list.width.saturating_sub(4) as usize;
    let items: Vec<ListItem> = app
        .visible
        .iter()
        .map(|&i| {
            let file = &app.files[i];
            let kind = match file.kind {
                Some(Kind::Group) => Span::styled("G ", Style::new().fg(M_RED)),
                Some(Kind::Program) => Span::styled("P ", Style::new().fg(M_LIGHT)),
                None => Span::raw("  "),
            };
            let name = format!("{:<16}", file.name);
            let room = width.saturating_sub(name.chars().count() + 3);
            let comment: String = file.comment.chars().take(room).collect();
            ListItem::new(Line::from(vec![kind, Span::raw(name), Span::styled(format!(" {comment}"), Style::new().fg(DIM))]))
        })
        .collect();

    let indexing = if app.indexed < app.files.len() {
        format!(" indexing {}/{} ", app.indexed, app.files.len())
    } else {
        String::new()
    };
    let title = Line::from(vec![
        Span::raw(format!(" ECU files {}/{} ", app.visible.len(), app.files.len())),
        Span::styled(indexing, Style::new().fg(WARN)),
    ]);
    let block = pane(title, app.focus == Focus::Files)
        .title_bottom(Line::styled(format!(" {} ", app.ecu_dir.display()), Style::new().fg(DIM)));
    let widget = List::new(items).block(block).highlight_style(highlight());
    f.render_stateful_widget(widget, list, &mut app.file_state);
}

fn draw_info(f: &mut Frame, area: Rect, app: &App) {
    let block = pane(" Info ", false);
    let Some(loaded) = &app.loaded else {
        f.render_widget(Paragraph::new("no file").block(block), area);
        return;
    };
    let text = match &loaded.sgbd {
        Err(e) => Text::styled(format!("{}: {e}", loaded.name), Style::new().fg(M_RED)),
        Ok(s) => {
            let kv = |k: &str, v: String| Line::from(vec![Span::styled(format!("{k:<8}"), Style::new().fg(DIM)), Span::raw(v)]);
            let d = &s.description;
            Text::from(vec![
                Line::from(vec![
                    Span::styled(d.get("ECU").unwrap_or(&loaded.name).to_string(), Style::new().fg(ACCENT).bold()),
                    Span::styled(if s.kind == Kind::Group { "  group" } else { "  program" }, Style::new().fg(DIM)),
                ]),
                Line::from(d.ecu_comment()),
                kv("rev", format!("{}  bip {:#x}", s.version.revision, s.version.bip_version)),
                kv("origin", d.get("ORIGIN").unwrap_or("").to_string()),
                kv("uses", if s.uses.is_empty() { "-".into() } else { s.uses.join(", ") }),
                kv("size", format!("{} jobs, {} tables", s.jobs.len(), s.tables.len())),
            ])
        }
    };
    f.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }).block(block), area);
}

fn draw_items(f: &mut Frame, area: Rect, app: &mut App) {
    let names: Vec<ListItem> = app.sgbd().map_or_else(Vec::new, |s| match app.items {
        Items::Jobs => s.jobs.iter().map(|j| ListItem::new(j.name.clone())).collect(),
        Items::Tables => s
            .tables
            .iter()
            .map(|t| {
                ListItem::new(Line::from(vec![
                    Span::raw(t.name.clone()),
                    Span::styled(format!(" {}", t.rows.len()), Style::new().fg(DIM)),
                ]))
            })
            .collect(),
    });
    let title = match app.items {
        Items::Jobs => Line::from(vec![Span::styled(" Jobs ", Style::new().bold()), Span::styled("Tables ", Style::new().fg(DIM))]),
        Items::Tables => Line::from(vec![Span::styled(" Jobs ", Style::new().fg(DIM)), Span::styled("Tables ", Style::new().bold())]),
    };
    let widget = List::new(names).block(pane(title, app.focus == Focus::Items)).highlight_style(highlight());
    f.render_stateful_widget(widget, area, &mut app.item_state);
}

fn draw_detail(f: &mut Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Focus::Detail;
    let (Some(s), Some(i)) = (app.sgbd(), app.item_state.selected()) else {
        f.render_widget(pane(" Detail ", focused), area);
        return;
    };
    match app.items {
        Items::Jobs => {
            let job = &s.jobs[i];
            let text = job_text(s, &job.name, job.address);
            let widget = Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((app.detail_scroll, 0))
                .block(pane(format!(" {} ", job.name), focused));
            f.render_widget(widget, area);
        }
        Items::Tables => {
            let table = &s.tables[i];
            let widths: Vec<Constraint> = (0..table.columns.len())
                .map(|c| {
                    let max = table.rows.iter().map(|r| r[c].chars().count()).chain([table.columns[c].len()]).max().unwrap_or(0);
                    Constraint::Max(max.min(60) as u16)
                })
                .collect();
            let header = Row::new(table.columns.iter().map(|c| Cell::from(c.clone()))).style(Style::new().fg(ACCENT).bold());
            let rows = table.rows.iter().map(|r| Row::new(r.iter().map(|c| Cell::from(c.clone()))));
            let title = format!(" {} ({} rows) ", table.name, table.rows.len());
            let widget = Table::new(rows, widths)
                .header(header)
                .column_spacing(2)
                .block(pane(title, focused))
                .row_highlight_style(highlight());
            f.render_stateful_widget(widget, area, &mut app.table_state);
        }
    }
}

fn job_text(s: &Sgbd, name: &str, address: u32) -> Text<'static> {
    let mut lines = vec![Line::styled(format!("bytecode @ {address:#08x}"), Style::new().fg(DIM))];
    let Some(doc) = s.description.job(name) else {
        lines.push(Line::styled("no description", Style::new().fg(DIM)));
        return Text::from(lines);
    };
    lines.extend(doc.comments.iter().map(|c| Line::from(c.clone())));
    let mut section = |title: &str, params: &[Param]| {
        if params.is_empty() {
            return;
        }
        lines.push(Line::default());
        lines.push(Line::styled(title.to_string(), Style::new().fg(M_RED).bold()));
        for p in params {
            lines.push(Line::from(vec![
                Span::styled(format!("  {}", p.name), Style::new().fg(ACCENT)),
                Span::styled(format!("  {}", p.ty), Style::new().fg(DIM)),
            ]));
            lines.extend(p.comments.iter().map(|c| Line::from(format!("      {c}"))));
        }
    };
    section("Arguments", &doc.args);
    section("Results", &doc.results);
    Text::from(lines)
}

fn draw_faults(f: &mut Frame, area: Rect, app: &mut App) {
    let [top, body] = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);
    let [button, progress] = Layout::horizontal([Constraint::Length(20), Constraint::Fill(1)]).areas(top);
    let rounded = || Block::bordered().border_type(BorderType::Rounded);

    let scanning = matches!(app.memory.scan, ScanState::Running { .. });
    let (label, style) = if scanning {
        (" Scanning... ", Style::new().fg(DIM).bold())
    } else {
        (" [S] Scan car ", Style::new().fg(Color::Black).bg(ACCENT).bold())
    };
    let button_block = rounded().border_style(Style::new().fg(ACCENT));
    f.render_widget(Paragraph::new(Line::styled(label, style)).centered().block(button_block), button);

    match &app.memory.scan {
        ScanState::Running { done, total, current } => {
            let ratio = if *total == 0 { 0.0 } else { (*done as f64 / *total as f64).min(1.0) };
            let label = format!(" {current}  {}/{total} ", (done + 1).min(*total));
            let gauge = Gauge::default()
                .block(rounded().border_style(Style::new().fg(DIM)))
                .gauge_style(ACCENT)
                .ratio(ratio)
                .label(label);
            f.render_widget(gauge, progress);
        }
        ScanState::Idle => {
            let text = app.memory.notice.clone().unwrap_or_else(|| "Press S to look for the modules in this car.".into());
            let line = Line::styled(text, Style::new().fg(DIM));
            f.render_widget(Paragraph::new(line).block(rounded().border_style(Style::new().fg(DIM))), progress);
        }
    }

    let [list, faults] = Layout::horizontal([Constraint::Percentage(35), Constraint::Fill(1)]).areas(body);
    draw_modules(f, list, app);
    let selected = app.selected_module().cloned();
    draw_fault_cards(f, faults, app, selected.as_ref());
    if app.confirm_clear
        && let Some(m) = &selected
    {
        draw_confirm(f, body, module_name(&m.id));
    }
}

fn module_name(id: &str) -> &str {
    modules::find(id).map_or(id, |m| m.name_en)
}

fn draw_modules(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .memory
        .found
        .iter()
        .map(|m| {
            let (badge, color) = match m.fault_count {
                0 => (" OK ".to_string(), OK),
                1 => (" 1 fault ".to_string(), M_RED),
                n => (format!(" {n} faults "), M_RED),
            };
            let german = modules::find(&m.id).map_or("", |md| md.name_de);
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<24}", module_name(&m.id))),
                Span::styled(badge, Style::new().fg(Color::Black).bg(color).bold()),
                Span::styled(format!(" {german}"), Style::new().fg(DIM)),
            ]))
        })
        .collect();
    let empty = if items.is_empty() {
        match app.memory.scan {
            ScanState::Running { .. } => "Scanning...",
            ScanState::Idle => "No modules found yet.",
        }
    } else {
        ""
    };
    let block = pane(format!(" Modules ({}) ", items.len()), true);
    if items.is_empty() {
        f.render_widget(Paragraph::new(Line::styled(empty, Style::new().fg(DIM))).block(block), area);
        return;
    }
    let widget = List::new(items).block(block).highlight_style(highlight());
    f.render_stateful_widget(widget, area, &mut app.fault_list);
}

fn draw_fault_cards(f: &mut Frame, area: Rect, app: &App, selected: Option<&crate::diag::Found>) {
    let Some(m) = selected else {
        f.render_widget(pane(" Faults ", false), area);
        return;
    };
    let block = pane(format!(" {} ", module_name(&m.id)), false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let message = |text: String, color: Color| Paragraph::new(Line::styled(text, Style::new().fg(color))).wrap(Wrap { trim: true });
    if let Some(faults) = app.memory.faults.get(&m.id) {
        if faults.is_empty() {
            f.render_widget(message("No stored faults.".into(), OK), inner);
            return;
        }
        let shown = faults.len().min(inner.height as usize / CARD_HEIGHT as usize);
        let rows = Layout::vertical(std::iter::repeat_n(Constraint::Length(CARD_HEIGHT), shown)).split(inner);
        for (fault, row) in faults.iter().zip(rows.iter()) {
            draw_card(f, *row, fault);
        }
    } else if let Some(err) = app.memory.errors.get(&m.id) {
        f.render_widget(message(err.clone(), M_RED), inner);
    } else {
        f.render_widget(message("Reading...".into(), DIM), inner);
    }
}

const CARD_HEIGHT: u16 = 5;

/// One stored fault: plain text first, code small in the corner, tags below.
fn draw_card(f: &mut Frame, area: Rect, fault: &Fault) {
    let block = rounded_card(fault);
    let text = if fault.text.is_empty() { "Unknown fault" } else { fault.text.as_str() };
    let mut tags = Vec::new();
    if !fault.present.is_empty() {
        tags.push(Span::styled(format!(" {} ", fault.present), Style::new().fg(Color::Black).bg(WARN)));
        tags.push(Span::raw(" "));
    }
    if !fault.symptom.is_empty() {
        tags.push(Span::styled(format!(" {} ", fault.symptom), Style::new().fg(Color::Black).bg(ACCENT)));
    }
    let mut lines = vec![Line::styled(text.to_string(), Style::new().bold()), Line::from(tags)];
    if !fault.warning.is_empty() {
        lines.push(Line::styled(fault.warning.clone(), Style::new().fg(DIM)));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }).block(block), area);
}

fn rounded_card(fault: &Fault) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(DIM))
        .title_top(Line::styled(format!(" {} ", diag::code_hex(fault.code)), Style::new().fg(DIM)).right_aligned())
}

fn draw_confirm(f: &mut Frame, area: Rect, name: &str) {
    let w = area.width.min(60);
    let h = area.height.min(7);
    let rect = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, rect);
    let text = Text::from(vec![
        Line::from(format!("Delete stored faults of {name}? Faults that are still present will come back.")),
        Line::default(),
        Line::styled("[Y]es / [N]o", Style::new().bold()),
    ]);
    let widget = Paragraph::new(text).wrap(Wrap { trim: true }).block(pane(" Clear faults ", true));
    f.render_widget(widget, rect);
}

const LIVE_TODO: &str = "Live values from STATUS_* jobs, polled and graphed.\n\n\
Needs: BEST/2 interpreter (milestone 2) and K-line transport for the K+DCAN cable (milestone 3).";

fn draw_planned(f: &mut Frame, area: Rect, title: &str, text: &str) {
    let block = pane(format!(" {title} "), false);
    let widget = Paragraph::new(Text::from(vec![
        Line::styled("not implemented yet", Style::new().fg(WARN).bold()),
        Line::default(),
        Line::from(text.to_string()),
    ]))
    .wrap(Wrap { trim: false })
    .block(block);
    f.render_widget(widget, area);
}

fn draw_adapter(f: &mut Frame, area: Rect, app: &App) {
    let check = |ok: bool, text: String| {
        let (mark, color) = if ok { ("✔", OK) } else { ("✘", WARN) };
        Line::from(vec![Span::styled(format!("  {mark} "), Style::new().fg(color)), Span::raw(text)])
    };
    let mut lines = Vec::new();
    if app.ports.is_empty() {
        lines.push(Line::styled("No USB serial adapter found. Plug in the K+DCAN cable.", Style::new().fg(WARN)));
    }
    for p in &app.ports {
        lines.push(Line::from(vec![
            Span::styled(p.dev.display().to_string(), Style::new().fg(ACCENT).bold()),
            Span::raw(format!("  {}  [{}]  serial {}", p.product, p.vid_pid, p.serial)),
        ]));
        lines.push(check(p.is_ftdi(), format!("driver {} (K+DCAN cables use ftdi_sio)", p.driver)));
        lines.push(check(p.accessible, "read/write access".into()));
        if !p.accessible {
            lines.push(Line::styled("      fix: sudo usermod -aG dialout $USER   (then log out and in)", Style::new().fg(DIM)));
        }
        let latency_ok = p.latency_ms == Some(1);
        let latency = p.latency_ms.map_or("unknown".into(), |l| format!("{l} ms"));
        lines.push(check(latency_ok, format!("latency timer {latency} (K-line needs 1 ms)")));
        if !latency_ok && p.is_ftdi() {
            let name = p.dev.file_name().unwrap_or_default().to_string_lossy();
            lines.push(Line::styled(
                format!("      fix: echo 1 | sudo tee /sys/bus/usb-serial/devices/{name}/latency_timer"),
                Style::new().fg(DIM),
            ));
        }
        lines.push(Line::default());
    }
    lines.push(Line::styled("Car: E87 (pre-LCI, 2006) talks K-line. Set the cable's switch to K-line if it has one.", Style::new().fg(DIM)));
    lines.push(Line::styled("The K-line side is powered by OBD pin 16, so echo tests need the car.", Style::new().fg(DIM)));
    let widget = Paragraph::new(lines).wrap(Wrap { trim: false }).block(pane(" Adapter ", false));
    f.render_widget(widget, area);
}
