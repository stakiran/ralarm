//! 日本語フォントの読み込み。egui標準フォントはCJK非対応なので、
//! OS標準フォントを読み込んでフォールバック（families末尾）に追加する。
//! 全滅してもクラッシュせず、英数のみ表示で続行する。

use eframe::egui;

#[cfg(target_os = "windows")]
const CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\meiryo.ttc",
    r"C:\Windows\Fonts\YuGothM.ttc",
    r"C:\Windows\Fonts\msgothic.ttc",
];

#[cfg(target_os = "macos")]
const CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/Supplemental/Osaka.ttf",
];

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const CANDIDATES: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
];

pub fn install_japanese_font(ctx: &egui::Context) {
    let mut bytes = None;
    for path in CANDIDATES {
        if let Ok(data) = std::fs::read(path) {
            bytes = Some(data);
            break;
        }
    }
    let Some(data) = bytes else {
        return; // 全滅：デフォルトフォントのまま続行
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(
            "jp".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(data)),
        );

    // 末尾に追加してフォールバックとして使う（先頭に挿すと英数の見た目が変わる）。
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("jp".to_owned());
    }

    ctx.set_fonts(fonts);
}
