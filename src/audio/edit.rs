use crate::audio::analyze::{trim_silence_bounds, DEFAULT_SILENCE_THRESHOLD};
use crate::audio::buffer::AudioBuffer;
use crate::audio::selection::Selection;
use crate::error::{Result, WaveSculptorError};

pub const DEFAULT_NORMALIZE_TARGET: f32 = 0.9;

pub fn mute_selection(buffer: &mut AudioBuffer, selection: Selection) -> Result<()> {
    validate_selection(buffer, selection)?;
    let range = buffer.frame_to_sample_range(selection.start_frame, selection.end_frame);
    for sample in &mut buffer.samples[range] {
        *sample = 0;
    }
    Ok(())
}

pub fn amplify_selection(buffer: &mut AudioBuffer, selection: Selection, gain: f32) -> Result<()> {
    validate_gain(gain)?;
    validate_selection(buffer, selection)?;

    let range = buffer.frame_to_sample_range(selection.start_frame, selection.end_frame);
    for sample in &mut buffer.samples[range] {
        *sample = scale_sample(*sample, gain);
    }

    Ok(())
}

pub fn normalize_buffer(buffer: &mut AudioBuffer, target_peak: f32) -> Result<()> {
    // 归一化按绝对峰值计算统一增益，保持原始动态比例。
    if !target_peak.is_finite() || target_peak <= 0.0 || target_peak > 1.0 {
        return Err(WaveSculptorError::InvalidParameter(
            "归一化目标必须位于 0 到 1 之间".to_string(),
        ));
    }

    let max_amplitude = buffer
        .samples
        .iter()
        .map(|sample| i32::from(*sample).abs())
        .max()
        .unwrap_or(0);

    if max_amplitude == 0 {
        return Ok(());
    }

    let gain = target_peak * f32::from(i16::MAX) / max_amplitude as f32;
    for sample in &mut buffer.samples {
        *sample = scale_sample(*sample, gain);
    }

    Ok(())
}

pub fn fade_in_selection(buffer: &mut AudioBuffer, selection: Selection) -> Result<()> {
    validate_selection(buffer, selection)?;
    let frame_count = selection.duration_frames();
    if frame_count == 0 {
        return Ok(());
    }

    for frame_offset in 0..frame_count {
        let factor = if frame_count == 1 {
            1.0
        } else {
            frame_offset as f32 / (frame_count - 1) as f32
        };
        apply_frame_gain(buffer, selection.start_frame + frame_offset, factor)?;
    }

    Ok(())
}

pub fn fade_out_selection(buffer: &mut AudioBuffer, selection: Selection) -> Result<()> {
    validate_selection(buffer, selection)?;
    let frame_count = selection.duration_frames();
    if frame_count == 0 {
        return Ok(());
    }

    for frame_offset in 0..frame_count {
        let factor = if frame_count == 1 {
            1.0
        } else {
            1.0 - frame_offset as f32 / (frame_count - 1) as f32
        };
        apply_frame_gain(buffer, selection.start_frame + frame_offset, factor)?;
    }

    Ok(())
}

pub fn reverse_selection(buffer: &mut AudioBuffer, selection: Selection) -> Result<()> {
    validate_selection(buffer, selection)?;
    let channels = buffer.channels_usize();
    let start = selection.start_frame;
    let end = selection.end_frame;
    let frame_len = end - start;

    // 以帧为单位交换，保证多声道音频的同一时间点一起反转。
    for offset in 0..frame_len / 2 {
        let left_frame = start + offset;
        let right_frame = end - 1 - offset;
        for channel_index in 0..channels {
            let left_index = left_frame * channels + channel_index;
            let right_index = right_frame * channels + channel_index;
            buffer.samples.swap(left_index, right_index);
        }
    }

    Ok(())
}

pub fn cut_selection(buffer: &AudioBuffer, selection: Selection) -> Result<AudioBuffer> {
    validate_selection(buffer, selection)?;
    Ok(buffer.slice_frames(selection.start_frame, selection.end_frame))
}

pub fn cut_selection_in_place(buffer: &mut AudioBuffer, selection: Selection) -> Result<()> {
    let cut = cut_selection(buffer, selection)?;
    *buffer = cut;
    Ok(())
}

pub fn trim_silence(buffer: &mut AudioBuffer) -> Result<()> {
    let min_frames = (buffer.sample_rate / 100).max(1) as usize;
    // 只裁掉首尾静音，中间的停顿保留为原音频内容。
    match trim_silence_bounds(buffer, DEFAULT_SILENCE_THRESHOLD, min_frames)? {
        Some((start_frame, end_frame)) => {
            *buffer = buffer.slice_frames(start_frame, end_frame);
        }
        None => buffer.samples.clear(),
    }

    Ok(())
}

fn validate_gain(gain: f32) -> Result<()> {
    if !gain.is_finite() || gain < 0.0 {
        return Err(WaveSculptorError::InvalidParameter(
            "音量倍数必须是有限且非负的数值".to_string(),
        ));
    }

    Ok(())
}

