use crate::types::SortDirection;

pub const PAGE_SIZES: [u32; 7] = [10, 25, 50, 100, 250, 500, 1000];

pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub fn escape_literal(value: &str) -> String {
    value.replace('\'', "''")
}

pub fn build_filter_clauses(headers: &[String], filters: &[String]) -> Vec<String> {
    filters
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            let header = headers.get(i)?;
            compile_filter(header, f)
        })
        .collect()
}

/// DB Browser-style filter box:
/// empty = no filter; `NULL` / `NOT NULL`; operators `= > < >= <= <> !=`;
/// otherwise `LIKE '%text%'`.
pub fn compile_filter(header: &str, raw: &str) -> Option<String> {
    let f = raw.trim();
    if f.is_empty() {
        return None;
    }
    let col = quote_ident(header);
    let upper = f.to_ascii_uppercase();
    if upper == "NULL" || upper == "IS NULL" {
        return Some(format!("{col} IS NULL"));
    }
    if upper == "NOT NULL" || upper == "IS NOT NULL" {
        return Some(format!("{col} IS NOT NULL"));
    }
    for op in [">=", "<=", "<>", "!=", "=", ">", "<"] {
        if let Some(rest) = f.strip_prefix(op) {
            return Some(format!("{col} {op} {}", sql_literal(rest)));
        }
    }
    Some(format!("{col} LIKE '%{}%'", escape_literal(f)))
}

fn sql_literal(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        return "''".into();
    }
    if t.parse::<i64>().is_ok() || t.parse::<f64>().is_ok() {
        t.to_string()
    } else {
        let unquoted = t.trim_matches('\'').trim_matches('"');
        format!("'{}'", escape_literal(unquoted))
    }
}

pub fn build_browse_sql(
    table: &str,
    headers: &[String],
    filters: &[String],
    sort: Option<(usize, SortDirection)>,
) -> String {
    let mut sql = format!("SELECT * FROM {}", quote_ident(table));
    let clauses = build_filter_clauses(headers, filters);
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    if let Some((i, dir)) = sort {
        if let Some(header) = headers.get(i) {
            sql.push_str(&format!(" ORDER BY {} {}", quote_ident(header), dir.sql()));
        }
    }
    sql
}

pub fn wrap_select_with_filters(sql: &str, headers: &[String], filters: &[String]) -> String {
    let clauses = build_filter_clauses(headers, filters);
    if clauses.is_empty() {
        sql.to_string()
    } else {
        format!("SELECT * FROM ({sql}) WHERE {}", clauses.join(" AND "))
    }
}

pub fn apply_sort(
    sql: String,
    headers: &[String],
    sort: Option<(usize, SortDirection)>,
) -> String {
    if let Some((i, dir)) = sort {
        if let Some(header) = headers.get(i) {
            return format!(
                "SELECT * FROM ({sql}) ORDER BY {} {}",
                quote_ident(header),
                dir.sql()
            );
        }
    }
    sql
}

pub fn is_select(sql: &str) -> bool {
    sql.trim()
        .to_uppercase()
        .starts_with("SELECT")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_escapes_inner_quotes() {
        assert_eq!(quote_ident("foo\"bar"), "\"foo\"\"bar\"");
    }

    #[test]
    fn browse_sql_with_filter_and_sort() {
        let sql = build_browse_sql(
            "users",
            &["id".into(), "name".into()],
            &["".into(), "al".into()],
            Some((1, SortDirection::Desc)),
        );
        assert_eq!(
            sql,
            "SELECT * FROM \"users\" WHERE \"name\" LIKE '%al%' ORDER BY \"name\" DESC"
        );
    }

    #[test]
    fn filter_operators_and_null() {
        assert_eq!(
            compile_filter("age", "> 18").as_deref(),
            Some("\"age\" > 18")
        );
        assert_eq!(
            compile_filter("name", "= bob").as_deref(),
            Some("\"name\" = 'bob'")
        );
        assert_eq!(
            compile_filter("n", "NULL").as_deref(),
            Some("\"n\" IS NULL")
        );
        assert_eq!(
            compile_filter("n", "not null").as_deref(),
            Some("\"n\" IS NOT NULL")
        );
        assert_eq!(compile_filter("n", "  ").as_deref(), None);
    }

    #[test]
    fn apply_sort_wraps_select() {
        let sql = apply_sort(
            "SELECT * FROM t".into(),
            &["id".into(), "name".into()],
            Some((1, SortDirection::Asc)),
        );
        assert_eq!(sql, "SELECT * FROM (SELECT * FROM t) ORDER BY \"name\" ASC");
        let unchanged = apply_sort("SELECT 1".into(), &["id".into()], None);
        assert_eq!(unchanged, "SELECT 1");
    }
}
