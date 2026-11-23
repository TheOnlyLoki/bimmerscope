use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Points};
use ratatui::widgets::{
    Axis, Block, BorderType, Cell, Chart, Clear, Dataset, Gauge, GraphType, LineGauge, List,
    ListItem, ListState, Paragraph, Row, Sparkline, Table, Wrap,
};
use sgbd::{Kind, Param, Sgbd};

use crate::app::{App, Focus, Items, Tab};
use crate::diag::{self, Fault, ScanState};
use crate::live::{ARC_START, ARC_SWEEP, Dial, Item, Key, Zone, dial_for, needle_angle, range_for};
use crate::modules;

const M_LIGHT: Color = Color::Rgb(0x00, 0x9A, 0xDA);
const M_DARK: Color = Color::Rgb(0x2B, 0x3E, 0x8C);
const M_RED: Color = Color::Rgb(0xE2, 0x27, 0x18);
const ACCENT: Color = M_LIGHT;
const DIM: Color = Color::DarkGray;
const OK: Color = Color::Green;
const WARN: Color = Color::Yellow;

pub fn draw(f: &mut Frame, app: &mut App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(f.area());
    draw_header(f, header, app);
    match app.tab {
        Tab::Ecus => draw_ecus(f, body, app),
        Tab::Faults => draw_faults(f, body, app),
        Tab::Live => draw_live(f, body, app),
        Tab::Adapter => draw_adapter(f, body, app),
    }
    draw_footer(f, footer, app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled("▐█", Style::new().fg(M_LIGHT)),
        Span::styled("█", Style::new().fg(M_DARK)),
        Span::styled("█▌", Style::new().fg(M_RED)),
        Span::styled(" BIMMERSCOPE ", Style::new().bold()),
    ];
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, tab.title());
        spans.push(if *tab == app.tab {
            Span::styled(label, Style::new().fg(Color::Black).bg(ACCENT).bold())
        } else {
            Span::styled(label, Style::new().fg(DIM))
        });
    }
    if app.live.recorder.is_some() {
        spans.push(Span::styled(" ● REC", Style::new().fg(M_RED).bold()));
    }
    f.render_widget(Line::from(spans), area);

    let cable = match app.ports.iter().find(|p| p.is_ftdi()) {
        Some(p) if p.accessible => {
            Span::styled(format!("● {} ", p.dev.display()), Style::new().fg(OK))
        }
        Some(p) => Span::styled(
            format!("● {} no access ", p.dev.display()),
            Style::new().fg(WARN),
        ),
        None => Span::styled("○ no cable ", Style::new().fg(DIM)),
    };
    f.render_widget(Line::from(cable).right_aligned(), area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let keys: &[(&str, &str)] = match (app.tab, app.editing_filter) {
        (Tab::Ecus, true) => &[
            ("type", "filter"),
            ("↑↓", "move"),
            ("enter", "done"),
            ("esc", "clear"),
        ],
        (Tab::Ecus, false) => &[
            ("/", "search"),
            ("tab/←→", "pane"),
            ("↑↓ pgup/pgdn g/G", "move"),
            ("t", "jobs/tables"),
            ("1-4", "tabs"),
            ("q", "quit"),
        ],
        (Tab::Faults, _) => &[
            ("S", "scan"),
            ("↑↓", "module"),
            ("PgUp/PgDn", "faults"),
            ("C", "clear faults"),
            ("1-4", "tabs"),
            ("q", "quit"),
        ],
        (Tab::Live, _) if app.live.expert => &[
            ("enter", "start/stop"),
            ("↑↓ PgUp/PgDn", "move"),
            ("space", "add/remove"),
            ("←→", "module"),
            ("A", "back to main values"),
            ("R", "record"),
            ("1-4", "tabs"),
            ("q", "quit"),
        ],
        (Tab::Live, _) => &[
            ("enter", "start/stop"),
            ("←→", "module"),
            ("↑↓", "chart"),
            ("G", "overlay"),
            ("A", "all values"),
            ("R", "record"),
            ("1-4", "tabs"),
            ("q", "quit"),
        ],
        _ => &[("1-4", "tabs"), ("q", "quit")],
    };
    let mut spans = Vec::new();
    for (k, v) in keys {
        spans.push(Span::styled(
            format!(" {k} "),
            Style::new().fg(Color::Black).bg(DIM),
        ));
        spans.push(Span::raw(format!(" {v}  ")));
    }
    f.render_widget(Line::from(spans), area);
}

