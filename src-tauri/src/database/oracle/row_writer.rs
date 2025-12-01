use serde_json::value::RawValue;
use oracle::sql_type::{Blob, OracleType};
use std::io::Read;
use std::fmt::Write;

pub struct RowWriter {
    buf: String,
    row_count: usize,
    column_info: Vec<oracle::ColumnInfo>,
    cfg: RowFormatCfg,
    sender: Option<crate::database::types::ExecSender>,
}

impl RowWriter {
    pub fn new(column_info: Vec<oracle::ColumnInfo>) -> Self {
        let cfg = RowFormatCfg::from_env();
        Self { buf: String::new(), row_count: 0, column_info, cfg, sender: None }
    }

    pub fn with_settings(column_info: Vec<oracle::ColumnInfo>, settings: Option<&crate::database::types::OracleSettings>) -> Self {
        let cfg = RowFormatCfg::from_settings(settings);
        Self { buf: String::new(), row_count: 0, column_info, cfg, sender: None }
    }

    pub fn with_sender(mut self, sender: crate::database::types::ExecSender) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn add_row(&mut self, row: &oracle::Row, row_index: usize) -> Result<(), anyhow::Error> {
        if self.row_count == 0 { self.buf.push('['); }
        if self.row_count > 0 { self.buf.push(','); }
        self.buf.push('[');
        for i in 0..self.column_info.len() {
            if i > 0 { self.buf.push(','); }
            let is_bool = is_boolean_candidate(&self.column_info[i]);
            let prefer_string_number = match self.column_info[i].oracle_type() {
                OracleType::Number(precision, scale) => match self.cfg.numeric_policy.as_str() {
                    "always" => true,
                    "never" => false,
                    _ => usize::from(*precision) > self.cfg.numeric_threshold || *scale != 0,
                },
                _ => false,
            };

            if prefer_string_number {
                if let Ok(opt) = row.get::<usize, Option<String>>(i) {
                    match opt {
                        None => self.write_json_string("NULL"),
                        Some(s) => self.write_json_string(&s),
                    }
                    continue;
                }
            }

            if let Ok(opt) = row.get::<usize, Option<i64>>(i) {
                match opt {
                    None => self.write_json_string("NULL"),
                    Some(v) => {
                        if is_bool && v == 0 { self.buf.push_str("false"); }
                        else if is_bool && v == 1 { self.buf.push_str("true"); }
                        else { write!(&mut self.buf, "{}", v)?; }
                    }
                }
                continue;
            }

            if let Ok(opt) = row.get::<usize, Option<f64>>(i) {
                match opt {
                    None => self.write_json_string("NULL"),
                    Some(v) => write!(&mut self.buf, "{}", v)?,
                }
                continue;
            }

            // Dates/timestamps handled via string branch below

                if let Ok(opt) = row.get::<usize, Option<String>>(i) {
                    match opt {
                        None => self.write_json_string("NULL"),
                        Some(s) => {
                            let sl = s.trim();
                            if is_bool && sl.len() == 1 {
                            match sl.chars().next().unwrap() {
                                'Y' | 'y' | 'T' | 't' | '1' => { self.buf.push_str("true"); }
                                'N' | 'n' | 'F' | 'f' | '0' => { self.buf.push_str("false"); }
                                _ => {
                                    if val_is_json(s.as_bytes()) { self.buf.write_str(&s)?; }
                                    else { self.write_json_string(&s); }
                                }
                            }
                        } else if self.cfg.json_detection != "off" && s.len() >= self.cfg.json_min_length && val_is_json(s.as_bytes()) {
                            self.buf.write_str(&s)?;
                                } else {
                            let is_large_text = is_large_text_candidate(&self.column_info[i]);
                            if is_large_text && s.len() > TEXT_PREVIEW_THRESHOLD {
                                write_size_tag(&mut self.buf, "Clob", s.len());
                            } else {
                                let s2 = if prefer_string_number { crate::database::oracle::numeric::normalize_number_string(&s) } else { s };
                                self.write_json_string(&s2);
                            }
                        }
                    }
                }
                continue;
            }

            if let Ok(opt) = row.get::<usize, Option<Vec<u8>>>(i) {
                match opt {
                    None => self.write_json_string("NULL"),
                    Some(bytes) => {
                        let len = bytes.len();
                        let raw_mode = self.cfg.raw_format.as_str();
                        let chunk_size = self.cfg.raw_chunk_size;
                        match raw_mode {
                            "off" => self.write_json_string(&format!("Raw({})", len)),
                            "full_hex" => {
                                let hex_full = if len > 0 { hex::encode(&bytes[..]) } else { String::new() };
                                self.write_json_string(&format!("Raw({}): 0x{}", len, hex_full));
                            }
                            _ => {
                                let preview_len = usize::min(len, chunk_size);
                                let hex_preview = if preview_len > 0 { hex::encode(&bytes[..preview_len]) } else { String::new() };
                                if preview_len >= len {
                                    self.write_json_string(&format!("Raw({}): 0x{}", len, hex_preview));
                                } else {
                                    self.write_json_string(&format!("Raw({}) preview(0..{}): 0x{}…", len, preview_len, hex_preview));
                                }
                            }
                        }
                    }
                }
                continue;
            }

            if let Ok(opt) = row.get::<usize, Option<Blob>>(i) {
                match opt {
                    None => self.write_json_string("NULL"),
                    Some(mut blob) => {
                        let mode = self.cfg.blob_stream.as_str();
                        let chunk = self.cfg.blob_chunk_size;
                        match mode {
                            "off" | "len" | "" => {
                                self.write_json_string("Blob");
                            }
                            "preview" => {
                                let mut buf = vec![0u8; chunk];
                                if let Ok(n) = blob.read(&mut buf) {
                                    if n > 0 {
                                        let hex_preview = hex::encode(&buf[..n]);
                                        self.write_json_string(&format!("Blob preview(0..{}): 0x{}…", n, hex_preview));
                                    } else {
                                        self.write_json_string("Blob");
                                    }
                                } else {
                                    self.write_json_string("Blob");
                                }
                            }
                            "stream" => {
                                self.write_json_string("Blob");
                                if let Some(s) = &self.sender {
                                    let mut offset = 0usize;
                                    let mut buf = vec![0u8; chunk];
                                    loop {
                                        match blob.read(&mut buf) {
                                            Ok(n) if n > 0 => {
                                                let hex_chunk = hex::encode(&buf[..n]);
                                                let _ = s.send(crate::database::types::QueryExecEvent::BlobChunk { row_index, column_index: i, offset, hex_chunk });
                                                offset += n;
                                            }
                                            _ => break,
                                        }
                                    }
                                }
                            }
                            _ => {
                                self.write_json_string("Blob");
                            }
                        }
                    }
                }
                continue;
            }

            self.write_json_string("NULL");
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
    looks_like_json && crate::utils::is_json(value)
}

fn is_boolean_candidate(info: &oracle::ColumnInfo) -> bool {
    match info.oracle_type() {
        OracleType::Char(size) => *size == 1,
        OracleType::NChar(size) => *size == 1,
        OracleType::Varchar2(size) => *size == 1,
        OracleType::NVarchar2(size) => *size == 1,
        OracleType::Number(precision, scale) => *precision == 1 && *scale == 0,
        _ => false,
    }
}

fn is_large_text_candidate(info: &oracle::ColumnInfo) -> bool {
    matches!(info.oracle_type(), OracleType::CLOB | OracleType::NCLOB)
}

const TEXT_PREVIEW_THRESHOLD: usize = 1024;

fn write_size_tag(buf: &mut String, tag: &str, len: usize) {
    write!(buf, "\"{}({})\"", tag, len).unwrap();
}

struct RowFormatCfg {
    raw_format: String,
    raw_chunk_size: usize,
    blob_stream: String,
    blob_chunk_size: usize,
    numeric_policy: String,
    numeric_threshold: usize,
    json_detection: String,
    json_min_length: usize,
}

impl RowFormatCfg {
    fn from_env() -> Self {
        Self {
            raw_format: std::env::var("ORACLE_RAW_FORMAT").unwrap_or_else(|_| String::from("preview")).to_lowercase(),
            raw_chunk_size: std::env::var("ORACLE_RAW_CHUNK_SIZE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n > 0).unwrap_or(16),
            blob_stream: std::env::var("ORACLE_BLOB_STREAM").unwrap_or_else(|_| String::from("len")).to_lowercase(),
            blob_chunk_size: std::env::var("ORACLE_BLOB_CHUNK_SIZE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n > 0).unwrap_or(4096),
            numeric_policy: std::env::var("PGPAD_NUMERIC_STRING_POLICY").unwrap_or_else(|_| String::from("precision_threshold")).to_lowercase(),
            numeric_threshold: std::env::var("PGPAD_NUMERIC_PRECISION_THRESHOLD").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&n| n>0).unwrap_or(18),
            json_detection: std::env::var("PGPAD_JSON_DETECTION").unwrap_or_else(|_| String::from("auto")).to_lowercase(),
            json_min_length: std::env::var("PGPAD_JSON_MIN_LENGTH").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(1),
        }
    }

    fn from_settings(settings: Option<&crate::database::types::OracleSettings>) -> Self {
        if let Some(s) = settings {
            Self {
                raw_format: s.raw_format.clone().unwrap_or_else(|| "preview".into()).to_lowercase(),
                raw_chunk_size: s.raw_chunk_size.unwrap_or(16),
                blob_stream: s.blob_stream.clone().unwrap_or_else(|| "len".into()).to_lowercase(),
                blob_chunk_size: s.blob_chunk_size.unwrap_or(4096),
                numeric_policy: s.numeric_string_policy.clone().unwrap_or_else(|| "precision_threshold".into()).to_lowercase(),
                numeric_threshold: s.numeric_precision_threshold.unwrap_or(18),
                json_detection: s.json_detection.clone().unwrap_or_else(|| "auto".into()).to_lowercase(),
                json_min_length: s.json_min_length.unwrap_or(1),
            }
        } else {
            Self::from_env()
        }
    }
}
