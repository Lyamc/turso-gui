pub mod grid;
pub mod ids;
pub mod theme;

use iced::mouse;
use iced::widget::operation::snap_to;
use iced::widget::Id;
use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_editor, text_input,
    Space,
};
use iced::{event, window, Alignment, Element, Event, Length, Point, Size, Task, Theme};
use rfd::FileDialog;
use turso_gui_core::{AppModel, StatusKind, Tab, WindowPlacement};

use self::grid::view_results;
use self::ids::ScrollIds;
use self::theme::*;
use crate::Args;

pub fn run(args: Args) -> anyhow::Result<()> {
    let place = WindowPlacement::suggested();
    iced::application(
        move || TursoGui::new(args.clone()),
        TursoGui::update,
        TursoGui::view,
    )
    .title(get_title)
    .theme(get_theme)
    .subscription(TursoGui::subscription)
    .window(window::Settings {
        size: Size::new(place.logical_width, place.logical_height),
        position: window::Position::Specific(Point::new(place.logical_x, place.logical_y)),
        min_size: Some(Size::new(place.min_logical_width, place.min_logical_height)),
        ..Default::default()
    })
    .run()?;
    Ok(())
}

fn get_title(_state: &TursoGui) -> String {
    "Turso DB Browser (iced)".to_string()
}

fn get_theme(_state: &TursoGui) -> Theme {
    Theme::Dark
}

struct TursoGui {
    model: AppModel,
    cell_editor_content: text_editor::Content,
    browse_ids: ScrollIds,
    sql_ids: ScrollIds,
    resizing_column: Option<usize>,
    last_mouse_pos: f32,
    filter_version: u32,
}

#[derive(Debug, Clone)]
enum Message {
    DbPathChanged(String),
    Connect,
    CloseDatabase,
    SwitchTab(Tab),
    NewDatabase,
    OpenDatabase,
    WriteChanges,
    RevertChanges,
    SelectTableStructure(String),
    BrowseTableSelected(String),
    BrowseFilterChanged(usize, String),
    SqlFilterChanged(usize, String),
    FilterDebounce(u32, bool),
    ChangePage(u32),
    ChangePageSize(u32),
    ToggleAllResults(bool),
    SortColumn(usize),
    CellSelected(usize, usize, String),
    CellEditorAction(text_editor::Action),
    StartResize(usize),
    ResizeMoved(f32),
    EndResize,
    Scrolled(Id, scrollable::Viewport),
    QueryChanged(String),
    ExecuteQuery,
}

