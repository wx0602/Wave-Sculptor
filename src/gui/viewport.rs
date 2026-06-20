use std::ops::Range;

use crate::audio::buffer::AudioBuffer;

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub offset_sample: usize,
    pub samples_per_pixel: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            offset_sample: 0,
            samples_per_pixel: 1.0,
        }
    }
}

impl Viewport {
    pub fn fit_to_audio(&mut self, audio: &AudioBuffer, width: f32) {
        let width = width.max(1.0);
        self.offset_sample = 0;
        // samples_per_pixel 使用交错采样数，最小值允许放大到单帧以下。
        self.samples_per_pixel =
            (audio.total_samples().max(1) as f32 / width).max(audio.channels_usize() as f32 / 32.0);
        self.clamp(audio, width);
    }

    pub fn visible_frame_range(&self, audio: &AudioBuffer, width: f32) -> Range<usize> {
        let channels = audio.channels_usize();
        // 视口从采样下标存储，但绘制和选区统一使用帧下标。
        let start_sample = audio.align_sample_index(self.offset_sample.min(audio.total_samples()));
        let visible_samples = (width.max(1.0) * self.samples_per_pixel).ceil() as usize;
        let end_sample = (start_sample + visible_samples).min(audio.total_samples());
        let start_frame = start_sample / channels;
        let end_frame =
            ((end_sample + channels.saturating_sub(1)) / channels).min(audio.frame_count());

        if end_frame <= start_frame && audio.frame_count() > 0 {
            start_frame..(start_frame + 1).min(audio.frame_count())
        } else {
            start_frame..end_frame
        }
    }

    pub fn zoom_around(&mut self, audio: &AudioBuffer, width: f32, anchor_x: f32, factor: f32) {
        let width = width.max(1.0);
        let old_spp = self
            .samples_per_pixel
            .max(audio.channels_usize() as f32 / 32.0);
        let fit_spp = (audio.total_samples().max(1) as f32 / width).max(1.0);
        let min_spp = (audio.channels_usize() as f32 / 64.0).max(0.01);
        let max_spp = (fit_spp * 32.0).max(min_spp);
        let anchor_sample = self.offset_sample as f32 + anchor_x.clamp(0.0, width) * old_spp;

        // 缩放后反推 offset，让鼠标所在的音频位置保持不动。
        self.samples_per_pixel = (old_spp * factor).clamp(min_spp, max_spp);
        let new_offset = anchor_sample - anchor_x.clamp(0.0, width) * self.samples_per_pixel;
        self.offset_sample = new_offset.max(0.0).round() as usize;
        self.offset_sample = audio.align_sample_index(self.offset_sample);
        self.clamp(audio, width);
    }

    pub fn pan_pixels(&mut self, audio: &AudioBuffer, width: f32, delta_pixels: f32) {
        let delta_samples = (delta_pixels * self.samples_per_pixel).round() as isize;
        self.pan_samples(audio, width, delta_samples);
    }

    pub fn pan_by_fraction(&mut self, audio: &AudioBuffer, width: f32, fraction: f32) {
        let visible_samples = (width.max(1.0) * self.samples_per_pixel).round() as isize;
        self.pan_samples(
            audio,
            width,
            (visible_samples as f32 * fraction).round() as isize,
        );
    }

    fn pan_samples(&mut self, audio: &AudioBuffer, width: f32, delta_samples: isize) {
        if delta_samples >= 0 {
            self.offset_sample = self.offset_sample.saturating_add(delta_samples as usize);
        } else {
            self.offset_sample = self
                .offset_sample
                .saturating_sub(delta_samples.unsigned_abs());
        }
        self.offset_sample = audio.align_sample_index(self.offset_sample);
        self.clamp(audio, width.max(1.0));
    }

    fn clamp(&mut self, audio: &AudioBuffer, width: f32) {
        let width = width.max(1.0);
        let visible_samples = (width * self.samples_per_pixel).ceil() as usize;
        if visible_samples >= audio.total_samples() {
            self.offset_sample = 0;
            return;
        }

        let max_offset = audio.total_samples().saturating_sub(visible_samples);
        self.offset_sample = audio.align_sample_index(self.offset_sample.min(max_offset));
    }
}
