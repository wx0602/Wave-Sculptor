use std::path::{Path, PathBuf};

use eframe::egui;
use rfd::FileDialog;

use crate::audio::analyze::{analyze_buffer, AudioAnalysis};
use crate::audio::buffer::AudioBuffer;
use crate::audio::edit::{
    amplify_selection, cut_selection, cut_selection_in_place, fade_in_selection,
    fade_out_selection, mute_selection, normalize_buffer, reverse_selection, trim_silence,
    DEFAULT_NORMALIZE_TARGET,
};
use crate::audio::history::AudioDocument;
use crate::audio::selection::Selection;
use crate::error::{Result, WaveSculptorError};
use crate::gui::viewport::Viewport;
use crate::gui::waveform::{draw_waveform, x_to_frame};
use crate::gui::waveform_mode::WaveformMode;
use crate::playback::player::{PlaybackStatus, Player};
use crate::wav::{reader, writer};

pub struct AppState {
    document: Option<AudioDocument>,
    file_path: Option<PathBuf>,
    selection: Option<Selection>,
    status_message: String,
    player: Player,
    analysis: Option<AudioAnalysis>,
    amplify_factor: f32,
    waveform_mode: WaveformMode,
    viewport: Viewport,
    fit_view_requested: bool,
    drag_anchor: Option<usize>,
    click_anchor: Option<usize>,
    suppress_waveform_input_once: bool,
    last_pan_drag_delta: f32,
    last_waveform_width: f32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            document: None,
            file_path: None,
            selection: None,
            status_message: "就绪。请先打开一个 16 位 PCM WAV 文件。".to_string(),
            player: Player::new(),
            analysis: None,
            amplify_factor: 1.5,
            waveform_mode: WaveformMode::Mixed,
            viewport: Viewport::default(),
            fit_view_requested: true,
            drag_anchor: None,
            click_anchor: None,
            suppress_waveform_input_once: false,
            last_pan_drag_delta: 0.0,
            last_waveform_width: 800.0,
        }
    }
}

impl AppState {
    fn set_status_ok(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
    }

    fn set_status_error(&mut self, error: &WaveSculptorError) {
        self.status_message = format!("错误：{error}");
    }

    fn current_buffer(&self) -> Option<&AudioBuffer> {
        self.document.as_ref().map(AudioDocument::buffer)
    }

    fn current_selection(&self) -> Option<Selection> {
        let selection = self.selection?;
        let buffer = self.current_buffer()?;
        Selection::from_frame_bounds(buffer, selection.start_frame, selection.end_frame).ok()
    }

    fn has_selection_state(&self) -> bool {
        self.selection.is_some() || self.drag_anchor.is_some() || self.click_anchor.is_some()
    }

    fn clear_selection(&mut self) {
        self.selection = None;
        self.drag_anchor = None;
        self.click_anchor = None;
    }

    fn refresh_analysis(&mut self) {
        self.analysis = self.current_buffer().and_then(|buffer| analyze_buffer(buffer).ok());
    }

    fn after_buffer_change(&mut self, fit_view: bool) {
        self.player.stop();
        self.refresh_analysis();
        if let Some(buffer) = self.current_buffer() {
            if self
                .selection
                .map(|selection| selection.end_frame > buffer.frame_count())
                .unwrap_or(false)
            {
                self.clear_selection();
            }
        } else {
            self.clear_selection();
        }
        if fit_view {
            self.fit_view_requested = true;
        }
    }

    fn open_audio(&mut self) {
        let Some(path) = FileDialog::new()
            .add_filter("WAV 文件", &["wav"])
            .pick_file()
        else {
            return;
        };

        match reader::read_wav_file(&path) {
            Ok(audio) => {
                self.player.stop();
                self.document = Some(AudioDocument::new(audio));
                self.file_path = Some(path.clone());
                self.clear_selection();
                self.fit_view_requested = true;
                self.refresh_analysis();
                let file_name = display_file_name(&path);
                self.set_status_ok(format!("已打开 {file_name}。"));
            }
            Err(error) => self.set_status_error(&error),
        }
    }

