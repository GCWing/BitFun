use crate::canvas::types::{
    CanvasArtifactRef, CanvasId, CanvasSessionId, CANVAS_ARTIFACT_REF_SCHEME,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasArtifactRefParseError {
    InvalidScheme,
    InvalidShape,
    EmptySessionId,
    EmptyCanvasId,
    UnsafeSessionId,
    UnsafeCanvasId,
    InvalidPercentEncoding,
}

impl std::fmt::Display for CanvasArtifactRefParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CanvasArtifactRefParseError {}

pub fn parse_canvas_artifact_ref(
    uri: &str,
) -> Result<CanvasArtifactRef, CanvasArtifactRefParseError> {
    let prefix = format!("{CANVAS_ARTIFACT_REF_SCHEME}://");
    let rest = uri
        .strip_prefix(&prefix)
        .ok_or(CanvasArtifactRefParseError::InvalidScheme)?;
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() != 4 || parts[0] != "session" || parts[2] != "canvas" {
        return Err(CanvasArtifactRefParseError::InvalidShape);
    }

    let session_id = percent_decode_segment(parts[1])?;
    if session_id.is_empty() {
        return Err(CanvasArtifactRefParseError::EmptySessionId);
    }
    if !is_safe_canvas_ref_segment(&session_id) {
        return Err(CanvasArtifactRefParseError::UnsafeSessionId);
    }

    let canvas_id = percent_decode_segment(parts[3])?;
    if canvas_id.is_empty() {
        return Err(CanvasArtifactRefParseError::EmptyCanvasId);
    }
    if !is_safe_canvas_ref_segment(&canvas_id) {
        return Err(CanvasArtifactRefParseError::UnsafeCanvasId);
    }

    Ok(CanvasArtifactRef::new(
        CanvasSessionId::new(session_id),
        CanvasId::new(canvas_id),
    ))
}

pub fn is_safe_canvas_ref_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value
            .chars()
            .any(|ch| ch == '/' || ch == '\\' || ch.is_control())
}

fn percent_decode_segment(value: &str) -> Result<String, CanvasArtifactRefParseError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            output.push(bytes[index]);
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len() {
            return Err(CanvasArtifactRefParseError::InvalidPercentEncoding);
        }
        let high = from_hex(bytes[index + 1])?;
        let low = from_hex(bytes[index + 2])?;
        output.push((high << 4) | low);
        index += 3;
    }
    String::from_utf8(output).map_err(|_| CanvasArtifactRefParseError::InvalidPercentEncoding)
}

fn from_hex(byte: u8) -> Result<u8, CanvasArtifactRefParseError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(CanvasArtifactRefParseError::InvalidPercentEncoding),
    }
}
