use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    error: ErrorTag,
    in_reply_to: usize,
    code: ErrorCode,
    text: String,
}

impl Error {
    #[must_use]
    pub fn crash(msg_id: usize) -> Self {
        Self {
            error: ErrorTag::Error,
            in_reply_to: msg_id,
            code: ErrorCode::Crash,
            text: "internal server error".to_string(),
        }
    }
    pub fn new(code: ErrorCode, text: String, in_reply_to: usize) -> Self {
        Self {
            error: ErrorTag::Error,
            in_reply_to,
            code,
            text,
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn in_reply_to(&self) -> usize {
        self.in_reply_to
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorCode {
    #[serde(rename = "0")]
    Timeout,
    #[serde(rename = "10")]
    NotSupported,
    #[serde(rename = "11")]
    TemporarilyUnavailable,
    #[serde(rename = "12")]
    MalformedRequest,
    #[serde(rename = "13")]
    Crash,
    #[serde(rename = "14")]
    Abort,
    #[serde(rename = "20")]
    KeyDoesNotExist,
    #[serde(rename = "22")]
    PreconditionFailed,
    #[serde(rename = "30")]
    TxnConflict,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorTag {
    Error,
}
