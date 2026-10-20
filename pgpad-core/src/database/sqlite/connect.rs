use anyhow::Context;
use rusqlite::Connection;
use secrecy::SecretString;

use crate::{error::Error, utils::sqlite_cipher};

/// Opens a SQLite database for querying.
///
/// When a passphrase is supplied the file is opened as a SQLCipher database and
/// the key is verified up front, so a wrong passphrase surfaces here instead of
/// as an opaque "file is not a database" on the first query.
pub fn open(db_path: &str, passphrase: Option<&str>) -> Result<Connection, Error> {
    // Checked before opening: `Connection::open` creates the file and SQLCipher
    // immediately writes a header, so afterwards an empty database no longer
    // looks empty on disk.
    let had_content = std::fs::metadata(db_path)
        .map(|meta| meta.len() > 0)
        .unwrap_or(false);

    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open SQLite database at {}", db_path))?;

    if let Some(passphrase) = passphrase {
        let key = SecretString::new(passphrase.to_string());
        sqlite_cipher::apply_cipher_settings_with(&conn, &key, &Default::default())
            .map_err(|e| Error::Any(anyhow::anyhow!(e)))?;

        // An empty database has nothing to decrypt yet, so SQLCipher has
        // nothing to integrity-check. Any existing content must decrypt with
        // this key, which is what rejects a wrong passphrase here instead of
        // as an opaque "file is not a database" on the first query.
        if had_content {
            sqlite_cipher::verify_cipher_ok(&conn).map_err(|e| Error::Any(anyhow::anyhow!(e)))?;
        }
    }

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database written with SQLCipher cannot be read as plain SQLite, so a
    /// wrong or missing passphrase must fail at open time.
    #[test]
    fn wrong_passphrase_is_rejected() -> anyhow::Result<()> {
        if !sqlite_cipher::has_sqlcipher(&Connection::open_in_memory()?) {
            eprintln!("skipped: SQLCipher is not compiled in");
            return Ok(());
        }

        let path = std::env::temp_dir().join("pgpad-sqlcipher-wrong-passphrase.db");
        let _ = std::fs::remove_file(&path);
        let path = path.to_string_lossy().to_string();

        {
            let conn = open(&path, Some("correct horse"))?;
            conn.execute_batch("CREATE TABLE t (a INTEGER);")?;
        }

        // A wrong passphrase is rejected up front.
        assert!(open(&path, Some("wrong horse")).is_err());

        // The right one reads the table back.
        let conn = open(&path, Some("correct horse"))?;
        let rows: i64 = conn.query_row("SELECT count(*) FROM t", [], |row| row.get(0))?;
        assert_eq!(rows, 0);
        drop(conn);

        // Opening without a passphrase cannot even read the header, so it
        // opens but fails as soon as it is queried.
        let conn = open(&path, None)?;
        assert!(conn
            .query_row("SELECT count(*) FROM t", [], |row| row.get::<_, i64>(0))
            .is_err());
        drop(conn);

        let _ = std::fs::remove_file(&path);
        Ok(())
    }

    /// Without a passphrase the driver must keep behaving like plain SQLite.
    #[test]
    fn plain_database_opens_without_passphrase() -> anyhow::Result<()> {
        let path = std::env::temp_dir().join("pgpad-sqlcipher-plain.db");
        let _ = std::fs::remove_file(&path);
        let path = path.to_string_lossy().to_string();

        let conn = open(&path, None)?;
        conn.execute_batch("CREATE TABLE t (a INTEGER); INSERT INTO t VALUES (42);")?;
        drop(conn);

        let value: i64 = {
            // Scoped so the handle is closed before the file is removed:
            // Windows refuses to delete a file that is still open.
            let conn = open(&path, None)?;
            conn.query_row("SELECT a FROM t", [], |row| row.get(0))?
        };
        assert_eq!(value, 42);

        let _ = std::fs::remove_file(&path);
        Ok(())
    }
}
