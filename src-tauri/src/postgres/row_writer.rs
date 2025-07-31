use bytes::Buf;
use chrono::Utc;
use std::{error::Error, fmt::Write};
use tokio_postgres::{
    types::{FromSql, Type},
    Row,
};

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

/// Taken from https://github.com/sfackler/rust-postgres/issues/879#issuecomment-1084149480
#[derive(Debug)]
pub struct NullChecker(pub bool);

impl tokio_postgres::types::FromSql<'_> for NullChecker {
    fn from_sql(
        _ty: &tokio_postgres::types::Type,
        _raw: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(Self(false))
    }

    fn from_sql_null(
        _ty: &tokio_postgres::types::Type,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        Ok(Self(true))
    }

    fn accepts(_ty: &tokio_postgres::types::Type) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct PgInterval {
    pub months: i32,
    pub days: i32,
    pub microseconds: i64,
}

impl<'a> FromSql<'a> for PgInterval {
    fn from_sql(_: &Type, mut raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let microseconds = raw.get_i64();
        let days = raw.get_i32();
        let months = raw.get_i32();
        Ok(PgInterval {
            months,
            days,
            microseconds,
        })
    }

    fn accepts(ty: &Type) -> bool {
        matches!(*ty, Type::INTERVAL)
    }
}

