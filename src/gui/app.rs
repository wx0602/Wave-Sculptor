use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align, FontId, Frame, Label, Layout, Margin, RichText, ScrollArea, Stroke,
};
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
use crate::gui::theme::{apply_theme, ThemeMode, ThemePalette};
use crate::gui::viewport::Viewport;
use crate::gui::waveform::{draw_waveform, x_to_frame};
use crate::gui::waveform_mode::WaveformMode;
use crate::playback::player::{PlaybackStatus, Player};
use crate::wav::{reader, writer};

const APP_OUTER_PADDING: f32 = 4.0;
const TOOLBAR_GROUP_HEIGHT: f32 = 60.0;
const TOOLBAR_GROUP_SPACING: f32 = 8.0;
const TOOLBAR_OUTER_PADDING: f32 = 6.0;
const ACTION_BUTTON_HEIGHT: f32 = 35.0;
const ACTION_BUTTON_WIDTH: f32 = 74.0;
const GROUP_TITLE_WIDTH: f32 = 40.0;
const INFO_KEY_WIDTH: f32 = 90.0;
const INFO_ROW_HEIGHT: f32 = 24.0;
const CONTENT_PANEL_SPACING: f32 = 8.0;
const INFO_PANEL_WIDTH: f32 = 348.0;

#[derive(Clone, Copy)]
enum ToolbarGroupKind {
    File,
    Playback,
    Theme,
    Edit,
    Process,
}

impl ToolbarGroupKind {
    fn width(self) -> f32 {
        match self {
            Self::File => 360.0,
            Self::Playback => 430.0,
            Self::Theme => 260.0,
            Self::Edit => 620.0,
            Self::Process => 520.0,
        }
    }
}

pub struct AppState {
    document: Option<AudioDocument>,
    file_path: Option<PathBuf>,
    selection: Option<Selection>,
    status_message: String,
    player: Player,
    analysis: Option<AudioAnalysis>,
    theme_mode: ThemeMode,
    amplify_factor: f32,
    waveform_mode: WaveformMode,
    viewport: Viewport,
    fit_view_requested: bool,
    drag_anchor: Option<usize>,
    click_anchor: Option<usize>,
    suppress_waveform_input_once: bool,
    last_pan_drag_delta: f32,
    last_waveform_width: f32,
    live_amplify_source: Option<AudioBuffer>,
    live_amplify_selection: Option<Selection>,
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
            theme_mode: ThemeMode::Dark,
            amplify_factor: 1.0,
            waveform_mode: WaveformMode::Mixed,
            viewport: Viewport::default(),
            fit_view_requested: true,
            drag_anchor: None,
            click_anchor: None,
            suppress_waveform_input_once: false,
            last_pan_drag_delta: 0.0,
            last_waveform_width: 800.0,
            live_amplify_source: None,
            live_amplify_selection: None,
        }
    }
}

