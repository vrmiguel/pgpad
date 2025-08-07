use std::error::Error;

use tokio_postgres::types::{FromSql, Type};

#[derive(Debug)]
pub struct PgBytes<'a> {
    pub bytes: &'a [u8],
}

impl<'a> FromSql<'a> for PgBytes<'a> {
    fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        Ok(Self { bytes: raw })
    }

    fn accepts(_: &Type) -> bool {
        true
    }
}