/// A somewhat efficient way of converting the raw Postgres query results into a JSON string.
///
/// This replaces the previous Vec<Vec<Json>> code, improving memory usage and performance
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

    pub fn add_row(&mut self, row: &Row) -> Result<(), anyhow::Error> {
        if self.row_count > 0 {
            self.json.push(',');
        }

        self.json.push('[');
        for i in 0..row.len() {
            if i > 0 {
                self.json.push(',');
            }
            self.write_pg_value_as_json(row, i)?;
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

    pub fn finish(&mut self) -> String {
        self.json.push(']');

        std::mem::replace(&mut self.json, String::new())
    }

    pub fn clear(&mut self) {
        self.json.reserve(2);
        self.json.push('[');
        self.row_count = 0;
    }

    fn write_pg_value_as_json(
        &mut self,
        row: &Row,
        column_index: usize,
    ) -> Result<(), anyhow::Error> {
        let column = &row.columns()[column_index];
        let pg_type = column.type_();

        // Check for NULL using your existing NullChecker
        if row.try_get::<_, NullChecker>(column_index)?.0 {
            self.json.push_str("null");
            return Ok(());
        }

        use tokio_postgres::types::Type;

        match *pg_type {
            Type::BOOL => {
                let value: bool = row.try_get(column_index)?;
                if value {
                    self.json.push_str("true");
                } else {
                    self.json.push_str("false");
                }
            }

            Type::INT2 => {
                let value: i16 = row.try_get(column_index)?;
                write!(&mut self.json, "{}", value)?;
            }
            Type::INT4 => {
                let value: i32 = row.try_get(column_index)?;
                write!(&mut self.json, "{}", value)?;
            }
            Type::INT8 => {
                let value: i64 = row.try_get(column_index)?;
                write!(&mut self.json, "{}", value)?;
            }

            Type::FLOAT4 => {
                let value: f32 = row.try_get(column_index)?;
                if value.is_finite() {
                    write!(&mut self.json, "{}", value)?;
                } else {
                    write!(&mut self.json, "\"{}\"", value)?; // NaN, Infinity as strings
                }
            }
            Type::FLOAT8 => {
                let value: f64 = row.try_get(column_index)?;
                if value.is_finite() {
                    write!(&mut self.json, "{}", value)?;
                } else {
                    write!(&mut self.json, "\"{}\"", value)?;
                }
            }

            Type::NUMERIC => {
                let value: String = row.try_get(column_index)?;
                // Try to parse as number, fallback to string
                if value.parse::<f64>().is_ok() {
                    self.json.push_str(&value);
                } else {
                    self.write_json_string(&value);
                }
            }

            Type::JSON | Type::JSONB => {
                let value: serde_json::Value = row.try_get(column_index)?;
                self.json.push_str(&value.to_string());
            }

            Type::TEXT_ARRAY => {
                let value: Vec<String> = row.try_get(column_index)?;
                self.json.push('[');
                for (i, item) in value.iter().enumerate() {
                    if i > 0 {
                        self.json.push(',');
                    }
                    self.write_json_string(item);
                }
                self.json.push(']');
            }
            Type::INT4_ARRAY => {
                let value: Vec<i32> = row.try_get(column_index)?;
                self.json.push('[');
                for (i, item) in value.iter().enumerate() {
                    if i > 0 {
                        self.json.push(',');
                    }
                    write!(&mut self.json, "{}", item)?;
                }
                self.json.push(']');
            }
            Type::INT8_ARRAY => {
                let value: Vec<i64> = row.try_get(column_index)?;
                self.json.push('[');
                for (i, item) in value.iter().enumerate() {
                    if i > 0 {
                        self.json.push(',');
                    }
                    write!(&mut self.json, "{}", item)?;
                }
                self.json.push(']');
            }

            Type::TIMESTAMP => {
                let value: chrono::NaiveDateTime = row.try_get(column_index)?;
                self.write_json_string(&value.to_string());
            }

            Type::TIMESTAMPTZ => {
                let value: chrono::DateTime<chrono::Utc> = row.try_get(column_index)?;
                self.write_json_string(&value.to_string());
            }

            Type::INTERVAL => {
                let value: PgInterval = row.try_get(column_index)?;
                self.write_json_string(&format_pg_interval(&value));
            }

            Type::INET => {
                let value: std::net::IpAddr = row.try_get(column_index)?;
                self.write_json_string(&value.to_string());
            }

            Type::RECORD => {
                let value: PgRecord = row.try_get(column_index)?;
                self.json.push('[');
                for (i, field) in value.fields.iter().enumerate() {
                    if i > 0 {
                        self.json.push(',');
                    }
                    self.json.push_str(&field.to_string());
                }
                self.json.push(']');
            }

            // Default to string
            _ => {
                let value: String = row.try_get(column_index)?;
                self.write_json_string(&value);
            }
        }

        Ok(())
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

fn format_pg_interval(interval: &PgInterval) -> String {
    let mut parts = Vec::new();

    // Handle years and months
    if interval.months != 0 {
        let years = interval.months / 12;
        let months = interval.months % 12;

        if years != 0 {
            if years == 1 {
                parts.push("1 year".to_string());
            } else {
                parts.push(format!("{} years", years));
            }
        }

        if months != 0 {
            if months == 1 {
                parts.push("1 mon".to_string());
            } else {
                parts.push(format!("{} mons", months));
            }
        }
    }

    // Handle days
    if interval.days != 0 {
        if interval.days == 1 {
            parts.push("1 day".to_string());
        } else {
            parts.push(format!("{} days", interval.days));
        }
    }

    // Handle time portion (microseconds)
    if interval.microseconds != 0 {
        let total_seconds = interval.microseconds / 1_000_000;
        let microseconds = interval.microseconds % 1_000_000;

        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 || minutes > 0 || seconds > 0 || microseconds > 0 {
            if microseconds == 0 {
                parts.push(format!("{:02}:{:02}:{:02}", hours, minutes, seconds));
            } else {
                // Format microseconds as fractional seconds
                let fractional = microseconds as f64 / 1_000_000.0;
                parts.push(format!(
                    "{:02}:{:02}:{:06.3}",
                    hours,
                    minutes,
                    seconds as f64 + fractional
                ));
            }
        }
    }

    if parts.is_empty() {
        "00:00:00".to_string()
    } else {
        parts.join(" ")
    }
}

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
            let pg_epoch = chrono::DateTime::<Utc>::from_utc(
                chrono::NaiveDate::from_ymd_opt(2000, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
                Utc,
            );
            let timestamp = pg_epoch + chrono::Duration::microseconds(microseconds);
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

#[cfg(test)]
mod tests {
    use crate::postgres::row_writer::RowWriter;

    // cargo test --package app --lib -- postgres::row_writer::tests::test_pg_val_to_json --exact --show-output --ignored
    #[ignore = "Requires a local Postgres instance"]
    #[tokio::test]
    async fn test_pg_val_to_json() {
        let (client, conn) = tokio_postgres::connect(
            "postgresql://postgres:postgres@localhost:5432/postgres",
            tokio_postgres::NoTls,
        )
        .await
        .unwrap();

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("Connection error: {}", e);
            }
        });

        let sql = r#"
            SELECT
                1::int AS int_col,
                3.14::float AS float_col,
                'Hello'::text AS text_col,
                true AS bool_col,
                now()::timestamp AS ts_col,
                now()::timestamptz AS ts_tz_col,
                '1 day'::interval AS interval_col,
                ARRAY['a','b','c'] AS array_col,
                '{"x": 10}'::json AS json_col,
                '192.168.0.1'::inet AS inet_col,
                ROW(1, 'foo') AS record_col
            ;
        "#;

        let rows = client.query(sql, &[]).await.unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];

        let mut writer = RowWriter::new();
        writer.add_row(&row).unwrap();
        let result = writer.finish();

        println!("{:?}", result);
    }
}
