use std::fmt::Write;

use duckdb::{types::ValueRef, Row};
use serde_json::value::RawValue;

use crate::utils;

pub struct RowWriter {
    buf: String,
    row_count: usize,
    column_decltypes: Vec<Option<String>>,
}

impl RowWriter {
    pub fn new(column_decltypes: Vec<Option<String>>) -> Self {
        Self { buf: String::new(), row_count: 0, column_decltypes }
    }

    pub fn add_row(&mut self, row: &Row) -> Result<(), anyhow::Error> {
        if self.row_count == 0 { self.buf.reserve(2); self.buf.push('['); }
        if self.row_count > 0 { self.buf.push(','); }

        self.buf.push('[');
        for i in 0..self.column_decltypes.len() {
            if i > 0 { self.buf.push(','); }

            match row.get_ref(i)? {
                ValueRef::Null => self.buf.push_str("null"),
                ValueRef::TinyInt(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::SmallInt(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::Int(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::BigInt(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::UTinyInt(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::USmallInt(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::UInt(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::UBigInt(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::Float(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::Double(value) => write!(&mut self.buf, "{value}")?,
                ValueRef::Boolean(value) => write!(&mut self.buf, "{}", value)?,
                ValueRef::Text(value) => {
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
                    self.write_json_string(&format!("Blob({})", value.len()));
                }
                other => {
                    let s = format!("{:?}", other);
                    self.write_json_string(&s);
                }
            };
        }
        self.buf.push(']');
        self.row_count += 1;
        Ok(())
    }

    pub fn len(&self) -> usize { self.row_count }
    pub fn is_empty(&self) -> bool { self.row_count == 0 }

    pub fn finish(&mut self) -> Box<RawValue> {
        if self.row_count == 0 { self.buf.push('['); }
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
                c if c.is_control() => { write!(&mut self.buf, "\\u{:04x}", c as u32).unwrap(); }
                c => self.buf.push(c),
            }
        }
        self.buf.push('"');
    }
}

#[inline]
fn val_is_json(value: &[u8]) -> bool {
    let looks_like_json = (value.starts_with(b"[") && value.ends_with(b"]"))
        || (value.starts_with(b"{") && value.ends_with(b"}"));
    looks_like_json && utils::is_json(value)
}
