use crate::error::{Result, WaveSculptorError};

#[derive(Clone, Debug, PartialEq)]
pub struct AudioBuffer {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

impl AudioBuffer {
    pub fn new(samples: Vec<i16>, sample_rate: u32, channels: u16, bits_per_sample: u16) -> Self {
        Self {
            samples,
            sample_rate,
            channels,
            bits_per_sample,
        }
    }

    pub fn total_samples(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn channels_usize(&self) -> usize {
        usize::from(self.channels.max(1))
    }

    pub fn frame_count(&self) -> usize {
        // 一帧包含所有声道的同一时间点采样。
        self.samples.len() / self.channels_usize()
    }

    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frame_count() as f64 / self.sample_rate as f64
        }
    }

    pub fn frame_to_sample_range(
        &self,
        start_frame: usize,
        end_frame: usize,
    ) -> std::ops::Range<usize> {
        // 样本数组按声道交错存储，帧范围要转换成采样下标范围。
        let channels = self.channels_usize();
        start_frame * channels..end_frame * channels
    }

    pub fn slice_frames(&self, start_frame: usize, end_frame: usize) -> Self {
        let range = self.frame_to_sample_range(start_frame, end_frame);
        Self::new(
            self.samples[range].to_vec(),
            self.sample_rate,
            self.channels,
            self.bits_per_sample,
        )
    }

    pub fn frame(&self, frame_index: usize) -> Option<&[i16]> {
        let channels = self.channels_usize();
        let start = frame_index.checked_mul(channels)?;
        let end = start.checked_add(channels)?;
        self.samples.get(start..end)
    }

    pub fn frame_mut(&mut self, frame_index: usize) -> Option<&mut [i16]> {
        let channels = self.channels_usize();
        let start = frame_index.checked_mul(channels)?;
        let end = start.checked_add(channels)?;
        self.samples.get_mut(start..end)
    }

    pub fn frame_to_seconds(&self, frame_index: usize) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            frame_index as f64 / self.sample_rate as f64
        }
    }

    pub fn seconds_to_frame_clamped(&self, seconds: f64) -> Result<usize> {
        // 外部输入的时间会被限制到音频末尾，避免调用方自行处理越界。
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(WaveSculptorError::InvalidParameter(
                "时间参数必须是非负有限数值".to_string(),
            ));
        }
        if self.sample_rate == 0 {
            return Ok(0);
        }

        let frame = (seconds * self.sample_rate as f64).round() as usize;
        Ok(frame.min(self.frame_count()))
    }

    pub fn align_sample_index(&self, sample_index: usize) -> usize {
        // 视口和平移必须落在帧边界，防止立体声左右声道错位。
        let channels = self.channels_usize().max(1);
        sample_index - sample_index % channels
    }
}
