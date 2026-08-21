use iced::widget::Id;
use iced::widget::{
    button, checkbox, column, container, mouse_area, pick_list, row, scrollable, text, text_input,
    Space,
};
use iced::{Alignment, Element, Font, Length, Theme};
use turso_gui_core::{GridState, SortDirection};

use crate::gui::ids::ScrollIds;
use crate::gui::theme::*;

pub fn view_results<'a, Message>(
    state: &'a GridState,
    ids: &'a ScrollIds,
    filterable: bool,
    on_sort: impl Fn(usize) -> Message + Clone + 'a,
    on_filter: impl Fn(usize, String) -> Message + Clone + 'a,
    on_select: impl Fn(usize, usize, String) -> Message + Clone + 'a,
    on_resize_start: impl Fn(usize) -> Message + Clone + 'a,
    on_page_change: impl Fn(u32) -> Message + Clone + 'a,
    on_page_size_change: impl Fn(u32) -> Message + Clone + 'a,
    on_toggle_all: impl Fn(bool) -> Message + Clone + 'a,
    on_scroll: impl Fn(Id, scrollable::Viewport) -> Message + Clone + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    if state.headers.is_empty() {
        return container(text("No data to display").color(COLOR_TEXT_PRIMARY))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    }

    let header_height = if filterable { 55.0 } else { 30.0 };

    let corner = container(
        text("#")
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Default::default()
            })
            .color(COLOR_TEXT_DIM),
    )
    .width(40)
    .height(header_height)
    .align_y(Alignment::Center)
    .center_x(40)
    .style(header_container_style);

    let mut header_elements = Vec::new();
    for (i, name) in state.headers.iter().enumerate() {
        let width = state.widths.get(i).copied().unwrap_or(150.0);
        let sort_icon = match state.sort {
            Some((col, SortDirection::Asc)) if col == i => " ▲",
            Some((col, SortDirection::Desc)) if col == i => " ▼",
            _ => "",
        };

        let title_on_sort = on_sort.clone();
        let title_btn = button(
            container(
                text(format!("{}{}", name, sort_icon))
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..Default::default()
                    })
                    .color(COLOR_TEXT_PRIMARY)
                    .width(Length::Fill)
                    .shaping(text::Shaping::Advanced),
            )
            .width(Length::Fill)
            .height(30)
            .align_y(Alignment::Center)
            .padding([0, 5])
            .style(header_container_style),
        )
        .on_press(title_on_sort(i))
        .padding(0)
        .style(transparent_button_style);

        let filter_box: Element<Message> = if filterable {
            let val = state
                .filters
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or_default();
            let filter_on_input = on_filter.clone();
            container(
                text_input("Filter (=, >, NULL)…", val)
                    .on_input(move |v| filter_on_input(i, v))
                    .size(12)
                    .padding(2),
            )
            .width(Length::Fill)
            .height(25)
            .padding(2)
            .style(|_: &Theme| container::Style {
                background: Some(COLOR_BG_WINDOW.into()),
                text_color: Some(COLOR_TEXT_PRIMARY),
                ..Default::default()
            })
            .into()
        } else {
            Space::new().height(0).into()
        };

        let resize_on_press = on_resize_start.clone();
        header_elements.push(
            row![
                column![title_btn, filter_box]
                    .width(Length::Fixed(width))
                    .spacing(0),
                mouse_area(
                    container(Space::new())
                        .width(4)
                        .height(Length::Fill)
                        .style(|_| container::Style {
                            background: Some(COLOR_HANDLE.into()),
                            ..Default::default()
                        })
                )
                .on_press(resize_on_press(i))
            ]
            .spacing(0)
            .into(),
        );
    }

    header_elements.push(
        container(Space::new())
            .width(Length::Fill)
            .height(header_height)
            .style(header_container_style)
            .into(),
    );

    let hidden_scrollbar = scrollable::Scrollbar::default()
        .width(0.0)
        .scroller_width(0.0)
        .margin(0.0);

    let header_scroll = scrollable(row(header_elements).width(Length::Shrink).spacing(0))
        .direction(scrollable::Direction::Horizontal(hidden_scrollbar))
        .id(ids.header.clone());

    let mut row_number_elements = Vec::new();
    let mut data_row_elements = Vec::new();

    for (r_idx, r) in state.rows.iter().enumerate() {
        let is_alt = r_idx % 2 == 1;
        let row_on_select = on_select.clone();
        let row_string = r.join(" | ");
        let row_on_select_fn = row_on_select.clone();
        let row_string_for_num = row_string.clone();

        row_number_elements.push(
            button(
                container(
                    text(format!(
                        "{}",
                        (state.page * state.page_size) + r_idx as u32 + 1
                    ))
                    .color(COLOR_TEXT_DIM)
                    .size(12),
                )
                .width(40)
                .height(30)
                .align_y(Alignment::Center)
                .center_x(40)
                .style(move |_| container::Style {
                    background: Some(COLOR_BG_HEADER.into()),
                    ..Default::default()
                }),
            )
            .on_press(row_on_select_fn(r_idx, 0, row_string_for_num))
            .padding(0)
            .height(30)
            .style(transparent_button_style)
            .into(),
        );

        let mut row_cells = Vec::new();
        for (c_idx, val) in r.iter().enumerate() {
            let width = state.widths.get(c_idx).copied().unwrap_or(150.0);
            let is_selected = state.selected_cell == Some((r_idx, c_idx));
            let cell_on_press = row_on_select.clone();
            let cell_val = val.clone();

            row_cells.push(
                row![
                    button(
                        container(
                            text(val)
                                .color(COLOR_TEXT_PRIMARY)
                                .width(Length::Fill)
                                .shaping(text::Shaping::Advanced)
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_y(Alignment::Center)
                        .padding(5)
                        .style(cell_container_style(is_selected, is_alt))
                    )
                    .on_press(cell_on_press(r_idx, c_idx, cell_val))
                    .padding(0)
                    .height(30)
                    .style(transparent_button_style),
                    container(Space::new())
                        .width(4)
                        .height(Length::Fill)
                        .style(|_| container::Style {
                            background: Some(COLOR_HANDLE.into()),
                            ..Default::default()
                        })
                ]
                .width(Length::Fixed(width + 4.0))
                .height(30)
                .spacing(0)
                .into(),
            );
        }

        row_cells.push(
            container(Space::new())
                .width(Length::Fill)
                .height(30)
                .style(cell_container_style(false, is_alt))
                .into(),
        );

        data_row_elements.push(
            row(row_cells)
                .width(Length::Shrink)
                .height(30)
                .spacing(0)
                .into(),
        );
    }

    let row_index_scroll = scrollable(column(row_number_elements).spacing(0))
        .direction(scrollable::Direction::Vertical(hidden_scrollbar))
        .id(ids.row_index.clone());

    let data_scroll_id_capture = ids.data.clone();
    let data_scroll = scrollable(column(data_row_elements).width(Length::Shrink).spacing(0))
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .id(ids.data.clone())
        .on_scroll(move |vp| on_scroll(data_scroll_id_capture.clone(), vp))
        .width(Length::Fill)
        .height(Length::Fill);

    let content = column![
        row![
            corner,
            container(header_scroll)
                .height(header_height)
                .width(Length::Fill)
        ]
        .spacing(0)
        .height(header_height)
        .width(Length::Fill),
        row![
            container(row_index_scroll).width(40),
            container(data_scroll).style(|_| container::Style {
                background: Some(COLOR_BG_CELL.into()),
                ..Default::default()
            })
            .width(Length::Fill)
        ]
        .spacing(0)
        .height(Length::Fill)
        .width(Length::Fill)
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill);

    let page_sizes: [u32; 7] = turso_gui_core::PAGE_SIZES;
    let total_pages = state.total_pages();

    let mut pages_list: Vec<u32> = Vec::new();
    for p in 1..=total_pages {
        pages_list.push(p);
    }

    let footer = container(
        row![
            text(format!("Total Records: {}", state.total_records))
                .color(COLOR_TEXT_DIM)
                .size(12),
            Space::new().width(20),
            text("Page:").color(COLOR_TEXT_DIM).size(12),
            pick_list(pages_list, Some(state.page + 1), move |p| on_page_change(
                p - 1
            ))
            .text_size(12)
            .padding(2),
            text(format!("of {}", total_pages))
                .color(COLOR_TEXT_DIM)
                .size(12),
            Space::new().width(20),
            text("Page Size:").color(COLOR_TEXT_DIM).size(12),
            pick_list(page_sizes, Some(state.page_size), move |s| {
                on_page_size_change(s)
            })
            .text_size(12)
            .padding(2),
            row![
                checkbox(state.all_results)
                    .on_toggle(on_toggle_all)
                    .size(16),
                text("Return All").size(12).color(COLOR_TEXT_DIM)
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(5)
    .style(|_| container::Style {
        background: Some(COLOR_BG_HEADER.into()),
        ..Default::default()
    });

    column![content, footer]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
