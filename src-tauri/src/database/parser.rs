use sqlparser::{
    ast::Statement, dialect::Dialect, keywords::Keyword, parser::Parser, tokenizer::Token,
};

#[derive(Debug)]
pub struct ParsedStatement {
    pub statement: String,
    pub returns_values: bool,
    #[expect(unused)]
    pub is_read_only: bool,
}

pub trait SqlDialectExt {
    fn returns_values(stmt: &Statement) -> bool;
    fn is_read_only(stmt: &Statement) -> bool {
        use Statement::*;

        match stmt {
            Query(_) => true,
            Explain {
                analyze, statement, ..
            } => {
                // If the explain will execute the inner statement, check if it's read-only
                if *analyze {
                    Self::is_read_only(statement)
                } else {
                    true
                }
            }
            ExplainTable { .. }
            | ShowFunctions { .. }
            | ShowVariable { .. }
            | ShowStatus { .. }
            | ShowVariables { .. }
            | ShowCreate { .. }
            | ShowColumns { .. }
            | ShowDatabases { .. }
            | ShowSchemas { .. }
            | ShowObjects { .. }
            | ShowTables { .. }
            | ShowViews { .. }
            | ShowCollation { .. }
            | List(_) => true,

            Case(_)
            | If(_)
            | While(_)
            | Raise(_)
            | RaisError { .. }
            | Print(_)
            | Return(_)
            | Assert { .. } => true,
            Open(_) => true,
            Close { .. } => true,
            Fetch { into, .. } => into.is_none(),
            StartTransaction { statements, .. } => statements.iter().all(Self::is_read_only),
            Commit { .. } | Rollback { .. } | Savepoint { .. } | ReleaseSavepoint { .. } => true,
            Set(_) | Use(_) | Pragma { .. } => false,
            LISTEN { .. } | UNLISTEN { .. } | NOTIFY { .. } => false,
            Insert(_) | Update { .. } | Delete(_) | Merge { .. } | Truncate { .. } => false,

            // TODO(vini): maybe we should allow `COPY TO`
            Copy { .. } => false,
            CopyIntoSnowflake { .. } => false,
            Unload { .. } => false,
            Directory { .. } => false,
            Analyze { .. } | Msck { .. } | OptimizeTable { .. } | Comment { .. } => false,
            CreateView { .. }
            | CreateTable(_)
            | CreateVirtualTable { .. }
            | CreateIndex(_)
            | CreateRole { .. }
            | CreateSecret { .. }
            | CreateServer(_)
            | CreatePolicy { .. }
            | CreateConnector(_)
            | AlterTable { .. }
            | AlterIndex { .. }
            | AlterView { .. }
            | AlterType(_)
            | AlterRole { .. }
            | AlterPolicy { .. }
            | AlterConnector { .. }
            | AlterSession { .. }
            | AttachDatabase { .. }
            | AttachDuckDBDatabase { .. }
            | DetachDuckDBDatabase { .. }
            | Drop { .. }
            | DropFunction { .. }
            | DropDomain(_)
            | DropProcedure { .. }
            | DropSecret { .. }
            | DropPolicy { .. }
            | DropConnector { .. }
            | CreateExtension { .. }
            | DropExtension { .. }
            | CreateSchema { .. }
            | CreateDatabase { .. }
            | CreateFunction(_)
            | CreateTrigger { .. }
            | DropTrigger { .. }
            | CreateProcedure { .. }
            | CreateMacro { .. }
            | CreateStage { .. }
            | CreateType { .. }
            | CreateDomain(_)
            | CreateSequence { .. }
            | RenameTable(_)
            | Remove(_) => false,
            Grant { .. } | Deny(_) | Revoke { .. } => false,
            Cache { .. } | UNCache { .. } => false,
            LockTables { .. } | UnlockTables | Flush { .. } | Kill { .. } => false,
            LoadData { .. } => false,
            Install { .. } | Load { .. } => false,
            Call(_) | Execute { .. } | Prepare { .. } => false,
            Declare { .. } => true,
            Discard { .. } => false,
            Deallocate { .. } => false,
        }
    }
}

pub fn parse_statements<T>(dialect: &T, query: &str) -> anyhow::Result<Vec<ParsedStatement>>
where
    T: Dialect + SqlDialectExt,
{
    let mut parser = Parser::new(dialect).try_with_sql(query)?;

    let mut statements = vec![];

    loop {
        while parser.consume_token(&Token::SemiColon) {}

        match parser.peek_token().token {
            Token::EOF => break,
            Token::Word(word) if word.keyword == Keyword::END => break,
            _ => {}
        }

        let statement = parser.parse_statement()?;
        statements.push(ParsedStatement {
            statement: statement.to_string(),
            returns_values: T::returns_values(&statement),
            is_read_only: T::is_read_only(&statement),
        });
    }

    Ok(statements)
}
