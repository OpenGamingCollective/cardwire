use thiserror::Error;

#[derive(Debug, Error)]
pub enum CardwireGuiError {
    #[error("iced error: {0}")]
    Iced(#[from] iced::Error),

    #[error("zbus error: {0}")]
    Zbus(#[from] zbus::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CardwireGuiError>;