impl TursoGui {
    fn new(args: Args) -> (Self, Task<Message>) {
        let gui = Self {
            model: AppModel::from_args(args.database, args.token, args.debug),
            cell_editor_content: text_editor::Content::new(),
            browse_ids: ScrollIds::default(),
            sql_ids: ScrollIds::default(),
            resizing_column: None,
            last_mouse_pos: 0.0,
            filter_version: 0,
        };
        (gui, Task::none())
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        if self.resizing_column.is_some() {
            event::listen_with(|event, _, _| match event {
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    Some(Message::ResizeMoved(position.x))
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    Some(Message::EndResize)
                }
                _ => None,
            })
        } else {
            iced::Subscription::none()
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DbPathChanged(p) => {
                self.model.db_path = p;
                Task::none()
            }
            Message::Connect => {
                let _ = self.model.connect();
                Task::none()
            }
            Message::CloseDatabase => {
                self.model.close();
                self.cell_editor_content = text_editor::Content::new();
                Task::none()
            }
            Message::SwitchTab(t) => {
                self.model.switch_tab(t);
                Task::none()
            }
            Message::NewDatabase => {
                if let Some(p) = FileDialog::new()
                    .add_filter("SQLite", &["db", "sqlite"])
                    .save_file()
                {
                    let _ = self.model.open_path(p.to_string_lossy().to_string());
                }
                Task::none()
            }
            Message::OpenDatabase => {
                if let Some(p) = FileDialog::new()
                    .add_filter("SQLite", &["db", "sqlite"])
                    .pick_file()
                {
                    let _ = self.model.open_path(p.to_string_lossy().to_string());
                }
                Task::none()
            }
            Message::WriteChanges => {
                let _ = self.model.write_changes();
                Task::none()
            }
            Message::RevertChanges => {
                let _ = self.model.revert_changes();
                Task::none()
            }
            Message::SelectTableStructure(name) => {
                let _ = self.model.select_structure_table(&name);
                Task::none()
            }
            Message::BrowseTableSelected(name) => {
                let _ = self.model.browse_table_named(&name);
                Task::none()
            }
            Message::BrowseFilterChanged(i, v) => {
                if i >= self.model.browse_state.filters.len() {
                    self.model
                        .browse_state
                        .filters
                        .resize(i + 1, String::new());
                }
                self.model.browse_state.filters[i] = v;
                self.model.browse_state.page = 0;
                self.filter_version += 1;
                let version = self.filter_version;
                Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        version
                    },
                    move |v| Message::FilterDebounce(v, true),
                )
            }
            Message::SqlFilterChanged(i, v) => {
                if i >= self.model.sql_state.filters.len() {
                    self.model.sql_state.filters.resize(i + 1, String::new());
                }
                self.model.sql_state.filters[i] = v;
                self.model.sql_state.page = 0;
                self.filter_version += 1;
                let version = self.filter_version;
                Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        version
                    },
                    move |v| Message::FilterDebounce(v, false),
                )
            }
            Message::FilterDebounce(v, browse) => {
                if v == self.filter_version {
                    if browse {
                        let _ = self.model.load_browse_data();
                    } else {
                        let _ = self.model.execute_query();
                    }
                }
                Task::none()
            }
            Message::SortColumn(i) => {
                let _ = self.model.sort_column(i);
                Task::none()
            }
            Message::ChangePage(p) => {
                let _ = self.model.change_page(p);
                Task::none()
            }
            Message::ChangePageSize(s) => {
                let _ = self.model.change_page_size(s);
                Task::none()
            }
            Message::ToggleAllResults(all) => {
                let _ = self.model.toggle_all_results(all);
                Task::none()
            }
            Message::CellSelected(r, c, v) => {
                let state = match self.model.active_tab {
                    Tab::ExecuteSql => &mut self.model.sql_state,
                    _ => &mut self.model.browse_state,
                };
                state.selected_cell = Some((r, c));
                self.model.cell_editor = v.clone();
                self.cell_editor_content = text_editor::Content::with_text(&v);
                Task::none()
            }
            Message::CellEditorAction(a) => {
                self.cell_editor_content.perform(a);
                self.model.cell_editor = self.cell_editor_content.text();
                Task::none()
            }
            Message::StartResize(i) => {
                self.resizing_column = Some(i);
                Task::none()
            }
            Message::ResizeMoved(x) => {
                if let Some(i) = self.resizing_column {
                    if self.last_mouse_pos != 0.0 {
                        let delta = x - self.last_mouse_pos;
                        self.model.adjust_column_width(i, delta);
                    }
                }
                self.last_mouse_pos = x;
                Task::none()
            }
            Message::EndResize => {
                self.resizing_column = None;
                self.last_mouse_pos = 0.0;
                Task::none()
            }
            Message::Scrolled(id, viewport) => {
                let ids = if self.model.active_tab == Tab::BrowseData {
                    &self.browse_ids
                } else {
                    &self.sql_ids
                };
                if id == ids.data {
                    let rel = viewport.relative_offset();
                    return Task::batch(vec![
                        snap_to(
                            ids.header.clone(),
                            scrollable::RelativeOffset { x: rel.x, y: 0.0 },
                        ),
                        snap_to(
                            ids.row_index.clone(),
                            scrollable::RelativeOffset { x: 0.0, y: rel.y },
                        ),
                    ]);
                }
                Task::none()
            }
            Message::QueryChanged(q) => {
                self.model.query = q;
                Task::none()
            }
            Message::ExecuteQuery => {
                let _ = self.model.execute_query();
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let body: Element<_> = if self.model.is_connected() {
            column![
                self.view_toolbar(),
                self.view_tabs(),
                self.view_status(),
                container(match self.model.active_tab {
                    Tab::Structure => self.view_structure(),
                    Tab::BrowseData => self.view_browse(),
                    Tab::ExecuteSql => self.view_sql(),
                })
                .width(Length::Fill)
                .height(Length::Fill)
            ]
            .spacing(0)
            .into()
        } else {
            self.view_connect()
        };

        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(window_container_style)
            .into()
    }

    fn view_status(&self) -> Element<'_, Message> {
        let content = match self.model.status_text() {
            Some((msg, StatusKind::Info)) => text(msg).color(COLOR_ACCENT),
            Some((msg, StatusKind::Error)) => text(msg).color(COLOR_ERROR),
            Some((msg, StatusKind::Success)) => text(msg).color(COLOR_SUCCESS),
            None => text(""),
        };

        container(content.size(12))
            .height(25)
            .padding([2, 10])
            .width(Length::Fill)
            .into()
    }

    fn view_connect(&self) -> Element<'_, Message> {
        column![
            text("Connect to Turso / SQLite")
                .size(30)
                .color(COLOR_TEXT_PRIMARY),
            text_input("Database Path", &self.model.db_path)
                .on_input(Message::DbPathChanged)
                .padding(10),
            button(text("Connect").color(COLOR_TEXT_PRIMARY))
                .on_press(Message::Connect)
                .padding(10)
                .style(|_, _| button::Style::default().with_background(COLOR_BG_SELECTED)),
            row![
                self.tool_btn("New Database", Message::NewDatabase, true),
                self.tool_btn("Open Database", Message::OpenDatabase, true),
            ]
            .spacing(10)
        ]
        .spacing(20)
        .padding(50)
        .max_width(500)
        .align_x(Alignment::Center)
        .into()
    }

    fn view_toolbar(&self) -> Element<'_, Message> {
        row![
            self.tool_btn("New", Message::NewDatabase, true),
            self.tool_btn("Open", Message::OpenDatabase, true),
            self.tool_btn("Close", Message::CloseDatabase, true),
            self.tool_btn("Write", Message::WriteChanges, self.model.has_changes),
            self.tool_btn("Revert", Message::RevertChanges, self.model.has_changes),
            Space::new().width(Length::Fill),
            text(&self.model.db_path).size(14).color(COLOR_TEXT_DIM),
        ]
        .spacing(10)
        .padding(10)
        .into()
    }

    fn tool_btn<'a>(&self, label: &'a str, msg: Message, active: bool) -> Element<'a, Message> {
        button(text(label).color(if active {
            COLOR_TEXT_PRIMARY
        } else {
            COLOR_TEXT_DIM
        }))
        .on_press_maybe(if active { Some(msg) } else { None })
        .padding(8)
        .style(tool_button_style)
        .into()
    }

    fn view_tabs(&self) -> Element<'_, Message> {
        row![
            self.tab_btn("Structure", Tab::Structure),
            self.tab_btn("Browse", Tab::BrowseData),
            self.tab_btn("SQL", Tab::ExecuteSql),
        ]
        .spacing(0)
        .into()
    }

    fn tab_btn<'a>(&self, label: &'a str, t: Tab) -> Element<'a, Message> {
        let active = self.model.active_tab == t;
        button(
            container(text(label).color(if active {
                COLOR_TEXT_PRIMARY
            } else {
                COLOR_TEXT_DIM
            }))
            .padding([10, 20])
            .style(move |_| container::Style {
                background: Some(
                    (if active {
                        COLOR_BG_SELECTED
                    } else {
                        COLOR_BG_HEADER
                    })
                    .into(),
                ),
                ..Default::default()
            }),
        )
        .on_press(Message::SwitchTab(t))
        .padding(0)
        .style(|_, _| button::Style::default())
        .into()
    }

    fn view_structure(&self) -> Element<'_, Message> {
        let list = scrollable(
            column(
                self.model
                    .tables
                    .iter()
                    .map(|t| {
                        button(text(&t.name).color(COLOR_TEXT_PRIMARY))
                            .on_press(Message::SelectTableStructure(t.name.clone()))
                            .width(Length::Fill)
                            .style(|_, _| button::Style::default().with_background(COLOR_BG_CELL))
                            .into()
                    })
                    .collect::<Vec<_>>(),
            )
            .spacing(2),
        )
        .width(200)
        .height(Length::Fill);

        let details = scrollable(
            column![
                text("Columns").size(20).color(COLOR_TEXT_PRIMARY),
                column(
                    self.model
                        .selected_table_columns
                        .iter()
                        .map(|c| {
                            row![
                                text(if c.pk { "🔑" } else { "  " }),
                                text(&c.name).width(150).color(COLOR_TEXT_PRIMARY),
                                text(&c.data_type).width(100).color(COLOR_TEXT_DIM)
                            ]
                            .spacing(10)
                            .into()
                        })
                        .collect::<Vec<_>>(),
                )
                .spacing(5),
                rule::horizontal(1),
                text("Schema").size(20).color(COLOR_TEXT_PRIMARY),
                text(self.model.structure_sql()).color(COLOR_TEXT_PRIMARY)
            ]
            .spacing(20)
            .padding(20),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        row![list, rule::vertical(1), details]
            .height(Length::Fill)
            .into()
    }

    fn view_browse(&self) -> Element<'_, Message> {
        column![
            row![
                text("Table:").color(COLOR_TEXT_PRIMARY),
                pick_list(
                    self.model.table_names.as_slice(),
                    self.model.browse_table.clone(),
                    Message::BrowseTableSelected
                ),
                self.tool_btn(
                    "Refresh",
                    match &self.model.browse_table {
                        Some(name) => Message::BrowseTableSelected(name.clone()),
                        None => Message::Connect,
                    },
                    true
                ),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .padding(5),
            row![
                container(view_results(
                    &self.model.browse_state,
                    &self.browse_ids,
                    true,
                    Message::SortColumn,
                    Message::BrowseFilterChanged,
                    Message::CellSelected,
                    Message::StartResize,
                    Message::ChangePage,
                    Message::ChangePageSize,
                    Message::ToggleAllResults,
                    Message::Scrolled
                ))
                .width(Length::FillPortion(3))
                .height(Length::Fill),
                rule::vertical(1),
                self.view_editor()
            ]
            .height(Length::Fill)
            .spacing(0)
        ]
        .spacing(0)
        .height(Length::Fill)
        .into()
    }

    fn view_sql(&self) -> Element<'_, Message> {
        column![
            row![
                text_input("Enter SQL query here", &self.model.query)
                    .on_input(Message::QueryChanged)
                    .on_submit(Message::ExecuteQuery)
                    .padding(10),
                self.tool_btn("Execute", Message::ExecuteQuery, true),
            ]
            .spacing(10)
            .padding(5),
            row![
                container(view_results(
                    &self.model.sql_state,
                    &self.sql_ids,
                    true,
                    Message::SortColumn,
                    Message::SqlFilterChanged,
                    Message::CellSelected,
                    Message::StartResize,
                    Message::ChangePage,
                    Message::ChangePageSize,
                    Message::ToggleAllResults,
                    Message::Scrolled
                ))
                .width(Length::FillPortion(3))
                .height(Length::Fill),
                rule::vertical(1),
                self.view_editor()
            ]
            .height(Length::Fill)
            .spacing(0)
        ]
        .spacing(0)
        .height(Length::Fill)
        .into()
    }

    fn view_editor(&self) -> Element<'_, Message> {
        container(
            column![
                text("Cell Editor").size(18).color(COLOR_TEXT_PRIMARY),
                container(
                    text_editor(&self.cell_editor_content).on_action(Message::CellEditorAction)
                )
                .height(Length::Fill)
                .width(Length::Fill)
                .padding(5)
                .style(|_| container::Style {
                    background: Some(COLOR_BG_WINDOW.into()),
                    ..Default::default()
                }),
            ]
            .spacing(10),
        )
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .padding(10)
        .style(|_| container::Style {
            background: Some(COLOR_BG_CELL.into()),
            ..Default::default()
        })
        .into()
    }
}
