use sqlparser::ast::Statement;
use sqlparser::dialect::*;
use sqlparser::parser::Parser;
/// "read" only if every statement parses to a plain Query / Show / Explain (non-analyze). Unparseable => "write".
pub fn classify(sql: &str, engine: &str) -> &'static str {
    let d: Box<dyn Dialect> = match engine {
        "postgresql" => Box::new(PostgreSqlDialect {}),
        "mysql" | "mariadb" => Box::new(MySqlDialect {}),
        "sqlserver" => Box::new(MsSqlDialect {}),
        "sqlite" => Box::new(SQLiteDialect {}),
        _ => Box::new(GenericDialect {}),
    };
    match Parser::parse_sql(&*d, sql) {
        Ok(v) if !v.is_empty() => {
            for s in v {
                let ok = match &s {
                    Statement::Query(q) => { let t = q.to_string().to_uppercase(); !(t.contains(" INTO ") || t.contains("FOR UPDATE") || t.contains("FOR SHARE")) }
                    Statement::Explain { analyze, .. } => !*analyze,
                    _ => matches!(s.to_string().split_whitespace().next().map(|w| w.to_uppercase()).as_deref(), Some("SHOW")),
                };
                if !ok { return "write"; }
            }
            "read"
        }
        _ => "write",
    }
}