impl AppState {
    fn palette(&self) -> ThemePalette {
        self.theme_mode.palette()
    }

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
        self.reset_live_amplify();
    }

    fn reset_live_amplify(&mut self) {
        self.live_amplify_source = None;
        self.live_amplify_selection = None;
    }

    fn refresh_analysis(&mut self) {
        self.analysis = self.current_buffer().and_then(|buffer| analyze_buffer(buffer).ok());
    }

    fn after_buffer_change(&mut self, fit_view: bool) {
        self.player.stop();
        self.reset_live_amplify();
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
                self.waveform_mode = if audio.channels > 1 {
                    WaveformMode::SplitStereo
                } else {
                    WaveformMode::Mixed
                };
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

    fn apply_live_amplify(&mut self) {
        let Some(selection) = self.current_selection() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };
        if self.document.is_none() {
            self.set_status_error(&WaveSculptorError::NoAudioLoaded);
            return;
        }

        let gain = self.amplify_factor;
        let is_new_session = self.live_amplify_selection != Some(selection)
            || self.live_amplify_source.is_none();

        if is_new_session {
            let result = {
                let Some(document) = self.document.as_mut() else {
                    self.set_status_error(&WaveSculptorError::NoAudioLoaded);
                    return;
                };
                let source = document.buffer().clone();
                match document.apply_edit("放大", move |buffer| amplify_selection(buffer, selection, gain)) {
                    Ok(()) => Ok(source),
                    Err(error) => Err(error),
                }
            };

            match result {
                Ok(source) => {
                    self.live_amplify_source = Some(source);
                    self.live_amplify_selection = Some(selection);
                    self.player.stop();
                    self.refresh_analysis();
                    self.set_status_ok(format!("已将选区调整为 {:.2} 倍音量。", gain));
                }
                Err(error) => self.set_status_error(&error),
            }
            return;
        }

        let Some(source) = self.live_amplify_source.clone() else {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        };

        let mut next = source;
        let result = amplify_selection(&mut next, selection, gain);
        match result {
            Ok(()) => {
                if let Some(document) = self.document.as_mut() {
                    document.overwrite_buffer(next);
                }
                self.player.stop();
                self.refresh_analysis();
                self.set_status_ok(format!("已将选区调整为 {:.2} 倍音量。", gain));
            }
            Err(error) => self.set_status_error(&error),
        }
    }

    fn commit_amplify_factor_if_needed(&mut self, response: &egui::Response, had_selection: bool) {
        if !response.changed() {
            return;
        }

        if !had_selection {
            self.set_status_error(&WaveSculptorError::InvalidSelection);
            return;
        }

        self.apply_live_amplify();
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
        let palette = self.palette();
        let has_audio = self.current_buffer().is_some();
        let has_selection = self.has_selection_state();
        let can_undo = self.document.as_ref().map(AudioDocument::can_undo).unwrap_or(false);
        let can_redo = self.document.as_ref().map(AudioDocument::can_redo).unwrap_or(false);
        let first_row = [
            ToolbarGroupKind::File,
            ToolbarGroupKind::Playback,
            ToolbarGroupKind::Theme,
        ];
        let second_row = [ToolbarGroupKind::Edit, ToolbarGroupKind::Process];
        let aligned_row_width =
            toolbar_groups_width(&first_row).max(toolbar_groups_width(&second_row));

        Frame::none()
            .fill(palette.panel_bg)
            .inner_margin(Margin::same(TOOLBAR_OUTER_PADDING))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(TOOLBAR_GROUP_SPACING, TOOLBAR_GROUP_SPACING);
                self.render_toolbar_aligned_row(
                    ui,
                    playback,
                    &first_row,
                    aligned_row_width,
                    has_audio,
                    has_selection,
                    can_undo,
                    can_redo,
                );
                ui.add_space(TOOLBAR_GROUP_SPACING);
                self.render_toolbar_aligned_row(
                    ui,
                    playback,
                    &second_row,
                    aligned_row_width,
                    has_audio,
                    has_selection,
                    can_undo,
                    can_redo,
                );
            });
    }

    fn render_info_panel(&self, ui: &mut egui::Ui, playback: PlaybackStatus) {
        let palette = self.palette();
        Frame::none()
            .fill(palette.panel_bg)
            .inner_margin(Margin::same(6.0))
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing =
                            egui::vec2(TOOLBAR_GROUP_SPACING, TOOLBAR_GROUP_SPACING);
                        ui.vertical(|ui| {
                            info_card(ui, palette, "音频信息", |ui| {
                                if let Some(buffer) = self.current_buffer() {
                                    let file_name = self
                                        .file_path
                                        .as_deref()
                                        .map(display_file_name)
                                        .unwrap_or_else(|| "（未知）".to_string());
                                    kv_row(ui, palette, "文件", &file_name);
                                    kv_row(ui, palette, "采样率", &format!("{} 赫兹", buffer.sample_rate));
                                    kv_row(ui, palette, "声道数", &buffer.channels.to_string());
                                    kv_row(ui, palette, "位深", &format!("{} 位", buffer.bits_per_sample));
                                    kv_row(ui, palette, "时长", &format_duration(buffer.duration_seconds()));
                                    kv_row(ui, palette, "采样点总数", &buffer.total_samples().to_string());
                                    kv_row(ui, palette, "帧数", &buffer.frame_count().to_string());
                                    kv_row(
                                        ui,
                                        palette,
                                        "播放进度",
                                        &format!(
                                            "{} / {}",
                                            format_duration(playback.current_seconds),
                                            format_duration(buffer.duration_seconds())
                                        ),
                                    );
                                } else {
                                    ui.label(subtle_text("未加载文件。", palette));
                                }
                            });

                            info_card(ui, palette, "选区", |ui| {
                                if let (Some(buffer), Some(selection)) =
                                    (self.current_buffer(), self.current_selection())
                                {
                                    kv_row(ui, palette, "起点", &format_duration(selection.start_seconds(buffer)));
                                    kv_row(ui, palette, "终点", &format_duration(selection.end_seconds(buffer)));
                                    kv_row(ui, palette, "长度", &format_duration(selection.duration_seconds(buffer)));
                                    kv_row(
                                        ui,
                                        palette,
                                        "帧范围",
                                        &format!("{} - {}", selection.start_frame, selection.end_frame),
                                    );
                                } else {
                                    ui.label(subtle_text("当前没有有效选区。", palette));
                                }
                            });

                            info_card(ui, palette, "分析", |ui| {
                                if let Some(analysis) = &self.analysis {
                                    kv_row(ui, palette, "峰值振幅", &format!("{:.2}%", analysis.peak * 100.0));
                                    kv_row(ui, palette, "均方根", &format!("{:.4}", analysis.rms));
                                    kv_row(ui, palette, "削波采样点", &analysis.clipping_samples.to_string());
                                    kv_row(ui, palette, "静音片段数", &analysis.silent_segments.len().to_string());
                                } else {
                                    ui.label(subtle_text("暂无分析结果。", palette));
                                }
                            });
                        });
                    });
            });
    }

    fn render_waveform_panel(&mut self, ui: &mut egui::Ui, playback: PlaybackStatus) {
        let palette = self.palette();
        Frame::none()
            .fill(palette.panel_bg)
            .inner_margin(Margin::same(6.0))
            .show(ui, |ui| {
                Frame::none()
                    .fill(palette.card_bg)
                    .stroke(Stroke::new(1.0, palette.card_border))
                    .rounding(egui::Rounding::same(12.0))
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("波形")
                                        .size(19.0)
                                        .strong()
                                        .color(palette.text_primary),
                                );
                                ui.label(subtle_text("支持缩放、平移、选区高亮与播放进度线", palette));
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if action_button_sized(ui, "适配视图", self.current_buffer().is_some(), 92.0)
                                    .clicked()
                                {
                                    self.fit_view_requested = true;
                                }
                                if action_button_sized(ui, "→ 向右", self.current_buffer().is_some(), 82.0)
                                    .clicked()
                                {
                                    if let Some(document) = self.document.as_ref() {
                                        self.viewport
                                            .pan_by_fraction(document.buffer(), self.last_waveform_width, 0.25);
                                    }
                                }
                                if action_button_sized(ui, "← 向左", self.current_buffer().is_some(), 82.0)
                                    .clicked()
                                {
                                    if let Some(document) = self.document.as_ref() {
                                        self.viewport
                                            .pan_by_fraction(document.buffer(), self.last_waveform_width, -0.25);
                                    }
                                }
                                ui.allocate_ui_with_layout(
                                    egui::vec2(160.0, ACTION_BUTTON_HEIGHT),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.spacing_mut().interact_size.y = ACTION_BUTTON_HEIGHT;
                                        egui::ComboBox::from_id_source("waveform_mode")
                                            .width(144.0)
                                            .selected_text(self.waveform_mode.label())
                                            .show_ui(ui, |ui| {
                                                for mode in WaveformMode::ALL {
                                                    ui.selectable_value(
                                                        &mut self.waveform_mode,
                                                        mode,
                                                        mode.label(),
                                                    );
                                                }
                                            });
                                    },
                                );
                            });
                        });
                        ui.add_space(8.0);
                        self.render_waveform(ui, playback, palette);
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(6.0);
                        self.render_status_bar(ui, playback);
                    });
            });
    }

    fn render_toolbar_aligned_row(
        &mut self,
        ui: &mut egui::Ui,
        playback: PlaybackStatus,
        groups: &[ToolbarGroupKind],
        aligned_width: f32,
        has_audio: bool,
        has_selection: bool,
        can_undo: bool,
        can_redo: bool,
    ) {
        let intrinsic_width = toolbar_groups_width(groups);
        let available_width = ui.available_width();

        if available_width < intrinsic_width {
            self.render_toolbar_row(
                ui,
                playback,
                groups,
                has_audio,
                has_selection,
                can_undo,
                can_redo,
            );
            return;
        }

        let row_width = aligned_width.min(available_width);
        let side_padding = ((row_width - intrinsic_width) * 0.5).max(0.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(TOOLBAR_GROUP_SPACING, TOOLBAR_GROUP_SPACING);
            for &group in groups {
                self.render_toolbar_group(
                    ui,
                    playback,
                    group,
                    has_audio,
                    has_selection,
                    can_undo,
                    can_redo,
                );
            }
            if side_padding > 0.0 {
                ui.add_space(side_padding * 2.0);
            }
        });
    }

    fn render_toolbar_row(
        &mut self,
        ui: &mut egui::Ui,
        playback: PlaybackStatus,
        groups: &[ToolbarGroupKind],
        has_audio: bool,
        has_selection: bool,
        can_undo: bool,
        can_redo: bool,
    ) {
        let available_width = ui.available_width();
        let mut line_groups = Vec::new();
        let mut used_width = 0.0;
        let mut rendered_any_line = false;

        for &group in groups {
            let group_width = group.width();
            let next_width = if line_groups.is_empty() {
                group_width
            } else {
                used_width + TOOLBAR_GROUP_SPACING + group_width
            };

            if !line_groups.is_empty() && next_width > available_width {
                if rendered_any_line {
                    ui.add_space(TOOLBAR_GROUP_SPACING);
                }
                self.render_toolbar_line(
                    ui,
                    playback,
                    &line_groups,
                    has_audio,
                    has_selection,
                    can_undo,
                    can_redo,
                );
                rendered_any_line = true;
                line_groups.clear();
                line_groups.push(group);
                used_width = group_width;
            } else {
                used_width = next_width;
                line_groups.push(group);
            }
        }

        if !line_groups.is_empty() {
            if rendered_any_line {
                ui.add_space(TOOLBAR_GROUP_SPACING);
            }
            self.render_toolbar_line(
                ui,
                playback,
                &line_groups,
                has_audio,
                has_selection,
                can_undo,
                can_redo,
            );
        }
    }

    fn render_toolbar_line(
        &mut self,
        ui: &mut egui::Ui,
        playback: PlaybackStatus,
        groups: &[ToolbarGroupKind],
        has_audio: bool,
        has_selection: bool,
        can_undo: bool,
        can_redo: bool,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(TOOLBAR_GROUP_SPACING, TOOLBAR_GROUP_SPACING);
            for &group in groups {
                self.render_toolbar_group(
                    ui,
                    playback,
                    group,
                    has_audio,
                    has_selection,
                    can_undo,
                    can_redo,
                );
            }
        });
    }

    fn render_toolbar_group(
        &mut self,
        ui: &mut egui::Ui,
        playback: PlaybackStatus,
        group: ToolbarGroupKind,
        has_audio: bool,
        has_selection: bool,
        can_undo: bool,
        can_redo: bool,
    ) {
        let palette = self.palette();

        match group {
            ToolbarGroupKind::File => {
                control_group_fixed(
                    ui,
                    palette,
                    "文件",
                    group.width(),
                    TOOLBAR_GROUP_HEIGHT,
                    |ui| {
                        if action_button(ui, "📂 打开", true).clicked() {
                            self.open_audio();
                        }
                        if action_button(ui, "💾 另存为", has_audio).clicked() {
                            self.save_audio_as();
                        }
                        if action_button_sized(ui, "📤 导出选区", has_selection, 104.0).clicked() {
                            self.export_selection();
                        }
                    },
                );
            }
            ToolbarGroupKind::Playback => {
                control_group_fixed(
                    ui,
                    palette,
                    "播放",
                    group.width(),
                    TOOLBAR_GROUP_HEIGHT,
                    |ui| {
                        if action_button(ui, "▶ 播放", has_audio).clicked() {
                            self.play_current();
                        }
                        if action_button(ui, "⏹ 停止", playback.is_playing).clicked() {
                            self.stop_playback();
                        }
                        if action_button(ui, "🔇 静音", has_selection).clicked() {
                            self.mute_selection();
                        }
                        let amplify_response = value_control_sized(
                            ui,
                            palette,
                            "音量",
                            118.0,
                            has_selection,
                            |ui| {
                                ui.add(
                                    egui::DragValue::new(&mut self.amplify_factor)
                                        .speed(0.1)
                                        .clamp_range(0.0..=8.0)
                                        .fixed_decimals(1)
                                        .prefix("× "),
                                )
                            },
                        );
                        self.commit_amplify_factor_if_needed(&amplify_response, has_selection);
                    },
                );
            }
            ToolbarGroupKind::Theme => {
                control_group_fixed(
                    ui,
                    palette,
                    "主题",
                    group.width(),
                    TOOLBAR_GROUP_HEIGHT,
                    |ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(162.0, ACTION_BUTTON_HEIGHT),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.spacing_mut().interact_size.y = ACTION_BUTTON_HEIGHT;
                                egui::ComboBox::from_id_source("theme_mode")
                                    .width(146.0)
                                    .selected_text(self.theme_mode.label())
                                    .show_ui(ui, |ui| {
                                        for mode in ThemeMode::ALL {
                                            ui.selectable_value(&mut self.theme_mode, mode, mode.label());
                                        }
                                    });
                            },
                        );
                    },
                );
            }
            ToolbarGroupKind::Edit => {
                control_group_fixed(
                    ui,
                    palette,
                    "编辑",
                    group.width(),
                    TOOLBAR_GROUP_HEIGHT,
                    |ui| {
                        if action_button(ui, "↶ 撤销", can_undo).clicked() {
                            self.undo();
                        }
                        if action_button(ui, "↷ 重做", can_redo).clicked() {
                            self.redo();
                        }
                        if action_button_sized(ui, "🔄 反转选区", has_selection, 96.0).clicked() {
                            self.reverse_selection();
                        }
                        if action_button_sized(ui, "✂ 裁剪为选区", has_selection, 104.0).clicked() {
                            self.cut_to_selection();
                        }
                        if action_button_sized(ui, "🧹 清除选区", has_selection, 104.0).clicked() {
                            self.clear_selection();
                            self.suppress_waveform_input_once = true;
                            self.set_status_ok("已清除选区。");
                        }
                    },
                );
            }
            ToolbarGroupKind::Process => {
                control_group_fixed(
                    ui,
                    palette,
                    "处理",
                    group.width(),
                    TOOLBAR_GROUP_HEIGHT,
                    |ui| {
                        if action_button_sized(ui, "📏 归一化", has_audio, 90.0).clicked() {
                            self.normalize_audio();
                        }
                        if action_button(ui, "⤴ 淡入", has_selection).clicked() {
                            self.fade_in_selection();
                        }
                        if action_button(ui, "⤵ 淡出", has_selection).clicked() {
                            self.fade_out_selection();
                        }
                        if action_button_sized(ui, "🪄 去除首尾静音", has_audio, 132.0).clicked() {
                            self.trim_silence();
                        }
                    },
                );
            }
        }
    }

    fn render_waveform(&mut self, ui: &mut egui::Ui, playback: PlaybackStatus, palette: ThemePalette) {
        if self.current_buffer().is_none() {
            ui.vertical(|ui| {
                ui.add_space(16.0);
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
                palette,
                current_selection,
                playback.playhead_frame,
            )
        };

        self.last_waveform_width = waveform.plot_rect.width();

        if self.fit_view_requested {
            if let Some(document) = self.document.as_ref() {
                self.viewport
                    .fit_to_audio(document.buffer(), waveform.plot_rect.width());
                self.fit_view_requested = false;
            }
        }

        if waveform.response.hovered() {
            let scroll_delta = ui.input(|input| input.raw_scroll_delta.y);
            if scroll_delta.abs() > f32::EPSILON {
                let anchor_x = waveform
                    .response
                    .hover_pos()
                    .map(|pos| pos.x - waveform.plot_rect.left())
                    .unwrap_or(waveform.plot_rect.width() * 0.5);
                let zoom_factor = if scroll_delta > 0.0 { 0.85 } else { 1.15 };
                if let Some(document) = self.document.as_ref() {
                    self.viewport.zoom_around(
                        document.buffer(),
                        waveform.plot_rect.width(),
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
                let frame = x_to_frame(pointer_pos.x, waveform.plot_rect, &waveform.visible_frame_range);
                self.drag_anchor = Some(frame);
                self.click_anchor = None;
                let next_selection = Selection::new(frame, (frame + 1).min(frame_count)).ok();
                if self.selection != next_selection {
                    self.reset_live_amplify();
                }
                self.selection = next_selection;
            }
        }

        if let Some(anchor) = self.drag_anchor {
            if pointer_primary_down {
                if let Some(pointer_pos) = waveform.response.interact_pointer_pos() {
                    let current = x_to_frame(pointer_pos.x, waveform.plot_rect, &waveform.visible_frame_range);
                    let start = anchor.min(current);
                    let end = anchor.max(current).saturating_add(1).min(frame_count);
                    if let Some(buffer) = self.current_buffer() {
                        let next_selection = Selection::from_frame_bounds(buffer, start, end).ok();
                        if self.selection != next_selection {
                            self.reset_live_amplify();
                        }
                        self.selection = next_selection;
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
                    .pan_pixels(document.buffer(), waveform.plot_rect.width(), -delta);
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
            .map(|pointer_pos| x_to_frame(pointer_pos.x, waveform.plot_rect, &waveform.visible_frame_range));

        if let Some(frame) = clicked_frame {
            match self.click_anchor {
                Some(anchor) if anchor != frame => {
                    let start = anchor.min(frame);
                    let end = anchor.max(frame).saturating_add(1).min(frame_count);
                    if let Some(buffer) = self.current_buffer() {
                        let next_selection = Selection::from_frame_bounds(buffer, start, end).ok();
                        if self.selection != next_selection {
                            self.reset_live_amplify();
                        }
                        self.selection = next_selection;
                    }
                    self.click_anchor = None;
                    self.set_status_ok("已通过点击更新选区。");
                }
                _ => {
                    self.click_anchor = Some(frame);
                    let next_selection = Selection::new(frame, (frame + 1).min(frame_count)).ok();
                    if self.selection != next_selection {
                        self.reset_live_amplify();
                    }
                    self.selection = next_selection;
                    self.set_status_ok("已设置选区起点。请再点击另一处，或拖动以完成选择。");
                }
            }
        }
    }

    fn render_status_bar(&self, ui: &mut egui::Ui, playback: PlaybackStatus) {
        let palette = self.palette();
        let selection_hint = self
            .current_selection()
            .and_then(|selection| self.current_buffer().map(|buffer| selection.duration_seconds(buffer)))
            .map(|seconds| format!("选区长度 {}", format_duration(seconds)))
            .unwrap_or_else(|| "当前无有效选区".to_string());
        let playback_hint = self
            .current_buffer()
            .map(|buffer| {
                format!(
                    "播放 {} / {}",
                    format_duration(playback.current_seconds),
                    format_duration(buffer.duration_seconds())
                )
            })
            .unwrap_or_else(|| "未加载音频".to_string());

        ui.scope(|ui| {
            ui.visuals_mut().override_text_color = Some(palette.text_primary);
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
            let row_width = ui.available_width();
            ui.allocate_ui_with_layout(
                egui::vec2(row_width, INFO_ROW_HEIGHT),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.columns(3, |columns| {
                        let status_width = columns[0].available_width();
                        columns[0].add_sized(
                            [status_width, INFO_ROW_HEIGHT],
                            Label::new(RichText::new(&self.status_message).color(palette.text_primary))
                                .truncate(true),
                        );
                        columns[1].with_layout(Layout::left_to_right(Align::Min), |ui| {
                            let playback_width = ui.available_width();
                            ui.add_sized(
                                [playback_width, INFO_ROW_HEIGHT],
                                Label::new(subtle_text(&playback_hint, palette)).truncate(true),
                            );
                        });
                        columns[2].with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let selection_width = ui.available_width();
                            ui.add_sized(
                                [selection_width, INFO_ROW_HEIGHT],
                                Label::new(subtle_text(&selection_hint, palette)).truncate(true),
                            );
                        });
                    });
                },
            );
        });
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx, self.theme_mode);
        let playback = self.player.status();
        if playback.is_playing {
            ctx.request_repaint();
        }

        let palette = self.palette();
        egui::CentralPanel::default()
            .frame(
                Frame::none()
                    .fill(palette.app_bg)
                    .inner_margin(Margin::same(APP_OUTER_PADDING)),
            )
            .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(CONTENT_PANEL_SPACING, 0.0);
                let total_width = ui.available_width();
                let info_width = INFO_PANEL_WIDTH.min((total_width - 220.0).max(260.0));
                let left_width = (total_width - info_width - CONTENT_PANEL_SPACING).max(220.0);
                let available_height = ui.available_height();

                ui.allocate_ui_with_layout(
                    egui::vec2(left_width, available_height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(left_width);
                        self.render_top_bar(ui, playback);
                        ui.add_space(CONTENT_PANEL_SPACING);
                        self.render_waveform_panel(ui, playback);
                    },
                );
                ui.add_space(CONTENT_PANEL_SPACING);
                ui.allocate_ui_with_layout(
                    egui::vec2(info_width, available_height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(info_width);
                        self.render_info_panel(ui, playback);
                    },
                );
            });
            });
    }
}

