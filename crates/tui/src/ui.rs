use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, HighlightSpacing, List, ListItem, Padding, Paragraph, Row,
    Table, Tabs, Wrap,
};
use ratatui::Frame;
use tui_textarea::TextArea;
use turso_gui_core::{GridState, StatusKind, Tab};

use crate::{App, Mode, Popup};

const BG: Color = Color::Rgb(13, 13, 13);
const HEADER_BG: Color = Color::Rgb(51, 51, 51);
const CELL_BG: Color = Color::Rgb(26, 26, 26);
const CELL_ALT: Color = Color::Rgb(18, 18, 18);
const SELECTED: Color = Color::Rgb(25, 102, 204);
const ACCENT: Color = Color::Cyan;
const TEXT: Color = Color::Rgb(235, 235, 235);
const DIM: Color = Color::Rgb(140, 140, 140);
const ERROR: Color = Color::Rgb(255, 102, 102);
const SUCCESS: Color = Color::Rgb(102, 220, 102);
const DIRTY: Color = Color::Yellow;
const PK: Color = Color::Rgb(255, 196, 72);

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(BG).fg(TEXT)),
        area,
    );

    if area.width < 40 || area.height < 8 {
        f.render_widget(
            Paragraph::new("Terminal too small")
                .alignment(Alignment::Center)
                .style(Style::default().fg(ERROR)),
            area,
        );
        return;
    }

    let title = header_title(app, area.width);
    let outer = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(ACCENT))
        .title(title)
        .style(Style::default().bg(BG).fg(TEXT));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    draw_tabs(f, app, chunks[0]);

    if !app.model.is_connected() && matches!(app.popup, Popup::None | Popup::Help) {
        draw_connect(f, app, chunks[1]);
    } else {
        match app.model.active_tab {
            Tab::Structure => draw_structure(f, app, chunks[1]),
            Tab::BrowseData => draw_browse(f, app, chunks[1]),
            Tab::ExecuteSql => draw_sql(f, app, chunks[1]),
        }
    }

    draw_status(f, app, chunks[2]);
    draw_footer(f, app, chunks[3]);

    match app.popup {
        Popup::Help => draw_help(f, area),
        Popup::Path { new } => draw_path_popup(f, app, area, new),
        Popup::Filter { col, sql } => draw_filter_popup(f, app, area, col, sql),
        Popup::None => {}
    }
}

fn header_title(app: &App, width: u16) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            " Turso DB Browser (TUI) ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];
    let path_budget = width.saturating_sub(42) as usize;
    let path = truncate(&app.model.db_path, path_budget.max(8));
    spans.push(Span::styled(path, Style::default().fg(TEXT)));
    if app.model.has_changes {
        spans.push(Span::styled(
            "  [dirty*] ",
            Style::default().fg(DIRTY).add_modifier(Modifier::BOLD),
        ));
    } else if app.model.is_connected() {
        spans.push(Span::styled("  connected", Style::default().fg(SUCCESS)));
    } else {
        spans.push(Span::styled("  not connected", Style::default().fg(DIM)));
    }
    Line::from(spans)
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::all()
        .into_iter()
        .enumerate()
        .map(|(i, tab)| {
            let label = format!(" {} {} ", i + 1, tab.label());
            Line::from(Span::raw(label))
        })
        .collect();
    let selected = Tab::all()
        .iter()
        .position(|t| *t == app.model.active_tab)
        .unwrap_or(0);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(36), Constraint::Length(40)])
        .split(area);

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(Style::default().fg(DIM).bg(HEADER_BG))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled("│", Style::default().fg(DIM)));
    f.render_widget(tabs, chunks[0]);

    let actions = Line::from(vec![
        Span::styled(" N", Style::default().fg(ACCENT).bold()),
        Span::styled(":new  ", Style::default().fg(DIM)),
        Span::styled("o", Style::default().fg(ACCENT).bold()),
        Span::styled(":open  ", Style::default().fg(DIM)),
        Span::styled("w", Style::default().fg(ACCENT).bold()),
        Span::styled(":write  ", Style::default().fg(DIM)),
        Span::styled("u", Style::default().fg(ACCENT).bold()),
        Span::styled(":revert", Style::default().fg(DIM)),
    ]);
    f.render_widget(
        Paragraph::new(actions)
            .alignment(Alignment::Right)
            .style(Style::default().bg(HEADER_BG)),
        chunks[1],
    );
}

