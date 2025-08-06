use sqlparser::{
    ast::Statement, dialect::PostgreSqlDialect, keywords::Keyword, parser::Parser, tokenizer::Token,
};

#[derive(Debug)]
pub struct ParsedStatement {
    pub statement: String,
    pub returns_values: bool,
}

pub fn parse_statements(query: &str) -> anyhow::Result<Vec<ParsedStatement>> {
    let dialect = PostgreSqlDialect {};
    let mut parser = Parser::new(&dialect).try_with_sql(query)?;

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
            returns_values: returns_values(&statement),
        });
    }

    Ok(statements)
}

fn returns_values(stmt: &Statement) -> bool {
    match stmt {
        Statement::Query { .. } => true,
        Statement::Insert(insert) if insert.returning.is_some() => true,
        Statement::Update { returning, .. } if returning.is_some() => true,
        Statement::Delete(delete) if delete.returning.is_some() => true,
        Statement::CreateView { .. } => true,
        Statement::ShowVariable { .. } => true,
        Statement::ShowColumns { .. } => true,
        Statement::Explain { .. } => true,
        Statement::Execute { .. } => true,
        Statement::Copy { .. } => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_statements() {
        let results = parse_statements("SELECT * FROM users");
        assert_eq!(results.len(), 1);
        assert!(results[0].returns_values);
        assert_eq!(results[0].statement.trim(), "SELECT * FROM users");

        let multi_query = r#"
            CREATE TABLE test_users (id SERIAL PRIMARY KEY, name TEXT);
            INSERT INTO test_users (name) VALUES ('Alice') RETURNING id;
            UPDATE test_users SET name = 'Bob' WHERE id = 1;
            SELECT * FROM test_users;
            DROP TABLE test_users;
        "#;

        let results = parse_statements(multi_query);

        assert_eq!(results.len(), 5);

        assert!(
            !results[0].returns_values,
            "CREATE TABLE should not return values"
        );
        assert!(results[0].statement.contains("CREATE TABLE"));

        assert!(
            results[1].returns_values,
            "INSERT with RETURNING should return values"
        );
        assert!(results[1].statement.contains("INSERT"));
        assert!(results[1].statement.contains("RETURNING"));

        assert!(
            !results[2].returns_values,
            "UPDATE without RETURNING should not return values"
        );
        assert!(results[2].statement.contains("UPDATE"));
        assert!(!results[2].statement.contains("RETURNING"));

        assert!(results[3].returns_values, "SELECT should return values");
        assert!(results[3].statement.contains("SELECT"));

        assert!(
            !results[4].returns_values,
            "DROP TABLE should not return values"
        );
        assert!(results[4].statement.contains("DROP TABLE"));

        let explain_query = r#"
            EXPLAIN SELECT * FROM users;
            EXPLAIN ANALYZE SELECT * FROM users WHERE id = 1;
            EXPLAIN (ANALYZE, BUFFERS) SELECT COUNT(*) FROM users;
        "#;

        let results = parse_statements(explain_query);
        assert_eq!(results.len(), 3);
        for result in &results {
            assert!(result.returns_values, "explain statements return values");
            assert!(result.statement.contains("EXPLAIN"));
        }

        let results = parse_statements("SHOW search_path;");
        assert_eq!(results.len(), 1);
        assert!(results[0].returns_values, "SHOW statements return values");
        assert!(results[0].statement.contains("SHOW"));

        let mixed_crud = r#"
            INSERT INTO products (name, price) VALUES ('Widget', 19.99);
            INSERT INTO products (name, price) VALUES ('Gadget', 29.99) RETURNING id, name;
            UPDATE products SET price = 24.99 WHERE name = 'Widget';
            UPDATE products SET price = 34.99 WHERE name = 'Gadget' RETURNING *;
            DELETE FROM products WHERE price < 25;
            DELETE FROM products WHERE price > 30 RETURNING id;
        "#;

        let results = parse_statements(mixed_crud);
        assert_eq!(results.len(), 6);
        assert!(
            !results[0].returns_values,
            "INSERT without RETURNING should not return values"
        );
        assert!(
            results[1].returns_values,
            "INSERT with RETURNING should return values"
        );
        assert!(
            !results[2].returns_values,
            "UPDATE without RETURNING should not return values"
        );
        assert!(
            results[3].returns_values,
            "UPDATE with RETURNING should return values"
        );
        assert!(
            !results[4].returns_values,
            "DELETE without RETURNING should not return values"
        );
        assert!(
            results[5].returns_values,
            "DELETE with RETURNING should return values"
        );

        let ddl_transaction_query = r#"
            START TRANSACTION;
            CREATE INDEX idx_users_email ON users (email);
            ALTER TABLE users ADD COLUMN created_at TIMESTAMP DEFAULT NOW();
            COMMIT;
            CREATE VIEW active_users AS SELECT * FROM users WHERE active = true;
        "#;

        let results = parse_statements(ddl_transaction_query);
        assert_eq!(results.len(), 5);
        assert!(
            !results[0].returns_values,
            "START TRANSACTION should not return values"
        );
        assert!(
            !results[1].returns_values,
            "CREATE INDEX should not return values"
        );
        assert!(
            !results[2].returns_values,
            "ALTER TABLE should not return values"
        );
        assert!(
            !results[3].returns_values,
            "COMMIT should not return values"
        );
        assert!(
            results[4].returns_values,
            "CREATE VIEW should return values"
        );

        let prepared_query = r#"
            PREPARE user_query AS SELECT * FROM users WHERE id = $1;
            EXECUTE user_query(123);
            DEALLOCATE user_query;
        "#;

        let results = parse_statements(prepared_query);
        assert_eq!(results.len(), 3);

        assert!(
            !results[0].returns_values,
            "PREPARE should not return values"
        );
        assert!(results[1].returns_values, "Assume EXECUTE returns values");
        assert!(
            !results[2].returns_values,
            "DEALLOCATE should not return values"
        );

        let copy_query = r#"
            COPY users TO STDOUT;
            COPY users (id, name) TO STDOUT WITH CSV;
        "#;

        let results = parse_statements(copy_query);
        assert_eq!(results.len(), 2);

        for result in &results {
            assert!(result.returns_values);
            assert!(result.statement.contains("COPY"));
        }

        let complicated_select = r#"
            SELECT u.name, COUNT(o.id) as order_count 
            FROM users u 
            LEFT JOIN orders o ON u.id = o.user_id 
            GROUP BY u.id, u.name 
            HAVING COUNT(o.id) > 5;
            
            WITH recent_orders AS (
                SELECT * FROM orders WHERE created_at > NOW() - INTERVAL '30 days'
            )
            SELECT u.name, ro.total 
            FROM users u 
            JOIN recent_orders ro ON u.id = ro.user_id;
        "#;

        let results = parse_statements(complicated_select);
        assert_eq!(results.len(), 2);

        for result in &results {
            assert!(result.returns_values);
            assert!(result.statement.contains("SELECT"));
        }

        let with_whitespace = r#"
            
            SELECT 1;
            
            
            INSERT INTO test VALUES (1);
            
        "#;

        let results = parse_statements(with_whitespace);
        assert_eq!(results.len(), 2);
        assert!(results[0].returns_values, "SELECT should return values");
        assert!(
            !results[1].returns_values,
            "INSERT without RETURNING should not return values"
        );
    }
}
