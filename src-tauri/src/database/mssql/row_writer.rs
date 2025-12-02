use serde_json::value::RawValue;
use std::fmt::Write;

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, Utc};
use tiberius::numeric::Numeric;
use tiberius::xml::XmlData;
use tiberius::ColumnType;
use tiberius::{Column, Row};

pub struct RowWriter {
    buf: String,
    row_count: usize,
    cfg: RowFormatCfg,
}

impl RowWriter {
    pub fn with_settings(settings: Option<&crate::database::types::OracleSettings>) -> Self {
        Self {
            buf: String::new(),
            row_count: 0,
            cfg: RowFormatCfg::from_settings(settings),
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
        for i in 0..row.columns().len() {
            if i > 0 {
                self.buf.push(',');
            }
            self.write_value(row, i)?;
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

    fn write_value(&mut self, row: &Row, idx: usize) -> Result<(), anyhow::Error> {
        let _col: &Column = &row.columns()[idx];
        let col_ty: ColumnType = _col.column_type();
        if let Ok(Some(v)) = row.try_get::<i64, _>(idx) {
            write!(&mut self.buf, "{}", v)?;
            return Ok(());
        }
        if let Ok(Some(v)) = row.try_get::<i32, _>(idx) {
            write!(&mut self.buf, "{}", v)?;
            return Ok(());
        }
        if let Ok(Some(v)) = row.try_get::<i16, _>(idx) {
            write!(&mut self.buf, "{}", v)?;
            return Ok(());
        }
        if let Ok(Some(v)) = row.try_get::<u8, _>(idx) {
            write!(&mut self.buf, "{}", v)?;
            return Ok(());
        }
        match col_ty {
            ColumnType::Money => {
                if let Ok(Some(v)) = row.try_get::<f64, _>(idx) {
                    if self.cfg.money_as_string {
                        self.write_json_string(&format!("{:.1$}", v, self.cfg.money_decimals));
                    } else {
                        write!(&mut self.buf, "{}", v)?;
                    }
                    return Ok(());
                }
            }
            ColumnType::SSVariant => {
                if let Ok(Some(s)) = row.try_get::<&str, _>(idx) {
                    self.write_json_object_variant_with_base("sql_variant", s, None);
                    return Ok(());
                }
            }
            _ => {}
        }
        if let Ok(Some(v)) = row.try_get::<bool, _>(idx) {
            write!(&mut self.buf, "{}", v)?;
            return Ok(());
        }
        if let Ok(Some(v)) = row.try_get::<NaiveDateTime, _>(idx) {
            self.write_json_string(&v.to_string());
            return Ok(());
        }
        if let Ok(Some(v)) = row.try_get::<DateTime<FixedOffset>, _>(idx) {
            match self.cfg.timestamp_tz_mode.as_str() {
                "local" => {
                    let lv = v.with_timezone(&Local);
                    self.write_json_string(&lv.to_rfc3339());
                }
                "offset" => self.write_json_string(&v.to_rfc3339()),
                _ => {
                    let uv = v.with_timezone(&Utc);
                    self.write_json_string(&uv.to_rfc3339());
                }
            }
            return Ok(());
        }
        // MSSQL Numeric/Decimal
        if let Ok(Some(n)) = row.try_get::<Numeric, _>(idx) {
            let s = n.to_string();
            let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
            if self.cfg.numeric_policy == "precision_threshold"
                && digits > self.cfg.numeric_threshold
            {
                self.write_json_string(&s);
            } else {
                write!(&mut self.buf, "{}", s)?;
            }
            return Ok(());
        }
        if let Ok(Some(v)) = row.try_get::<&str, _>(idx) {
            self.write_json_string(v);
            return Ok(());
        }
        if let Ok(Some(v)) = row.try_get::<&[u8], _>(idx) {
            let len = v.len();
            match self.cfg.bytes_format.as_str() {
                "off" | "len" => self.write_json_string(&format!("Bytes({})", len)),
                "full_hex" => self.write_json_string(&format!("0x{}", hex::encode(v))),
                _ => {
                    let n = usize::min(len, self.cfg.bytes_chunk_size);
                    let hexp = hex::encode(&v[..n]);
                    if n >= len {
                        self.write_json_string(&format!("0x{}", hexp));
                    } else {
                        self.write_json_string(&format!(
                            "Bytes({}) preview(0..{}): 0x{}…",
                            len, n, hexp
                        ));
                    }
                }
            }
            return Ok(());
        }
        // xml
        if let Ok(Some(xml)) = row.try_get::<&XmlData, _>(idx) {
            self.write_json_string(&format!("{}", xml));
            return Ok(());
        }
        self.buf.push_str("null");
        Ok(())
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

    fn write_json_object_variant_with_base(&mut self, t: &str, v: &str, base: Option<&str>) {
        self.buf.push('{');
        self.buf.push_str("\"type\":");
        self.write_json_string(t);
        self.buf.push(',');
        self.buf.push_str("\"value\":");
        self.write_json_string(v);
        self.buf.push(',');
        self.buf.push_str("\"base_type\":");
        match base {
            Some(b) => self.write_json_string(b),
            None => self.buf.push_str("null"),
        }
        self.buf.push('}');
    }
}

struct RowFormatCfg {
    bytes_format: String,
    bytes_chunk_size: usize,
    timestamp_tz_mode: String,
    numeric_policy: String,
    numeric_threshold: usize,
    money_as_string: bool,
    money_decimals: usize,
}
impl RowFormatCfg {
    fn from_env() -> Self {
        Self {
            bytes_format: std::env::var("PGPAD_BYTES_FORMAT")
                .unwrap_or_else(|_| String::from("len"))
                .to_lowercase(),
            bytes_chunk_size: std::env::var("PGPAD_BYTES_CHUNK_SIZE")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(4096),
            timestamp_tz_mode: std::env::var("PGPAD_TIMESTAMP_TZ_MODE")
                .unwrap_or_else(|_| String::from("utc"))
                .to_lowercase(),
            numeric_policy: std::env::var("PGPAD_NUMERIC_STRING_POLICY")
                .unwrap_or_else(|_| String::from("precision_threshold"))
                .to_lowercase(),
            numeric_threshold: std::env::var("PGPAD_NUMERIC_PRECISION_THRESHOLD")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(18),
            money_as_string: std::env::var("PGPAD_MONEY_AS_STRING")
                .ok()
                .map(|v| v.to_lowercase())
                .map(|v| v == "1" || v == "true" || v == "yes")
                .unwrap_or(true),
            money_decimals: std::env::var("PGPAD_MONEY_DECIMALS")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(4),
        }
    }
    fn from_settings(settings: Option<&crate::database::types::OracleSettings>) -> Self {
        if let Some(s) = settings {
            Self {
                bytes_format: s
                    .bytes_format
                    .clone()
                    .unwrap_or_else(|| "len".into())
                    .to_lowercase(),
                bytes_chunk_size: s.bytes_chunk_size.unwrap_or(4096),
                timestamp_tz_mode: s
                    .timestamp_tz_mode
                    .clone()
                    .unwrap_or_else(|| "utc".into())
                    .to_lowercase(),
                numeric_policy: s
                    .numeric_string_policy
                    .clone()
                    .unwrap_or_else(|| "precision_threshold".into())
                    .to_lowercase(),
                numeric_threshold: s.numeric_precision_threshold.unwrap_or(18),
                money_as_string: s.money_as_string.unwrap_or(true),
                money_decimals: s.money_decimals.unwrap_or(4),
            }
        } else {
            Self::from_env()
        }
    }
}
