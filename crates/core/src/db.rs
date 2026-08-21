use anyhow::Result;
use turso::{Builder, Connection, Database, Value};

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub pk: bool,
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub sql: String,
}

#[derive(Debug, Clone, Default)]
pub struct QueryResult {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl QueryResult {
    pub fn from_rows(mut rows: Vec<Vec<String>>) -> Self {
        if rows.is_empty() {
            return Self::default();
        }
        let headers = rows.remove(0);
        Self { headers, rows }
    }

    pub fn is_empty(&self) -> bool {
        self.headers.is_empty() && self.rows.is_empty()
    }
}

#[derive(Debug)]
pub struct Db {
    #[allow(dead_code)]
    db: Database,
    conn: Connection,
    pub debug: bool,
}

impl Db {
    pub async fn open(url: &str, _token: Option<String>, debug: bool) -> Result<Self> {
        if url.starts_with("libsql://") || url.starts_with("http") {
            return Err(anyhow::anyhow!(
                "Remote databases are not supported in this version. Use a local file path."
            ));
        }

        let db = Builder::new_local(url).build().await?;
        let conn = db.connect()?;
        Ok(Self { db, conn, debug })
    }

    pub async fn execute(
        &self,
        sql: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Vec<String>>> {
        let result = self.query(sql, limit, offset).await?;
        let mut rows = Vec::with_capacity(result.rows.len() + 1);
        if !result.headers.is_empty() {
            rows.push(result.headers);
        }
        rows.extend(result.rows);
        Ok(rows)
    }

    pub async fn query(
        &self,
        sql: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<QueryResult> {
        let mut sql_to_exec = sql.to_string();
        let upper_sql = sql.to_uppercase();

        if limit.is_some() && !upper_sql.contains(" LIMIT ") {
            if let Some(l) = limit {
                sql_to_exec.push_str(&format!(" LIMIT {l}"));
            }
        }

        if offset.is_some() && !upper_sql.contains(" OFFSET ") {
            if let Some(o) = offset {
                sql_to_exec.push_str(&format!(" OFFSET {o}"));
            }
        }

        if self.debug {
            println!("Executing SQL: {sql_to_exec}");
        }

        let mut stmt = self.conn.prepare(&sql_to_exec).await?;
        let mut rows = stmt.query(()).await?;
        let mut results = QueryResult::default();

        for col in stmt.columns() {
            results.headers.push(col.name().to_string());
        }

        if self.debug {
            println!("Columns found: {:?}", results.headers);
        }

        while let Some(row) = rows.next().await? {
            let mut row_data = Vec::new();
            for i in 0..row.column_count() {
                let val: Value = row.get_value(i)?;
                row_data.push(Self::value_to_string(val));
            }
            results.rows.push(row_data);
        }

        if self.debug {
            println!("Returned {} data rows", results.rows.len());
        }

        Ok(results)
    }

    fn value_to_string(val: Value) -> String {
        match val {
            Value::Null => "NULL".to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Real(f) => f.to_string(),
            Value::Text(s) => s,
            Value::Blob(b) => format!("<blob {} bytes>", b.len()),
        }
    }

    pub async fn list_tables_full(&self) -> Result<Vec<TableInfo>> {
        let sql =
            "SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name";
        if self.debug {
            println!("Executing SQL: {sql}");
        }
        let mut stmt = self.conn.prepare(sql).await?;
        let mut rows = stmt.query(()).await?;
        let mut tables = Vec::new();
        while let Some(row) = rows.next().await? {
            let name = match row.get_value(0)? {
                Value::Text(s) => s,
                _ => continue,
            };
            let sql = match row.get_value(1)? {
                Value::Text(s) => s,
                _ => String::new(),
            };
            tables.push(TableInfo { name, sql });
        }
        if self.debug {
            println!("Found {} tables", tables.len());
        }
        Ok(tables)
    }

    pub async fn get_table_columns(&self, table: &str) -> Result<Vec<TableColumn>> {
        let sql = format!("PRAGMA table_info({})", crate::query::quote_ident(table));
        if self.debug {
            println!("Executing SQL: {sql}");
        }
        let mut stmt = self.conn.prepare(&sql).await?;
        let mut rows = stmt.query(()).await?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next().await? {
            let name = Self::value_to_string(row.get_value(1)?);
            let data_type = Self::value_to_string(row.get_value(2)?);
            let not_null = match row.get_value(3)? {
                Value::Integer(i) => i != 0,
                _ => false,
            };
            let pk = match row.get_value(5)? {
                Value::Integer(i) => i != 0,
                _ => false,
            };
            columns.push(TableColumn {
                name,
                data_type,
                not_null,
                pk,
            });
        }
        if self.debug {
            println!("Found {} columns for table {table}", columns.len());
        }
        Ok(columns)
    }

    pub async fn count_table(&self, table: &str) -> Result<u64> {
        let sql = format!("SELECT COUNT(*) FROM {}", crate::query::quote_ident(table));
        let res = self.query(&sql, None, None).await?;
        if let Some(row) = res.rows.first() {
            if let Some(val) = row.first() {
                return Ok(val.parse().unwrap_or(0));
            }
        }
        Ok(0)
    }

    pub async fn begin_transaction(&self) -> Result<()> {
        self.conn.execute("BEGIN", ()).await?;
        Ok(())
    }

    pub async fn commit_transaction(&self) -> Result<()> {
        self.conn.execute("COMMIT", ()).await?;
        Ok(())
    }

    pub async fn rollback_transaction(&self) -> Result<()> {
        self.conn.execute("ROLLBACK", ()).await?;
        Ok(())
    }
}
