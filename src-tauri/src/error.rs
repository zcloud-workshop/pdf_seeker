use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Config(String),

    #[error("{0}")]
    Pdf(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Toml(#[from] toml::de::Error),

    #[error("{0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("{0}")]
    Serde(#[from] serde_json::Error),

    #[error("S3 error: {0}")]
    S3(String),

    #[error("S3 presign error: {0}")]
    S3Presign(#[from] aws_sdk_s3::presigning::PresigningConfigError),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
