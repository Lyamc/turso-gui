use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use turso_gui_core::Tab;

use crate::{App, Mode, Popup};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if is_quit_chord(key) {
        app.should_quit = true;
        return;
    }

    if matches!(key.code, KeyCode::F(1)) {
        if matches!(app.popup, Popup::Help) {
            app.close_popup();
        } else {
            app.popup = Popup::Help;
        }
        return;
    }

    if matches!(app.popup, Popup::Help) {
        handle_help(app, key);
        return;
    }

    if matches!(app.popup, Popup::Path { .. }) {
        handle_path_popup(app, key);
        return;
    }

    if matches!(app.popup, Popup::Filter { .. }) {
        handle_filter_popup(app, key);
        return;
    }

    if !app.model.is_connected() {
        handle_connect(app, key);
        return;
    }

    if is_execute_key(key) {
        app.execute_sql();
        return;
    }

    match app.mode {
        Mode::Insert => handle_insert(app, key),
        Mode::Normal => handle_normal(app, key),
    }
}

fn handle_help(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {
            app.close_popup();
        }
        _ => {}
    }
}

fn handle_connect(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Enter => {
            app.apply_path();
        }
        KeyCode::Char('?') => {
            app.popup = Popup::Help;
        }
        _ => {
            app.path_editor.input(key);
        }
    }
}

fn handle_path_popup(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.close_popup(),
        KeyCode::Enter => app.apply_path(),
        _ => {
            app.path_editor.input(key);
        }
    }
}

fn handle_filter_popup(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.close_popup(),
        KeyCode::Enter => app.apply_filter(),
        _ => {
            app.filter_editor.input(key);
        }
    }
}

fn handle_insert(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Esc {
        if app.cell_focus {
            app.sync_cell_editor();
            app.cell_focus = false;
            app.mode = Mode::Normal;
            return;
        }
        app.mode = Mode::Normal;
        if app.model.active_tab == Tab::ExecuteSql {
            app.sync_query();
        }
        return;
    }

    if app.cell_focus {
        app.cell_editor.input(key);
        app.sync_cell_editor();
        return;
    }

    if key.code == KeyCode::BackTab {
        app.cycle_tab(true);
        return;
    }
    if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.cycle_tab(false);
        return;
    }

    if app.model.active_tab == Tab::ExecuteSql {
        // Keep Tab as indent inside the SQL editor.
        app.sql_editor.input(key);
        app.sync_query();
        return;
    }

    app.mode = Mode::Normal;
    handle_normal(app, key);
}

fn handle_normal(app: &mut App, key: KeyEvent) {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char('q') if !ctrl => {
            app.should_quit = true;
        }
        KeyCode::Char('?') => {
            app.popup = Popup::Help;
        }
        KeyCode::Tab => app.cycle_tab(false),
        KeyCode::BackTab => app.cycle_tab(true),
        KeyCode::Char('1') => app.switch_tab(Tab::Structure),
        KeyCode::Char('2') => app.switch_tab(Tab::BrowseData),
        KeyCode::Char('3') => app.switch_tab(Tab::ExecuteSql),
        KeyCode::Char('o') if !ctrl => app.open_path_popup(false),
        KeyCode::Char('O') => app.open_path_popup(false),
        KeyCode::Char('n') | KeyCode::Char('N') if shift || key.code == KeyCode::Char('N') => {
            app.open_path_popup(true);
        }
        KeyCode::Char('w') if !ctrl => {
            let _ = app.model.write_changes();
        }
        KeyCode::Char('u') | KeyCode::Char('R') => {
            let _ = app.model.revert_changes();
        }
        KeyCode::Char('r') if shift => {
            let _ = app.model.revert_changes();
        }
        KeyCode::Esc => {
            if app.show_cell_pane {
                app.show_cell_pane = false;
                app.cell_focus = false;
            }
        }
        KeyCode::Char('i') => handle_insert_key(app),
        _ => match app.model.active_tab {
            Tab::Structure => handle_structure(app, key),
            Tab::BrowseData => handle_browse(app, key),
            Tab::ExecuteSql => handle_sql_normal(app, key),
        },
    }
}

