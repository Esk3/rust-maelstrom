use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    error: ErrorTag,
    in_reply_to: usize,
    code: ErrorCode,
    text: String,
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
