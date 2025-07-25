pub type Result<T = ()> = std::result::Result<T, Error>;

// TODO: this really can't be the best way to use anyhow
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Any(#[from] anyhow::Error),
}

#[derive(serde::Serialize)]
#[serde(tag = "name", content = "message")]
#[serde(rename_all = "camelCase")]
enum ErrorName {
    Error(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let message = self.to_string();
        let name = match self {
            Self::Any(_) => ErrorName::Error(message),
        };
        name.serialize(serializer)
    }
}
