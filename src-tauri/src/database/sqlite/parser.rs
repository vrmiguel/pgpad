use sqlparser::{
    ast::{ObjectName, ObjectNamePart, Statement},
    dialect::SQLiteDialect,
};

use crate::database::{
    self,
    parser::{ParsedStatement, SqlDialectExt},
};

pub fn parse_statements(query: &str) -> anyhow::Result<Vec<ParsedStatement>> {
    database::parser::parse_statements(&SQLiteDialect {}, query)
}

impl SqlDialectExt for SQLiteDialect {
    fn returns_values(stmt: &Statement) -> bool {
        match stmt {
            Statement::Query { .. } => true,
            Statement::Insert(insert) if insert.returning.is_some() => true,
            Statement::Update { returning, .. } if returning.is_some() => true,
            Statement::Delete(delete) if delete.returning.is_some() => true,
            Statement::CreateView { .. } => true,
            Statement::Explain { .. } => true,
            Statement::Execute { .. } => true,
            Statement::Pragma { name, value, .. } => {
                let is_assignment = value.is_some();
                !is_assignment && pragma_returns_values(name)
            }
            _ => false,
        }
    }
}

fn pragma_returns_values(name: &ObjectName) -> bool {
    const VALUE_RETURNING_PRAGMAS: &[&str] = &[
        "table_info",
        "index_info",
        "foreign_key_list",
        "database_list",
        "compile_options",
        "integrity_check",
        "schema_version",
        "user_version",
        "freelist_count",
        "page_count",
        "page_size",
        "cache_size",
        "temp_store",
    ];

    let Some(first_part) = name.0.first() else {
        return false;
    };

    let ident = match first_part {
        ObjectNamePart::Identifier(ident) => ident,
        ObjectNamePart::Function(func) => &func.name,
    };

    VALUE_RETURNING_PRAGMAS
        .iter()
        .any(|&pragma| ident.value.eq_ignore_ascii_case(pragma))
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: check these:
    // ATTACH/DETACH DATABASE
    // vacuum
    // PRAGMA table_info(users);;
    // PRAGMA foreign_key_list(orders);
    // PRAGMA foreign_keys = ON;

    #[test]
    fn parses_statements() {
        let results = parse_statements("SELECT * FROM users").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].returns_values);
        assert_eq!(results[0].statement.trim(), "SELECT * FROM users");

        let multi_query = r#"
            CREATE TABLE test_users (id INTEGER PRIMARY KEY, name TEXT);
            INSERT INTO test_users (name) VALUES ('Alice') RETURNING id;
            UPDATE test_users SET name = 'Bob' WHERE id = 1;
            SELECT * FROM test_users;
            DROP TABLE test_users;
        "#;

        let results = parse_statements(multi_query).unwrap();

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
    }

    #[test]
    fn test_explain_statements() {
        let explain_query = r#"
            EXPLAIN SELECT * FROM users;
            EXPLAIN QUERY PLAN SELECT * FROM users WHERE id = 1;
        "#;

        let results = parse_statements(explain_query).unwrap();
        assert_eq!(results.len(), 2);
        for result in &results {
            assert!(result.returns_values, "EXPLAIN statements return values");
            assert!(result.statement.contains("EXPLAIN"));
        }
    }

    #[test]
    fn test_pragma_statements() {
        let pragma_query = r#"
            PRAGMA database_list;
            PRAGMA compile_options;
            PRAGMA integrity_check;
            PRAGMA cache_size = 2000;
        "#;

        let results = parse_statements(pragma_query).unwrap();
        assert_eq!(results.len(), 4);

        assert!(
            results[0].returns_values,
            "PRAGMA database_list returns values"
        );
        assert!(
            results[1].returns_values,
            "PRAGMA compile_options returns values"
        );
        assert!(
            results[2].returns_values,
            "PRAGMA integrity_check returns values"
        );
        assert!(
            !results[3].returns_values,
            "PRAGMA cache_size = value doesn't return values"
        );
    }

    #[test]
    fn test_crud_with_returning() {
        let mixed_crud = r#"
            INSERT INTO products (name, price) VALUES ('Widget', 19.99);
            INSERT INTO products (name, price) VALUES ('Gadget', 29.99) RETURNING id, name;
            UPDATE products SET price = 24.99 WHERE name = 'Widget';
            UPDATE products SET price = 34.99 WHERE name = 'Gadget' RETURNING *;
            DELETE FROM products WHERE price < 25;
            DELETE FROM products WHERE price > 30 RETURNING id;
        "#;

        let results = parse_statements(mixed_crud).unwrap();
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
    }

    #[test]
    fn test_ddl_and_transactions() {
        let ddl_transaction_query = r#"
            BEGIN TRANSACTION;
            CREATE INDEX idx_users_email ON users (email);
            ALTER TABLE users ADD COLUMN created_at TEXT DEFAULT (datetime('now'));
            COMMIT;
            CREATE VIEW active_users AS SELECT * FROM users WHERE active = 1;
        "#;

        let results = parse_statements(ddl_transaction_query).unwrap();
        assert_eq!(results.len(), 5);
        assert!(
            !results[0].returns_values,
            "BEGIN TRANSACTION should not return values"
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
    }

    #[test]
    fn test_complex_queries() {
        let complex_select = r#"
            SELECT u.name, COUNT(o.id) as order_count 
            FROM users u 
            LEFT JOIN orders o ON u.id = o.user_id 
            GROUP BY u.id, u.name 
            HAVING COUNT(o.id) > 5;
            
            WITH recent_orders AS (
                SELECT * FROM orders WHERE created_at > datetime('now', '-30 days')
            )
            SELECT u.name, ro.total 
            FROM users u 
            JOIN recent_orders ro ON u.id = ro.user_id;
        "#;

        let results = parse_statements(complex_select).unwrap();
        assert_eq!(results.len(), 2);

        for result in &results {
            assert!(result.returns_values);
            assert!(result.statement.contains("SELECT"));
        }
    }

    #[test]
    fn test_sqlite_specific_features() {
        let sqlite_features = r#"
            CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY AUTOINCREMENT, data BLOB);
            INSERT OR REPLACE INTO test (data) VALUES (x'48656c6c6f');
            INSERT OR IGNORE INTO test (data) VALUES (x'576f726c64');
            SELECT last_insert_rowid();
            SELECT changes();
        "#;

        let results = parse_statements(sqlite_features).unwrap();
        assert_eq!(results.len(), 5);

        assert!(
            !results[0].returns_values,
            "CREATE TABLE IF NOT EXISTS should not return values"
        );
        assert!(
            !results[1].returns_values,
            "INSERT OR REPLACE should not return values"
        );
        assert!(
            !results[2].returns_values,
            "INSERT OR IGNORE should not return values"
        );
        assert!(
            results[3].returns_values,
            "SELECT last_insert_rowid() should return values"
        );
        assert!(
            results[4].returns_values,
            "SELECT changes() should return values"
        );
    }

    #[test]
    fn test_whitespace_handling() {
        let with_whitespace = r#"
            
            SELECT 1;
            
            
            INSERT INTO test VALUES (1);
            
        "#;

        let results = parse_statements(with_whitespace).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].returns_values, "SELECT should return values");
        assert!(
            !results[1].returns_values,
            "INSERT without RETURNING should not return values"
        );
    }

    #[test]
    fn test_more_pragma_cases() {
        let more_pragmas = r#"
            PRAGMA schema_version;
            PRAGMA user_version;
            PRAGMA freelist_count;
            PRAGMA page_count;
            PRAGMA page_size;
            PRAGMA temp_store;
            PRAGMA unknown_pragma;
        "#;

        let results = parse_statements(more_pragmas).unwrap();
        assert_eq!(results.len(), 7);

        for result in results.iter().take(6) {
            assert!(
                result.returns_values,
                "Known value-returning PRAGMA should return values"
            );
        }

        assert!(
            !results[6].returns_values,
            "Unknown PRAGMA should not return values"
        );
    }
}
