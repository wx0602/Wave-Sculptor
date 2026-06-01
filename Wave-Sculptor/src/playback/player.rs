use std::io::{BufReader, Cursor};
use std::time::Instant;

use rodio::{Decoder, OutputStream, Sink};

use crate::audio::buffer::AudioBuffer;
use crate::audio::edit::cut_selection;
use crate::audio::selection::Selection;
use crate::error::{Result, WaveSculptorError};
use crate::wav::writer::write_wav_to_vec;

#[derive(Clone, Copy, Debug, Default)]
pub struct PlaybackStatus {
    pub is_playing: bool,
    pub current_seconds: f64,
    pub total_seconds: f64,
    pub playhead_frame: Option<usize>,
    pub active_selection: Option<Selection>,
}

pub struct Player {
    stream: Option<OutputStream>,
    sink: Option<Sink>,
    started_at: Option<Instant>,
    playback_start_frame: usize,
    playback_end_frame: usize,
    playback_sample_rate: u32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            stream: None,
            sink: None,
            started_at: None,
            playback_start_frame: 0,
            playback_end_frame: 0,
            playback_sample_rate: 0,
        }
    }

    pub fn stop(&mut self) {
        self.stop_internal();
    }

    pub fn status(&mut self) -> PlaybackStatus {
        self.refresh();

        let total_frames = self.playback_end_frame.saturating_sub(self.playback_start_frame);
        let total_seconds = if self.playback_sample_rate == 0 {
            0.0
        } else {
            total_frames as f64 / self.playback_sample_rate as f64
        };

        let is_playing = self.is_playing();
        let current_seconds = if is_playing {
            self.started_at
                .map(|started| started.elapsed().as_secs_f64().min(total_seconds))
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let playhead_frame = if is_playing && self.playback_sample_rate > 0 {
            let relative_frame = (current_seconds * self.playback_sample_rate as f64).round() as usize;
            Some((self.playback_start_frame + relative_frame).min(self.playback_end_frame))
        } else {
            None
        };
        let active_selection = if total_frames > 0 {
            Selection::new(self.playback_start_frame, self.playback_end_frame).ok()
        } else {
            None
        };

        PlaybackStatus {
            is_playing,
            current_seconds,
            total_seconds,
            playhead_frame,
            active_selection,
        }
    }

    fn stop_internal(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.stream = None;
        self.started_at = None;
        self.playback_start_frame = 0;
        self.playback_end_frame = 0;
        self.playback_sample_rate = 0;
    }

    pub fn is_playing(&self) -> bool {
        self.sink.as_ref().is_some_and(|sink| !sink.empty())
    }

    pub fn play_buffer(&mut self, buffer: &AudioBuffer) -> Result<()> {
        let bytes = write_wav_to_vec(buffer)?;
        self.play_wav_bytes(bytes, 0, buffer.frame_count(), buffer.sample_rate)
    }

    pub fn play_selection(&mut self, buffer: &AudioBuffer, selection: Selection) -> Result<()> {
        self.play_range(buffer, selection)
    }

    pub fn play_range(&mut self, buffer: &AudioBuffer, selection: Selection) -> Result<()> {
        let selected = cut_selection(buffer, selection)?;
        let bytes = write_wav_to_vec(&selected)?;
        self.play_wav_bytes(
            bytes,
            selection.start_frame,
            selection.end_frame,
            buffer.sample_rate,
        )
    }

    fn play_wav_bytes(
        &mut self,
        bytes: Vec<u8>,
        start_frame: usize,
        end_frame: usize,
        sample_rate: u32,
    ) -> Result<()> {
        self.stop_internal();

        let cursor = Cursor::new(bytes);
        let decoder = Decoder::new(BufReader::new(cursor))
            .map_err(|err| WaveSculptorError::Playback(err.to_string()))?;
        let (stream, handle) =
            OutputStream::try_default().map_err(|err| WaveSculptorError::Playback(err.to_string()))?;
        let sink = Sink::try_new(&handle).map_err(|err| WaveSculptorError::Playback(err.to_string()))?;
        sink.append(decoder);
        sink.play();

        self.stream = Some(stream);
        self.sink = Some(sink);
        self.started_at = Some(Instant::now());
        self.playback_start_frame = start_frame;
        self.playback_end_frame = end_frame;
        self.playback_sample_rate = sample_rate;

        Ok(())
    }

    fn refresh(&mut self) {
        if self.sink.as_ref().is_some_and(Sink::empty) {
            self.stop_internal();
        }
    }
}