fn pane(title: impl Into<Line<'static>>, focused: bool) -> Block<'static> {
    let border = if focused { ACCENT } else { DIM };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title(title)
}

fn highlight() -> Style {
    Style::new()
        .fg(Color::Black)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

fn draw_ecus(f: &mut Frame, area: Rect, app: &mut App) {
    let [files, middle, detail] = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(25),
        Constraint::Fill(1),
    ])
    .areas(area);
    draw_files(f, files, app);
    let [info, items] =
        Layout::vertical([Constraint::Length(10), Constraint::Fill(1)]).areas(middle);
    draw_info(f, info, app);
    draw_items(f, items, app);
    draw_detail(f, detail, app);
}

fn draw_files(f: &mut Frame, area: Rect, app: &mut App) {
    let [search, list] = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

    let cursor = if app.editing_filter { "▏" } else { "" };
    let text = if app.filter.is_empty() && !app.editing_filter {
        Line::styled(
            "press / to search names and descriptions",
            Style::new().fg(DIM),
        )
    } else {
        Line::from(format!("{}{cursor}", app.filter))
    };
    f.render_widget(
        Paragraph::new(text).block(pane(" Search ", app.editing_filter)),
        search,
    );

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
            ListItem::new(Line::from(vec![
                kind,
                Span::raw(name),
                Span::styled(format!(" {comment}"), Style::new().fg(DIM)),
            ]))
        })
        .collect();

    let indexing = if app.indexed < app.files.len() {
        format!(" indexing {}/{} ", app.indexed, app.files.len())
    } else {
        String::new()
    };
    let title = Line::from(vec![
        Span::raw(format!(
            " ECU files {}/{} ",
            app.visible.len(),
            app.files.len()
        )),
        Span::styled(indexing, Style::new().fg(WARN)),
    ]);
    let block = pane(title, app.focus == Focus::Files).title_bottom(Line::styled(
        format!(" {} ", app.ecu_dir.display()),
        Style::new().fg(DIM),
    ));
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
            let kv = |k: &str, v: String| {
                Line::from(vec![
                    Span::styled(format!("{k:<8}"), Style::new().fg(DIM)),
                    Span::raw(v),
                ])
            };
            let d = &s.description;
            Text::from(vec![
                Line::from(vec![
                    Span::styled(
                        d.get("ECU").unwrap_or(&loaded.name).to_string(),
                        Style::new().fg(ACCENT).bold(),
                    ),
                    Span::styled(
                        if s.kind == Kind::Group {
                            "  group"
                        } else {
                            "  program"
                        },
                        Style::new().fg(DIM),
                    ),
                ]),
                Line::from(d.ecu_comment()),
                kv(
                    "rev",
                    format!("{}  bip {:#x}", s.version.revision, s.version.bip_version),
                ),
                kv("origin", d.get("ORIGIN").unwrap_or("").to_string()),
                kv(
                    "uses",
                    if s.uses.is_empty() {
                        "-".into()
                    } else {
                        s.uses.join(", ")
                    },
                ),
                kv(
                    "size",
                    format!("{} jobs, {} tables", s.jobs.len(), s.tables.len()),
                ),
            ])
        }
    };
    f.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: true }).block(block),
        area,
    );
}

