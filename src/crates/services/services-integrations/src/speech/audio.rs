use super::{BitFunError, BitFunResult};

pub(super) fn pcm16_le_to_f32_samples(bytes: &[u8]) -> BitFunResult<Vec<f32>> {
    if bytes.len() % 2 != 0 {
        return Err(BitFunError::validation(
            "PCM16 audio payload must have an even number of bytes",
        ));
    }

    let mut samples = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(sample as f32 / i16::MAX as f32);
    }
    Ok(samples)
}

pub(super) fn pcm16_duration_seconds(byte_len: u64, sample_rate: u32) -> f64 {
    if sample_rate == 0 {
        return 0.0;
    }
    byte_len as f64 / 2.0 / sample_rate as f64
}