fn draw_connect(f: &mut Frame, app: &mut App, area: Rect) {
    let box_area = centered_rect(74, 80, area);
    f.render_widget(Clear, box_area);
    f.render_widget(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .title(" Connect ")
            .style(Style::default().bg(BG).fg(TEXT)),
        box_area,
    );
    let inner = Block::bordered().inner(box_area);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(3),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("Open a local SQLite / Turso file")
            .alignment(Alignment::Center)
            .style(Style::default().fg(TEXT)),
        parts[0],
    );

    style_input(&mut app.path_editor, " Path  Enter connect  Esc quit ", true);
    f.render_widget(&app.path_editor, parts[1]);

    f.render_widget(
        Paragraph::new("type a path   Enter connect   Esc / Ctrl+C quit")
            .alignment(Alignment::Center)
            .style(Style::default().fg(DIM)),
        parts[2],
    );

    let help = Text::from(vec![
        Line::from(Span::styled(
            "Keyboard-driven DB browser",
            Style::default().fg(ACCENT),
        )),
        Line::from(""),
        Line::from("1 Structure   2 Browse   3 SQL"),
        Line::from("Enter inspect cell    / filter    s sort"),
        Line::from("F5 / Ctrl+Enter run SQL     ? help"),
    ]);
    f.render_widget(
        Paragraph::new(help)
            .alignment(Alignment::Center)
            .style(Style::default().fg(DIM)),
        parts[3],
    );
}

fn draw_structure(f: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
        .split(area);

    let n_tables = app.model.table_names.len();
    let items: Vec<ListItem> = if n_tables == 0 {
        vec![ListItem::new(Span::styled(
            " (no tables) ",
            Style::default().fg(DIM).italic(),
        ))]
    } else {
        app.model
            .table_names
            .iter()
            .map(|n| ListItem::new(Span::raw(format!(" {n}"))))
            .collect()
    };
    let list = List::new(items)
        .block(
            Block::bordered()
                .border_style(active_border(true))
                .title(format!(" Tables ({n_tables}) "))
                .title_bottom(Line::from(Span::styled(
                    " Enter/l load  j/k move ",
                    Style::default().fg(DIM),
                ))),
        )
        .highlight_style(
            Style::default()
                .bg(SELECTED)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    f.render_stateful_widget(list, cols[0], &mut app.table_list);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cols[1]);

    draw_structure_columns(f, app, right[0]);
    draw_structure_sql(f, app, right[1]);
}

fn draw_structure_columns(f: &mut Frame, app: &App, area: Rect) {
    let name = app
        .model
        .selected_structure_table
        .as_deref()
        .unwrap_or("—");
    let header = Row::new(["PK", "Name", "Type", "Not Null"]).style(
        Style::default()
            .fg(ACCENT)
            .bg(HEADER_BG)
            .add_modifier(Modifier::BOLD),
    );
    let rows: Vec<Row> = app
        .model
        .selected_table_columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let pk = if c.pk {
                Span::styled(" ●", Style::default().fg(PK).bold())
            } else {
                Span::styled("  ", Style::default().fg(DIM))
            };
            let nn = if c.not_null { "NOT NULL" } else { "null" };
            let nn_style = if c.not_null {
                Style::default().fg(TEXT)
            } else {
                Style::default().fg(DIM)
            };
            Row::new(vec![
                Cell::from(pk),
                Cell::from(Span::styled(c.name.clone(), Style::default().fg(TEXT))),
                Cell::from(Span::styled(
                    c.data_type.clone(),
                    Style::default().fg(ACCENT),
                )),
                Cell::from(Span::styled(nn, nn_style)),
            ])
            .style(if i % 2 == 1 {
                Style::default().bg(CELL_ALT)
            } else {
                Style::default().bg(CELL_BG)
            })
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Min(12),
            Constraint::Min(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::bordered()
            .border_style(active_border(false))
            .title(format!(" Columns · {name} ")),
    )
    .row_highlight_style(Style::default().bg(SELECTED));
    f.render_widget(table, area);
}

fn draw_structure_sql(f: &mut Frame, app: &App, area: Rect) {
    let sql = app.model.structure_sql();
    let text = if sql.is_empty() {
        Text::from(Span::styled(
            "Select a table and press Enter to load CREATE SQL.",
            Style::default().fg(DIM).italic(),
        ))
    } else {
        Text::from(sql.to_string())
    };
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(TEXT))
            .block(
                Block::bordered()
                    .border_style(active_border(false))
                    .title(" SQL ")
                    .padding(Padding::horizontal(1)),
            ),
        area,
    );
}

