use std::error::Error;

use bytes::Buf;
use chrono::Utc;
use tokio_postgres::types::{FromSql, Type};

/// Deserializes record types into a JSON array
/// E.g. `ROW('("fuzzy dice",42,1.99)'` -> `["fuzzy dice", 42, 1.99]`
// TODO(vini): make this write directly into the RowWriter's buffer
#[derive(Debug)]
pub struct PgRecord {
    pub fields: Vec<serde_json::Value>,
}

impl<'a> FromSql<'a> for PgRecord {
    fn from_sql(_: &Type, mut raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let field_count = raw.get_i32() as usize;
        let mut fields = Vec::with_capacity(field_count);

        for _ in 0..field_count {
            let field_oid = raw.get_u32();
            let field_length = raw.get_i32();

            if field_length == -1 {
                // NULL value
                fields.push(serde_json::Value::Null);
            } else {
                let field_length = field_length as usize;
                if raw.remaining() < field_length {
                    return Err("Not enough data for field".into());
                }

                let field_data = &raw[..field_length];
                raw.advance(field_length);

                let pg_type = Type::from_oid(field_oid).unwrap_or(Type::TEXT);
                let field_value = convert_pg_binary_to_json(&pg_type, field_data)?;
                fields.push(field_value);
            }
        }

        Ok(PgRecord { fields })
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::RECORD)
    }
}

#[allow(clippy::wildcard_in_or_patterns)]
// TODO(vini): this is a pretty bad implementation
fn convert_pg_binary_to_json(
    pg_type: &Type,
    data: &[u8],
) -> Result<serde_json::Value, Box<dyn Error + Sync + Send>> {
    let mut buf = data;

    match *pg_type {
        Type::BOOL => {
            let value = buf.get_u8() != 0;
            Ok(serde_json::Value::Bool(value))
        }

        Type::INT2 => {
            let value = buf.get_i16();
            Ok(serde_json::Value::Number(value.into()))
        }
        Type::INT4 => {
            let value = buf.get_i32();
            Ok(serde_json::Value::Number(value.into()))
        }
        Type::INT8 => {
            let value = buf.get_i64();
            Ok(serde_json::Value::Number(value.into()))
        }

        Type::FLOAT4 => {
            let value = buf.get_f32();
            if let Some(num) = serde_json::Number::from_f64(value as f64) {
                Ok(serde_json::Value::Number(num))
            } else {
                Ok(serde_json::Value::String(value.to_string()))
            }
        }
        Type::FLOAT8 => {
            let value = buf.get_f64();
            if let Some(num) = serde_json::Number::from_f64(value) {
                Ok(serde_json::Value::Number(num))
            } else {
                Ok(serde_json::Value::String(value.to_string()))
            }
        }

        Type::TIMESTAMP => {
            let microseconds = buf.get_i64();
            // PostgreSQL epoch is 2000-01-01 00:00:00 UTC
            let pg_epoch = chrono::NaiveDate::from_ymd_opt(2000, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let timestamp = pg_epoch + chrono::Duration::microseconds(microseconds);
            Ok(serde_json::Value::String(timestamp.to_string()))
        }
        Type::TIMESTAMPTZ => {
            let microseconds = buf.get_i64();
            let pg_epoch = chrono::NaiveDate::from_ymd_opt(2000, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let timestamp = chrono::DateTime::<Utc>::from_naive_utc_and_offset(pg_epoch, Utc)
                + chrono::Duration::microseconds(microseconds);
            Ok(serde_json::Value::String(timestamp.to_rfc3339()))
        }

        // For text types and unknown types, convert to string
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | _ => {
            match std::str::from_utf8(data) {
                Ok(s) => Ok(serde_json::Value::String(s.to_string())),
                Err(_) => {
                    // If it's not valid UTF-8, return as hex string
                    Ok(serde_json::Value::String(format!(
                        "\\x{}",
                        hex::encode(data)
                    )))
                }
            }
        }
    }
}