fn validate_selection(buffer: &AudioBuffer, selection: Selection) -> Result<()> {
    if selection.end_frame > buffer.frame_count() {
        return Err(WaveSculptorError::InvalidSelection);
    }

    Ok(())
}

fn apply_frame_gain(buffer: &mut AudioBuffer, frame_index: usize, gain: f32) -> Result<()> {
    let Some(frame) = buffer.frame_mut(frame_index) else {
        return Err(WaveSculptorError::InvalidSelection);
    };

    for sample in frame {
        *sample = scale_sample(*sample, gain);
    }

    Ok(())
}

fn scale_sample(sample: i16, gain: f32) -> i16 {
    let scaled = f32::from(sample) * gain;
    // 所有增益类操作都在 i16 范围内钳制，避免溢出回绕。
    scaled
        .clamp(f32::from(i16::MIN), f32::from(i16::MAX))
        .round() as i16
}

#[cfg(test)]
mod tests {
    use super::{
        amplify_selection, cut_selection, fade_in_selection, fade_out_selection, normalize_buffer,
        reverse_selection,
    };
    use crate::audio::buffer::AudioBuffer;
    use crate::audio::selection::Selection;

    fn sample_buffer() -> AudioBuffer {
        AudioBuffer::new(vec![100, -200, 300, -400, 500, -600], 48_000, 1, 16)
    }

    #[test]
    fn amplify_clamps_overflow() {
        let mut buffer = AudioBuffer::new(vec![20_000, -20_000, 100], 44_100, 1, 16);
        let selection = Selection::from_frame_bounds(&buffer, 0, 3);

        match selection {
            Ok(selection) => {
                let result = amplify_selection(&mut buffer, selection, 2.0);
                assert!(result.is_ok());
                assert_eq!(buffer.samples, vec![32_767, -32_768, 200]);
            }
            Err(error) => panic!("selection should be valid: {error}"),
        }
    }

    #[test]
    fn normalize_targets_ninety_percent_peak() {
        let mut buffer = AudioBuffer::new(vec![1000, -2000, 4000], 44_100, 1, 16);
        let result = normalize_buffer(&mut buffer, 0.9);
        assert!(result.is_ok());

        let peak = buffer
            .samples
            .iter()
            .map(|sample| i32::from(*sample).abs())
            .max()
            .unwrap_or(0);
        let expected = (f32::from(i16::MAX) * 0.9).round() as i32;
        assert!((peak - expected).abs() <= 1);
    }

    #[test]
    fn applies_fade_in_and_out() {
        let mut fade_in_buffer = AudioBuffer::new(vec![1000, 1000, 1000, 1000], 44_100, 1, 16);
        let selection = Selection::from_frame_bounds(&fade_in_buffer, 0, 4);
        match selection {
            Ok(selection) => {
                let fade_in_result = fade_in_selection(&mut fade_in_buffer, selection);
                assert!(fade_in_result.is_ok());
                assert_eq!(fade_in_buffer.samples[0], 0);
                assert!(fade_in_buffer.samples[3] >= 999);
            }
            Err(error) => panic!("selection should be valid: {error}"),
        }

        let mut fade_out_buffer = AudioBuffer::new(vec![1000, 1000, 1000, 1000], 44_100, 1, 16);
        let selection = Selection::from_frame_bounds(&fade_out_buffer, 0, 4);
        match selection {
            Ok(selection) => {
                let fade_out_result = fade_out_selection(&mut fade_out_buffer, selection);
                assert!(fade_out_result.is_ok());
                assert_eq!(fade_out_buffer.samples[3], 0);
                assert!(fade_out_buffer.samples[0] >= 999);
            }
            Err(error) => panic!("selection should be valid: {error}"),
        }
    }

    #[test]
    fn reverses_selection_by_frame() {
        let mut buffer = sample_buffer();
        let selection = Selection::from_frame_bounds(&buffer, 1, 5);

        match selection {
            Ok(selection) => {
                let result = reverse_selection(&mut buffer, selection);
                assert!(result.is_ok());
                assert_eq!(buffer.samples, vec![100, 500, -400, 300, -200, -600]);
            }
            Err(error) => panic!("selection should be valid: {error}"),
        }
    }

    #[test]
    fn cut_returns_selected_frames() {
        let buffer = sample_buffer();
        let selection = Selection::from_frame_bounds(&buffer, 2, 5);

        match selection {
            Ok(selection) => {
                let cut = cut_selection(&buffer, selection);
                match cut {
                    Ok(cut) => {
                        assert_eq!(cut.samples, vec![300, -400, 500]);
                        assert_eq!(cut.sample_rate, 48_000);
                    }
                    Err(error) => panic!("cut should succeed: {error}"),
                }
            }
            Err(error) => panic!("selection should be valid: {error}"),
        }
    }
}
