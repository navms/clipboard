use serde::{Serialize, Serializer};

/// Every fallible boundary in the app funnels through this error so that
/// Tauri commands can hand a readable string back to the webview.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("clipboard error: {0}")]
    Clipboard(String),

    #[error("clip not found: {0}")]
    NotFound(i64),

    #[error("{0}")]
    Other(String),

    #[error("serialisation error: {0}")]
    Json(#[from] serde_json::Error),
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