fn draw_items(f: &mut Frame, area: Rect, app: &mut App) {
    let names: Vec<ListItem> = app.sgbd().map_or_else(Vec::new, |s| match app.items {
        Items::Jobs => s
            .jobs
            .iter()
            .map(|j| ListItem::new(j.name.clone()))
            .collect(),
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
        Items::Jobs => Line::from(vec![
            Span::styled(" Jobs ", Style::new().bold()),
            Span::styled("Tables ", Style::new().fg(DIM)),
        ]),
        Items::Tables => Line::from(vec![
            Span::styled(" Jobs ", Style::new().fg(DIM)),
            Span::styled("Tables ", Style::new().bold()),
        ]),
    };
    let widget = List::new(names)
        .block(pane(title, app.focus == Focus::Items))
        .highlight_style(highlight());
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
                    let max = table
                        .rows
                        .iter()
                        .map(|r| r[c].chars().count())
                        .chain([table.columns[c].len()])
                        .max()
                        .unwrap_or(0);
                    Constraint::Max(max.min(60) as u16)
                })
                .collect();
            let header = Row::new(table.columns.iter().map(|c| Cell::from(c.clone())))
                .style(Style::new().fg(ACCENT).bold());
            let rows = table
                .rows
                .iter()
                .map(|r| Row::new(r.iter().map(|c| Cell::from(c.clone()))));
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
    let mut lines = vec![Line::styled(
        format!("bytecode @ {address:#08x}"),
        Style::new().fg(DIM),
    )];
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
        lines.push(Line::styled(
            title.to_string(),
            Style::new().fg(M_RED).bold(),
        ));
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
    let [button, progress] =
        Layout::horizontal([Constraint::Length(20), Constraint::Fill(1)]).areas(top);
    let rounded = || Block::bordered().border_type(BorderType::Rounded);

    let scanning = matches!(app.memory.scan, ScanState::Running { .. });
    let (label, style) = if scanning {
        (" Scanning... ", Style::new().fg(DIM).bold())
    } else {
        (
            " [S] Scan car ",
            Style::new().fg(Color::Black).bg(ACCENT).bold(),
        )
    };
    let button_block = rounded().border_style(Style::new().fg(ACCENT));
    f.render_widget(
        Paragraph::new(Line::styled(label, style))
            .centered()
            .block(button_block),
        button,
    );

    match &app.memory.scan {
        ScanState::Running {
            done,
            total,
            current,
        } => {
            let ratio = if *total == 0 {
                0.0
            } else {
                (*done as f64 / *total as f64).min(1.0)
            };
            let label = format!(" {current}  {}/{total} ", (done + 1).min(*total));
            let gauge = Gauge::default()
                .block(rounded().border_style(Style::new().fg(DIM)))
                .gauge_style(ACCENT)
                .ratio(ratio)
                .label(label);
            f.render_widget(gauge, progress);
        }
        ScanState::Idle => {
            let text = app
                .memory
                .notice
                .clone()
                .unwrap_or_else(|| "Press S to look for the modules in this car.".into());
            let line = Line::styled(text, Style::new().fg(DIM));
            f.render_widget(
                Paragraph::new(line).block(rounded().border_style(Style::new().fg(DIM))),
                progress,
            );
        }
    }

    let [list, faults] =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Fill(1)]).areas(body);
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
        f.render_widget(
            Paragraph::new(Line::styled(empty, Style::new().fg(DIM))).block(block),
            area,
        );
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

    let message = |text: String, color: Color| {
        Paragraph::new(Line::styled(text, Style::new().fg(color))).wrap(Wrap { trim: true })
    };
    if let Some(faults) = app.memory.faults.get(&m.id) {
        if faults.is_empty() {
            f.render_widget(message("No stored faults.".into(), OK), inner);
            return;
        }
        let start = app.fault_scroll.min(faults.len() - 1);
        let left = faults.len() - start;
        let fits = inner.height as usize / CARD_HEIGHT as usize;
        // When cards are cut off below, the last row holds the "+N more" line.
        let hidden = left > fits && inner.height > 0;
        let shown = if hidden {
            (inner.height.saturating_sub(1) as usize / CARD_HEIGHT as usize).min(left)
        } else {
            left.min(fits)
        };
        let cards = Rect {
            height: shown as u16 * CARD_HEIGHT,
            ..inner
        };
        let rows = Layout::vertical(std::iter::repeat_n(Constraint::Length(CARD_HEIGHT), shown))
            .split(cards);
        for (fault, row) in faults[start..].iter().zip(rows.iter()) {
            draw_card(f, *row, fault);
        }
        if hidden {
            let more = Rect {
                y: inner.y + inner.height - 1,
                height: 1,
                ..inner
            };
            let text = format!("+{} more (PgDn)", left - shown);
            f.render_widget(
                Paragraph::new(Line::styled(text, Style::new().fg(DIM))),
                more,
            );
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
    let text = if fault.text.is_empty() {
        "Unknown fault"
    } else {
        fault.text.as_str()
    };
    let mut tags = Vec::new();
    if !fault.present.is_empty() {
        tags.push(Span::styled(
            format!(" {} ", fault.present),
            Style::new().fg(Color::Black).bg(WARN),
        ));
        tags.push(Span::raw(" "));
    }
    if !fault.symptom.is_empty() {
        tags.push(Span::styled(
            format!(" {} ", fault.symptom),
            Style::new().fg(Color::Black).bg(ACCENT),
        ));
    }
    let mut lines = vec![
        Line::styled(text.to_string(), Style::new().bold()),
        Line::from(tags),
    ];
    if !fault.warning.is_empty() {
        lines.push(Line::styled(fault.warning.clone(), Style::new().fg(DIM)));
    }
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(block),
        area,
    );
}