fn control_group_fixed(
    ui: &mut egui::Ui,
    palette: ThemePalette,
    title: &str,
    width: f32,
    height: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        Layout::left_to_right(Align::Center),
        |ui| {
            Frame::none()
                .fill(palette.card_bg)
                .stroke(Stroke::new(1.0, palette.card_border))
                .rounding(egui::Rounding::same(10.0))
                .inner_margin(Margin::same(8.0))
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(
                        (width - 16.0).max(0.0),
                        (height - 16.0).max(ACTION_BUTTON_HEIGHT),
                    ));
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add_sized(
                            [GROUP_TITLE_WIDTH, ACTION_BUTTON_HEIGHT],
                            Label::new(RichText::new(title).strong().color(palette.text_primary)),
                        );
                        ui.separator();
                        ui.add_space(2.0);
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            add_contents(ui);
                        });
                    });
                });
        },
    );
}

fn info_card(
    ui: &mut egui::Ui,
    palette: ThemePalette,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    Frame::none()
        .fill(palette.card_bg)
        .stroke(Stroke::new(1.0, palette.card_border))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(Margin::same(10.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .strong()
                    .size(15.0)
                    .color(palette.text_primary),
            );
            ui.add_space(6.0);
            add_contents(ui);
        });
}

fn kv_row(ui: &mut egui::Ui, palette: ThemePalette, key: &str, value: &str) {
    let row_width = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(row_width, INFO_ROW_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
            ui.add_sized([INFO_KEY_WIDTH, INFO_ROW_HEIGHT], Label::new(subtle_text(key, palette)));
            let value_width = ui.available_width().max(0.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_sized(
                    [value_width, INFO_ROW_HEIGHT],
                    Label::new(
                        RichText::new(value)
                            .color(palette.text_primary)
                            .font(FontId::proportional(14.0))
                            .strong(),
                    )
                    .truncate(true),
                );
            });
        },
    );
}

