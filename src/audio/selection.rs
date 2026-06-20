use crate::audio::buffer::AudioBuffer;
use crate::error::{Result, WaveSculptorError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub start_frame: usize,
    pub end_frame: usize,
}

impl Selection {
    pub fn new(start_frame: usize, end_frame: usize) -> Result<Self> {
        // 选区使用左闭右开帧范围，空范围无效。
        if start_frame >= end_frame {
            return Err(WaveSculptorError::InvalidSelection);
        }

        Ok(Self {
            start_frame,
            end_frame,
        })
    }

    pub fn full(buffer: &AudioBuffer) -> Result<Self> {
        Self::from_frame_bounds(buffer, 0, buffer.frame_count())
    }

    pub fn from_frame_bounds(
        buffer: &AudioBuffer,
        start_frame: usize,
        end_frame: usize,
    ) -> Result<Self> {
        if end_frame > buffer.frame_count() {
            return Err(WaveSculptorError::InvalidSelection);
        }

        Self::new(start_frame, end_frame)
    }

    pub fn from_times(buffer: &AudioBuffer, start_seconds: f64, end_seconds: f64) -> Result<Self> {
        if !start_seconds.is_finite() || !end_seconds.is_finite() {
            return Err(WaveSculptorError::InvalidParameter(
                "时间参数必须是有限数值".to_string(),
            ));
        }
        if start_seconds < 0.0 || end_seconds < 0.0 {
            return Err(WaveSculptorError::InvalidParameter(
                "时间参数必须是非负数".to_string(),
            ));
        }
        if end_seconds <= start_seconds {
            return Err(WaveSculptorError::InvalidSelection);
        }
        if buffer.sample_rate == 0 {
            return Err(WaveSculptorError::InvalidParameter(
                "采样率为 0，无法按时间创建选区".to_string(),
            ));
        }

        // 起点向下取整、终点向上取整，保证覆盖用户指定的完整时间段。
        let start_frame = (start_seconds * buffer.sample_rate as f64).floor() as usize;
        let end_frame = (end_seconds * buffer.sample_rate as f64).ceil() as usize;
        let clamped_end = end_frame.min(buffer.frame_count());

        Self::from_frame_bounds(buffer, start_frame, clamped_end)
    }

    pub fn duration_frames(self) -> usize {
        self.end_frame - self.start_frame
    }

    pub fn duration_seconds(self, buffer: &AudioBuffer) -> f64 {
        self.duration_frames() as f64 / buffer.sample_rate.max(1) as f64
    }

    pub fn start_sample_index(self, buffer: &AudioBuffer) -> usize {
        self.start_frame * buffer.channels_usize()
    }

    pub fn end_sample_index(self, buffer: &AudioBuffer) -> usize {
        self.end_frame * buffer.channels_usize()
    }

    pub fn start_seconds(self, buffer: &AudioBuffer) -> f64 {
        buffer.frame_to_seconds(self.start_frame)
    }

    pub fn end_seconds(self, buffer: &AudioBuffer) -> f64 {
        buffer.frame_to_seconds(self.end_frame)
    }
}

#[cfg(test)]
mod tests {
    use super::Selection;
    use crate::audio::buffer::AudioBuffer;

    #[test]
    fn converts_seconds_to_frame_selection() {
        let buffer = AudioBuffer::new(vec![0; 16], 8, 1, 16);
        let result = Selection::from_times(&buffer, 0.25, 0.75);

        match result {
            Ok(selection) => {
                assert_eq!(selection.start_frame, 2);
                assert_eq!(selection.end_frame, 6);
                assert_eq!(selection.start_sample_index(&buffer), 2);
                assert_eq!(selection.end_sample_index(&buffer), 6);
                assert!((selection.duration_seconds(&buffer) - 0.5).abs() < f64::EPSILON);
            }
            Err(error) => panic!("selection conversion failed: {error}"),
        }
    }

    #[test]
    fn converts_frame_range_to_sample_indexes_for_stereo() {
        let buffer = AudioBuffer::new(vec![0; 32], 8, 2, 16);
        let result = Selection::from_frame_bounds(&buffer, 2, 6);

        match result {
            Ok(selection) => {
                assert_eq!(selection.start_sample_index(&buffer), 4);
                assert_eq!(selection.end_sample_index(&buffer), 12);
            }
            Err(error) => panic!("selection conversion failed: {error}"),
        }
    }
}
