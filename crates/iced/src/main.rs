#![windows_subsystem = "windows"]

use clap::Parser;
use turso_gui_core::setup_console;

mod gui;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Turso / SQLite DB Browser (iced)", long_about = None)]
struct Args {
    /// Path to the database file or Turso URL
    #[arg(short, long)]
    database: Option<String>,

    /// Authentication token for Turso (if using a remote URL)
    #[arg(short, long)]
    token: Option<String>,

    /// Run in CLI mode even if no command is provided
    #[arg(long)]
    cli: bool,

    /// SQL command to execute (runs in CLI mode)
    #[arg(short, long)]
    command: Option<String>,

    /// Enable debug output
    #[arg(short = 'D', long)]
    debug: bool,

    /// Open a console for logs. A terminal that already launched this process is reused.
    #[arg(long)]
    console: bool,
}

fn main() -> anyhow::Result<()> {
    let force_console = args_force_console();
    setup_console(force_console);
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    if args.command.is_some() || args.cli {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(turso_gui_core::cli::run(
            args.database,
            args.token,
            args.command,
            args.debug,
        ))?;
    } else {
        gui::run(args)?;
    }

    Ok(())
}

fn args_force_console() -> bool {
    std::env::args().any(|a| {
        a == "--console" || a == "--cli" || a == "--command" || a == "-c"
    })
}
