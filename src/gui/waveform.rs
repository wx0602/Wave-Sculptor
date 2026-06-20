use std::ops::Range;

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::audio::buffer::AudioBuffer;
use crate::audio::selection::Selection;
use crate::gui::theme::ThemePalette;
use crate::gui::viewport::Viewport;
use crate::gui::waveform_mode::WaveformMode;

pub struct WaveformView {
    pub response: Response,
    pub rect: Rect,
    pub plot_rect: Rect,
    pub visible_frame_range: Range<usize>,
}

pub fn draw_waveform(
    ui: &mut Ui,
    audio: &AudioBuffer,
    viewport: &Viewport,
    mode: WaveformMode,
    palette: ThemePalette,
    selection: Option<Selection>,
    playhead_frame: Option<usize>,
) -> WaveformView {
    // 波形区域一次性完成背景、选区、包络、播放头和时间轴绘制。
    let desired_size = Vec2::new(ui.available_width().max(240.0), 340.0);
    let (response, painter) = ui.allocate_painter(desired_size, Sense::click_and_drag());
    let rect = response.rect;
    let content_rect = rect.shrink2(Vec2::new(18.0, 16.0));
    let axis_height = 30.0;
    let plot_rect = Rect::from_min_max(
        content_rect.min,
        Pos2::new(content_rect.max.x, content_rect.max.y - axis_height),
    );
    let visible_frame_range = viewport.visible_frame_range(audio, plot_rect.width());

    paint_background(
        &painter,
        rect,
        content_rect,
        plot_rect,
        palette,
        mode,
        audio.channels_usize(),
    );
    paint_selection(
        &painter,
        plot_rect,
        &visible_frame_range,
        selection,
        palette,
    );
    paint_waveform(
        &painter,
        plot_rect,
        audio,
        &visible_frame_range,
        mode,
        palette,
    );
    paint_playhead(
        &painter,
        plot_rect,
        &visible_frame_range,
        playhead_frame,
        palette,
    );
    paint_time_axis(
        &painter,
        content_rect,
        plot_rect,
        audio,
        &visible_frame_range,
        palette,
    );

    WaveformView {
        response,
        rect,
        plot_rect,
        visible_frame_range,
    }
}

pub fn x_to_frame(x: f32, rect: Rect, visible_range: &Range<usize>) -> usize {
    // 鼠标坐标映射到可见帧范围，用于点击和拖拽选区。
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

fn paint_background(
    painter: &Painter,
    rect: Rect,
    content_rect: Rect,
    plot_rect: Rect,
    palette: ThemePalette,
    mode: WaveformMode,
    channels: usize,
) {
    painter.rect_filled(rect, 10.0, palette.panel_bg);
    painter.rect_stroke(
        rect,
        10.0,
        Stroke::new(1.0, palette.card_border.gamma_multiply(0.55)),
    );
    painter.rect_filled(content_rect, 8.0, palette.card_alt_bg);
    painter.rect_stroke(
        content_rect,
        8.0,
        Stroke::new(1.0, palette.card_border.gamma_multiply(0.45)),
    );
    let center_y = plot_rect.center().y;
    painter.line_segment(
        [
            Pos2::new(plot_rect.left(), center_y),
            Pos2::new(plot_rect.right(), center_y),
        ],
        Stroke::new(1.0, palette.axis_line),
    );

    if matches!(mode, WaveformMode::SplitStereo) && channels > 1 {
        painter.text(
            Pos2::new(plot_rect.left() + 8.0, plot_rect.top() + 8.0),
            Align2::LEFT_TOP,
            "L",
            FontId::proportional(13.0),
            palette.axis_text,
        );
        painter.text(
            Pos2::new(plot_rect.left() + 8.0, plot_rect.center().y + 8.0),
            Align2::LEFT_TOP,
            "R",
            FontId::proportional(13.0),
            palette.axis_text,
        );
        painter.line_segment(
            [
                Pos2::new(plot_rect.left(), plot_rect.center().y),
                Pos2::new(plot_rect.right(), plot_rect.center().y),
            ],
            Stroke::new(1.0, palette.axis_line),
        );
    }
}

fn paint_selection(
    painter: &Painter,
    rect: Rect,
    visible_range: &Range<usize>,
    selection: Option<Selection>,
    palette: ThemePalette,
) {
    let Some(selection) = selection else {
        return;
    };
    // 只绘制和当前视口相交的选区部分。
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
    painter.rect_filled(selection_rect, 6.0, palette.selection_fill);
}

fn paint_waveform(
    painter: &Painter,
    rect: Rect,
    audio: &AudioBuffer,
    visible_range: &Range<usize>,
    mode: WaveformMode,
    palette: ThemePalette,
) {
    // 每个屏幕像素列只画该时间片的最小/最大采样包络。
    let columns = downsample_for_width(audio, rect.width().max(1.0) as usize, visible_range, mode);
    if columns.is_empty() {
        return;
    }

    for (index, column) in columns.iter().enumerate() {
        let x = rect.left() + index as f32;
        paint_envelope(painter, rect, x, column, mode, palette);
    }
}

fn paint_playhead(
    painter: &Painter,
    rect: Rect,
    visible_range: &Range<usize>,
    playhead_frame: Option<usize>,
    palette: ThemePalette,
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
        Stroke::new(2.0, palette.playhead),
    );
    painter.circle_filled(Pos2::new(x, rect.top() + 8.0), 4.5, palette.playhead);
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

    // 按像素列分桶，避免在长音频上逐采样绘制。
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
                Some(collect_channel_envelope(
                    audio,
                    start_frame,
                    end_frame,
                    Some(1),
                ))
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
    // 返回归一化到 -1.0..1.0 的包络范围。
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
    palette: ThemePalette,
) {
    // 分离立体声时上下两半分别显示左右声道。
    match (mode, column.secondary) {
        (WaveformMode::SplitStereo, Some(secondary)) => {
            let top_rect = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y));
            let bottom_rect = Rect::from_min_max(Pos2::new(rect.min.x, rect.center().y), rect.max);
            draw_column(painter, top_rect, x, column.primary, palette.waveform_left);
            draw_column(painter, bottom_rect, x, secondary, palette.waveform_right);
        }
        _ => draw_column(
            painter,
            rect,
            x,
            column.primary,
            match mode {
                WaveformMode::Mixed => palette.waveform_mixed,
                WaveformMode::Left => palette.waveform_left,
                WaveformMode::Right => palette.waveform_right,
                WaveformMode::SplitStereo => palette.waveform_mixed,
            },
        ),
    }
}