fn rounded_card(fault: &Fault) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(DIM))
        .title_top(
            Line::styled(
                format!(" {} ", diag::code_hex(fault.code)),
                Style::new().fg(DIM),
            )
            .right_aligned(),
        )
}

fn draw_confirm(f: &mut Frame, area: Rect, name: &str) {
    let w = area.width.min(60);
    let h = area.height.min(7);
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    f.render_widget(Clear, rect);
    let text = Text::from(vec![
        Line::from(format!(
            "Delete stored faults of {name}? Faults that are still present will come back."
        )),
        Line::default(),
        Line::styled("[Y]es / [N]o", Style::new().bold()),
    ]);
    let widget = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .block(pane(" Clear faults ", true));
    f.render_widget(widget, rect);
}

fn draw_live(f: &mut Frame, area: Rect, app: &mut App) {
    let [list, right] =
        Layout::horizontal([Constraint::Percentage(28), Constraint::Fill(1)]).areas(area);
    draw_live_modules(f, list, app);
    let [top, body] = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(right);
    let group = app.live.selected.clone().unwrap_or_default();
    draw_live_status(f, top, app, &group);
    match app.live.catalogs.get(&group) {
        None => draw_live_message(f, body, "Reading the module...", DIM),
        Some(Err(e)) => {
            let text = format!("{e}\nPress Enter to try again.");
            draw_live_message(f, body, &text, M_RED);
        }
        Some(Ok(_)) if app.live.expert => draw_live_expert(f, body, app, &group),
        Some(Ok(_)) => draw_live_dashboard(f, body, app, &group),
    }
}

fn draw_live_message(f: &mut Frame, area: Rect, text: &str, color: Color) {
    let widget = Paragraph::new(Line::styled(text.to_string(), Style::new().fg(color)))
        .wrap(Wrap { trim: true })
        .block(pane(" Values ", false));
    f.render_widget(widget, area);
}

fn draw_live_modules(f: &mut Frame, area: Rect, app: &mut App) {
    let modules = app.live_modules();
    let items: Vec<ListItem> = modules
        .iter()
        .map(|m| {
            let live = app.live.running && app.live.group.as_deref() == Some(m.id);
            let mark = if live { "● " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(mark, Style::new().fg(OK)),
                Span::raw(m.name_en),
            ]))
        })
        .collect();
    let selected = app
        .live
        .selected
        .as_deref()
        .and_then(|s| modules.iter().position(|m| m.id == s));
    let scanned = !app.memory.found.is_empty();
    let title = if scanned {
        format!(" Modules ({}) ", modules.len())
    } else {
        " Modules (not scanned) ".to_string()
    };
    let widget = List::new(items)
        .block(pane(title, true))
        .highlight_style(highlight());
    let mut state = ListState::default().with_selected(selected);
    f.render_stateful_widget(widget, area, &mut state);
}

/// Start/stop button, what is polled, update rate and the latest notice.
fn draw_live_status(f: &mut Frame, area: Rect, app: &App, group: &str) {
    let [button, info] =
        Layout::horizontal([Constraint::Length(22), Constraint::Fill(1)]).areas(area);
    let rounded = || Block::bordered().border_type(BorderType::Rounded);
    let (label, style) = if app.live.running {
        (
            " [Enter] Stop live ",
            Style::new().fg(Color::Black).bg(M_RED).bold(),
        )
    } else {
        (
            " [Enter] Start live ",
            Style::new().fg(Color::Black).bg(ACCENT).bold(),
        )
    };
    f.render_widget(
        Paragraph::new(Line::styled(label, style))
            .centered()
            .block(rounded().border_style(Style::new().fg(ACCENT))),
        button,
    );

    let title = live_title(app, group);
    let text = app.live.notice.clone().unwrap_or_else(|| {
        if app.live.running {
            String::new()
        } else {
            "Press Enter to read live values from the car.".into()
        }
    });
    let mut block = rounded().border_style(Style::new().fg(DIM)).title(title);
    if let Some(rate) = app.live.rate().filter(|_| app.live.running) {
        block = block.title(
            Line::styled(format!(" {rate:.1} rounds/s "), Style::new().fg(DIM)).right_aligned(),
        );
    }
    f.render_widget(
        Paragraph::new(Line::styled(text, Style::new().fg(DIM))).block(block),
        info,
    );
}

