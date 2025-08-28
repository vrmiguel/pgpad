use std::fmt::Write;

use rusqlite::{types::ValueRef, Row};
use serde_json::value::RawValue;

/// A somewhat efficient way of converting the raw SQLite query results into JSON values.
pub struct RowWriter {
    json: String,
    row_count: usize,
}

impl RowWriter {
    pub fn new() -> Self {
        let mut json = String::with_capacity(2);
        json.push('[');

        Self { json, row_count: 0 }
    }

    pub fn add_row(&mut self, row: &Row, columns: usize) -> Result<(), anyhow::Error> {
        if self.row_count > 0 {
            self.json.push(',');
        }

        self.json.push('[');
        for i in 0..columns {
            if i > 0 {
                self.json.push(',');
            }

            match row.get_ref(i)? {
                ValueRef::Null => self.write_json_string("NULL"),
                ValueRef::Integer(value) => write!(&mut self.json, "{}", value)?,
                ValueRef::Real(value) => write!(&mut self.json, "{}", value)?,
                ValueRef::Text(value) => {
                    self.write_json_string(
                        // TODO: fallback to lossy
                        std::str::from_utf8(value)?,
                    );
                }
                ValueRef::Blob(_value) => {
                    self.write_json_string("TODO-noblobsyet");
                }
            };
        }
        self.json.push(']');
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
        self.json.push(']');

        let json = std::mem::replace(&mut self.json, String::new());
        RawValue::from_string(json).unwrap()
    }

    pub fn clear(&mut self) {
        self.json.reserve(2);
        self.json.push('[');
        self.row_count = 0;
    }

    fn write_json_string(&mut self, s: &str) {
        self.json.push('"');
        for ch in s.chars() {
            match ch {
                '"' => self.json.push_str("\\\""),
                '\\' => self.json.push_str("\\\\"),
                '\n' => self.json.push_str("\\n"),
                '\r' => self.json.push_str("\\r"),
                '\t' => self.json.push_str("\\t"),
                c if c.is_control() => {
                    write!(&mut self.json, "\\u{:04x}", c as u32).unwrap();
                }
                c => self.json.push(c),
            }
        }
        self.json.push('"');
    }
}
