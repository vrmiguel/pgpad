use std::error::Error;

use bytes::Buf;
use tokio_postgres::types::{FromSql, Type};

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