fn draw_browse(f: &mut Frame, app: &mut App, area: Rect) {
    let show_cell = app.show_cell_pane;
    let chunks = if show_cell {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(7),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(4)])
            .split(area)
    };

    draw_browse_toolbar(f, app, chunks[0]);
    let focused = app.mode == Mode::Normal && matches!(app.popup, Popup::None);
    let title = grid_title("Browse", &app.model.browse_state);
    render_grid(
        f,
        chunks[1],
        &app.model.browse_state,
        &mut app.browse_table,
        title,
        focused,
    );
    if show_cell {
        draw_cell_pane(f, app, chunks[2]);
    }
}

fn draw_browse_toolbar(f: &mut Frame, app: &App, area: Rect) {
    let table = app.model.browse_table.as_deref().unwrap_or("—");
    let idx = app
        .model
        .browse_table
        .as_ref()
        .and_then(|b| app.model.table_names.iter().position(|n| n == b))
        .map(|i| i + 1)
        .unwrap_or(0);
    let n = app.model.table_names.len();
    let line = Line::from(vec![
        Span::styled(" Table ", Style::default().fg(DIM)),
        Span::styled(
            table.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  [{idx}/{n}]  "), Style::default().fg(DIM)),
        Span::styled("[", Style::default().fg(ACCENT)),
        Span::raw(" "),
        Span::styled("]", Style::default().fg(ACCENT)),
        Span::styled(" or t cycle   ", Style::default().fg(DIM)),
        Span::styled("s", Style::default().fg(ACCENT)),
        Span::styled(" sort  ", Style::default().fg(DIM)),
        Span::styled("/", Style::default().fg(ACCENT)),
        Span::styled(" filter  ", Style::default().fg(DIM)),
        Span::styled("n/p", Style::default().fg(ACCENT)),
        Span::styled(" page", Style::default().fg(DIM)),
    ]);
    f.render_widget(
        Paragraph::new(line).block(
            Block::bordered()
                .border_style(active_border(false))
                .border_type(BorderType::Plain),
        ),
        area,
    );
}

fn draw_sql(f: &mut Frame, app: &mut App, area: Rect) {
    let show_cell = app.show_cell_pane;
    let cell_h = if show_cell { 7 } else { 0 };
    let rest = area.height.saturating_sub(cell_h);
    let editor_h = (rest / 2).clamp(5, 14);
    let chunks = if show_cell {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(editor_h),
                Constraint::Min(4),
                Constraint::Length(7),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(editor_h), Constraint::Min(4)])
            .split(area)
    };

    let sql_focused = app.is_editing()
        && matches!(app.popup, Popup::None)
        && !app.cell_focus
        && app.model.active_tab == Tab::ExecuteSql;
    style_sql_editor(&mut app.sql_editor, sql_focused);
    f.render_widget(&app.sql_editor, chunks[0]);

    let results_focused = app.mode == Mode::Normal && matches!(app.popup, Popup::None);
    let title = grid_title("Results", &app.model.sql_state);
    render_grid(
        f,
        chunks[1],
        &app.model.sql_state,
        &mut app.sql_table,
        title,
        results_focused,
    );
    if show_cell {
        draw_cell_pane(f, app, chunks[2]);
    }
}

fn draw_cell_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let (row, col) = match app.model.active_tab {
        Tab::ExecuteSql => app.sql_table.selected_cell(),
        _ => app.browse_table.selected_cell(),
    }
    .unwrap_or((0, 0));
    let header = match app.model.active_tab {
        Tab::ExecuteSql => app.model.sql_state.headers.get(col).cloned(),
        _ => app.model.browse_state.headers.get(col).cloned(),
    }
    .unwrap_or_else(|| "?".into());
    let focused = app.cell_focus && app.mode == Mode::Insert && matches!(app.popup, Popup::None);
    let title = format!(" Cell  r{row}  {header}  Enter inspect  i edit  Esc close ");
    style_input(&mut app.cell_editor, &title, focused);
    f.render_widget(&app.cell_editor, area);
}