/// Module name and the variant file its values come from.
fn live_title(app: &App, group: &str) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!(" {} ", module_name(group)),
        Style::new().bold(),
    )];
    if let Some(c) = app.live.catalog(group) {
        spans.push(Span::styled(
            format!("{} ", c.variant.to_uppercase()),
            Style::new().fg(DIM),
        ));
    }
    Line::from(spans)
}

const LIVE_CARD_W: u16 = 28;
const LIVE_CARD_H: u16 = 5;
/// Dial row, chart and cards each need this much room, or they are left out.
/// Dials go first, then the chart; the cards stay.
const DIAL_H: u16 = 14;
const CHART_MIN_H: u16 = 10;
const CHART_MAX_H: u16 = 16;
const CARDS_MIN_H: u16 = 14;

fn zone_color(zone: Zone) -> Color {
    match zone {
        Zone::Ok => OK,
        Zone::Warn => WARN,
        Zone::Bad => M_RED,
    }
}

/// Cuts `h` rows off the top of `area`.
fn take_top(area: &mut Rect, h: u16) -> Rect {
    let h = h.min(area.height);
    let top = Rect { height: h, ..*area };
    area.y += h;
    area.height -= h;
    top
}

/// Dials on top, chart below, cards for every value at the bottom.
fn draw_live_dashboard(f: &mut Frame, area: Rect, app: &App, group: &str) {
    let items = app.live.items(group);
    let dials: Vec<(&Item, Dial)> = items
        .iter()
        .filter_map(|i| dial_for(&i.label_en).map(|d| (i, d)))
        .take(2)
        .collect();
    let mut rest = area;
    let show_dials =
        !dials.is_empty() && area.width >= 56 && area.height >= DIAL_H + CHART_MIN_H + CARDS_MIN_H;
    if show_dials {
        let row = take_top(&mut rest, DIAL_H);
        let n = dials.len() as u32;
        let cells = Layout::horizontal(vec![Constraint::Ratio(1, n); n as usize]).split(row);
        for (cell, (item, dial)) in cells.iter().zip(&dials) {
            draw_dial(f, *cell, app, item, dial);
        }
    }
    if !items.is_empty() && rest.height >= CHART_MIN_H + CARDS_MIN_H {
        let h = (rest.height - CARDS_MIN_H).clamp(CHART_MIN_H, CHART_MAX_H);
        let chart = take_top(&mut rest, h);
        draw_live_chart(f, chart, app, group);
    }
    draw_live_cards(f, rest, app, group);
}

