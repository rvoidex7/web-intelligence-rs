use thiserror::Error;

#[derive(Error, Debug)]
pub enum WebIntelError {
    #[error("Browser executable not found. Please specify a path or ensure Chrome/Edge is installed.")]
    BrowserNotFound,

    #[error("Failed to create profile directory: {0}")]
    ProfileCreationFailure(std::io::Error),

    #[error("Failed to launch browser process: {0}")]
    LaunchFailure(std::io::Error),

    #[error("Failed to capture WebSocket URL from browser output.")]
    WebSocketUrlNotFound,

    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to read browser output.")]
    OutputReadFailure,
}