fn render_grid(
    f: &mut Frame,
    area: Rect,
    grid: &GridState,
    state: &mut ratatui::widgets::TableState,
    title: String,
    focused: bool,
) {
    let block = Block::bordered()
        .border_style(active_border(focused))
        .title(format!(" {title} "));

    if !grid.rows.is_empty() && !grid.headers.is_empty() {
        if state.selected().is_none() {
            state.select(Some(0));
        }
        if state.selected_column().is_none() {
            state.select_column(Some(0));
        }
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    if grid.headers.is_empty() {
        f.render_widget(
            Paragraph::new("No rows.")
                .style(Style::default().fg(DIM).italic())
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let (filter_area, table_area) = if inner.height >= 4 {
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);
        (Some(parts[0]), parts[1])
    } else {
        (None, inner)
    };

    if let Some(filter_area) = filter_area {
        render_filter_row(f, filter_area, grid, state.selected_column());
    }

    let header_cells: Vec<Cell> = grid
        .headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let mut label = h.clone();
            if let Some((c, dir)) = grid.sort {
                if c == i {
                    label.push(' ');
                    label.push_str(dir.icon());
                }
            }
            if grid
                .filters
                .get(i)
                .map(|f| !f.is_empty())
                .unwrap_or(false)
            {
                label.push_str(" *");
            }
            let mut style = Style::default()
                .fg(ACCENT)
                .bg(HEADER_BG)
                .add_modifier(Modifier::BOLD);
            if grid
                .filters
                .get(i)
                .map(|f| !f.is_empty())
                .unwrap_or(false)
            {
                style = style.fg(DIRTY);
            }
            Cell::from(Span::styled(label, style))
        })
        .collect();
    let header = Row::new(header_cells).style(Style::default().bg(HEADER_BG));

    let rows: Vec<Row> = grid
        .rows
        .iter()
        .enumerate()
        .map(|(ri, row)| {
            let cells: Vec<Cell> = grid
                .headers
                .iter()
                .enumerate()
                .map(|(ci, _)| {
                    let raw = row.get(ci).map(String::as_str).unwrap_or("");
                    let shown = display_cell(raw);
                    let style = if raw == "NULL" {
                        Style::default().fg(DIM).italic()
                    } else {
                        Style::default().fg(TEXT)
                    };
                    Cell::from(Span::styled(shown, style))
                })
                .collect();
            Row::new(cells).style(if ri % 2 == 1 {
                Style::default().bg(CELL_ALT)
            } else {
                Style::default().bg(CELL_BG)
            })
        })
        .collect();

    let widths = grid_constraints(grid);
    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(Color::Rgb(20, 50, 80)))
        .column_highlight_style(Style::default().fg(ACCENT))
        .cell_highlight_style(
            Style::default()
                .bg(SELECTED)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ")
        .highlight_spacing(HighlightSpacing::Always);
    f.render_stateful_widget(table, table_area, state);
}

fn render_filter_row(f: &mut Frame, area: Rect, grid: &GridState, selected_col: Option<usize>) {
    let cells: Vec<Cell> = grid
        .headers
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let raw = grid.filters.get(i).map(String::as_str).unwrap_or("");
            let (text, mut style) = if raw.is_empty() {
                ("Filter…", Style::default().fg(DIM).italic())
            } else {
                (raw, Style::default().fg(DIRTY))
            };
            style = style.bg(HEADER_BG);
            if selected_col == Some(i) {
                style = Style::default()
                    .fg(Color::White)
                    .bg(SELECTED)
                    .add_modifier(Modifier::BOLD);
            }
            Cell::from(Span::styled(text.to_string(), style))
        })
        .collect();
    let table = Table::new(vec![Row::new(cells)], grid_constraints(grid));
    f.render_widget(table, area);
}

fn grid_title(kind: &str, grid: &GridState) -> String {
    let pages = grid.total_pages();
    let all = if grid.all_results { "  ALL" } else { "" };
    format!(
        "{kind}  page {}/{}  {} rows  size {}{all}",
        grid.page + 1,
        pages,
        grid.total_records,
        grid.page_size
    )
}

