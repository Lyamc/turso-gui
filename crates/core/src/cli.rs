use std::io::{self, Write};

use crate::db::Db;

pub async fn run(
    database: Option<String>,
    token: Option<String>,
    command: Option<String>,
    debug: bool,
) -> anyhow::Result<()> {
    let db_path = database.unwrap_or_else(|| "local.db".to_string());
    let db = Db::open(&db_path, token, debug).await?;

    if let Some(cmd) = command {
        execute_and_print(&db, &cmd).await?;
    } else {
        println!("Connected to {db_path}. Type 'exit' to quit.");
        loop {
            print!("turso> ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let cmd = input.trim();
            if cmd == "exit" || cmd == ".quit" {
                break;
            }
            if cmd.is_empty() {
                continue;
            }
            if let Err(e) = execute_and_print(&db, cmd).await {
                eprintln!("Error: {e}");
            }
        }
    }

    Ok(())
}

async fn execute_and_print(db: &Db, sql: &str) -> anyhow::Result<()> {
    let results = db.execute(sql, None, None).await?;
    for row in results {
        println!("{}", row.join(" | "));
    }
    Ok(())
}
