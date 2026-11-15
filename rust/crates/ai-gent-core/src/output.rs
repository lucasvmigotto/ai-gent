//! The `--json` envelope: exactly one object per run (`contracts/cli.md`).
use serde::Serialize;

/// Error class carried by a failed envelope.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorCode {
    /// Bad arguments.
    Usage,
    /// The operation failed.
    Runtime,
    /// Refused by policy or verification.
    Refused,
    /// A bug (panic).
    Internal,
}

/// Error body of a failed envelope.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    /// Error class.
    pub code: ErrorCode,
    /// What happened.
    pub message: String,
    /// Optional next step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Success envelope: `{"schema":1,"ok":true,"data":...}`.
#[derive(Debug, Serialize)]
pub struct Ok<T: Serialize> {
    /// Shape version of `data`.
    pub schema: u32,
    /// Always true.
    pub ok: bool,
    /// The payload.
    pub data: T,
}

/// Failure envelope: `{"schema":1,"ok":false,"error":...}`.
#[derive(Debug, Serialize)]
pub struct Failure {
    /// Shape version.
    pub schema: u32,
    /// Always false.
    pub ok: bool,
    /// The error.
    pub error: ErrorBody,
}

/// Render a success envelope as one line of JSON (no trailing newline).
///
/// # Panics
/// Never for the plain data types of this crate; serialization of `T` failing is a bug.
#[must_use]
pub fn ok_json<T: Serialize>(schema: u32, data: T) -> String {
    serde_json::to_string(&Ok {
        schema,
        ok: true,
        data,
    })
    .expect("envelope serializes")
}

/// Render a failure envelope as one line of JSON (no trailing newline).
///
/// # Panics
/// Never; the envelope holds only strings.
#[must_use]
pub fn error_json(code: ErrorCode, message: &str, hint: Option<&str>) -> String {
    let f = Failure {
        schema: 1,
        ok: false,
        error: ErrorBody {
            code,
            message: message.to_string(),
            hint: hint.map(str::to_string),
        },
    };
    serde_json::to_string(&f).expect("envelope serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_envelope_shape() {
        assert_eq!(
            ok_json(1, serde_json::json!({"a": 1})),
            r#"{"schema":1,"ok":true,"data":{"a":1}}"#
        );
    }

    #[test]
    fn error_envelope_shape() {
        assert_eq!(
            error_json(ErrorCode::Refused, "bad sum", Some("retry")),
            r#"{"schema":1,"ok":false,"error":{"code":"refused","message":"bad sum","hint":"retry"}}"#
        );
        assert_eq!(
            error_json(ErrorCode::Usage, "x", None),
            r#"{"schema":1,"ok":false,"error":{"code":"usage","message":"x"}}"#
        );
    }
}
