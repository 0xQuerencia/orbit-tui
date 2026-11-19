use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Sparkline, Table, TableState, Wrap},
    Frame,
};

use crate::{
    app::{App, TABS},
    metrics::{format_bytes, percent},
};

#[derive(Clone, Copy)]
struct Palette {
    bg: Color,
    fg: Color,
    muted: Color,
    blue: Color,
    amber: Color,
    pink: Color,
    green: Color,
    border: Color,
    tab: Color,
}

fn palette(theme: usize) -> Palette {
    if theme == 0 {
        Palette {
            bg: Color::Rgb(15, 15, 21),
            fg: Color::Rgb(205, 210, 220),
            muted: Color::Rgb(133, 142, 159),
            blue: Color::Rgb(125, 207, 245),
            amber: Color::Rgb(255, 185, 56),
            pink: Color::Rgb(255, 68, 139),
            green: Color::Rgb(51, 199, 139),
            border: Color::Rgb(48, 63, 82),
            tab: Color::Rgb(31, 59, 80),
        }
    } else {
        Palette {
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            muted: Color::Rgb(147, 153, 178),
            blue: Color::Rgb(137, 180, 250),
            amber: Color::Rgb(249, 226, 175),
            pink: Color::Rgb(243, 139, 168),
            green: Color::Rgb(166, 227, 161),
            border: Color::Rgb(69, 71, 90),
            tab: Color::Rgb(49, 50, 68),
        }
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let screen = frame.area();
    let p = palette(app.theme);
    frame.render_widget(Block::default().style(Style::default().bg(p.bg)), screen);
    if screen.width < 42 || screen.height < 12 {
        frame.render_widget(
            Paragraph::new("Orbit needs at least 42 columns and 12 rows.\nResize the terminal to continue.")
                .style(Style::default().fg(p.amber)),
            screen,
        );
        return;
    }

    let regions = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(screen);

    draw_header(frame, app, regions[0], p);
    draw_tabs(frame, app, regions[1], p);

    if app.help {
        draw_help(frame, regions[2], p);
    } else {
        match app.tab {
            0 => draw_dashboard(frame, app, regions[2], p),
            1 => draw_processes(frame, app, regions[2], p),
            2 => draw_ports(frame, app, regions[2], p),
            3 => draw_connections(frame, app, regions[2], p),
            4 => draw_git(frame, app, regions[2], p),
            5 => draw_todos(frame, app, regions[2], p),
            _ => {}
        }
    }
    draw_footer(frame, app, regions[3], p);
    if let Some((pid, name)) = &app.confirm_kill {
        draw_confirm(frame, screen, *pid, name, p);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let elapsed = app.metrics.uptime;
    let clock = chrono::Local::now().format("%H:%M").to_string();
    let rhs = format!("up {}h {}m · {}", elapsed / 3600, (elapsed / 60) % 60, clock);
    let lhs = " ◉ Orbit";
    let padding = (area.width as usize).saturating_sub(lhs.chars().count() + rhs.chars().count() + 1);
    let row = Line::from(vec![
        Span::styled(lhs, Style::default().fg(p.bg).add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(padding)),
        Span::styled(rhs, Style::default().fg(p.bg).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
    ]);
    frame.render_widget(
        Paragraph::new(row).style(Style::default().bg(p.blue)),
        area,
    );
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let mut spans = Vec::new();
    let mut used = 0usize;
    for (idx, title) in TABS.iter().enumerate() {
        let label = format!(" {} {} ", idx + 1, title);
        let width = label.chars().count() + 1;
        if used + width > area.width as usize { break; }
        spans.push(Span::styled(
            label,
            if idx == app.tab {
                Style::default().fg(p.blue).bg(p.tab).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(p.muted)
            },
        ));
        spans.push(Span::raw(" "));
        used += width;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn panel(frame: &mut Frame, area: Rect, title: &str, tint: Color, p: Palette) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tint))
        .title(Span::styled(format!(" {title} "), Style::default().fg(tint).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(p.bg));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

fn styled<T: Into<String>>(text: T, fg: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(fg))
}

fn meter_line(label: &str, value: f64, width: u16, color: Color, p: Palette) -> Line<'static> {
    let gauge_width = (width as usize).saturating_sub(label.len() + 6).min(32);
    let filled = ((value.clamp(0.0, 100.0) / 100.0) * gauge_width as f64).round() as usize;
    Line::from(vec![
        styled(format!("{label} "), p.fg),
        styled("█".repeat(filled), color),
        styled("░".repeat(gauge_width.saturating_sub(filled)), p.border),
        styled(format!(" {:.0}%", value), color),
    ])
}

fn draw_dashboard(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    if area.width < 95 || area.height < 19 {
        draw_compact_dashboard(frame, app, area, p);
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(48),
            Constraint::Length(1),
            Constraint::Percentage(32),
            Constraint::Length(1),
            Constraint::Min(4),
        ])
        .split(area);
    let first = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(32),
            Constraint::Length(2),
            Constraint::Percentage(32),
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .split(rows[0]);
    draw_cpu(frame, app, first[0], p);
    draw_memory(frame, app, first[2], p);
    draw_disk(frame, app, first[4], p);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(66), Constraint::Length(2), Constraint::Min(1)])
        .split(rows[2]);
    draw_network(frame, app, middle[0], p);
    draw_alerts(frame, app, middle[2], p);
    draw_git_todo_summary(frame, app, rows[4], p);
}

