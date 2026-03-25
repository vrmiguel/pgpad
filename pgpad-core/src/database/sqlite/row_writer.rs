use std::fmt::Write;

use rusqlite::{types::ValueRef, Row};
use serde_json::value::RawValue;

use crate::utils;

/// A somewhat efficient way of converting the raw SQLite query results into JSON values.
pub struct RowWriter {
    buf: String,
    row_count: usize,
    column_decltypes: Vec<Option<String>>,
}

impl RowWriter {
    pub fn new(column_decltypes: Vec<Option<String>>) -> Self {
        Self {
            buf: String::new(),
            row_count: 0,
            column_decltypes,
        }
    }

    pub fn add_row(&mut self, row: &Row) -> Result<(), anyhow::Error> {
        if self.row_count == 0 {
            self.buf.reserve(2);
            self.buf.push('[');
        }

        if self.row_count > 0 {
            self.buf.push(',');
        }

        self.buf.push('[');
        for i in 0..self.column_decltypes.len() {
            if i > 0 {
                self.buf.push(',');
            }

            match row.get_ref(i)? {
                ValueRef::Null => self.write_json_string("NULL"),
                ValueRef::Integer(value) if value == 0 || value == 1 => {
                    let decltype = self.column_decltypes[i].as_deref();
                    let looks_like_bool = decltype
                        .map(|s| {
                            s.eq_ignore_ascii_case("boolean")
                                || s.eq_ignore_ascii_case("bool")
                                || s.to_ascii_lowercase().contains("tinyint(1)")
                        })
                        .unwrap_or(false);

                    if looks_like_bool {
                        write!(&mut self.buf, "{}", value == 1)?
                    } else {
                        write!(&mut self.buf, "{value}")?
                    }
                }
                ValueRef::Integer(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::Real(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::Text(value) => {
                    // If this is a JSON object or array, convert it so that it's picked up by JsonInspector in the front-end
                    let is_json = val_is_json(value);
                    let Ok(utf8) = std::str::from_utf8(value) else {
                        let utf8_lossy = String::from_utf8_lossy(value);
                        self.write_json_string(&utf8_lossy);
                        continue;
                    };

                    if is_json {
                        self.buf.write_str(utf8)?;
                    } else {
                        self.write_json_string(utf8);
                    }
                }
                ValueRef::Blob(value) => {
                    // TODO(vini): we can make this more informative eventually
                    self.write_json_string(&format!("Blob({})", value.len()));
                }
            };
        }
        self.buf.push(']');
        self.row_count += 1;

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.row_count
    }

    pub fn is_empty(&self) -> bool {
        self.row_count == 0
    }

    pub fn finish(&mut self) -> Box<RawValue> {
        if self.row_count == 0 {
            self.buf.push('[');
        }
        self.buf.push(']');

        let json = std::mem::take(&mut self.buf);
        self.row_count = 0;

        RawValue::from_string(json).unwrap()
    }

    fn write_json_string(&mut self, s: &str) {
        self.buf.push('"');
        for ch in s.chars() {
            match ch {
                '"' => self.buf.push_str("\\\""),
                '\\' => self.buf.push_str("\\\\"),
                '\n' => self.buf.push_str("\\n"),
                '\r' => self.buf.push_str("\\r"),
                '\t' => self.buf.push_str("\\t"),
                c if c.is_control() => {
                    write!(&mut self.buf, "\\u{:04x}", c as u32).unwrap();
                }
                c => self.buf.push(c),
            }
        }
        self.buf.push('"');
    }
}

#[inline]
fn val_is_json(value: &[u8]) -> bool {
    // Serves as a simple check to short-circuit JSON parsing for most TEXT cases
    let looks_like_json = (value.starts_with(b"[") && value.ends_with(b"]"))
        || (value.starts_with(b"{") && value.ends_with(b"}"));
    looks_like_json && utils::is_json(value)
}

#[cfg(test)]
mod tests {
    use crate::Error;

    use super::*;
    use rusqlite::Connection;
    use serde_json::Value;

    fn create_test_db() -> Result<Connection, Error> {
        let conn = Connection::open_in_memory()?;

        conn.execute(
            "CREATE TABLE test_data (
                id INTEGER PRIMARY KEY,
                int_col INTEGER,
                real_col REAL,
                text_col TEXT,
                blob_col BLOB,
                null_col INTEGER,
                bool_col BOOLEAN,
                bool_tinyint TINYINT(1),
                json_col TEXT,
                large_int INTEGER
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO test_data VALUES (
                1, 
                42, 
                3.14159, 
                'Hello World', 
                X'48656C6C6F', 
                NULL, 
                1, 
                0, 
                '{\"key\": \"value\", \"number\": 123}',
                9223372036854775807
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO test_data VALUES (
                2, 
                -100, 
                -2.5, 
                'Special chars: \"\\n\\t', 
                X'DEADBEEF', 
                NULL, 
                0, 
                1, 
                '[1, 2, 3, \"test\"]',
                -9223372036854775808
            )",
            [],
        )?;

        conn.execute(
            "INSERT INTO test_data VALUES (
                3, 
                0, 
                0.0, 
                '', 
                X'', 
                NULL, 
                1, 
                0, 
                'not json content',
                1
            )",
            [],
        )?;

        Ok(conn)
    }

    fn execute_query(conn: &Connection, sql: &str) -> Result<Value, Error> {
        let mut stmt = conn.prepare(sql)?;
        let column_decltypes = stmt
            .columns()
            .iter()
            .map(|col| col.decl_type().map(|s| s.to_string()))
            .collect();

        let mut rows = stmt.query([])?;
        let mut writer = RowWriter::new(column_decltypes);

        while let Some(row) = rows.next()? {
            writer.add_row(row)?;
        }

        let result = writer.finish();
        let json_result: Value = serde_json::from_str(result.get())?;
        Ok(json_result)
    }

    fn execute_query_one_row(conn: &Connection, sql: &str) -> Result<Value, Error> {
        let mut stmt = conn.prepare(sql)?;
        let column_decltypes = stmt
            .columns()
            .iter()
            .map(|col| col.decl_type().map(|s| s.to_string()))
            .collect();

        let mut rows = stmt.query([])?;
        let row = rows.next()?.unwrap();

        let mut writer = RowWriter::new(column_decltypes);
        writer.add_row(row)?;
        let result = writer.finish();

        let json_result: Value = serde_json::from_str(result.get())?;
        Ok(json_result)
    }

    #[allow(clippy::approx_constant)]
    #[test]
    fn basic_types() -> Result<(), Error> {
        let conn = create_test_db()?;
        let result = execute_query_one_row(
            &conn,
            "SELECT int_col, real_col, text_col FROM test_data WHERE id = 1",
        )?;
        assert_eq!(result, serde_json::json!([[42, 3.14159, "Hello World"]]));
        Ok(())
    }

    #[test]
    fn null_handling() -> Result<(), Error> {
        let conn = create_test_db()?;
        let result = execute_query_one_row(&conn, "SELECT null_col FROM test_data WHERE id = 1")?;
        assert_eq!(result, serde_json::json!([["NULL"]]));
        Ok(())
    }

    #[test]
    fn boolean_detection() -> Result<(), Error> {
        let conn = create_test_db()?;

        let result = execute_query_one_row(&conn, "SELECT bool_col FROM test_data WHERE id = 1")?;
        assert_eq!(result, serde_json::json!([[true]]));

        let result =
            execute_query_one_row(&conn, "SELECT bool_tinyint FROM test_data WHERE id = 2")?;
        assert_eq!(result, serde_json::json!([[true]]));

        Ok(())
    }

    #[test]
    fn json_detection() -> Result<(), Error> {
        let conn = create_test_db()?;

        let result = execute_query_one_row(&conn, "SELECT json_col FROM test_data WHERE id = 1")?;
        assert_eq!(
            result,
            serde_json::json!([[{"key": "value", "number": 123}]])
        );

        let result = execute_query_one_row(&conn, "SELECT json_col FROM test_data WHERE id = 2")?;
        assert_eq!(result, serde_json::json!([[[1, 2, 3, "test"]]]));

        let result = execute_query_one_row(&conn, "SELECT json_col FROM test_data WHERE id = 3")?;
        assert_eq!(result, serde_json::json!([["not json content"]]));

        Ok(())
    }

    #[test]
    fn blob_handling() -> Result<(), Error> {
        let conn = create_test_db()?;

        let result = execute_query_one_row(&conn, "SELECT blob_col FROM test_data WHERE id = 1")?;
        assert_eq!(result, serde_json::json!([["Blob(5)"]]));

        let result = execute_query_one_row(&conn, "SELECT blob_col FROM test_data WHERE id = 3")?;
        assert_eq!(result, serde_json::json!([["Blob(0)"]]));

        Ok(())
    }

    #[test]
    fn special_characters() -> Result<(), Error> {
        let conn = create_test_db()?;
        let result = execute_query_one_row(&conn, "SELECT text_col FROM test_data WHERE id = 2")?;
        assert_eq!(result, serde_json::json!([["Special chars: \"\\n\\t"]]));
        Ok(())
    }

    #[test]
    fn large_integers() -> Result<(), Error> {
        let conn = create_test_db()?;

        let result = execute_query_one_row(&conn, "SELECT large_int FROM test_data WHERE id = 1")?;
        assert_eq!(result, serde_json::json!([[9223372036854775807i64]]));

        let result = execute_query_one_row(&conn, "SELECT large_int FROM test_data WHERE id = 2")?;
        assert_eq!(result, serde_json::json!([[-9223372036854775808i64]]));

        Ok(())
    }

    #[test]
    fn multiple_rows() -> Result<(), Error> {
        let conn = create_test_db()?;
        let result = execute_query(&conn, "SELECT id, int_col FROM test_data ORDER BY id")?;
        assert_eq!(result, serde_json::json!([[1, 42], [2, -100], [3, 0]]));
        Ok(())
    }

    #[test]
    fn empty_result() -> Result<(), Error> {
        let conn = create_test_db()?;
        let result = execute_query(&conn, "SELECT id FROM test_data WHERE id = 999")?;
        assert_eq!(result, serde_json::json!([]));
        Ok(())
    }

    #[test]
    fn clear_functionality() -> Result<(), Error> {
        let conn = create_test_db()?;

        let mut stmt = conn.prepare("SELECT id FROM test_data WHERE id = 1")?;
        let column_decltypes: Vec<Option<String>> = stmt
            .columns()
            .iter()
            .map(|col| col.decl_type().map(|s| s.to_string()))
            .collect();

        let mut rows = stmt.query([])?;
        let row = rows.next()?.unwrap();

        let mut writer = RowWriter::new(column_decltypes.clone());
        writer.add_row(row)?;

        assert_eq!(writer.len(), 1);
        assert!(!writer.is_empty());

        let first_result = writer.finish();
        let first_result: Value = serde_json::from_str(first_result.get())?;
        assert_eq!(first_result, serde_json::json!([[1]]));

        assert_eq!(writer.len(), 0);
        assert!(writer.is_empty());

        writer.add_row(row)?;
        let result = writer.finish();
        let result: Value = serde_json::from_str(result.get())?;
        assert_eq!(result, serde_json::json!([[1]]));

        Ok(())
    }

    #[test]
    fn non_boolean_zero_one() -> Result<(), Error> {
        let conn = Connection::open_in_memory()?;

        conn.execute(
            "CREATE TABLE test_integers (
                regular_int INTEGER
            )",
            [],
        )?;

        conn.execute("INSERT INTO test_integers VALUES (0)", [])?;
        conn.execute("INSERT INTO test_integers VALUES (1)", [])?;
        conn.execute("INSERT INTO test_integers VALUES (2)", [])?;

        let result = execute_query(
            &conn,
            "SELECT regular_int FROM test_integers ORDER BY regular_int",
        )?;

        assert_eq!(result, serde_json::json!([[0], [1], [2]]));

        Ok(())
    }
}