/// Round dial: coloured arc, ticks with labels, needle, digital value below.
fn draw_dial(f: &mut Frame, area: Rect, app: &App, item: &Item, dial: &Dial) {
    let value = app.live.latest(item);
    let unit = app.live.unit(item);
    let block = pane(format!(" {} ", item.label_en), false);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [face, text] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);
    if face.width == 0 || face.height == 0 {
        return;
    }

    // Braille dots are 2 wide and 4 high per cell. Fit the arc (about 2.4 wide, 1.75
    // high around y = 0.225) into the face without squashing the circle.
    let aspect = (face.width as f64 * 2.0) / (face.height as f64 * 4.0);
    let (x_span, y_span) = if aspect * 1.75 >= 2.4 {
        (1.75 * aspect, 1.75)
    } else {
        (2.4, 2.4 / aspect)
    };
    let (cx, cy) = (0.0, 0.225);
    let cell_w = x_span / face.width as f64;

    let at = |deg: f64, r: f64| (r * deg.to_radians().cos(), r * deg.to_radians().sin());
    let ticks = ((dial.max - dial.min) / dial.major).round() as usize;
    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([cx - x_span / 2.0, cx + x_span / 2.0])
        .y_bounds([cy - y_span / 2.0, cy + y_span / 2.0])
        .paint(|ctx| {
            for zone in [Zone::Ok, Zone::Warn, Zone::Bad] {
                let mut dots = Vec::new();
                for step in 0..=480 {
                    let frac = step as f64 / 480.0;
                    let v = dial.min + (dial.max - dial.min) * frac;
                    if dial.zone(v) != zone {
                        continue;
                    }
                    let deg = ARC_START - ARC_SWEEP * frac;
                    for r in [0.94, 0.97, 1.0] {
                        dots.push(at(deg, r));
                    }
                }
                ctx.draw(&Points {
                    coords: &dots,
                    color: zone_color(zone),
                });
            }
            for k in 0..=ticks {
                let v = dial.min + k as f64 * dial.major;
                let deg = needle_angle(v, dial.min, dial.max);
                let ((x1, y1), (x2, y2)) = (at(deg, 0.82), at(deg, 0.93));
                ctx.draw(&CanvasLine {
                    x1,
                    y1,
                    x2,
                    y2,
                    color: Color::White,
                });
                if k % dial.label_every == 0 {
                    let label = format!("{}", (v / dial.label_div).round());
                    let (x, y) = at(deg, 0.68);
                    let x = x - label.len() as f64 * cell_w / 2.0;
                    ctx.print(x, y, Span::styled(label, Style::new().fg(Color::Gray)));
                }
            }
            if let Some(v) = value {
                let deg = needle_angle(v, dial.min, dial.max);
                let (x2, y2) = at(deg, 0.85);
                ctx.draw(&CanvasLine {
                    x1: 0.0,
                    y1: 0.0,
                    x2,
                    y2,
                    color: zone_color(dial.zone(v)),
                });
            }
            let hub: Vec<(f64, f64)> = (0..36).map(|d| at(d as f64 * 10.0, 0.05)).collect();
            ctx.draw(&Points {
                coords: &hub,
                color: Color::White,
            });
        });
    f.render_widget(canvas, face);

    let line = match value {
        Some(v) => Line::from(vec![
            Span::styled(
                crate::live::format_value(v),
                Style::new().fg(zone_color(dial.zone(v))).bold(),
            ),
            Span::styled(format!(" {unit}"), Style::new().fg(DIM)),
        ]),
        None => Line::styled("...", Style::new().fg(DIM)),
    };
    f.render_widget(line.centered(), text);
}

const CHART_COLORS: [Color; 3] = [M_LIGHT, WARN, Color::Magenta];

/// Line chart of the selected value, or up to three overlaid and normalised.
fn draw_live_chart(f: &mut Frame, area: Rect, app: &App, group: &str) {
    let picked = app.live.chart_items(group);
    let overlay = picked.len() > 1;
    let now = app.live.last_t;
    // Points per value, scaled to 0..1 when overlaid so different units share the axis.
    let mut names = Vec::new();
    let mut series: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut span = 1.0f64;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for item in &picked {
        let pts = app
            .live
            .samples
            .get(&item.key())
            .map_or_else(Vec::new, |h| h.points(now));
        let (min, max) = pts
            .iter()
            .map(|p| p.1)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), y| {
                (a.min(y), b.max(y))
            });
        if let Some(first) = pts.first() {
            span = span.max(-first.0);
        }
        let unit = app.live.unit(item);
        if overlay {
            let flat = (max - min).abs() < f64::EPSILON;
            let scaled = pts
                .iter()
                .map(|&(x, y)| (x, if flat { 0.5 } else { (y - min) / (max - min) }))
                .collect();
            let range = if min.is_finite() {
                format!(
                    "  {}..{} {unit}",
                    crate::live::format_value(min),
                    crate::live::format_value(max)
                )
            } else {
                String::new()
            };
            names.push(format!("{}{range}", item.label_en));
            series.push(scaled);
        } else {
            lo = min;
            hi = max;
            names.push(item.label_en.clone());
            series.push(pts);
        }
    }
    let unit = picked.first().map(|i| app.live.unit(i)).unwrap_or_default();
    let (y_lo, y_hi, y_labels) = if overlay {
        (0.0, 1.0, vec!["low".to_string(), "high".to_string()])
    } else if lo.is_finite() {
        // A little room above and below; flat data gets a band around it.
        let pad = if (hi - lo).abs() < f64::EPSILON {
            1.0
        } else {
            (hi - lo) * 0.1
        };
        let (a, b) = (lo - pad, hi + pad);
        let label = |v: f64| format!("{} {unit}", crate::live::format_value(v));
        (a, b, vec![label(a), label(b)])
    } else {
        (0.0, 1.0, vec![String::new(), String::new()])
    };

    let datasets: Vec<Dataset> = series
        .iter()
        .zip(&names)
        .enumerate()
        .map(|(i, (pts, name))| {
            Dataset::default()
                .name(name.clone())
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::new().fg(CHART_COLORS[i % CHART_COLORS.len()]))
                .data(pts)
        })
        .collect();
    let title = if overlay {
        " Chart: values side by side (G: one value) "
    } else {
        " Chart (↑↓ pick value, G: overlay) "
    };
    let axis_style = Style::new().fg(DIM);
    let x_axis = Axis::default()
        .style(axis_style)
        .bounds([-span, 0.0])
        .labels(vec![format!("-{span:.0} s"), "now".into()]);
    let y_axis = Axis::default()
        .style(axis_style)
        .bounds([y_lo, y_hi])
        .labels(y_labels);
    let chart = Chart::new(datasets)
        .block(pane(title, false))
        .x_axis(x_axis)
        .y_axis(y_axis);
    f.render_widget(chart, area);
}

