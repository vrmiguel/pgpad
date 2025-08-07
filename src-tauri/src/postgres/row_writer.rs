use rust_decimal::Decimal;
use std::fmt::Write;
use tokio_postgres::{types::Type, Row};

/// Accepts any type, returning their raw bytes
mod bytes;
/// Deserializes Postgres intervals
mod interval;
/// Checks if a Pg value is null
mod null;
/// Deserializes record types
mod record;

use null::NullChecker;
use record::PgRecord;

use crate::postgres::row_writer::{bytes::PgBytes, interval::PgInterval};

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
                // Decode NUMERIC into Decimal (full precision)
                let value: Decimal = row.try_get(column_index)?;
                // Send as string to the frontend to avoid precision loss
                self.write_json_string(&value.to_string());
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

            // TODO(vini): BPCHAR and NAME are correct here?
            Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
                let value: &str = row.try_get(column_index)?;
                self.write_json_string(&value);
            }

            _ => {
                let bytes = row.try_get::<_, PgBytes>(column_index)?;
                if let Ok(value) = std::str::from_utf8(bytes.bytes) {
                    self.write_json_string(value);
                } else {
                    log::error!("Unknown type `{:?}`, kind: {:?}", pg_type, pg_type.kind());
                    self.write_json_string(&format!("\\x{}", hex::encode(bytes.bytes)));
                }
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

/// TODO(vini): convert to fmt::Display impl
fn format_pg_interval(interval: &PgInterval) -> String {
    let mut parts = Vec::new();

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

    if interval.days != 0 {
        if interval.days == 1 {
            parts.push("1 day".to_string());
        } else {
            parts.push(format!("{} days", interval.days));
        }
    }

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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use pgtemp::PgTempDB;
    use serde_json::Value;

    use crate::postgres::row_writer::RowWriter;

    #[tokio::test]
    async fn test_row_writer() {
        let now = Instant::now();
        let db = PgTempDB::async_new().await;
        println!("Created DB in {:?}ms", now.elapsed().as_millis());

        let (client, conn) = tokio_postgres::connect(&db.connection_uri(), tokio_postgres::NoTls)
            .await
            .unwrap();

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("Connection error: {}", e);
            }
        });

        client
            .batch_execute("CREATE TYPE pg_temp.mood AS ENUM ('sad','ok','happy');")
            .await
            .unwrap();

        let sql = r#"
            SELECT
                1::int AS int_col,
                3.14::float AS float_col,
                'Hello'::text AS text_col,
                true AS bool_col,
                '2025-08-07 12:00:00'::timestamp AS ts_col,
                '2025-08-07 12:00:00+00'::timestamptz AS ts_tz_col,
                '1 day'::interval AS interval_col,
                ARRAY['a','b','c'] AS array_col,
                '{"x": 10}'::json AS json_col,
                '192.168.0.1'::inet AS inet_col,
                'happy'::pg_temp.mood AS enum_col,
                ROW(1, 'foo') AS record_col
            ;
        "#;

        let rows = client.query(sql, &[]).await.unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];

        let mut writer = RowWriter::new();
        writer.add_row(&row).unwrap();
        let result = writer.finish();
        let result: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            result,
            serde_json::json!([
                [
                    1,
                    3.14,
                    "Hello",
                    true,
                    "2025-08-07 12:00:00",
                    "2025-08-07 12:00:00 UTC",
                    "1 day",
                    ["a", "b", "c"],
                    {"x": 10},
                    "192.168.0.1",
                    "happy",
                    [1, "foo"]
                ]
            ])
        );

        println!("{:?}", result);
    }
}
