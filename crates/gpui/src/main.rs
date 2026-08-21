#![windows_subsystem = "windows"]

mod input;
mod ui;

use clap::Parser;
use gpui::{
    point, prelude::*, px, size, App, Application, Bounds, TitlebarOptions, WindowBounds,
    WindowOptions,
};
use turso_gui_core::{init_gui_host, WindowPlacement};
use ui::TursoApp;

#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version,
    about = "Turso / SQLite DB Browser (GPUI)",
    long_about = None
)]
struct Args {
    /// Path to the database file or Turso URL
    #[arg(short, long)]
    database: Option<String>,

    /// Authentication token for Turso (if using a remote URL)
    #[arg(short, long)]
    token: Option<String>,

    /// Enable debug output
    #[arg(short = 'D', long)]
    debug: bool,

    /// Open a console for logs. A terminal that already launched this process is reused.
    #[arg(long)]
    console: bool,
}

fn main() {
    init_gui_host();
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let place = WindowPlacement::suggested();

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds {
            origin: point(px(place.logical_x), px(place.logical_y)),
            size: size(px(place.logical_width), px(place.logical_height)),
        };
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Turso DB Browser (GPUI)".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| {
                    TursoApp::new(args.database.clone(), args.token.clone(), args.debug, cx)
                });
                if !view.read(cx).model.is_connected() {
                    let handle = view.read(cx).path_focus.clone();
                    window.focus(&handle);
                }
                view
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