fn grid_constraints(grid: &GridState) -> Vec<Constraint> {
    let n = grid.headers.len();
    if n == 0 {
        return vec![Constraint::Percentage(100)];
    }
    (0..n)
        .map(|i| {
            let mut w = grid.headers.get(i).map(|h| h.len()).unwrap_or(4) + 2;
            if let Some((c, _)) = grid.sort {
                if c == i {
                    w += 2;
                }
            }
            for row in grid.rows.iter().take(40) {
                if let Some(v) = row.get(i) {
                    w = w.max(v.chars().count().min(28));
                }
            }
            Constraint::Min((w as u16).clamp(6, 36))
        })
        .collect()
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let (msg, style) = if let Some((text, kind)) = app.model.status_text() {
        let color = match kind {
            StatusKind::Error => ERROR,
            StatusKind::Success => SUCCESS,
            StatusKind::Info => ACCENT,
        };
        (
            truncate(text, area.width.saturating_sub(2) as usize),
            Style::default().fg(color).bg(HEADER_BG),
        )
    } else {
        let hint = match app.model.active_tab {
            Tab::Structure => "Structure · Enter loads columns and CREATE SQL",
            Tab::BrowseData => "Browse · arrows move  Enter inspect  / filter  s sort",
            Tab::ExecuteSql => "SQL · i edit  Esc results  F5 run  / filter  s sort",
        };
        (
            hint.to_string(),
            Style::default().fg(DIM).bg(HEADER_BG),
        )
    };
    f.render_widget(
        Paragraph::new(msg).style(style).alignment(Alignment::Left),
        area,
    );
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let pairs: &[(&str, &str)] = if !app.model.is_connected() {
        &[("Enter", "connect"), ("Esc", "quit"), ("Ctrl+C", "quit")]
    } else if matches!(app.popup, Popup::Help) {
        &[("Esc", "close"), ("?", "close")]
    } else if matches!(app.popup, Popup::Path { .. } | Popup::Filter { .. }) {
        &[("Enter", "apply"), ("Esc", "cancel")]
    } else if app.mode == Mode::Insert {
        if app.model.active_tab == Tab::ExecuteSql && !app.cell_focus {
            &[
                ("Esc", "normal"),
                ("F5", "run"),
                ("Ctrl+Enter", "run"),
                ("Ctrl+C", "quit"),
            ]
        } else {
            &[("Esc", "normal"), ("Ctrl+C", "quit")]
        }
    } else {
        &[
            ("q", "quit"),
            ("Tab", "cycle"),
            ("Enter", "select"),
            ("?", "help"),
            ("i", "insert"),
        ]
    };
    f.render_widget(
        Paragraph::new(keys(pairs))
            .style(Style::default().fg(DIM).bg(BG))
            .alignment(Alignment::Left),
        area,
    );
}

fn keys(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::with_capacity(pairs.len() * 2);
    for (k, label) in pairs {
        spans.push(Span::styled(
            format!(" {k} "),
            Style::default().fg(Color::Black).bg(ACCENT),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::default().fg(DIM),
        ));
    }
    Line::from(spans)
}

fn draw_help(f: &mut Frame, area: Rect) {
    let popup = centered_rect(78, 85, area);
    f.render_widget(Clear, popup);
    let text = Text::from(vec![
        Line::from(Span::styled(
            "Keybindings",
            Style::default().fg(ACCENT).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled("Global (normal)", Style::default().fg(DIRTY))),
        Line::from("  q              quit          Ctrl+C        quit"),
        Line::from("  Tab / S-Tab    cycle tabs    1 2 3        Structure / Browse / SQL"),
        Line::from("  o              open path     N            new path"),
        Line::from("  w              write/commit  u / R        revert/rollback"),
        Line::from("  F5 / Ctrl+E    execute SQL   F1 / ?       help"),
        Line::from("  Esc            close popup / leave insert (does not quit)"),
        Line::from(""),
        Line::from(Span::styled("Connect", Style::default().fg(DIRTY))),
        Line::from("  type a path    Enter connect     Esc quit"),
        Line::from(""),
        Line::from(Span::styled("Structure", Style::default().fg(DIRTY))),
        Line::from("  j/k  ↑↓        move tables       Enter / l    load columns + SQL"),
        Line::from("  g / G          first / last"),
        Line::from(""),
        Line::from(Span::styled("Browse", Style::default().fg(DIRTY))),
        Line::from("  h/j/k/l arrows move cell         Enter        inspect cell"),
        Line::from("  [ ] / t        cycle tables      s            sort column"),
        Line::from("  /              filter column     n p PgUp/Dn  page"),
        Line::from("  + / -          page size         a            toggle all rows"),
        Line::from("  i              edit cell pane    Home / End   first / last column"),
        Line::from(""),
        Line::from(Span::styled("SQL", Style::default().fg(DIRTY))),
        Line::from("  i              edit query        Esc          results mode"),
        Line::from("  Ctrl+Enter/F5  execute           /            filter results"),
        Line::from("  s              sort column       n p + - a    paging"),
        Line::from(""),
        Line::from(Span::styled(
            "Esc / q / ?  close this overlay",
            Style::default().fg(DIM),
        )),
    ]);
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(TEXT).bg(BG))
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(ACCENT))
                    .title(" Help ")
                    .title_alignment(Alignment::Center)
                    .padding(Padding::new(2, 2, 0, 0))
                    .style(Style::default().bg(BG)),
            ),
        popup,
    );
}

