use sqlparser::{
    ast::Statement, dialect::Dialect, keywords::Keyword, parser::Parser, tokenizer::Token,
};

#[derive(Debug)]
pub struct ParsedStatement {
    pub statement: String,
    pub returns_values: bool,
}

pub trait SqlDialectExt {
    fn returns_values(stmt: &Statement) -> bool;
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
        });
    }

    Ok(statements)
}
