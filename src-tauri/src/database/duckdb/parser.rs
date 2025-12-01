use sqlparser::{
    ast::{ObjectName, ObjectNamePart, Statement},
    dialect::DuckDbDialect,
};

use crate::database::{self, parser::{ParsedStatement, SqlDialectExt}};

pub fn parse_statements(query: &str) -> anyhow::Result<Vec<ParsedStatement>> {
    database::parser::parse_statements(&DuckDbDialect {}, query)
}

impl SqlDialectExt for DuckDbDialect {
    fn returns_values(stmt: &Statement) -> bool {
        match stmt {
            Statement::Query { .. } => true,
            Statement::Insert(insert) if insert.returning.is_some() => true,
            Statement::Update { returning, .. } if returning.is_some() => true,
            Statement::Delete(delete) if delete.returning.is_some() => true,
            Statement::CreateView { .. } => true,
            Statement::Explain { .. } => true,
            Statement::Execute { .. } => true,
            Statement::ShowTables { .. }
            | Statement::ShowSchemas { .. }
            | Statement::ShowColumns { .. }
            | Statement::ShowDatabases { .. }
            | Statement::ShowVariables { .. }
            | Statement::ShowStatus { .. }
            | Statement::ShowFunctions { .. }
            | Statement::ShowCreate { .. }
            | Statement::List(_) => true,
            Statement::Pragma { name, value, .. } => {
                let is_assignment = value.is_some();
                !is_assignment && pragma_returns_values(name)
            }
            _ => false,
        }
    }
}

fn pragma_returns_values(name: &ObjectName) -> bool {
    // Broadly treat PRAGMAs without assignment as value-returning for DuckDB when they
    // match common inspection pragmas
    const VALUE_RETURNING_PRAGMAS: &[&str] = &[
        "database_size",
        "detailed_database_size",
        "storage_info",
        "collations",
        "functions",
        "types",
        "pragma_list",
        "version",
        "settings",
        "extensions",
        "enable_progress_bar",
    ];

    let Some(first_part) = name.0.first() else { return false; };
    let ident = match first_part {
        ObjectNamePart::Identifier(ident) => ident,
        ObjectNamePart::Function(func) => &func.name,
    };
    VALUE_RETURNING_PRAGMAS.iter().any(|&p| ident.value.eq_ignore_ascii_case(p))
}
