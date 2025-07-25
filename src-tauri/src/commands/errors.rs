#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("{0}")]
    Database(String),
    #[error(transparent)]
    Postgres(#[from] tokio_postgres::Error),
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::Database(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Error::Database(err.to_string())
    }
}

#[derive(serde::Serialize)]
#[serde(tag = "name", content = "message")]
#[serde(rename_all = "camelCase")]
enum ErrorName {
    Io(String),
    FromUtf8Error(String),
    Database(String),
    Postgres(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let message = self.to_string();
        let name = match self {
            Self::Io(_) => ErrorName::Io(message),
            Self::Utf8(_) => ErrorName::FromUtf8Error(message),
            Self::Database(_) => ErrorName::Database(message),
            Self::Postgres(_) => ErrorName::Postgres(message),
        };
        name.serialize(serializer)
    }
}
