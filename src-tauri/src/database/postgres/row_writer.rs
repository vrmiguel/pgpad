use serde_json::value::RawValue;
use std::fmt::Write;
use tokio_postgres::{types::Type, Row};

/// Accepts any type, returning their raw bytes
mod bytes;
/// Deserializes Postgres intervals
mod interval;
/// Checks if a Pg value is null
mod null;
/// Deserializes NUMERIC
mod numeric;
/// Deserializes record types
mod record;

use null::NullChecker;
use record::PgRecord;

use crate::database::postgres::row_writer::{
    bytes::PgBytes, interval::PgInterval, numeric::PostgresNumeric,
};

/// A somewhat efficient way of converting the raw Postgres query results into a JSON string.
/// Think of this as a writer of Vec<Vec<Json>>.
pub struct RowWriter {
    buf: String,
    row_count: usize,
}

impl RowWriter {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            row_count: 0,
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
        for i in 0..row.len() {
            if i > 0 {
                self.buf.push(',');
            }
            self.write_pg_value_as_json(row, i)?;
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

    fn write_pg_value_as_json(
        &mut self,
        row: &Row,
        column_index: usize,
    ) -> Result<(), anyhow::Error> {
        let column = &row.columns()[column_index];
        let pg_type = column.type_();

        // Is this row null?
        if row.try_get::<_, NullChecker>(column_index)?.0 {
            self.buf.push_str("null");
            return Ok(());
        }

        match *pg_type {
            Type::BOOL => {
                let value: bool = row.try_get(column_index)?;
                if value {
                    self.buf.push_str("true");
                } else {
                    self.buf.push_str("false");
                }
            }

            Type::INT2 => {
                let value: i16 = row.try_get(column_index)?;
                write!(&mut self.buf, "{}", value)?;
            }
            Type::INT4 => {
                let value: i32 = row.try_get(column_index)?;
                write!(&mut self.buf, "{}", value)?;
            }
            Type::INT8 => {
                let value: i64 = row.try_get(column_index)?;
                write!(&mut self.buf, "{}", value)?;
            }

            Type::FLOAT4 => {
                let value: f32 = row.try_get(column_index)?;
                if value.is_finite() {
                    write!(&mut self.buf, "{}", value)?;
                } else {
                    // NaN, Infinity
                    write!(&mut self.buf, "\"{}\"", value)?;
                }
            }
            Type::FLOAT8 => {
                let value: f64 = row.try_get(column_index)?;
                if value.is_finite() {
                    write!(&mut self.buf, "{}", value)?;
                } else {
                    write!(&mut self.buf, "\"{}\"", value)?;
                }
            }

            Type::NUMERIC => {
                // Decode NUMERIC into Decimal (full precision)
                let value: PostgresNumeric = row.try_get(column_index)?;
                // Send as string to the frontend to avoid precision loss
                self.write_json_string(&value.to_string());
            }

            Type::JSON | Type::JSONB => {
                let value: serde_json::Value = row.try_get(column_index)?;
                self.buf.push_str(&value.to_string());
            }

            Type::UUID => {
                let value: uuid::Uuid = row.try_get(column_index)?;
                self.write_json_string(&value.to_string());
            }

            Type::TEXT_ARRAY => {
                let value: Vec<String> = row.try_get(column_index)?;
                self.buf.push('[');
                for (i, item) in value.iter().enumerate() {
                    if i > 0 {
                        self.buf.push(',');
                    }
                    self.write_json_string(item);
                }
                self.buf.push(']');
            }
            Type::INT4_ARRAY => {
                let value: Vec<i32> = row.try_get(column_index)?;
                self.buf.push('[');
                for (i, item) in value.iter().enumerate() {
                    if i > 0 {
                        self.buf.push(',');
                    }
                    write!(&mut self.buf, "{}", item)?;
                }
                self.buf.push(']');
            }
            Type::INT8_ARRAY => {
                let value: Vec<i64> = row.try_get(column_index)?;
                self.buf.push('[');
                for (i, item) in value.iter().enumerate() {
                    if i > 0 {
                        self.buf.push(',');
                    }
                    write!(&mut self.buf, "{}", item)?;
                }
                self.buf.push(']');
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
                self.write_json_string(&value.to_string());
            }

            Type::INET => {
                let value: std::net::IpAddr = row.try_get(column_index)?;
                self.write_json_string(&value.to_string());
            }

            Type::RECORD => {
                let value: PgRecord = row.try_get(column_index)?;
                self.buf.push('[');
                for (i, field) in value.fields.iter().enumerate() {
                    if i > 0 {
                        self.buf.push(',');
                    }
                    self.buf.push_str(&field.to_string());
                }
                self.buf.push(']');
            }

            // TODO(vini): BPCHAR and NAME are correct here?
            Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
                let value: &str = row.try_get(column_index)?;
                self.write_json_string(value);
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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use pgtemp::PgTempDB;
    use serde_json::Value;

    use crate::database::postgres::row_writer::RowWriter;

    #[allow(clippy::approx_constant)]
    #[tokio::test]
    async fn test_row_writer() {
        let now = Instant::now();
        let db = PgTempDB::async_new().await;
        println!("Created DB in {:?}ms", now.elapsed().as_millis());

        let (client, conn) = tokio_postgres::connect(&db.connection_uri(), tokio_postgres::NoTls)
            .await
            .unwrap();

        tokio::task::spawn(async move {
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
                '{"x": 10}'::json AS jsonb_col,
                '192.168.0.1'::inet AS inet_col,
                'happy'::pg_temp.mood AS enum_col,
                ROW(1, 'foo') AS record_col,
                '550e8400-e29b-41d4-a716-446655440000'::uuid AS uuid_col
        "#;

        let rows = client.query(sql, &[]).await.unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];

        let mut writer = RowWriter::new();
        writer.add_row(row).unwrap();
        let result = writer.finish();
        let result: Value = serde_json::from_str(result.get()).unwrap();
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
                    {"x": 10},
                    "192.168.0.1",
                    "happy",
                    [1, "foo"],
                    "550e8400-e29b-41d4-a716-446655440000"
                ]
            ])
        );

        let empty_sql = "SELECT 1::int AS col WHERE false";
        let empty_rows = client.query(empty_sql, &[]).await.unwrap();
        assert_eq!(empty_rows.len(), 0);

        let mut empty_writer = RowWriter::new();
        let empty_result = empty_writer.finish();
        let empty_result: Value = serde_json::from_str(empty_result.get()).unwrap();
        assert_eq!(empty_result, serde_json::json!([]));
        assert!(empty_writer.is_empty());

        let null_sql = r#"
            SELECT
                NULL::int AS null_int,
                NULL::text AS null_text,
                NULL::boolean AS null_bool,
                NULL::timestamp AS null_timestamp,
                NULL::json AS null_json
        "#;

        let null_rows = client.query(null_sql, &[]).await.unwrap();
        let null_row = &null_rows[0];

        let mut null_writer = RowWriter::new();
        null_writer.add_row(null_row).unwrap();
        let null_result = null_writer.finish();
        let null_result: Value = serde_json::from_str(null_result.get()).unwrap();
        assert_eq!(
            null_result,
            serde_json::json!([[null, null, null, null, null]])
        );

        let multi_sql = r#"
            SELECT * FROM (VALUES 
                (1, 'first'),
                (2, 'second'),
                (3, 'third')
            ) AS t(id, name)
        "#;

        let multi_rows = client.query(multi_sql, &[]).await.unwrap();
        assert_eq!(multi_rows.len(), 3);

        let mut multi_writer = RowWriter::new();
        for row in &multi_rows {
            multi_writer.add_row(row).unwrap();
        }
        let multi_result = multi_writer.finish();
        let multi_result: Value = serde_json::from_str(multi_result.get()).unwrap();
        assert_eq!(
            multi_result,
            serde_json::json!([[1, "first"], [2, "second"], [3, "third"]])
        );
        assert_eq!(multi_writer.len(), 0);

        let float_sql = r#"
            SELECT
                'NaN'::float8 AS nan_val,
                'Infinity'::float8 AS inf_val,
                '-Infinity'::float8 AS neg_inf_val,
                0.0::float8 AS zero_val,
                1.23456789::float8 AS normal_val
        "#;

        let float_rows = client.query(float_sql, &[]).await.unwrap();
        let float_row = &float_rows[0];

        let mut float_writer = RowWriter::new();
        float_writer.add_row(float_row).unwrap();
        let float_result = float_writer.finish();
        let float_result: Value = serde_json::from_str(float_result.get()).unwrap();

        assert_eq!(
            float_result,
            serde_json::json!([["NaN", "inf", "-inf", 0, 1.23456789]])
        );

        let escape_sql = r#"
            SELECT
                '"quoted"' AS quotes,
                'line1' || chr(10) || 'line2' AS newlines,
                'tab' || chr(9) || 'here' AS tabs,
                'back\slash' AS backslash,
                'control' || chr(8) || 'chars' AS control_chars
        "#;

        let escape_rows = client.query(escape_sql, &[]).await.unwrap();
        let escape_row = &escape_rows[0];

        let mut escape_writer = RowWriter::new();
        escape_writer.add_row(escape_row).unwrap();
        let escape_result = escape_writer.finish();
        let escape_result: Value = serde_json::from_str(escape_result.get()).unwrap();

        assert!(escape_result.is_array());
        let first_row = &escape_result[0];
        assert_eq!(first_row[0], "\"quoted\"");
        assert_eq!(first_row[1], "line1\nline2");
        assert_eq!(first_row[2], "tab\there");
        assert_eq!(first_row[3], "back\\slash");

        let array_sql = r#"
            SELECT
                ARRAY[]::text[] AS empty_text_array,
                ARRAY[]::int[] AS empty_int_array,
                ARRAY[1,2,3,4,5] AS int_array,
                ARRAY[1,2,3,4,5]::bigint[] AS bigint_array,
                ARRAY['hello', 'world', 'test'] AS text_array
        "#;

        let array_rows = client.query(array_sql, &[]).await.unwrap();
        let array_row = &array_rows[0];

        let mut array_writer = RowWriter::new();
        array_writer.add_row(array_row).unwrap();
        let array_result = array_writer.finish();
        let array_result: Value = serde_json::from_str(array_result.get()).unwrap();
        assert_eq!(
            array_result,
            serde_json::json!([[
                [],
                [],
                [1, 2, 3, 4, 5],
                [1, 2, 3, 4, 5],
                ["hello", "world", "test"]
            ]])
        );

        let numeric_sql = r#"
            SELECT
                123.456789123456789::numeric AS high_precision,
                999999999999999999999999999999.123456789::numeric AS very_large,
                0.000000000000000001::numeric AS very_small
        "#;

        let numeric_rows = client.query(numeric_sql, &[]).await.unwrap();
        let numeric_row = &numeric_rows[0];

        let mut numeric_writer = RowWriter::new();
        numeric_writer.add_row(numeric_row).unwrap();
        let numeric_result = numeric_writer.finish();
        let numeric_result: Value = serde_json::from_str(numeric_result.get()).unwrap();

        let numeric_expected = serde_json::json!([[
            "123.456789123456789",
            "999999999999999999999999999999.123456789",
            "0.000000000000000001"
        ]]);

        assert_eq!(numeric_result, numeric_expected);

        println!("numeric_result: {:?}", numeric_result);

        let interval_sql = r#"
            SELECT
                '0 seconds'::interval AS zero_interval,
                '1 year 2 months 3 days 4 hours 5 minutes 6 seconds'::interval AS complex_interval,
                '-1 day'::interval AS negative_interval,
                '1 microsecond'::interval AS microsecond_interval
        "#;

        let interval_rows = client.query(interval_sql, &[]).await.unwrap();
        let interval_row = &interval_rows[0];

        let mut interval_writer = RowWriter::new();
        interval_writer.add_row(interval_row).unwrap();
        let interval_result = interval_writer.finish();
        let interval_result: Value = serde_json::from_str(interval_result.get()).unwrap();

        let expected_result = serde_json::json!([[
            "00:00:00",
            "1 year 2 mons 3 days 04:05:06",
            "-1 days",
            "00:00:00.000001"
        ]]);

        assert_eq!(interval_result, expected_result);
    }
}