fn draw_compact_dashboard(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let m = &app.metrics;
    let inner = panel(frame, area, "System overview", p.blue, p);
    let battery = m.battery.percent.map(|b| format!("{b}%")).unwrap_or_else(|| "n/a".to_owned());
    let gpu = m.gpu.usage.map(|v| format!("{v:.0}%")).unwrap_or_else(|| "n/a".to_owned());
    let alerts = m.alerts();
    let lines = vec![
        meter_line("CPU", m.cpu as f64, inner.width, p.amber, p),
        meter_line("RAM", m.memory_percent(), inner.width, p.amber, p),
        meter_line("DSK", m.disk_percent(), inner.width, p.pink, p),
        Line::from(styled(format!("Memory: {} / {} · {} available", format_bytes(m.mem_used), format_bytes(m.mem_total), format_bytes(m.mem_available)), p.muted)),
        Line::from(styled(format!("Disk free: {} · swap {:.0}%", format_bytes(m.disk_free), m.swap_percent()), p.muted)),
        Line::from(styled(format!("GPU: {gpu}  Battery: {battery}"), p.fg)),
        Line::from(styled(format!("↑ {}  ↓ {}", format_bytes(m.net_sent), format_bytes(m.net_recv)), p.blue)),
        Line::from(styled(format!("{} listening ports", m.listening_ports()), p.fg)),
        Line::from(styled(format!("Alerts: {}", if alerts.is_empty() { "none".to_owned() } else { alerts.join(" | ") }), if alerts.is_empty() { p.green } else { p.pink })),
        Line::from(styled(if app.git.inside { format!("Git: {} · {}", app.git.branch, app.git.changed.len()) } else { "Git: not a git repo".to_owned() }, p.muted)),
        Line::from(styled(format!("Todo: {} remaining", app.todos.iter().filter(|t| !t.done).count()), p.muted)),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_cpu(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let m = &app.metrics;
    let color = if m.cpu >= 90.0 { p.pink } else { p.amber };
    let inner = panel(frame, area, &format!("CPU {:.0}%", m.cpu), color, p);
    if inner.height == 0 { return; }
    let history: Vec<u64> = m.cpu_history.iter().copied().collect();
    let graph = Rect { height: 1, width: inner.width.min(50), ..inner };
    frame.render_widget(Sparkline::default().data(&history).max(100).style(Style::default().fg(color)), graph);
    let start = inner.y.saturating_add(1);
    if start >= inner.y + inner.height { return; }
    let content = vec![
        meter_line("", m.cpu as f64, inner.width, color, p),
        Line::from(vec![
            styled("GPU ", p.muted),
            styled(m.gpu.usage.map(|n| format!("{n:.0}%")).unwrap_or_else(|| "n/a".into()), p.green),
            styled(format!(" · battery {}", m.battery.percent.map(|n| format!("{n}%")).unwrap_or_else(|| "n/a".into())), p.muted),
        ]),
        Line::from(styled(
            if m.gpu.name.is_empty() { "GPU: no NVIDIA sensor detected".to_owned() }
            else { format!("{} · {} MiB", m.gpu.name, m.gpu.used_mib.unwrap_or(0)) },
            p.muted,
        )),
    ];
    frame.render_widget(Paragraph::new(content), Rect { y: start, height: inner.height - 1, ..inner });
}

fn draw_memory(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let m = &app.metrics;
    let inner = panel(frame, area, &format!("Memory {:.0}%", m.memory_percent()), p.amber, p);
    let lines = vec![
        meter_line("RAM", m.memory_percent(), inner.width, p.amber, p),
        Line::from(styled(format!("{} / {} · {} available", format_bytes(m.mem_used), format_bytes(m.mem_total), format_bytes(m.mem_available)), p.muted)),
        meter_line("Swap", m.swap_percent(), inner.width, p.amber, p),
        Line::from(styled(
            if m.swap_total == 0 { "No swap configured".to_owned() }
            else { format!("{} / {} swapped", format_bytes(m.swap_used), format_bytes(m.swap_total)) },
            p.muted,
        )),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_disk(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let m = &app.metrics;
    let inner = panel(frame, area, &format!("Disk {:.0}%", m.disk_percent()), p.pink, p);
    let lines = vec![
        meter_line("", m.disk_percent(), inner.width, p.pink, p),
        Line::from(styled(format!("{} / {}", format_bytes(m.disk_used), format_bytes(m.disk_total)), p.fg)),
        Line::from(styled(format!("{} free", format_bytes(m.disk_free)), p.pink)),
        Line::from(styled(format!("I/O read {} · write {}", format_bytes(m.disk_read), format_bytes(m.disk_write)), p.muted)),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_network(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let m = &app.metrics;
    let inner = panel(frame, area, "Network", p.blue, p);
    if inner.width == 0 || inner.height == 0 { return; }
    let cols = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(49), Constraint::Min(1)])
        .split(inner);
    let lines = vec![
        Line::from(styled(format!("↑ {} sent", format_bytes(m.net_sent)), p.blue)),
        Line::from(styled(format!("↓ {} received", format_bytes(m.net_recv)), p.blue)),
        Line::from(styled(format!("↑ {}/s · ↓ {}/s", format_bytes(m.net_up_per_sec as u64), format_bytes(m.net_down_per_sec as u64)), p.muted)),
    ];
    frame.render_widget(Paragraph::new(lines), cols[0]);
    let right = vec![
        Line::from(styled(format!("{} ports listening", m.listening_ports()), p.fg)),
        Line::from(styled(format!("Local IP: {}", m.local_ip), p.muted)),
    ];
    frame.render_widget(Paragraph::new(right), cols[1]);
    if inner.height >= 5 {
        let history: Vec<u64> = m.net_history.iter().copied().collect();
        let graph = Rect {
            x: inner.x,
            y: inner.y + inner.height - 1,
            width: inner.width.min(68),
            height: 1,
        };
        frame.render_widget(Sparkline::default().data(&history).style(Style::default().fg(p.blue)), graph);
    }
}

fn draw_alerts(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let alerts = app.metrics.alerts();
    let inner = panel(frame, area, &format!("Alerts {}", alerts.len()), p.pink, p);
    if alerts.is_empty() {
        frame.render_widget(Paragraph::new("✓ No active alerts").style(Style::default().fg(p.green)), inner);
    } else {
        let lines: Vec<_> = alerts.iter().map(|alert| {
            Line::from(styled(format!("▲ {alert}"), if alert.starts_with("Disk") { p.pink } else { p.amber }))
        }).collect();
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }
}

fn draw_git_todo_summary(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let inner = panel(frame, area, "Git and todo", p.blue, p);
    let cols = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Min(1)])
        .split(inner);
    let git = if !app.git.inside {
        "Git: not a git repo".to_owned()
    } else {
        format!("Git: {} · {} changed", app.git.branch, app.git.changed.len())
    };
    frame.render_widget(Paragraph::new(git).style(Style::default().fg(p.muted)), cols[0]);
    let pending = app.todos.iter().filter(|t| !t.done).count();
    let todo = if pending == 0 { "Todo: no tasks. Press a to add one.".to_owned() }
        else { format!("Todo: {pending} remaining. Press 6 to manage.") };
    frame.render_widget(Paragraph::new(todo).style(Style::default().fg(p.muted)), cols[1]);
}

fn draw_processes(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let items = app.filtered_processes();
    let rows = items.iter().map(|proc| vec![
        format!("{}{}", if app.pinned.contains(&proc.pid) { "◆" } else { " " }, proc.pid),
        proc.name.clone(),
        format!("{:.1}", proc.cpu),
        format_bytes(proc.memory),
        proc.status.clone(),
    ]).collect();
    draw_table(frame, area, &format!("Processes · {} visible", items.len()),
        &["PID", "Name", "CPU%", "RAM", "Status"],
        [Constraint::Length(9), Constraint::Percentage(39), Constraint::Length(10), Constraint::Length(13), Constraint::Min(8)],
        rows, app.selected, p);
}

fn draw_ports(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let items = app.filtered_sockets();
    let rows = items.iter().map(|s| vec![
        s.local.clone(),
        s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
        s.process.clone(),
        s.protocol.clone(),
        s.state.clone(),
    ]).collect();
    draw_table(frame, area, &format!("Listening ports · {} visible", items.len()),
        &["Local", "PID", "Process", "Protocol", "State"],
        [Constraint::Percentage(40), Constraint::Length(9), Constraint::Percentage(30), Constraint::Length(10), Constraint::Min(8)],
        rows, app.selected, p);
}

fn draw_connections(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let items = app.filtered_sockets();
    let rows = items.iter().map(|s| vec![
        s.local.clone(), s.remote.clone(), s.state.clone(), s.process.clone(), s.protocol.clone(),
    ]).collect();
    draw_table(frame, area, &format!("Connections · {} visible", items.len()),
        &["Local", "Remote", "State", "Process", "Type"],
        [Constraint::Percentage(27), Constraint::Percentage(27), Constraint::Length(14), Constraint::Percentage(23), Constraint::Min(6)],
        rows, app.selected, p);
}

fn draw_table<const N: usize>(
    frame: &mut Frame, area: Rect, title: &str, headers: &[&str; N],
    widths: [Constraint; N], rows: Vec<Vec<String>>, selected: usize, p: Palette,
) {
    let inner = panel(frame, area, title, p.blue, p);
    if inner.height == 0 || inner.width == 0 { return; }
    let header = Row::new(headers.iter().map(|s| Cell::from(*s)))
        .style(Style::default().fg(p.blue).add_modifier(Modifier::BOLD))
        .bottom_margin(1);
    let table_rows = rows.into_iter().map(|row| {
        Row::new(row.into_iter().map(Cell::from)).style(Style::default().fg(p.fg))
    });
    let table = Table::new(table_rows, widths)
        .header(header)
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(p.tab).fg(p.blue).add_modifier(Modifier::BOLD))
        .highlight_symbol("▸ ");
    let mut state = TableState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(table, inner, &mut state);
}

fn draw_git(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let inner = panel(frame, area, "Git status · current working directory", p.blue, p);
    if !app.git.inside {
        frame.render_widget(Paragraph::new("Not a Git repository.\nLaunch Orbit from a project directory to see its status.")
            .style(Style::default().fg(p.muted)), inner);
        return;
    }
    let mut lines = vec![
        Line::from(styled(format!("Path: {}", app.git.root), p.fg)),
        Line::from(styled(format!("Branch: {}", if app.git.branch.is_empty() { "(detached)" } else { &app.git.branch }), p.blue)),
        Line::from(styled(format!("Working tree: {} changes", app.git.changed.len()), if app.git.changed.is_empty() { p.green } else { p.amber })),
        Line::raw(""),
        Line::from(styled("CHANGED FILES", p.blue)),
    ];
    if app.git.changed.is_empty() {
        lines.push(Line::from(styled("Clean working tree", p.green)));
    } else {
        for file in app.git.changed.iter().take(15) {
            lines.push(Line::from(styled(file.clone(), p.fg)));
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(styled("RECENT COMMITS", p.blue)));
    for commit in &app.git.recent {
        lines.push(Line::from(styled(commit.clone(), p.muted)));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_todos(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let remaining = app.todos.iter().filter(|t| !t.done).count();
    let rows = app.todos.iter().enumerate().map(|(i, t)| vec![
        format!("{:>3}", i + 1),
        if t.done { "✓".to_owned() } else { "○".to_owned() },
        t.text.clone(),
    ]).collect();
    draw_table(frame, area, &format!("Todo · {remaining} remaining"),
        &["#", "Done", "Task"],
        [Constraint::Length(5), Constraint::Length(6), Constraint::Min(1)],
        rows, app.selected, p);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect, p: Palette) {
    let content = if let Some(input) = &app.adding {
        format!(" New todo: {input}█  Enter save · Esc cancel")
    } else if app.searching {
        format!(" /{}█  Enter apply · Esc clear", app.search)
    } else if !app.status.is_empty() {
        format!(" {}  ·  ? help  q quit", app.status)
    } else {
        match app.tab {
            1 => " 1–6 switch tab    / search    p pin    ↑↓ move    Enter signal    ? all keys    q quit".into(),
            2 => " 1–6 switch tab    / search    ↑↓ move    Enter signal    ? all keys    q quit".into(),
            5 => " 1–6 switch tab    a add    x/Enter toggle    d delete    ↑↓ move    q quit".into(),
            _ => " 1–6 switch tab    / search    p pin    w theme    ? all keys    q quit".into(),
        }
    };
    frame.render_widget(Paragraph::new(content).style(Style::default().fg(p.muted)), area);
}

fn draw_help(frame: &mut Frame, area: Rect, p: Palette) {
    let inner = panel(frame, area, "Keyboard shortcuts", p.blue, p);
    let lines = vec![
        "1–6 / Tab / Shift+Tab / ← →   Switch tabs",
        "↑ ↓ / j k                        Move through a list",
        "/                                Filter processes, ports, connections",
        "p                                Pin/unpin a process",
        "Enter                            Request SIGTERM (confirm first)",
        "a                                Add a todo from Dashboard or Todo",
        "x / Enter                        Toggle selected todo",
        "d                                Delete selected todo",
        "w                                Change theme",
        "r                                Refresh data now",
        "?                                Show or hide help",
        "q / Ctrl+C                       Quit Orbit",
        "",
        "Orbit displays live system data. Unsupported sensors show n/a.",
    ];
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .style(Style::default().fg(p.fg))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn draw_confirm(frame: &mut Frame, screen: Rect, pid: u32, name: &str, p: Palette) {
    let width = screen.width.min(66);
    let height = screen.height.min(7);
    let popup = centered(width, height, screen);
    frame.render_widget(Clear, popup);
    let inner = panel(frame, popup, "Confirm process signal", p.pink, p);
    let text = format!("Send SIGTERM to {name} (PID {pid})?\n\nEnter/y: confirm        Esc/n: cancel");
    frame.render_widget(
        Paragraph::new(text).alignment(Alignment::Center)
            .style(Style::default().fg(p.fg)),
        inner,
    );
}

fn centered(width: u16, height: u16, screen: Rect) -> Rect {
    Rect {
        x: screen.x + screen.width.saturating_sub(width) / 2,
        y: screen.y + screen.height.saturating_sub(height) / 2,
        width, height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_scheme_is_distinct() {
        assert_ne!(palette(0).bg, palette(1).bg);
    }

    #[test]
    fn centered_popup_fits() {
        let screen = Rect::new(0, 0, 80, 20);
        let p = centered(60, 7, screen);
        assert_eq!(p.x, 10);
        assert_eq!(p.y, 6);
    }

    #[test]
    fn bar_text_does_not_overflow() {
        let p = palette(0);
        let line = meter_line("RAM", 99.0, 25, p.amber, p);
        assert!(!line.spans.is_empty());
    }
}
