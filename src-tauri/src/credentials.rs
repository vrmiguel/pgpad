use crate::{database::types::DatabaseInfo, error::Error};
use anyhow::Context;
use keyring::Entry;
use std::fmt::Write;
use url::Url;
use uuid::Uuid;

const SERVICE_NAME: &str = "pgpad";

pub fn extract_sensitive_data(
    mut database_info: DatabaseInfo,
) -> Result<(DatabaseInfo, Option<String>), Error> {
    match &mut database_info {
        DatabaseInfo::Postgres {
            connection_string, ..
        } => {
            let mut url =
                Url::parse(connection_string).context("Failed to parse connection string")?;
            let password = url.password().map(ToOwned::to_owned);
            url.set_password(None).map_err(|_| {
                Error::Any(anyhow::anyhow!(
                    "Failed to remove password from connection string",
                ))
            })?;

            connection_string.clear();
            write!(connection_string, "{}", url)?;

            Ok((database_info, password))
        }
        DatabaseInfo::SQLite { passphrase, .. } => {
            let pw = passphrase.take();
            Ok((database_info, pw))
        }
        DatabaseInfo::DuckDB { .. } => Ok((database_info, None)),
        DatabaseInfo::Oracle {
            connection_string, ..
        } => {
            let mut url =
                Url::parse(connection_string).context("Failed to parse connection string")?;
            let password = url.password().map(ToOwned::to_owned);
            url.set_password(None).map_err(|_| {
                Error::Any(anyhow::anyhow!(
                    "Failed to remove password from connection string",
                ))
            })?;

            connection_string.clear();
            write!(connection_string, "{}", url)?;

            Ok((database_info, password))
        }
        DatabaseInfo::Mssql {
            connection_string, ..
        } => {
            let mut url =
                Url::parse(connection_string).context("Failed to parse connection string")?;
            let password = url.password().map(ToOwned::to_owned);
            url.set_password(None).map_err(|_| {
                Error::Any(anyhow::anyhow!(
                    "Failed to remove password from connection string",
                ))
            })?;

            connection_string.clear();
            write!(connection_string, "{}", url)?;

            Ok((database_info, password))
        }
    }
}

pub fn store_sensitive_data(connection_id: &Uuid, password: &str) -> Result<(), Error> {
    store_password(connection_id, password)
}

fn store_password(connection_id: &Uuid, password: &str) -> Result<(), Error> {
    let entry = Entry::new(SERVICE_NAME, &connection_id.to_string())
        .map_err(|e| Error::Any(anyhow::anyhow!("Failed to create keyring entry: {}", e)))?;
    entry.set_password(password).map_err(|e| {
        Error::Any(anyhow::anyhow!(
            "Failed to store password in keyring: {}",
            e
        ))
    })?;
    log::info!(
        "Stored password in keyring for connection: {}",
        connection_id
    );
    Ok(())
}

/// Retrieve password for a connection using connection ID as key
pub fn get_password(connection_id: &Uuid) -> Result<Option<String>, Error> {
    let entry = Entry::new(SERVICE_NAME, &connection_id.to_string())
        .map_err(|e| Error::Any(anyhow::anyhow!("Failed to create keyring entry: {}", e)))?;

    match entry.get_password() {
        Ok(password) => {
            log::debug!(
                "Retrieved password from keyring for connection: {}",
                connection_id
            );
            Ok(Some(password))
        }
        Err(keyring::Error::NoEntry) => {
            log::debug!(
                "No password found in keyring for connection: {}",
                connection_id
            );
            Ok(None)
        }
        Err(e) => {
            log::warn!(
                "Failed to retrieve password from keyring for connection {}: {}",
                connection_id,
                e
            );
            Ok(None)
        }
    }
}

