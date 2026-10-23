//! Reads a WAV file into mono samples, for `JUGLAR_FILE`: 16-bit, 24-bit
//! or 32-bit integer PCM, or 32-bit float, any number of channels.

use std::fmt;

pub struct Wav {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

#[derive(Debug)]
pub struct WavError(String);

impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn read(path: &str) -> Result<Wav, WavError> {
    let data = std::fs::read(path).map_err(|e| WavError(format!("{path}: {e}")))?;
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(WavError(format!("{path}: not a WAV file")));
    }
    let u16_at = |i: usize| u16::from_le_bytes([data[i], data[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    let mut format = None;
    let mut i = 12;
    while i + 8 <= data.len() {
        let size = u32_at(i + 4) as usize;
        let body = i + 8;
        let end = (body + size).min(data.len());
        match &data[i..i + 4] {
            b"fmt " if size >= 16 => {
                let mut tag = u16_at(body);
                // WAVE_FORMAT_EXTENSIBLE keeps the real format in its GUID.
                if tag == 0xFFFE && size >= 26 {
                    tag = u16_at(body + 24);
                }
                format = Some((tag, u16_at(body + 2) as usize, u32_at(body + 4), u16_at(body + 14)));
            }
            b"data" => {
                let (tag, channels, sample_rate, bits) = format.ok_or_else(|| WavError(format!("{path}: no format before the data")))?;
                let bytes = &data[body..end];
                let sample = |b: &[u8]| -> f32 {
                    match (tag, bits) {
                        (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                        (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
                        (1, 24) => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
                        (1, 32) => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0,
                        _ => 0.0,
                    }
                };
                if !matches!((tag, bits), (3, 32) | (1, 16) | (1, 24) | (1, 32)) || channels == 0 {
                    return Err(WavError(format!("{path}: unsupported format {tag}, {bits} bits")));
                }
                let width = usize::from(bits / 8);
                let samples = bytes
                    .chunks_exact(width * channels)
                    .map(|frame| frame.chunks_exact(width).map(sample).sum::<f32>() / channels as f32)
                    .collect();
                return Ok(Wav { samples, sample_rate });
            }
            _ => {}
        }
        i = body + size + (size & 1);
    }
    Err(WavError(format!("{path}: no audio data")))
}
