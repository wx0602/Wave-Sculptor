use std::path::{Path, PathBuf};

use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use image::ImageReader;

use crate::gui::theme::ThemeMode;

pub enum BackgroundKind {
    Main,
    Bar,
}

pub struct CardBackgrounds {
    dark: TextureHandle,
    light: TextureHandle,
    dark_bar: TextureHandle,
    light_bar: TextureHandle,
}

impl CardBackgrounds {
    pub fn load(ctx: &Context) -> Result<Self, String> {
        // 启动后懒加载纹理，失败时界面仍可退回纯色卡片。
        Ok(Self {
            dark: load_texture(ctx, "card_bg_dark", &asset_path("dark.png"))?,
            light: load_texture(ctx, "card_bg_light", &asset_path("light.png"))?,
            dark_bar: load_texture(ctx, "card_bg_dark_bar", &asset_path("dark_bar.png"))?,
            light_bar: load_texture(ctx, "card_bg_light_bar", &asset_path("light_bar.png"))?,
        })
    }

    pub fn get(&self, theme: ThemeMode, kind: BackgroundKind) -> &TextureHandle {
        match (theme, kind) {
            (ThemeMode::Dark, BackgroundKind::Main) => &self.dark,
            (ThemeMode::Light, BackgroundKind::Main) => &self.light,
            (ThemeMode::Dark, BackgroundKind::Bar) => &self.dark_bar,
            (ThemeMode::Light, BackgroundKind::Bar) => &self.light_bar,
        }
    }
}

fn asset_path(name: &str) -> PathBuf {
    // 资源路径基于 Cargo manifest，避免受运行目录影响。
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(name)
}

fn load_texture(ctx: &Context, name: &str, path: &Path) -> Result<TextureHandle, String> {
    // egui 纹理使用 RGBA8，image crate 负责识别 PNG 等具体格式。
    let reader =
        ImageReader::open(path).map_err(|error| format!("无法打开 {}：{error}", path.display()))?;
    let rgba = reader
        .with_guessed_format()
        .map_err(|error| format!("无法识别 {} 的图片格式：{error}", path.display()))?
        .decode()
        .map_err(|error| format!("无法解码 {}：{error}", path.display()))?
        .to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Ok(ctx.load_texture(name, image, TextureOptions::LINEAR))
}
