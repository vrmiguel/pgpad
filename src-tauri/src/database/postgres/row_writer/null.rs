use std::error::Error;

use tokio_postgres::types::{FromSql, Type};

/// Taken from https://github.com/sfackler/rust-postgres/issues/879#issuecomment-1084149480
#[derive(Debug)]
pub struct NullChecker(pub bool);

impl FromSql<'_> for NullChecker {
    fn from_sql(_ty: &Type, _raw: &[u8]) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        Ok(Self(false))
    }

    fn from_sql_null(_ty: &Type) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        Ok(Self(true))
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}
