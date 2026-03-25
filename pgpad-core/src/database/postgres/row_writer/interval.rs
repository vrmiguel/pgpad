use std::{error::Error, fmt};

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

impl fmt::Display for PgInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut has_written = false;

        if self.months != 0 {
            let years = self.months / 12;
            let months = self.months % 12;

            if years != 0 {
                if years == 1 {
                    write!(f, "1 year")?;
                } else {
                    write!(f, "{} years", years)?;
                }
                has_written = true;
            }

            if months != 0 {
                if has_written {
                    write!(f, " ")?;
                }
                if months == 1 {
                    write!(f, "1 mon")?;
                } else {
                    write!(f, "{} mons", months)?;
                }
                has_written = true;
            }
        }

        if self.days != 0 {
            if has_written {
                write!(f, " ")?;
            }
            if self.days == 1 {
                write!(f, "1 day")?;
            } else {
                write!(f, "{} days", self.days)?;
            }
            has_written = true;
        }

        if self.microseconds != 0 {
            let total_seconds = self.microseconds / 1_000_000;
            let microseconds = self.microseconds % 1_000_000;
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;

            if hours > 0 || minutes > 0 || seconds > 0 || microseconds > 0 {
                if has_written {
                    write!(f, " ")?;
                }
                if microseconds == 0 {
                    write!(f, "{:02}:{:02}:{:02}", hours, minutes, seconds)?;
                } else {
                    // Format microseconds as fractional seconds
                    let fractional = microseconds as f64 / 1_000_000.0;
                    write!(
                        f,
                        "{:02}:{:02}:{:09.6}",
                        hours,
                        minutes,
                        seconds as f64 + fractional
                    )?;
                }
                has_written = true;
            }
        }

        if !has_written {
            write!(f, "00:00:00")?;
        }

        Ok(())
    }
}