fn handle_insert_key(app: &mut App) {
    match app.model.active_tab {
        Tab::ExecuteSql => {
            app.cell_focus = false;
            app.mode = Mode::Insert;
        }
        Tab::BrowseData => {
            if !app.show_cell_pane {
                app.inspect_cell();
            }
            app.cell_focus = true;
            app.mode = Mode::Insert;
        }
        Tab::Structure => {}
    }
}

fn handle_structure(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => app.move_structure(1),
        KeyCode::Up | KeyCode::Char('k') => app.move_structure(-1),
        KeyCode::PageDown => app.move_structure(10),
        KeyCode::PageUp => app.move_structure(-10),
        KeyCode::Char('g') | KeyCode::Char('G') => {
            let last = key.modifiers.contains(KeyModifiers::SHIFT)
                || matches!(key.code, KeyCode::Char('G'));
            let len = app.model.table_names.len();
            if len > 0 {
                app.table_list.select(Some(if last { len - 1 } else { 0 }));
            }
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            app.load_structure_selected();
        }
        KeyCode::Char('n') => app.open_path_popup(true),
        _ => {}
    }
}

fn handle_browse(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => app.move_grid(1, 0),
        KeyCode::Up | KeyCode::Char('k') => app.move_grid(-1, 0),
        KeyCode::Left | KeyCode::Char('h') => app.move_grid(0, -1),
        KeyCode::Right | KeyCode::Char('l') => app.move_grid(0, 1),
        KeyCode::Home => app.jump_grid_col(false),
        KeyCode::End => app.jump_grid_col(true),
        KeyCode::Char('g') | KeyCode::Char('G') => {
            let last = key.modifiers.contains(KeyModifiers::SHIFT)
                || matches!(key.code, KeyCode::Char('G'));
            app.jump_grid_row(last);
        }
        KeyCode::Enter => app.inspect_cell(),
        KeyCode::Char('/') => app.open_filter_popup(),
        KeyCode::Char('s') => app.sort_current_column(),
        KeyCode::Char('n') | KeyCode::PageDown => app.page_delta(1),
        KeyCode::Char('p') | KeyCode::PageUp => app.page_delta(-1),
        KeyCode::Char('+') | KeyCode::Char('=') => app.cycle_page_size(true),
        KeyCode::Char('-') => app.cycle_page_size(false),
        KeyCode::Char('a') => app.toggle_all_results(),
        KeyCode::Char('[') => app.cycle_browse_table(-1),
        KeyCode::Char(']') | KeyCode::Char('t') => app.cycle_browse_table(1),
        _ => {}
    }
}

fn handle_sql_normal(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => app.move_grid(1, 0),
        KeyCode::Up | KeyCode::Char('k') => app.move_grid(-1, 0),
        KeyCode::Left | KeyCode::Char('h') => app.move_grid(0, -1),
        KeyCode::Right | KeyCode::Char('l') => app.move_grid(0, 1),
        KeyCode::Home => app.jump_grid_col(false),
        KeyCode::End => app.jump_grid_col(true),
        KeyCode::Char('g') | KeyCode::Char('G') => {
            let last = key.modifiers.contains(KeyModifiers::SHIFT)
                || matches!(key.code, KeyCode::Char('G'));
            app.jump_grid_row(last);
        }
        KeyCode::Enter => app.inspect_cell(),
        KeyCode::Char('/') => app.open_filter_popup(),
        KeyCode::Char('s') => app.sort_current_column(),
        KeyCode::Char('n') | KeyCode::PageDown => app.page_delta(1),
        KeyCode::Char('p') | KeyCode::PageUp => app.page_delta(-1),
        KeyCode::Char('+') | KeyCode::Char('=') => app.cycle_page_size(true),
        KeyCode::Char('-') => app.cycle_page_size(false),
        KeyCode::Char('a') => app.toggle_all_results(),
        _ => {}
    }
}

fn is_quit_chord(key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('q'))
}

fn is_execute_key(key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    matches!(key.code, KeyCode::F(5))
        || (ctrl && matches!(key.code, KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('E')))
}