    fn save_audio_as(&mut self) {
        let Some(buffer) = self.current_buffer() else {
            self.set_status_error(&WaveSculptorError::NoAudioLoaded);
            return;
        };

        let Some(path) = FileDialog::new()
            .add_filter("WAV 文件", &["wav"])
            .set_file_name("编辑后.wav")
            .save_file()
        else {
            return;
        };

        match writer::write_wav_file(&path, buffer) {
            Ok(()) => self.set_status_ok(format!("已将音频保存到 {}。", path.display())),
            Err(error) => self.set_status_error(&error),
        }
    }

    fn export_selection(&mut self) {
        let Some(buffer) = self.current_buffer() else {
            self.set_status_error(&WaveSculptorError::NoAudioLoaded);
            return;
        };
        let Some(selection) = self.current_selection() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };

        let Some(path) = FileDialog::new()
            .add_filter("WAV 文件", &["wav"])
            .set_file_name("选区.wav")
            .save_file()
        else {
            return;
        };

        match cut_selection(buffer, selection).and_then(|cut| writer::write_wav_file(&path, &cut)) {
            Ok(()) => self.set_status_ok(format!("已将选区导出到 {}。", path.display())),
            Err(error) => self.set_status_error(&error),
        }
    }

    fn play_current(&mut self) {
        let selection = self.current_selection();
        let Some(buffer) = self.current_buffer().cloned() else {
            self.set_status_error(&WaveSculptorError::NoAudioLoaded);
            return;
        };

        let result = if let Some(selection) = selection {
            self.player.play_range(&buffer, selection)
        } else {
            self.player.play_buffer(&buffer)
        };

        match result {
            Ok(()) => {
                if selection.is_some() {
                    self.set_status_ok("正在播放选区。");
                } else {
                    self.set_status_ok("正在播放完整音频。");
                }
            }
            Err(error) => self.set_status_error(&error),
        }
    }

    fn stop_playback(&mut self) {
        self.player.stop();
        self.set_status_ok("已停止播放。");
    }

    fn apply_edit<F>(&mut self, label: &str, success_message: impl Into<String>, edit: F)
    where
        F: FnOnce(&mut AudioBuffer) -> Result<()>,
    {
        let Some(document) = self.document.as_mut() else {
            self.set_status_error(&WaveSculptorError::NoAudioLoaded);
            return;
        };

        match document.apply_edit(label, edit) {
            Ok(()) => {
                self.after_buffer_change(false);
                self.set_status_ok(success_message);
            }
            Err(error) => self.set_status_error(&error),
        }
    }

    fn mute_selection(&mut self) {
        let Some(selection) = self.current_selection() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };
        self.apply_edit("静音", "已将选区静音。", move |buffer| {
            mute_selection(buffer, selection)
        });
    }

    fn amplify_selection(&mut self) {
        let Some(selection) = self.current_selection() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };
        let gain = self.amplify_factor;
        self.apply_edit("放大", format!("已将选区放大 {:.2} 倍。", gain), move |buffer| {
            amplify_selection(buffer, selection, gain)
        });
    }

    fn normalize_audio(&mut self) {
        self.apply_edit("归一化", "已将音频归一化到 90% 峰值。", |buffer| {
            normalize_buffer(buffer, DEFAULT_NORMALIZE_TARGET)
        });
    }

    fn fade_in_selection(&mut self) {
        let Some(selection) = self.current_selection() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };
        self.apply_edit("淡入", "已对选区应用淡入。", move |buffer| {
            fade_in_selection(buffer, selection)
        });
    }

    fn fade_out_selection(&mut self) {
        let Some(selection) = self.current_selection() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };
        self.apply_edit("淡出", "已对选区应用淡出。", move |buffer| {
            fade_out_selection(buffer, selection)
        });
    }

    fn reverse_selection(&mut self) {
        let Some(selection) = self.current_selection() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };
        self.apply_edit("反转选区", "已反转选区采样。", move |buffer| {
            reverse_selection(buffer, selection)
        });
    }

    fn cut_to_selection(&mut self) {
        let Some(selection) = self.current_selection() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };
        self.apply_edit("裁剪为选区", "已将音频裁剪为当前选区。", move |buffer| {
            cut_selection_in_place(buffer, selection)
        });
        self.clear_selection();
        self.fit_view_requested = true;
    }

    fn trim_silence(&mut self) {
        self.apply_edit("去除首尾静音", "已去除开头和结尾的静音片段。", trim_silence);
        self.fit_view_requested = true;
    }

    fn undo(&mut self) {
        let Some(document) = self.document.as_mut() else {
            self.set_status_error(&WaveSculptorError::NoAudioLoaded);
            return;
        };

        match document.undo() {
            Ok(Some(label)) => {
                self.after_buffer_change(false);
                self.set_status_ok(format!("已撤销：{label}。"));
            }
            Ok(None) => self.set_status_ok("没有可撤销的操作。"),
            Err(error) => self.set_status_error(&error),
        }
    }

    fn redo(&mut self) {
        let Some(document) = self.document.as_mut() else {
            self.set_status_error(&WaveSculptorError::NoAudioLoaded);
            return;
        };

        match document.redo() {
            Ok(Some(label)) => {
                self.after_buffer_change(false);
                self.set_status_ok(format!("已重做：{label}。"));
            }
            Ok(None) => self.set_status_ok("没有可重做的操作。"),
            Err(error) => self.set_status_error(&error),
        }
    }

    fn render_top_bar(&mut self, ui: &mut egui::Ui, playback: PlaybackStatus) {
        let has_audio = self.current_buffer().is_some();
        let has_selection = self.has_selection_state();
        let can_undo = self.document.as_ref().map(AudioDocument::can_undo).unwrap_or(false);
        let can_redo = self.document.as_ref().map(AudioDocument::can_redo).unwrap_or(false);

        ui.horizontal_wrapped(|ui| {
            if ui.button("打开").clicked() {
                self.open_audio();
            }
            if ui
                .add_enabled(has_audio, egui::Button::new("播放"))
                .clicked()
            {
                self.play_current();
            }
            if ui
                .add_enabled(playback.is_playing, egui::Button::new("停止"))
                .clicked()
            {
                self.stop_playback();
            }
            if ui
                .add_enabled(has_audio, egui::Button::new("另存为"))
                .clicked()
            {
                self.save_audio_as();
            }
            if ui
                .add_enabled(can_undo, egui::Button::new("撤销"))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(can_redo, egui::Button::new("重做"))
                .clicked()
            {
                self.redo();
            }
        });

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_selection, egui::Button::new("静音"))
                .clicked()
            {
                self.mute_selection();
            }
            ui.label("音量倍数：");
            ui.add(
                egui::DragValue::new(&mut self.amplify_factor)
                    .speed(0.1)
                    .clamp_range(0.0..=8.0),
            );
            if ui
                .add_enabled(has_selection, egui::Button::new("放大"))
                .clicked()
            {
                self.amplify_selection();
            }
            if ui
                .add_enabled(has_audio, egui::Button::new("归一化"))
                .clicked()
            {
                self.normalize_audio();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("淡入"))
                .clicked()
            {
                self.fade_in_selection();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("淡出"))
                .clicked()
            {
                self.fade_out_selection();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("反转选区"))
                .clicked()
            {
                self.reverse_selection();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("裁剪为选区"))
                .clicked()
            {
                self.cut_to_selection();
            }
            if ui
                .add_enabled(has_audio, egui::Button::new("去除首尾静音"))
                .clicked()
            {
                self.trim_silence();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("导出选区"))
                .clicked()
            {
                self.export_selection();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("清除选区"))
                .clicked()
            {
                self.clear_selection();
                self.suppress_waveform_input_once = true;
                self.set_status_ok("已清除选区。");
            }
        });

        ui.horizontal_wrapped(|ui| {
            ui.label("显示模式：");
            egui::ComboBox::from_id_source("waveform_mode")
                .selected_text(self.waveform_mode.label())
                .show_ui(ui, |ui| {
                    for mode in WaveformMode::ALL {
                        ui.selectable_value(&mut self.waveform_mode, mode, mode.label());
                    }
                });

            if ui
                .add_enabled(has_audio, egui::Button::new("向左"))
                .clicked()
            {
                if let Some(document) = self.document.as_ref() {
                    self.viewport
                        .pan_by_fraction(document.buffer(), self.last_waveform_width, -0.25);
                }
            }
            if ui
                .add_enabled(has_audio, egui::Button::new("向右"))
                .clicked()
            {
                if let Some(document) = self.document.as_ref() {
                    self.viewport
                        .pan_by_fraction(document.buffer(), self.last_waveform_width, 0.25);
                }
            }
            if ui
                .add_enabled(has_audio, egui::Button::new("适配视图"))
                .clicked()
            {
                self.fit_view_requested = true;
            }

            ui.separator();
            ui.label(format!(
                "播放进度：{} / {}",
                format_duration(playback.current_seconds),
                format_duration(playback.total_seconds.max(
                    self.current_buffer()
                        .map(AudioBuffer::duration_seconds)
                        .unwrap_or_default(),
                )),
            ));
        });
    }

    fn render_info_panel(&self, ui: &mut egui::Ui, playback: PlaybackStatus) {
        ui.heading("音频信息");
        ui.separator();

        if let Some(buffer) = self.current_buffer() {
            let file_name = self
                .file_path
                .as_deref()
                .map(display_file_name)
                .unwrap_or_else(|| "（未知）".to_string());

            ui.label(format!("文件：{file_name}"));
            ui.label(format!("采样率：{} 赫兹", buffer.sample_rate));
            ui.label(format!("声道数：{}", buffer.channels));
            ui.label(format!("位深：{} 位", buffer.bits_per_sample));
            ui.label(format!("时长：{}", format_duration(buffer.duration_seconds())));
            ui.label(format!("采样点总数：{}", buffer.total_samples()));
            ui.label(format!("帧数：{}", buffer.frame_count()));
            ui.label(format!(
                "播放：{} / {}",
                format_duration(playback.current_seconds),
                format_duration(buffer.duration_seconds())
            ));
        } else {
            ui.label("未加载文件。");
        }

        ui.separator();
        ui.heading("选区");
        if let (Some(buffer), Some(selection)) = (self.current_buffer(), self.current_selection()) {
            ui.label(format!(
                "起点：{}",
                format_duration(selection.start_seconds(buffer))
            ));
            ui.label(format!(
                "终点：{}",
                format_duration(selection.end_seconds(buffer))
            ));
            ui.label(format!(
                "长度：{}",
                format_duration(selection.duration_seconds(buffer))
            ));
            ui.label(format!(
                "帧范围：{} - {}",
                selection.start_frame, selection.end_frame
            ));
        } else {
            ui.label("已选区间：无");
        }

        ui.separator();
        ui.heading("分析");
        if let Some(analysis) = &self.analysis {
            ui.label(format!("峰值振幅：{:.2}%", analysis.peak * 100.0));
            ui.label(format!("均方根：{:.4}", analysis.rms));
            ui.label(format!("削波采样点：{}", analysis.clipping_samples));
            ui.label(format!("静音片段数：{}", analysis.silent_segments.len()));
        } else {
            ui.label("暂无分析结果。");
        }
    }

    fn render_waveform(&mut self, ui: &mut egui::Ui, playback: PlaybackStatus) {
        if self.current_buffer().is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("请打开一个 16 位 PCM WAV 文件以显示波形。");
            });
            return;
        }

        let current_selection = self.current_selection();
        let waveform = {
            let Some(buffer) = self.current_buffer() else {
                return;
            };
            draw_waveform(
                ui,
                buffer,
                &self.viewport,
                self.waveform_mode,
                current_selection,
                playback.playhead_frame,
            )
        };

        self.last_waveform_width = waveform.rect.width();

        if self.fit_view_requested {
            if let Some(document) = self.document.as_ref() {
                self.viewport
                    .fit_to_audio(document.buffer(), waveform.rect.width());
                self.fit_view_requested = false;
            }
        }

        if waveform.response.hovered() {
            let scroll_delta = ui.input(|input| input.raw_scroll_delta.y);
            if scroll_delta.abs() > f32::EPSILON {
                let anchor_x = waveform
                    .response
                    .hover_pos()
                    .map(|pos| pos.x - waveform.rect.left())
                    .unwrap_or(waveform.rect.width() * 0.5);
                let zoom_factor = if scroll_delta > 0.0 { 0.85 } else { 1.15 };
                if let Some(document) = self.document.as_ref() {
                    self.viewport.zoom_around(
                        document.buffer(),
                        waveform.rect.width(),
                        anchor_x,
                        zoom_factor,
                    );
                }
            }
        }

        let pointer_primary_down = ui.input(|input| input.pointer.primary_down());
        let pointer_secondary_down = ui.input(|input| input.pointer.secondary_down());
        let frame_count = self
            .current_buffer()
            .map(AudioBuffer::frame_count)
            .unwrap_or_default();

        if waveform.response.drag_started() && pointer_primary_down {
            if let Some(pointer_pos) = waveform.response.interact_pointer_pos() {
                let frame = x_to_frame(pointer_pos.x, waveform.rect, &waveform.visible_frame_range);
                self.drag_anchor = Some(frame);
                self.click_anchor = None;
                self.selection = Selection::new(frame, (frame + 1).min(frame_count)).ok();
            }
        }

        if let Some(anchor) = self.drag_anchor {
            if pointer_primary_down {
                if let Some(pointer_pos) = waveform.response.interact_pointer_pos() {
                    let current = x_to_frame(pointer_pos.x, waveform.rect, &waveform.visible_frame_range);
                    let start = anchor.min(current);
                    let end = anchor.max(current).saturating_add(1).min(frame_count);
                    if let Some(buffer) = self.current_buffer() {
                        self.selection = Selection::from_frame_bounds(buffer, start, end).ok();
                    }
                }
            } else {
                self.drag_anchor = None;
                if self.current_selection().is_some() {
                    self.set_status_ok("已通过拖动更新选区。");
                }
            }
        }

        if waveform.response.dragged() && pointer_secondary_down {
            let delta = waveform.response.drag_delta().x - self.last_pan_drag_delta;
            self.last_pan_drag_delta = waveform.response.drag_delta().x;
            if let Some(document) = self.document.as_ref() {
                self.viewport
                    .pan_pixels(document.buffer(), waveform.rect.width(), -delta);
            }
        } else {
            self.last_pan_drag_delta = 0.0;
        }

        if self.suppress_waveform_input_once {
            self.suppress_waveform_input_once = false;
            return;
        }

        let clicked_frame = waveform
            .response
            .clicked()
            .then(|| waveform.response.interact_pointer_pos())
            .flatten()
            .map(|pointer_pos| x_to_frame(pointer_pos.x, waveform.rect, &waveform.visible_frame_range));

        if let Some(frame) = clicked_frame {
            match self.click_anchor {
                Some(anchor) if anchor != frame => {
                    let start = anchor.min(frame);
                    let end = anchor.max(frame).saturating_add(1).min(frame_count);
                    if let Some(buffer) = self.current_buffer() {
                        self.selection = Selection::from_frame_bounds(buffer, start, end).ok();
                    }
                    self.click_anchor = None;
                    self.set_status_ok("已通过点击更新选区。");
                }
                _ => {
                    self.click_anchor = Some(frame);
                    self.selection = Selection::new(frame, (frame + 1).min(frame_count)).ok();
                    self.set_status_ok("已设置选区起点。请再点击另一处，或拖动以完成选择。");
                }
            }
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let playback = self.player.status();
        if playback.is_playing {
            ctx.request_repaint();
        }

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.render_top_bar(ui, playback);
        });

        egui::SidePanel::right("info_panel")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                self.render_info_panel(ui, playback);
            });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(&self.status_message);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("波形");
            ui.separator();
            self.render_waveform(ui, playback);
        });
    }
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| path.display().to_string(), ToString::to_string)
}

fn format_duration(seconds: f64) -> String {
    let total_millis = (seconds.max(0.0) * 1000.0).round() as u64;
    let minutes = total_millis / 60_000;
    let secs = (total_millis % 60_000) / 1000;
    let millis = total_millis % 1000;
    format!("{minutes:02}:{secs:02}.{millis:03}")
}