fn draw_column(painter: &Painter, rect: Rect, x: f32, envelope: (f32, f32), color: Color32) {
    let center_y = rect.center().y;
    let half_height = rect.height() * 0.42;
    let y1 = center_y - envelope.1 * half_height;
    let y2 = center_y - envelope.0 * half_height;
    painter.line_segment(
        [Pos2::new(x, y1), Pos2::new(x, y2)],
        Stroke::new(1.0, color),
    );
}

fn paint_time_axis(
    painter: &Painter,
    content_rect: Rect,
    plot_rect: Rect,
    audio: &AudioBuffer,
    visible_range: &Range<usize>,
    palette: ThemePalette,
) {
    // 时间轴跟随当前可见范围，而不是整段音频范围。
    let axis_top = plot_rect.bottom() + 8.0;
    painter.line_segment(
        [
            Pos2::new(plot_rect.left(), axis_top),
            Pos2::new(plot_rect.right(), axis_top),
        ],
        Stroke::new(1.0, palette.axis_line),
    );

    let ticks = 5;
    let visible_frames = visible_range.end.saturating_sub(visible_range.start).max(1);
    for index in 0..=ticks {
        let t = index as f32 / ticks as f32;
        let x = egui::lerp(plot_rect.left()..=plot_rect.right(), t);
        let frame = visible_range.start + ((visible_frames as f32 * t).round() as usize);
        let seconds = audio.frame_to_seconds(frame.min(audio.frame_count()));
        painter.line_segment(
            [Pos2::new(x, axis_top), Pos2::new(x, axis_top + 6.0)],
            Stroke::new(1.0, palette.axis_line),
        );
        painter.text(
            Pos2::new(x, axis_top + 9.0),
            Align2::CENTER_TOP,
            format_mm_ss(seconds),
            FontId::proportional(12.0),
            palette.axis_text,
        );
    }

    painter.text(
        Pos2::new(content_rect.right() - 4.0, axis_top + 10.0),
        Align2::RIGHT_TOP,
        "时间",
        FontId::proportional(12.0),
        palette.axis_text,
    );
}

fn format_mm_ss(seconds: f64) -> String {
    let total_seconds = seconds.max(0.0).round() as u64;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{minutes:02}:{secs:02}")
}