fn draw_path_popup(f: &mut Frame, app: &mut App, area: Rect, new: bool) {
    let popup = centered_rect(70, 30, area);
    f.render_widget(Clear, popup);
    let title = if new {
        " New database path "
    } else {
        " Open database "
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(title)
        .style(Style::default().bg(BG));
    f.render_widget(block, popup);
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Length(3), Constraint::Min(1)])
        .split(popup);

    f.render_widget(
        Paragraph::new("Enter applies  Esc cancels")
            .alignment(Alignment::Center)
            .style(Style::default().fg(DIM)),
        inner[0],
    );
    style_input(&mut app.path_editor, " Path ", true);
    f.render_widget(&app.path_editor, inner[1]);
}

fn draw_filter_popup(f: &mut Frame, app: &mut App, area: Rect, col: usize, sql: bool) {
    let popup = centered_rect(70, 32, area);
    f.render_widget(Clear, popup);
    let name = {
        let headers = if sql {
            &app.model.sql_state.headers
        } else {
            &app.model.browse_state.headers
        };
        headers
            .get(col)
            .cloned()
            .unwrap_or_else(|| "column".to_string())
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(format!(" Filter · {name} "))
        .style(Style::default().bg(BG));
    f.render_widget(block, popup);
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(2), Constraint::Length(3), Constraint::Min(1)])
        .split(popup);
    f.render_widget(
        Paragraph::new("Empty clears. NULL / NOT NULL / = > < >= <= <> else LIKE. Enter apply.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(DIM)),
        inner[0],
    );
    style_input(&mut app.filter_editor, " Filter ", true);
    f.render_widget(&app.filter_editor, inner[1]);
}

fn style_sql_editor(ta: &mut TextArea<'static>, focused: bool) {
    ta.set_style(Style::default().fg(TEXT).bg(CELL_BG));
    ta.set_line_number_style(Style::default().fg(DIM).bg(HEADER_BG));
    ta.set_selection_style(Style::default().bg(SELECTED));
    ta.set_placeholder_style(Style::default().fg(DIM).italic());
    let title = if focused {
        " SQL  Esc results  Ctrl+Enter / F5 run "
    } else {
        " SQL  i edit  F5 run "
    };
    ta.set_block(
        Block::bordered()
            .border_style(active_border(focused))
            .title(title),
    );
    if focused {
        ta.set_cursor_style(Style::default().bg(ACCENT).fg(Color::Black));
        ta.set_cursor_line_style(Style::default().bg(Color::Rgb(20, 32, 40)));
    } else {
        ta.set_cursor_style(Style::default());
        ta.set_cursor_line_style(Style::default());
    }
}

fn style_input(ta: &mut TextArea<'static>, title: &str, focused: bool) {
    ta.set_style(Style::default().fg(TEXT).bg(CELL_BG));
    ta.set_selection_style(Style::default().bg(SELECTED));
    ta.set_placeholder_style(Style::default().fg(DIM).italic());
    ta.set_block(
        Block::bordered()
            .border_style(active_border(focused))
            .title(title.to_string()),
    );
    if focused {
        ta.set_cursor_style(Style::default().bg(ACCENT).fg(Color::Black));
        ta.set_cursor_line_style(Style::default().bg(Color::Rgb(20, 32, 40)));
    } else {
        ta.set_cursor_style(Style::default());
        ta.set_cursor_line_style(Style::default());
    }
}

fn active_border(active: bool) -> Style {
    if active {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(DIM)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup[1])[1]
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        let take = max.saturating_sub(1);
        let t: String = s.chars().take(take).collect();
        format!("{t}…")
    }
}

fn display_cell(s: &str) -> String {
    s.replace('\n', "↵").replace('\t', " ")
}
