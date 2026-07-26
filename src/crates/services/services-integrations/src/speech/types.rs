use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID: &str = "sensevoice-small-int8";
pub const LOCAL_SENSEVOICE_SMALL_INT8_MODEL_REF: &str = "local:sensevoice-small-int8";
pub const LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_ID: &str = "qwen3-asr-0.6b-int8";
pub const LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_REF: &str = "local:qwen3-asr-0.6b-int8";
pub(super) use bitfun_core_types::speech::{
    SpeechModelInstallState, SpeechModelProgress, SpeechModelStatus, SpeechTranscriptionResult,
};

pub const DEFAULT_SPEECH_SAMPLE_RATE: u32 = 16_000;
pub const DEFAULT_MAX_RECORDING_SECONDS: u32 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SpeechModelManifest {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub version: String,
    pub variant: String,
    pub description: String,
    pub source_page_url: String,
    pub license_name: Option<String>,
    pub languages: Vec<String>,
    pub required_files: Vec<String>,
    pub recognizer: SpeechRecognizerKind,
    pub artifacts: Vec<SpeechModelArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SpeechRecognizerKind {
    SenseVoiceInt8,
    Qwen3AsrInt8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SpeechModelArtifact {
    pub id: String,
    pub file_name: String,
    pub kind: SpeechModelArtifactKind,
    pub source_url: String,
    #[serde(default)]
    pub fallback_source_urls: Vec<String>,
    pub size_bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub install_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SpeechModelArtifactKind {
    TarBz2,
    File,
}

impl SpeechModelManifest {
    pub(super) fn expected_bytes(&self) -> u64 {
        self.artifacts
            .iter()
            .map(|artifact| artifact.size_bytes)
            .sum()
    }
}

#[derive(Debug, Clone)]
pub(super) struct SpeechTranscribeRequest {
    pub model_dir: PathBuf,
    pub recognizer: SpeechRecognizerKind,
    pub pcm16_le: Vec<u8>,
    pub sample_rate: u32,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstalledSpeechModelRecord {
    pub id: String,
    pub version: String,
    pub installed_at_ms: i64,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub archive_sha256: String,
    #[serde(default)]
    pub artifacts: Vec<InstalledSpeechModelArtifactRecord>,
    #[serde(default)]
    pub files: Vec<InstalledSpeechModelFileRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstalledSpeechModelArtifactRecord {
    pub id: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InstalledSpeechModelFileRecord {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}
