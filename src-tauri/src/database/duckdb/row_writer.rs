use std::fmt::Write;

use duckdb::{types::{ValueRef, Value, TimeUnit}, Row};
use serde_json::value::RawValue;

use crate::utils;

pub struct RowWriter {
    buf: String,
    row_count: usize,
    column_decltypes: Vec<Option<String>>,
    cfg: RowFormatCfg,
}

impl RowWriter {
    pub fn new(column_decltypes: Vec<Option<String>>) -> Self { Self { buf: String::new(), row_count: 0, column_decltypes, cfg: RowFormatCfg::from_env() } }
    pub fn with_settings(column_decltypes: Vec<Option<String>>, settings: Option<&crate::database::types::OracleSettings>) -> Self { Self { buf: String::new(), row_count: 0, column_decltypes, cfg: RowFormatCfg::from_settings(settings) } }

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
                ValueRef::Float(value) => {
                    if value.is_finite() { write!(&mut self.buf, "{value}")?; }
                    else { self.write_json_string(&format!("{}", value)); }
                }
                ValueRef::Double(value) => {
                    if value.is_finite() { write!(&mut self.buf, "{value}")?; }
                    else { self.write_json_string(&format!("{}", value)); }
                }
                ValueRef::Boolean(value) => write!(&mut self.buf, "{}", value)?,
                ValueRef::Decimal(dec) => {
                    let s = format!("{}", dec);
                    match self.cfg.numeric_policy.as_str() {
                        "never" => {
                            if let Ok(i) = s.parse::<i128>() { write!(&mut self.buf, "{}", i)?; }
                            else if let Ok(f) = s.parse::<f64>() { write!(&mut self.buf, "{}", f)?; }
                            else { self.write_json_string(&s); }
                        }
                        "precision_threshold" => {
                            let is_fractional = s.contains('.') || s.contains('e') || s.contains('E');
                            let digits_only = s.chars().filter(|c| c.is_ascii_digit()).count();
                            if !is_fractional && digits_only <= self.cfg.numeric_threshold {
                                if let Ok(i) = s.parse::<i128>() { write!(&mut self.buf, "{}", i)?; } else { self.write_json_string(&s); }
                            } else if is_fractional && digits_only <= self.cfg.numeric_threshold {
                                if let Ok(f) = s.parse::<f64>() { write!(&mut self.buf, "{}", f)?; } else { self.write_json_string(&s); }
                            } else {
                                self.write_json_string(&s);
                            }
                        }
                        _ => self.write_json_string(&s),
                    }
                }
                ValueRef::Date32(days) => {
                    let base = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                    let d = base.checked_add_days(chrono::Days::new(days as u64)).unwrap_or(base);
                    self.write_json_string(&d.to_string());
                }
                ValueRef::Time64(unit, v) => {
                    let micros = match unit { TimeUnit::Second => v * 1_000_000, TimeUnit::Millisecond => v * 1_000, TimeUnit::Microsecond => v, TimeUnit::Nanosecond => v / 1_000 };
                    let total_secs = micros.div_euclid(1_000_000);
                    let sub_us = micros.rem_euclid(1_000_000);
                    let h = total_secs.div_euclid(3600);
                    let m = (total_secs.div_euclid(60)).rem_euclid(60);
                    let s = total_secs.rem_euclid(60);
                    if sub_us == 0 { self.write_json_string(&format!("{:02}:{:02}:{:02}", h, m, s)); }
                    else { self.write_json_string(&format!("{:02}:{:02}:{:02}.{:06}", h, m, s, sub_us)); }
                }
                ValueRef::Timestamp(unit, v) => {
                    let micros = match unit { TimeUnit::Second => v * 1_000_000, TimeUnit::Millisecond => v * 1_000, TimeUnit::Microsecond => v, TimeUnit::Nanosecond => v / 1_000 };
                    let secs = micros.div_euclid(1_000_000);
                    let sub_us = micros.rem_euclid(1_000_000);
                    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, (sub_us as u32) * 1000);
                    if let Some(dt) = dt { self.write_json_string(&dt.to_string()); }
                    else { self.write_json_string(&format!("{}", v)); }
                }
                ValueRef::Interval { months, days, nanos } => {
                    let years = months.div_euclid(12);
                    let mons = months.rem_euclid(12);
                    let total_micros = nanos.div_euclid(1000);
                    let total_secs = total_micros.div_euclid(1_000_000);
                    let sub_us = total_micros.rem_euclid(1_000_000);
                    let h = total_secs.div_euclid(3600);
                    let m = (total_secs.div_euclid(60)).rem_euclid(60);
                    let s = total_secs.rem_euclid(60);
                    let mut parts = Vec::new();
                    if years != 0 { parts.push(format!("{} year", years)); }
                    if mons != 0 { parts.push(format!("{} mons", mons)); }
                    if days != 0 { parts.push(format!("{} days", days)); }
                    let time_part = if sub_us == 0 { format!("{:02}:{:02}:{:02}", h, m, s) } else { format!("{:02}:{:02}:{:02}.{:06}", h, m, s, sub_us) };
                    parts.push(time_part);
                    self.write_json_string(&parts.join(" "));
                }
                ValueRef::Text(value) => {
                    let is_json = if self.cfg.json_detection == "off" { false } else { let min_len = self.cfg.json_min_length; if value.len() < min_len { false } else { val_is_json(value) } };
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
                    let len = value.len();
                    match self.cfg.bytes_format.as_str() {
                        "off" | "len" => self.write_json_string(&format!("Bytes({})", len)),
                        "full_hex" => self.write_json_string(&format!("0x{}", hex::encode(value))),
                        _ => {
                            let n = usize::min(len, self.cfg.bytes_chunk_size);
                            let hexp = hex::encode(&value[..n]);
                            if n >= len { self.write_json_string(&format!("0x{}", hexp)); }
                            else { self.write_json_string(&format!("Bytes({}) preview(0..{}): 0x{}…", len, n, hexp)); }
                        }
                    }
                }
                other => {
                    if let Ok(val) = row.get::<usize, Value>(i) {
                        self.write_duck_value_json(&val);
                    } else {
                        let s = format!("{:?}", other);
                        self.write_json_string(&s);
                    }
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

    fn write_duck_value_json(&mut self, v: &Value) {
        match v {
            Value::Null => self.buf.push_str("null"),
            Value::Boolean(b) => write!(&mut self.buf, "{}", b).unwrap(),
            Value::TinyInt(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::SmallInt(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::Int(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::BigInt(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::HugeInt(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::UTinyInt(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::USmallInt(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::UInt(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::UBigInt(x) => write!(&mut self.buf, "{}", x).unwrap(),
            Value::Float(f) => if f.is_finite() { write!(&mut self.buf, "{}", f).unwrap() } else { self.write_json_string(&format!("{}", f)); },
            Value::Double(f) => if f.is_finite() { write!(&mut self.buf, "{}", f).unwrap() } else { self.write_json_string(&format!("{}", f)); },
            Value::Decimal(dec) => {
                let s = format!("{}", dec);
                match self.cfg.numeric_policy.as_str() {
                    "never" => {
                        if let Ok(i) = s.parse::<i128>() { write!(&mut self.buf, "{}", i).unwrap(); }
                        else if let Ok(f) = s.parse::<f64>() { write!(&mut self.buf, "{}", f).unwrap(); }
                        else { self.write_json_string(&s); }
                    }
                    "precision_threshold" => {
                        let is_fractional = s.contains('.') || s.contains('e') || s.contains('E');
                        let digits_only = s.chars().filter(|c| c.is_ascii_digit()).count();
                        if !is_fractional && digits_only <= self.cfg.numeric_threshold {
                            if let Ok(i) = s.parse::<i128>() { write!(&mut self.buf, "{}", i).unwrap(); } else { self.write_json_string(&s); }
                        } else if is_fractional && digits_only <= self.cfg.numeric_threshold {
                            if let Ok(f) = s.parse::<f64>() { write!(&mut self.buf, "{}", f).unwrap(); } else { self.write_json_string(&s); }
                        } else {
                            self.write_json_string(&s);
                        }
                    }
                    _ => self.write_json_string(&s),
                }
            }
            Value::Timestamp(unit, v) => {
                let micros = match unit { TimeUnit::Second => *v * 1_000_000, TimeUnit::Millisecond => *v * 1_000, TimeUnit::Microsecond => *v, TimeUnit::Nanosecond => *v / 1_000 };
                let secs = micros.div_euclid(1_000_000);
                let sub_us = micros.rem_euclid(1_000_000);
                let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, (sub_us as u32) * 1000);
                if let Some(dt) = dt { self.write_json_string(&dt.to_string()); } else { self.write_json_string(&format!("{}", v)); }
            }
            Value::Text(s) => self.write_json_string(s),
            Value::Blob(b) => {
                let len = b.len();
                match self.cfg.bytes_format.as_str() {
                    "off" | "len" => self.write_json_string(&format!("Bytes({})", len)),
                    "full_hex" => self.write_json_string(&format!("0x{}", hex::encode(b))),
                    _ => {
                        let n = usize::min(len, self.cfg.bytes_chunk_size);
                        let hexp = hex::encode(&b[..n]);
                        if n >= len { self.write_json_string(&format!("0x{}", hexp)); }
                        else { self.write_json_string(&format!("Bytes({}) preview(0..{}): 0x{}…", len, n, hexp)); }
                    }
                }
            }
            Value::Date32(days) => {
                let base = chrono::NaiveDate::from_ymd_opt(1970,1,1).unwrap();
                let d = base.checked_add_days(chrono::Days::new(*days as u64)).unwrap_or(base);
                self.write_json_string(&d.to_string());
            }
            Value::Time64(unit, v) => {
                let micros = match unit { TimeUnit::Second => *v * 1_000_000, TimeUnit::Millisecond => *v * 1_000, TimeUnit::Microsecond => *v, TimeUnit::Nanosecond => *v / 1_000 };
                let total_secs = micros.div_euclid(1_000_000);
                let sub_us = micros.rem_euclid(1_000_000);
                let h = total_secs.div_euclid(3600);
                let m = (total_secs.div_euclid(60)).rem_euclid(60);
                let s = total_secs.rem_euclid(60);
                if sub_us == 0 { self.write_json_string(&format!("{:02}:{:02}:{:02}", h, m, s)); }
                else { self.write_json_string(&format!("{:02}:{:02}:{:02}.{:06}", h, m, s, sub_us)); }
            }
            Value::Interval { months, days, nanos } => {
                let years = months.div_euclid(12);
                let mons = months.rem_euclid(12);
                let total_micros = nanos.div_euclid(1_000);
                let total_secs = total_micros.div_euclid(1_000_000);
                let sub_us = total_micros.rem_euclid(1_000_000);
                let h = total_secs.div_euclid(3600);
                let m = (total_secs.div_euclid(60)).rem_euclid(60);
                let s = total_secs.rem_euclid(60);
                let mut parts = Vec::new();
                if years != 0 { parts.push(format!("{} year", years)); }
                if mons != 0 { parts.push(format!("{} mons", mons)); }
                let days = *days;
                if days != 0 { parts.push(format!("{days} days")); }
                let time_part = if sub_us == 0 { format!("{:02}:{:02}:{:02}", h, m, s) } else { format!("{:02}:{:02}:{:02}.{:06}", h, m, s, sub_us) };
                parts.push(time_part);
                self.write_json_string(&parts.join(" "));
            }
            Value::List(vals) | Value::Array(vals) => {
                self.buf.push('[');
                for (i, item) in vals.iter().enumerate() {
                    if i > 0 { self.buf.push(','); }
                    self.write_duck_value_json(item);
                }
                self.buf.push(']');
            }
            Value::Enum(s) => self.write_json_string(s),
            Value::Struct(map) => {
                self.buf.push('{');
                let mut first = true;
                for (k, v) in map.iter() {
                    if !first { self.buf.push(','); } else { first = false; }
                    self.write_json_string(k);
                    self.buf.push(':');
                    self.write_duck_value_json(v);
                }
                self.buf.push('}');
            }
            Value::Map(map) => {
                self.buf.push('{');
                let mut first = true;
                for (k, v) in map.iter() {
                    if !first { self.buf.push(','); } else { first = false; }
                    let ks = format!("{k:?}");
                    self.write_json_string(&ks);
                    self.buf.push(':');
                    self.write_duck_value_json(v);
                }
                self.buf.push('}');
            }
            Value::Union(inner) => { self.write_duck_value_json(inner); }
        }
    }

}

struct RowFormatCfg { bytes_format: String, bytes_chunk_size: usize, json_detection: String, json_min_length: usize, numeric_policy: String, numeric_threshold: usize }
impl RowFormatCfg {
    fn from_env() -> Self { Self { bytes_format: std::env::var("PGPAD_BYTES_FORMAT").unwrap_or_else(|_| String::from("len")).to_lowercase(), bytes_chunk_size: std::env::var("PGPAD_BYTES_CHUNK_SIZE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n>0).unwrap_or(4096), json_detection: std::env::var("PGPAD_JSON_DETECTION").unwrap_or_else(|_| String::from("auto")).to_lowercase(), json_min_length: std::env::var("PGPAD_JSON_MIN_LENGTH").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(1), numeric_policy: std::env::var("PGPAD_NUMERIC_STRING_POLICY").unwrap_or_else(|_| String::from("precision_threshold")).to_lowercase(), numeric_threshold: std::env::var("PGPAD_NUMERIC_PRECISION_THRESHOLD").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n>0).unwrap_or(18) } }
    fn from_settings(settings: Option<&crate::database::types::OracleSettings>) -> Self { if let Some(s) = settings { Self { bytes_format: s.bytes_format.clone().unwrap_or_else(|| "len".into()).to_lowercase(), bytes_chunk_size: s.bytes_chunk_size.unwrap_or(4096), json_detection: s.json_detection.clone().unwrap_or_else(|| "auto".into()).to_lowercase(), json_min_length: s.json_min_length.unwrap_or(1), numeric_policy: s.numeric_string_policy.clone().unwrap_or_else(|| "precision_threshold".into()).to_lowercase(), numeric_threshold: s.numeric_precision_threshold.unwrap_or(18) } } else { Self::from_env() } }
}
 

#[inline]
fn val_is_json(value: &[u8]) -> bool {
    let looks_like_json = (value.starts_with(b"[") && value.ends_with(b"]"))
        || (value.starts_with(b"{") && value.ends_with(b"}"));
    looks_like_json && utils::is_json(value)
}