fn draw_live_cards(f: &mut Frame, area: Rect, app: &App, group: &str) {
    let title = crate::live::preset_for(group).map_or("Values", |p| p.title);
    let block = pane(format!(" {title} "), false);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let items = app.live.items(group);
    if items.is_empty() {
        let text = if crate::live::preset_for(group).is_some() {
            "This car's module has none of the usual values. Press A for the full list."
        } else {
            "No quick view for this module. Press A for the full list of values."
        };
        f.render_widget(
            Paragraph::new(Line::styled(text, Style::new().fg(DIM))).wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }
    let cols = (inner.width / LIVE_CARD_W).max(1) as usize;
    let rows = (inner.height / LIVE_CARD_H).max(1) as usize;
    // Scroll just far enough to keep the value the chart shows in view.
    let selected = app.live.chart.min(items.len() - 1);
    let first = (selected / cols).saturating_sub(rows - 1) * cols;
    let shown: Vec<Key> = app
        .live
        .chart_items(group)
        .iter()
        .map(|i| i.key())
        .collect();
    let col_rects = Layout::horizontal(vec![Constraint::Fill(1); cols]).split(inner);
    for (n, item) in items.iter().skip(first).take(cols * rows).enumerate() {
        let (r, c) = (n / cols, n % cols);
        let y = inner.y + r as u16 * LIVE_CARD_H;
        let rect = Rect {
            x: col_rects[c].x,
            width: col_rects[c].width,
            y,
            height: LIVE_CARD_H.min(inner.bottom().saturating_sub(y)),
        };
        draw_live_card(f, rect, app, item, shown.contains(&item.key()));
    }
    let hidden = items.len().saturating_sub(first + cols * rows);
    if first > 0 || hidden > 0 {
        let hint = format!(" {hidden} more below, {first} above (↑↓) ");
        let line = Line::styled(hint, Style::new().fg(DIM)).right_aligned();
        f.render_widget(
            line,
            Rect {
                y: area.bottom().saturating_sub(1),
                height: 1,
                x: area.x + 1,
                width: area.width.saturating_sub(2),
            },
        );
    }
}

/// One value: label on the border, big number with unit, bar gauge where the
/// value has a sane range, sparkline below. The border marks what the chart shows.
fn draw_live_card(f: &mut Frame, area: Rect, app: &App, item: &Item, charted: bool) {
    let failed = app.live.errors.contains_key(&item.job.to_ascii_uppercase());
    let border = if failed {
        M_RED
    } else if charted {
        ACCENT
    } else {
        DIM
    };
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title(Line::styled(
            format!(" {} ", item.label_en),
            Style::new().bold(),
        ));
    if !item.label_de.is_empty() {
        block = block.title_bottom(
            Line::styled(format!(" {} ", item.label_de), Style::new().fg(DIM)).right_aligned(),
        );
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let latest = app.live.latest(item);
    let range = range_for(&item.label_en);
    let value_line = match (latest, failed) {
        (Some(v), false) => {
            let color = range.map_or(ACCENT, |r| zone_color(r.zone(v)));
            Line::from(vec![
                Span::styled(crate::live::format_value(v), Style::new().fg(color).bold()),
                Span::styled(format!(" {}", app.live.unit(item)), Style::new().fg(DIM)),
            ])
        }
        (_, true) => Line::styled("no answer", Style::new().fg(M_RED).bold()),
        (None, false) => Line::styled("...", Style::new().fg(DIM)),
    };
    let gauge_h = u16::from(range.is_some() && inner.height >= 3);
    let [value, gauge, spark] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(gauge_h),
        Constraint::Fill(1),
    ])
    .areas(inner);
    f.render_widget(value_line.centered(), value);
    if let (Some(r), Some(v)) = (range, latest.filter(|_| gauge_h > 0)) {
        let color = zone_color(r.zone(v));
        f.render_widget(
            LineGauge::default()
                .ratio(r.fraction(v))
                .label("")
                .filled_style(Style::new().fg(color))
                .unfilled_style(Style::new().fg(DIM)),
            gauge,
        );
    }
    if let Some(h) = app.live.samples.get(&item.key()) {
        let bars = h.bars(spark.width as usize);
        f.render_widget(
            Sparkline::default()
                .data(bars)
                .max(100)
                .style(Style::new().fg(M_LIGHT)),
            spark,
        );
    }
}

