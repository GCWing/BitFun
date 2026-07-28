use serde::{Deserialize, Serialize};

/// TTS voiceover config: voice selection + rate + SSML parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    /// Voice name, e.g. "zh-CN-YunjianNeural"
    pub voice: String,
    /// Speech rate, e.g. "+0%" (normal), "-20%" (slow), "+20%" (fast)
    pub rate: String,
    /// Pitch shift (Hz)
    pub pitch: f64,
    /// Output format, e.g. "audio-24khz-96kbitrate-mono-mp3"
    pub output_format: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            voice: "zh-CN-YunjianNeural".into(),
            rate: "+0%".into(),
            pitch: 0.0,
            output_format: "audio-24khz-96kbitrate-mono-mp3".into(),
        }
    }
}

/// TTS script: timestamp → text sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsScript {
    pub segments: Vec<TtsSegment>,
}

/// TTS text segment: narration text within a time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsSegment {
    /// Start time (seconds)
    pub start_sec: f64,
    /// End time (seconds)
    pub end_sec: f64,
    /// Narration text
    pub text: String,
    /// Voice used for this segment (default inherits TtsConfig.voice)
    pub voice: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_config_default() {
        let config = TtsConfig::default();
        assert_eq!(config.voice, "zh-CN-YunjianNeural");
        assert_eq!(config.rate, "+0%");
    }

    #[test]
    fn test_tts_script_roundtrip() {
        let script = TtsScript {
            segments: vec![
                TtsSegment {
                    start_sec: 0.0,
                    end_sec: 5.0,
                    text: "Today's silver main contract technical analysis".into(),
                    voice: "zh-CN-YunjianNeural".into(),
                },
                TtsSegment {
                    start_sec: 5.5,
                    end_sec: 12.0,
                    text: "Current bullish trend strength 73%, channel expanding".into(),
                    voice: "zh-CN-YunjianNeural".into(),
                },
            ],
        };
        let json = serde_json::to_string(&script).unwrap();
        let roundtrip: TtsScript = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.segments.len(), 2);
        assert_eq!(roundtrip.segments[0].start_sec, 0.0);
        assert_eq!(roundtrip.segments[1].end_sec, 12.0);
    }
}
