use sqlparser::{ast::Statement, dialect::Dialect};

use crate::database::{
    self,
    parser::{ParsedStatement, SqlDialectExt},
};

pub fn parse_statements(query: &str) -> anyhow::Result<Vec<ParsedStatement>> {
    let q = query.trim();
    let upper = q.to_uppercase();

    // Handle Oracle-specific statements without post-pass heuristics
    if upper.starts_with("DESCRIBE ") || upper.starts_with("DESC ") {
        return Ok(vec![ParsedStatement {
            statement: q.into(),
            returns_values: true,
            is_read_only: true,
            explain_plan: false,
        }]);
    }

    if upper.starts_with("EXPLAIN PLAN") {
        return Ok(vec![ParsedStatement {
            statement: q.into(),
            returns_values: true,
            is_read_only: true,
            explain_plan: true,
        }]);
    }

    if is_plsql_block(&upper) {
        return Ok(vec![ParsedStatement {
            statement: q.into(),
            returns_values: false,
            is_read_only: false,
            explain_plan: false,
        }]);
    }

    if upper.contains(" RETURNING INTO ") {
        return Ok(vec![ParsedStatement {
            statement: q.into(),
            returns_values: true,
            is_read_only: false,
            explain_plan: false,
        }]);
    }

    // Fallback to SQL parser with OracleDialect
    database::parser::parse_statements(&OracleDialect {}, query)
}

#[derive(Debug)]
pub struct OracleDialect;

impl Dialect for OracleDialect {
    fn is_identifier_start(&self, ch: char) -> bool {
        ch == '"' || ch == '_' || ch.is_ascii_alphabetic()
    }
    fn is_identifier_part(&self, ch: char) -> bool {
        ch == '_' || ch.is_ascii_alphanumeric() || ch == '$' || ch == '#'
    }
    fn supports_string_literal_backslash_escape(&self) -> bool {
        false
    }
}

impl SqlDialectExt for OracleDialect {
    fn returns_values(stmt: &Statement) -> bool {
        matches!(
            stmt,
            Statement::Query { .. }
                | Statement::Explain { .. }
                | Statement::ShowColumns { .. }
                | Statement::ShowVariable { .. }
        )
    }
}

fn is_plsql_block(s: &str) -> bool {
    let t = s.trim_start();
    let has_begin_end = t.contains(" BEGIN ") && t.contains(" END");
    let has_exception = t.contains(" EXCEPTION ");
    t.starts_with("DECLARE ") || t.starts_with("BEGIN ") || has_begin_end || has_exception
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oracle_analyzer_explain_and_desc() {
        let v = parse_statements("EXPLAIN PLAN FOR SELECT 1").unwrap();
        assert!(v[0].returns_values);
        assert!(v[0].is_read_only);
        assert!(v[0].explain_plan);
        let v2 = parse_statements("DESC my_table").unwrap();
        assert!(v2[0].returns_values);
        assert!(v2[0].is_read_only);
        assert!(!v2[0].explain_plan);
    }

    #[test]
    fn oracle_analyzer_plsql_block() {
        let v = parse_statements("BEGIN NULL; END;").unwrap();
        assert!(!v[0].returns_values);
        assert!(!v[0].is_read_only);
    }
}