/// Every number the module's variant file can report; Space picks what to watch.
fn draw_live_expert(f: &mut Frame, area: Rect, app: &mut App, group: &str) {
    let all = app.live.all_items.get(group).map_or(&[][..], Vec::as_slice);
    let rows: Vec<ListItem> = all
        .iter()
        .map(|item| {
            let (mark, color) = if app.live.is_watched(group, item) {
                ("[x] ", OK)
            } else {
                ("[ ] ", DIM)
            };
            let value = app.live.latest(item).map_or(String::new(), |v| {
                format!("{} {}", crate::live::format_value(v), app.live.unit(item))
            });
            ListItem::new(Line::from(vec![
                Span::styled(mark, Style::new().fg(color)),
                Span::raw(format!("{:<34}", item.label_en)),
                Span::styled(format!("{value:<16}"), Style::new().fg(ACCENT)),
                Span::styled(
                    format!("{}  {}", item.job, item.label_de),
                    Style::new().fg(DIM),
                ),
            ]))
        })
        .collect();
    let title = format!(" All values ({}) ", rows.len());
    if rows.is_empty() {
        draw_live_message(
            f,
            area,
            "This module reports no numbers without arguments.",
            DIM,
        );
        return;
    }
    let widget = List::new(rows)
        .block(pane(title, true))
        .highlight_style(highlight());
    f.render_stateful_widget(widget, area, &mut app.live.list);
}

fn draw_adapter(f: &mut Frame, area: Rect, app: &App) {
    let check = |ok: bool, text: String| {
        let (mark, color) = if ok { ("✔", OK) } else { ("✘", WARN) };
        Line::from(vec![
            Span::styled(format!("  {mark} "), Style::new().fg(color)),
            Span::raw(text),
        ])
    };
    let mut lines = Vec::new();
    if app.ports.is_empty() {
        lines.push(Line::styled(
            "No USB serial adapter found. Plug in the K+DCAN cable.",
            Style::new().fg(WARN),
        ));
    }
    for p in &app.ports {
        lines.push(Line::from(vec![
            Span::styled(p.dev.display().to_string(), Style::new().fg(ACCENT).bold()),
            Span::raw(format!(
                "  {}  [{}]  serial {}",
                p.product, p.vid_pid, p.serial
            )),
        ]));
        lines.push(check(
            p.is_ftdi(),
            format!("driver {} (K+DCAN cables use ftdi_sio)", p.driver),
        ));
        lines.push(check(p.accessible, "read/write access".into()));
        if !p.accessible {
            lines.push(Line::styled(
                "      fix: sudo usermod -aG dialout $USER   (then log out and in)",
                Style::new().fg(DIM),
            ));
        }
        let latency_ok = p.latency_ms == Some(1);
        let latency = p.latency_ms.map_or("unknown".into(), |l| format!("{l} ms"));
        lines.push(check(
            latency_ok,
            format!("latency timer {latency} (K-line needs 1 ms)"),
        ));
        if !latency_ok && p.is_ftdi() {
            let name = p.dev.file_name().unwrap_or_default().to_string_lossy();
            lines.push(Line::styled(
                format!(
                    "      fix: echo 1 | sudo tee /sys/bus/usb-serial/devices/{name}/latency_timer"
                ),
                Style::new().fg(DIM),
            ));
        }
        lines.push(Line::default());
    }
    lines.push(Line::styled(
        "Car: E87 (pre-LCI, 2006) talks K-line. Set the cable's switch to K-line if it has one.",
        Style::new().fg(DIM),
    ));
    lines.push(Line::styled(
        "The K-line side is powered by OBD pin 16, so echo tests need the car.",
        Style::new().fg(DIM),
    ));
    let widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(pane(" Adapter ", false));
    f.render_widget(widget, area);
}
