use std::ops::Range;

use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::audio::buffer::AudioBuffer;
use crate::audio::selection::Selection;
use crate::gui::viewport::Viewport;
use crate::gui::waveform_mode::WaveformMode;

pub struct WaveformView {
    pub response: Response,
    pub rect: Rect,
    pub visible_frame_range: Range<usize>,
}

pub fn draw_waveform(
    ui: &mut Ui,
    audio: &AudioBuffer,
    viewport: &Viewport,
    mode: WaveformMode,
    selection: Option<Selection>,
    playhead_frame: Option<usize>,
) -> WaveformView {
    let desired_size = Vec2::new(ui.available_width().max(240.0), 260.0);
    let (response, painter) = ui.allocate_painter(desired_size, Sense::click_and_drag());
    let rect = response.rect;
    let visible_frame_range = viewport.visible_frame_range(audio, rect.width());

    paint_background(&painter, rect);
    paint_selection(&painter, rect, &visible_frame_range, selection);
    paint_waveform(&painter, rect, audio, &visible_frame_range, mode);
    paint_playhead(&painter, rect, &visible_frame_range, playhead_frame);

    WaveformView {
        response,
        rect,
        visible_frame_range,
    }
}

pub fn x_to_frame(x: f32, rect: Rect, visible_range: &Range<usize>) -> usize {
    if visible_range.start >= visible_range.end {
        return visible_range.start;
    }

    let visible_frames = visible_range.end - visible_range.start;
    let normalized = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    let frame = (normalized * visible_frames as f32).floor() as usize;
    (visible_range.start + frame).min(visible_range.end.saturating_sub(1))
}

pub fn frame_to_x(frame: usize, rect: Rect, visible_range: &Range<usize>) -> f32 {
    let visible_frames = visible_range.end.saturating_sub(visible_range.start);
    if visible_frames == 0 {
        return rect.left();
    }

    let relative = frame.saturating_sub(visible_range.start);
    rect.left() + rect.width() * (relative as f32 / visible_frames as f32)
}

fn paint_background(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 6.0, Color32::from_rgb(18, 24, 34));
    let center_y = rect.center().y;
    painter.line_segment(
        [Pos2::new(rect.left(), center_y), Pos2::new(rect.right(), center_y)],
        Stroke::new(1.0, Color32::from_gray(70)),
    );
}

fn paint_selection(
    painter: &Painter,
    rect: Rect,
    visible_range: &Range<usize>,
    selection: Option<Selection>,
) {
    let Some(selection) = selection else {
        return;
    };
    if selection.end_frame <= visible_range.start || selection.start_frame >= visible_range.end {
        return;
    }

    let start = selection.start_frame.max(visible_range.start);
    let end = selection.end_frame.min(visible_range.end);
    let left = frame_to_x(start, rect, visible_range);
    let right = frame_to_x(end, rect, visible_range);
    let selection_rect = Rect::from_min_max(
        Pos2::new(left.min(right), rect.top()),
        Pos2::new(left.max(right), rect.bottom()),
    );
    painter.rect_filled(
        selection_rect,
        0.0,
        Color32::from_rgba_unmultiplied(100, 180, 255, 64),
    );
}

fn paint_waveform(
    painter: &Painter,
    rect: Rect,
    audio: &AudioBuffer,
    visible_range: &Range<usize>,
    mode: WaveformMode,
) {
    let columns = downsample_for_width(audio, rect.width().max(1.0) as usize, visible_range, mode);
    if columns.is_empty() {
        return;
    }

    for (index, column) in columns.iter().enumerate() {
        let x = rect.left() + index as f32;
        paint_envelope(painter, rect, x, column, mode);
    }
}

fn paint_playhead(
    painter: &Painter,
    rect: Rect,
    visible_range: &Range<usize>,
    playhead_frame: Option<usize>,
) {
    let Some(playhead_frame) = playhead_frame else {
        return;
    };
    if playhead_frame < visible_range.start || playhead_frame > visible_range.end {
        return;
    }

    let x = frame_to_x(playhead_frame, rect, visible_range);
    painter.line_segment(
        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
        Stroke::new(2.0, Color32::from_rgb(255, 96, 96)),
    );
}

#[derive(Clone, Copy)]
struct WaveformColumn {
    primary: (f32, f32),
    secondary: Option<(f32, f32)>,
}

