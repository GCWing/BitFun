use super::types::{
    SpeechModelArtifact, SpeechModelArtifactKind, SpeechModelManifest, SpeechRecognizerKind,
    LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_ID, LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID,
};
use super::{BitFunError, BitFunResult};

pub(super) fn builtin_speech_model_manifests() -> Vec<SpeechModelManifest> {
    vec![
        sensevoice_small_int8_manifest(),
        qwen3_asr_0_6b_int8_manifest(),
    ]
}

pub(super) fn get_builtin_speech_model_manifest(
    model_id: &str,
) -> BitFunResult<SpeechModelManifest> {
    builtin_speech_model_manifests()
        .into_iter()
        .find(|manifest| manifest.id == model_id)
        .ok_or_else(|| BitFunError::NotFound(format!("Unknown speech model: {model_id}")))
}

pub(super) fn sensevoice_small_int8_manifest() -> SpeechModelManifest {
    SpeechModelManifest {
        id: LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID.to_string(),
        display_name: "SenseVoice Small INT8".to_string(),
        provider: "k2-fsa/sherpa-onnx".to_string(),
        version: "2025-09-09".to_string(),
        variant: "int8".to_string(),
        description: "Local multilingual speech recognition for Mandarin, Cantonese, English, Japanese, and Korean.".to_string(),
        source_page_url: "https://k2-fsa.github.io/sherpa/onnx/sense-voice/index.html".to_string(),
        license_name: Some("Apache-2.0".to_string()),
        languages: vec![
            "auto".to_string(),
            "zh".to_string(),
            "yue".to_string(),
            "en".to_string(),
            "ja".to_string(),
            "ko".to_string(),
        ],
        required_files: vec![
            "model.int8.onnx".to_string(),
            "tokens.txt".to_string(),
            "README.md".to_string(),
        ],
        recognizer: SpeechRecognizerKind::SenseVoiceInt8,
        artifacts: vec![SpeechModelArtifact {
            id: "model-archive".to_string(),
            file_name: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2".to_string(),
            kind: SpeechModelArtifactKind::TarBz2,
            source_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2".to_string(),
            fallback_source_urls: vec![
                "https://gh-proxy.com/https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2".to_string(),
            ],
            size_bytes: 165_783_878,
            sha256: "7305f7905bfcf77fa0b39388a313f3da35c68d971661a65475b56fb2162c8e63".to_string(),
            install_path: None,
        }],
    }
}

pub(super) fn qwen3_asr_0_6b_int8_manifest() -> SpeechModelManifest {
    SpeechModelManifest {
        id: LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_ID.to_string(),
        display_name: "Qwen3-ASR 0.6B INT8".to_string(),
        provider: "k2-fsa/sherpa-onnx".to_string(),
        version: "2026-03-25".to_string(),
        variant: "int8".to_string(),
        description: "Higher-quality local multilingual speech recognition based on Qwen3-ASR.".to_string(),
        source_page_url: "https://k2-fsa.github.io/sherpa/onnx/qwen3-asr/pretrained.html".to_string(),
        license_name: Some("Apache-2.0".to_string()),
        languages: vec![
            "auto".to_string(),
            "zh".to_string(),
            "yue".to_string(),
            "en".to_string(),
            "ar".to_string(),
            "de".to_string(),
            "fr".to_string(),
            "es".to_string(),
            "pt".to_string(),
            "id".to_string(),
            "it".to_string(),
            "ko".to_string(),
            "ru".to_string(),
            "th".to_string(),
            "vi".to_string(),
            "ja".to_string(),
            "tr".to_string(),
            "hi".to_string(),
            "ms".to_string(),
            "nl".to_string(),
            "sv".to_string(),
            "da".to_string(),
            "fi".to_string(),
            "pl".to_string(),
            "cs".to_string(),
            "fil".to_string(),
            "fa".to_string(),
            "el".to_string(),
            "hu".to_string(),
            "mk".to_string(),
            "ro".to_string(),
        ],
        required_files: vec![
            "conv_frontend.onnx".to_string(),
            "encoder.int8.onnx".to_string(),
            "decoder.int8.onnx".to_string(),
            "tokenizer/merges.txt".to_string(),
            "tokenizer/tokenizer_config.json".to_string(),
            "tokenizer/vocab.json".to_string(),
            "README.md".to_string(),
        ],
        recognizer: SpeechRecognizerKind::Qwen3AsrInt8,
        artifacts: vec![SpeechModelArtifact {
            id: "model-archive".to_string(),
            file_name: "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2".to_string(),
            kind: SpeechModelArtifactKind::TarBz2,
            source_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2".to_string(),
            fallback_source_urls: vec![
                "https://gh-proxy.com/https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2".to_string(),
            ],
            size_bytes: 878_702_423,
            sha256: "393f8a14e2f5fb96746aaab342997a40641001fbd5bf9592a080a8329178ee96".to_string(),
            install_path: None,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensevoice_manifest_has_distinct_fallback_sources() {
        let manifest = sensevoice_small_int8_manifest();

        let artifact = manifest.artifacts.first().unwrap();
        assert!(!artifact.fallback_source_urls.is_empty());
        assert!(artifact
            .fallback_source_urls
            .iter()
            .all(|source| source != &artifact.source_url));
        assert_eq!(artifact.sha256.len(), 64);
    }

    #[test]
    fn qwen3_manifest_declares_required_runtime_files() {
        let manifest = qwen3_asr_0_6b_int8_manifest();

        assert_eq!(manifest.recognizer, SpeechRecognizerKind::Qwen3AsrInt8);
        assert!(manifest
            .required_files
            .contains(&"conv_frontend.onnx".to_string()));
        assert!(manifest
            .required_files
            .contains(&"encoder.int8.onnx".to_string()));
        assert!(manifest
            .required_files
            .contains(&"decoder.int8.onnx".to_string()));
        assert!(manifest
            .required_files
            .contains(&"tokenizer/vocab.json".to_string()));
        assert_eq!(
            manifest
                .artifacts
                .iter()
                .map(|artifact| artifact.size_bytes)
                .sum::<u64>(),
            878_702_423
        );
    }
}
