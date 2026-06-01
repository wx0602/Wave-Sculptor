use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

pub fn configure_fonts(ctx: &egui::Context) {
    let Some(font_bytes) = load_system_cjk_font() else {
        return;
    };

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "system_cjk".to_string(),
        FontData::from_owned(font_bytes).into(),
    );

    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "system_cjk".to_string());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.push("system_cjk".to_string());
    }

    ctx.set_fonts(fonts);
}

fn load_system_cjk_font() -> Option<Vec<u8>> {
    let windows_dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));

    let candidates = [
        windows_dir.join(r"Fonts\simhei.ttf"),
        windows_dir.join(r"Fonts\Deng.ttf"),
        windows_dir.join(r"Fonts\msyh.ttc"),
        windows_dir.join(r"Fonts\msyh.ttf"),
        windows_dir.join(r"Fonts\simsun.ttc"),
        windows_dir.join(r"Fonts\simkai.ttf"),
        PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc"),
        PathBuf::from("/System/Library/Fonts/PingFang.ttc"),
        PathBuf::from("/System/Library/Fonts/Hiragino Sans GB.ttc"),
    ];

    candidates
        .iter()
        .find(|path| path.exists())
        .and_then(|path| read_font_file(path))
}

fn read_font_file(path: &Path) -> Option<Vec<u8>> {
    fs::read(path).ok()
}
