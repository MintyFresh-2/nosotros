use std::fmt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum NostrError {
    InvalidPrivateKey(String),
    InvalidPublicKey(String),
    SigningFailed(String),
    EventCreationFailed(String),
    RelayConnectionFailed(String),
    RelayResponseTimeout,
    RelayRejectedEvent(String),
    InvalidEventId(String),
    SerializationFailed(String),
    CryptographicError(String),
    InvalidUrl(String),
    NetworkError(String),
}

impl fmt::Display for NostrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NostrError::InvalidPrivateKey(msg) => write!(f, "Invalid private key: {}", msg),
            NostrError::InvalidPublicKey(msg) => write!(f, "Invalid public key: {}", msg),
            NostrError::SigningFailed(msg) => write!(f, "Signing failed: {}", msg),
            NostrError::EventCreationFailed(msg) => write!(f, "Event creation failed: {}", msg),
            NostrError::RelayConnectionFailed(msg) => write!(f, "Relay connection failed: {}", msg),
            NostrError::RelayResponseTimeout => write!(f, "Relay response timeout"),
            NostrError::RelayRejectedEvent(msg) => write!(f, "Relay rejected event: {}", msg),
            NostrError::InvalidEventId(msg) => write!(f, "Invalid event ID: {}", msg),
            NostrError::SerializationFailed(msg) => write!(f, "Serialization failed: {}", msg),
            NostrError::CryptographicError(msg) => write!(f, "Cryptographic error: {}", msg),
            NostrError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            NostrError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for NostrError {}

impl From<anyhow::Error> for NostrError {
    fn from(err: anyhow::Error) -> Self {
        NostrError::CryptographicError(err.to_string())
    }
}

impl From<serde_json::Error> for NostrError {
    fn from(err: serde_json::Error) -> Self {
        NostrError::SerializationFailed(err.to_string())
    }
}

impl From<hex::FromHexError> for NostrError {
    fn from(err: hex::FromHexError) -> Self {
        NostrError::InvalidPrivateKey(err.to_string())
    }
}

impl From<secp256k1::Error> for NostrError {
    fn from(err: secp256k1::Error) -> Self {
        NostrError::CryptographicError(err.to_string())
    }
}

impl From<url::ParseError> for NostrError {
    fn from(err: url::ParseError) -> Self {
        NostrError::InvalidUrl(err.to_string())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for NostrError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        NostrError::NetworkError(err.to_string())
    }
}