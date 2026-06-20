use crate::audio::buffer::AudioBuffer;
use crate::error::{Result, WaveSculptorError};

pub const DEFAULT_SILENCE_THRESHOLD: i16 = 256;
pub const DEFAULT_SILENCE_MIN_DURATION_MS: u32 = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SilenceSegment {
    pub start_frame: usize,
    pub end_frame: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioAnalysis {
    pub peak: f32,
    pub rms: f32,
    pub clipping_samples: usize,
    pub silent_segments: Vec<SilenceSegment>,
}

pub fn analyze_buffer(buffer: &AudioBuffer) -> Result<AudioAnalysis> {
    // 分析结果集中生成，GUI 和 CLI 使用同一套统计口径。
    let silent_segments = detect_silence_segments(
        buffer,
        DEFAULT_SILENCE_THRESHOLD,
        default_min_silence_frames(buffer.sample_rate),
    )?;

    Ok(AudioAnalysis {
        peak: peak_amplitude(buffer),
        rms: rms_amplitude(buffer),
        clipping_samples: clipping_sample_count(buffer),
        silent_segments,
    })
}

pub fn peak_amplitude(buffer: &AudioBuffer) -> f32 {
    buffer
        .samples
        .iter()
        .map(|sample| normalized_amplitude(*sample))
        .fold(0.0_f32, f32::max)
}

pub fn rms_amplitude(buffer: &AudioBuffer) -> f32 {
    if buffer.samples.is_empty() {
        return 0.0;
    }

    let squared_sum = buffer
        .samples
        .iter()
        .map(|sample| {
            let normalized = f64::from(*sample) / f64::from(i16::MAX);
            normalized * normalized
        })
        .sum::<f64>();

    (squared_sum / buffer.samples.len() as f64).sqrt() as f32
}

pub fn clipping_sample_count(buffer: &AudioBuffer) -> usize {
    buffer
        .samples
        .iter()
        .filter(|sample| **sample == i16::MAX || **sample == i16::MIN)
        .count()
}

pub fn default_min_silence_frames(sample_rate: u32) -> usize {
    ((sample_rate as u64 * u64::from(DEFAULT_SILENCE_MIN_DURATION_MS)) / 1000).max(1) as usize
}

pub fn detect_silence_segments(
    buffer: &AudioBuffer,
    threshold: i16,
    min_frames: usize,
) -> Result<Vec<SilenceSegment>> {
    if threshold < 0 {
        return Err(WaveSculptorError::InvalidParameter(
            "静音阈值不能为负数".to_string(),
        ));
    }
    if min_frames == 0 {
        return Err(WaveSculptorError::InvalidParameter(
            "静音最短帧数必须大于 0".to_string(),
        ));
    }

    let mut segments = Vec::new();
    let mut current_start: Option<usize> = None;

    for frame_index in 0..buffer.frame_count() {
        // 多声道帧必须所有声道都低于阈值，才视为静音帧。
        let is_silent = buffer
            .frame(frame_index)
            .map(|frame| {
                frame
                    .iter()
                    .all(|sample| i32::from(*sample).abs() <= i32::from(threshold))
            })
            .unwrap_or(false);

        match (current_start, is_silent) {
            (None, true) => current_start = Some(frame_index),
            (Some(start), false) => {
                if frame_index - start >= min_frames {
                    segments.push(SilenceSegment {
                        start_frame: start,
                        end_frame: frame_index,
                    });
                }
                current_start = None;
            }
            _ => {}
        }
    }

    if let Some(start) = current_start {
        let end_frame = buffer.frame_count();
        if end_frame - start >= min_frames {
            segments.push(SilenceSegment {
                start_frame: start,
                end_frame,
            });
        }
    }

    Ok(segments)
}

pub fn trim_silence_bounds(
    buffer: &AudioBuffer,
    threshold: i16,
    min_frames: usize,
) -> Result<Option<(usize, usize)>> {
    let segments = detect_silence_segments(buffer, threshold, min_frames)?;
    let mut start_frame = 0;
    let mut end_frame = buffer.frame_count();

    // 只使用贴住开头或结尾的静音段来确定裁剪边界。
    if let Some(first) = segments.first() {
        if first.start_frame == 0 {
            start_frame = first.end_frame;
        }
    }
    if let Some(last) = segments.last() {
        if last.end_frame == buffer.frame_count() {
            end_frame = last.start_frame;
        }
    }

    if start_frame >= end_frame {
        return Ok(None);
    }

    Ok(Some((start_frame, end_frame)))
}

fn normalized_amplitude(sample: i16) -> f32 {
    (f32::from(sample).abs() / f32::from(i16::MAX)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{detect_silence_segments, rms_amplitude};
    use crate::audio::buffer::AudioBuffer;

    #[test]
    fn computes_rms() {
        let buffer = AudioBuffer::new(vec![i16::MAX, 0, i16::MAX, 0], 48_000, 1, 16);
        let rms = rms_amplitude(&buffer);
        let expected = (0.5_f32).sqrt();
        assert!((rms - expected).abs() < 0.001);
    }

    #[test]
    fn detects_silence_segments() {
        let buffer = AudioBuffer::new(vec![0, 0, 0, 1000, 1000, 0, 0, 0, 0], 48_000, 1, 16);
        let result = detect_silence_segments(&buffer, 10, 2);

        match result {
            Ok(segments) => {
                assert_eq!(segments.len(), 2);
                assert_eq!(segments[0].start_frame, 0);
                assert_eq!(segments[0].end_frame, 3);
                assert_eq!(segments[1].start_frame, 5);
                assert_eq!(segments[1].end_frame, 9);
            }
            Err(error) => panic!("silence detection failed: {error}"),
        }
    }
}
