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
    pub fn crash(in_reply_to: usize) -> Self {
        Self::crash_with_message("internal error", in_reply_to)
    }

    #[must_use]
    pub fn crash_with_message(text: impl Into<String>, in_reply_to: usize) -> Self {
        Self::new(ErrorCode::Crash, text, in_reply_to)
    }

    pub fn new(code: impl Into<ErrorCode>, text: impl Into<String>, in_reply_to: usize) -> Self {
        Self {
            error: ErrorTag::Error,
            in_reply_to,
            code: code.into(),
            text: text.into(),
        }
    }

    #[must_use]
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
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

impl From<usize> for ErrorCode {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Timeout,
            10 => Self::NotSupported,
            11 => Self::TemporarilyUnavailable,
            12 => Self::MalformedRequest,
            #[allow(clippy::match_same_arms)]
            13 => Self::Crash,
            14 => Self::Abort,
            20 => Self::KeyDoesNotExist,
            22 => Self::PreconditionFailed,
            30 => Self::TxnConflict,
            _ => Self::Crash,
        }
    }
}