fn downsample_for_width(
    audio: &AudioBuffer,
    width: usize,
    visible_range: &Range<usize>,
    mode: WaveformMode,
) -> Vec<WaveformColumn> {
    let total_frames = visible_range.end.saturating_sub(visible_range.start);
    if total_frames == 0 || width == 0 {
        return Vec::new();
    }

    let columns = width.min(total_frames.max(1));
    let frames_per_column = ((total_frames as f32 / columns as f32).ceil() as usize).max(1);
    let mut result = Vec::with_capacity(columns);

    for column in 0..columns {
        let start_frame = visible_range.start + column * frames_per_column;
        if start_frame >= visible_range.end {
            break;
        }
        let end_frame = (start_frame + frames_per_column).min(visible_range.end);
        result.push(extract_envelope(audio, start_frame, end_frame, mode));
    }

    result
}

fn extract_envelope(
    audio: &AudioBuffer,
    start_frame: usize,
    end_frame: usize,
    mode: WaveformMode,
) -> WaveformColumn {
    match mode {
        WaveformMode::Mixed => WaveformColumn {
            primary: collect_channel_envelope(audio, start_frame, end_frame, None),
            secondary: None,
        },
        WaveformMode::Left => WaveformColumn {
            primary: collect_channel_envelope(audio, start_frame, end_frame, Some(0)),
            secondary: None,
        },
        WaveformMode::Right => {
            let channel = if audio.channels_usize() > 1 { 1 } else { 0 };
            WaveformColumn {
                primary: collect_channel_envelope(audio, start_frame, end_frame, Some(channel)),
                secondary: None,
            }
        }
        WaveformMode::SplitStereo => {
            let left = collect_channel_envelope(audio, start_frame, end_frame, Some(0));
            let right = if audio.channels_usize() > 1 {
                Some(collect_channel_envelope(audio, start_frame, end_frame, Some(1)))
            } else {
                None
            };
            WaveformColumn {
                primary: left,
                secondary: right,
            }
        }
    }
}

fn collect_channel_envelope(
    audio: &AudioBuffer,
    start_frame: usize,
    end_frame: usize,
    channel: Option<usize>,
) -> (f32, f32) {
    let mut min_value = 1.0_f32;
    let mut max_value = -1.0_f32;

    for frame_index in start_frame..end_frame {
        if let Some(frame) = audio.frame(frame_index) {
            match channel {
                Some(channel_index) => {
                    if let Some(sample) = frame.get(channel_index) {
                        let normalized = f32::from(*sample) / f32::from(i16::MAX);
                        min_value = min_value.min(normalized);
                        max_value = max_value.max(normalized);
                    }
                }
                None => {
                    for sample in frame {
                        let normalized = f32::from(*sample) / f32::from(i16::MAX);
                        min_value = min_value.min(normalized);
                        max_value = max_value.max(normalized);
                    }
                }
            }
        }
    }

    if max_value < min_value {
        (0.0, 0.0)
    } else {
        (min_value, max_value)
    }
}

fn paint_envelope(
    painter: &Painter,
    rect: Rect,
    x: f32,
    column: &WaveformColumn,
    mode: WaveformMode,
) {
    match (mode, column.secondary) {
        (WaveformMode::SplitStereo, Some(secondary)) => {
            let top_rect = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y));
            let bottom_rect =
                Rect::from_min_max(Pos2::new(rect.min.x, rect.center().y), rect.max);
            draw_column(
                painter,
                top_rect,
                x,
                column.primary,
                Color32::from_rgb(120, 235, 200),
            );
            draw_column(
                painter,
                bottom_rect,
                x,
                secondary,
                Color32::from_rgb(255, 186, 84),
            );
        }
        _ => draw_column(
            painter,
            rect,
            x,
            column.primary,
            Color32::from_rgb(120, 235, 200),
        ),
    }
}

fn draw_column(
    painter: &Painter,
    rect: Rect,
    x: f32,
    envelope: (f32, f32),
    color: Color32,
) {
    let center_y = rect.center().y;
    let half_height = rect.height() * 0.42;
    let y1 = center_y - envelope.1 * half_height;
    let y2 = center_y - envelope.0 * half_height;
    painter.line_segment([Pos2::new(x, y1), Pos2::new(x, y2)], Stroke::new(1.0, color));
}