fn subtle_text(text: &str, palette: ThemePalette) -> RichText {
    RichText::new(text).color(palette.text_secondary)
}

fn action_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    action_button_sized(ui, label, enabled, ACTION_BUTTON_WIDTH)
}

fn action_button_sized(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    width: f32,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| {
        ui.add_sized([width, ACTION_BUTTON_HEIGHT], egui::Button::new(label))
    })
    .inner
}

fn value_control_sized(
    ui: &mut egui::Ui,
    palette: ThemePalette,
    label: &str,
    width: f32,
    enabled: bool,
    add_widget: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    let fill = if enabled {
        palette.card_alt_bg
    } else {
        palette.disabled_bg
    };
    let mut inner_response = None;

    ui.allocate_ui_with_layout(
        egui::vec2(width, ACTION_BUTTON_HEIGHT),
        Layout::left_to_right(Align::Center),
        |ui| {
            Frame::none()
                .fill(fill)
                .stroke(Stroke::new(1.0, palette.card_border))
                .rounding(egui::Rounding::same(10.0))
                .inner_margin(Margin::symmetric(8.0, 5.0))
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(
                        (width - 16.0).max(0.0),
                        (ACTION_BUTTON_HEIGHT - 10.0).max(0.0),
                    ));
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                    ui.spacing_mut().interact_size.y = ACTION_BUTTON_HEIGHT;
                    ui.add_sized(
                        [34.0, ACTION_BUTTON_HEIGHT],
                        Label::new(subtle_text(label, palette)),
                    );
                    inner_response = Some(ui.add_enabled_ui(enabled, add_widget).inner);
                });
        },
    );

    inner_response.unwrap_or_else(|| {
        ui.allocate_response(egui::vec2(width, ACTION_BUTTON_HEIGHT), egui::Sense::hover())
    })
}

fn toolbar_groups_width(groups: &[ToolbarGroupKind]) -> f32 {
    groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let spacing = if index == 0 { 0.0 } else { TOOLBAR_GROUP_SPACING };
            spacing + group.width()
        })
        .sum()
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