pub fn delete_password(connection_id: &Uuid) -> Result<(), Error> {
    let entry = Entry::new(SERVICE_NAME, &connection_id.to_string())
        .map_err(|e| Error::Any(anyhow::anyhow!("Failed to create keyring entry: {}", e)))?;

    match entry.delete_credential() {
        Ok(()) => {
            log::info!("Deleted password from keyring for connection {connection_id}");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            // Password wasn't there anyway
            Ok(())
        }
        Err(e) => Err(Error::Any(anyhow::anyhow!(
            "Failed to delete password from keyring: {e}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_with_pw() {
        let original = "postgres://pgpad:s3cr3t@localhost:5432/mydb";
        let dbi = DatabaseInfo::Postgres {
            connection_string: original.to_string(),
            ca_cert_path: None,
        };

        let (sanitized, pw) = extract_sensitive_data(dbi).expect("ok");
        assert_eq!(pw.as_deref(), Some("s3cr3t"));

        match sanitized {
            DatabaseInfo::Postgres {
                connection_string, ..
            } => {
                assert_eq!(connection_string, "postgres://pgpad@localhost:5432/mydb");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn postgres_with_percent_encoded_pw() {
        let original = "postgres://bob:p%404ss@db.example.com/app";
        let dbi = DatabaseInfo::Postgres {
            connection_string: original.to_string(),
            ca_cert_path: None,
        };

        let (sanitized, pw) = extract_sensitive_data(dbi).expect("ok");

        assert_eq!(pw.as_deref(), Some("p%404ss"));

        match sanitized {
            DatabaseInfo::Postgres {
                connection_string, ..
            } => {
                assert_eq!(connection_string, "postgres://bob@db.example.com/app");
            }
            _ => panic!("expected Postgres variant"),
        }

        let original = "postgres://u:pa%3Ass%40word@host/db";
        let dbi = DatabaseInfo::Postgres {
            connection_string: original.to_string(),
            ca_cert_path: None,
        };

        let (sanitized, pw) = extract_sensitive_data(dbi).expect("ok");

        assert_eq!(pw.as_deref(), Some("pa%3Ass%40word"));
        match sanitized {
            DatabaseInfo::Postgres {
                connection_string, ..
            } => {
                assert_eq!(connection_string, "postgres://u@host/db");
            }
            _ => panic!("expected Postgres variant"),
        }
    }

    #[test]
    fn postgres_with_empty_password() {
        let original = "postgres://john:@localhost/db";
        let dbi = DatabaseInfo::Postgres {
            connection_string: original.to_string(),
            ca_cert_path: None,
        };

        let (sanitized, pw) = extract_sensitive_data(dbi).expect("ok");

        // TODO(vini): maybe it is expected that this would be Some("")?
        assert_eq!(pw.as_deref(), None);

        match sanitized {
            DatabaseInfo::Postgres {
                connection_string, ..
            } => {
                assert_eq!(connection_string, "postgres://john@localhost/db");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn postgres_without_password() {
        let original = "postgres://dave@localhost/mydb";
        let dbi = DatabaseInfo::Postgres {
            connection_string: original.to_string(),
            ca_cert_path: None,
        };

        let (sanitized, pw) = extract_sensitive_data(dbi).expect("ok");

        assert!(pw.is_none());
        match sanitized {
            DatabaseInfo::Postgres {
                connection_string, ..
            } => {
                assert_eq!(connection_string, original);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn postgresql_scheme() {
        let original = "postgresql://erin:pw@localhost:5432/dbname?sslmode=prefer";
        let dbi = DatabaseInfo::Postgres {
            connection_string: original.to_string(),
            ca_cert_path: None,
        };

        let (sanitized, pw) = extract_sensitive_data(dbi).expect("ok");

        assert_eq!(pw.as_deref(), Some("pw"));
        match sanitized {
            DatabaseInfo::Postgres {
                connection_string, ..
            } => {
                assert_eq!(
                    connection_string,
                    "postgresql://erin@localhost:5432/dbname?sslmode=prefer"
                );
            }
            _ => panic!("expected Postgres variant"),
        }
    }

    #[test]
    fn sqlite_passphrase_extract() {
        let path = "/tmp/test.sqlite3".to_string();
        let pass = Some("secret".to_string());
        let dbi = DatabaseInfo::SQLite { db_path: path.clone(), passphrase: pass.clone() };

        let (sanitized, pw) = extract_sensitive_data(dbi).expect("ok");

        assert_eq!(pw.as_deref(), Some("secret"));
        match sanitized {
            DatabaseInfo::SQLite { db_path, passphrase } => {
                assert_eq!(db_path, path);
                assert!(passphrase.is_none());
            }
            _ => panic!("expected SQLite variant"),
        }
    }

    #[test]
    fn invalid_url_yields_error() {
        let dbi = DatabaseInfo::Postgres {
            connection_string: "not a url".to_string(),
            ca_cert_path: None,
        };
        let res = extract_sensitive_data(dbi);
        assert!(res.is_err(), "expected parse error for invalid URL");
    }
}
