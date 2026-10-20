use sqlparser::{ast::Statement, dialect::MsSqlDialect};

use crate::database::{
    self,
    parser::{ParsedStatement, SqlDialectExt},
};

pub fn parse_statements(query: &str) -> anyhow::Result<Vec<ParsedStatement>> {
    database::parser::parse_statements(&MsSqlDialect {}, query)
}

impl SqlDialectExt for MsSqlDialect {
    fn returns_values(stmt: &Statement) -> bool {
        match stmt {
            Statement::Query { .. } => true,
            Statement::Insert(insert) => {
                if insert.returning.is_some() {
                    return true;
                }
                let s = insert.to_string().to_uppercase();
                s.contains(" OUTPUT ")
            }
            Statement::Update { returning, .. } => {
                if returning.is_some() {
                    return true;
                }
                let s = stmt.to_string().to_uppercase();
                s.contains(" OUTPUT ")
            }
            Statement::Delete(delete) => {
                if delete.returning.is_some() {
                    return true;
                }
                let s = delete.to_string().to_uppercase();
                s.contains(" OUTPUT ")
            }
            Statement::Execute { .. } => true,
            Statement::Declare { .. } => false,
            Statement::Explain { .. } => true,
            _ => false,
        }
    }
}
