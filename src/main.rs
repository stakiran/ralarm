#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod fonts;
mod sound;

use core::{hhmm_to_display, move_to_front, next_alarm_label, parse_alarms, Alarm};

use chrono::{Local, Timelike};
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const POPUP_W: f32 = 300.0;
const POPUP_H: f32 = 150.0;

/// 発火中のポップアップ1つ分。
struct Popup {
    id: u64,
    hhmm: String,
    title: String,
    stop: Arc<AtomicBool>,
    pos: egui::Pos2,
}

struct AlarmApp {
    alarms_path: PathBuf,
    log_path: PathBuf,

    /// 発火判定に使う内部データ（ファイル由来）。
    alarms: Vec<Alarm>,
    /// アプリ内エディタの内容（変更されたら即自動保存）。
    editor_text: String,
    last_mtime: Option<SystemTime>,

    /// 履歴（log.txt、MRU順）。
    log_lines: Vec<String>,
    show_history: bool,

    /// (日付, hhmm, title) の発火済みセット。日付が変わったら掃除。
    fired: HashSet<(String, String, String)>,
    /// 直近ティックの hhmm。分が変わった瞬間の検出に使う。
    last_minute: String,

    popups: Vec<Popup>,
    next_popup_id: u64,
}

impl AlarmApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        fonts::install_japanese_font(&cc.egui_ctx);

        let dir = exe_dir();
        let alarms_path = dir.join("alarms.txt");
        let log_path = dir.join("log.txt");

        // alarms.txt が無ければ空で作成。
        if !alarms_path.exists() {
            let _ = std::fs::write(&alarms_path, "");
        }

        let editor_text = std::fs::read_to_string(&alarms_path).unwrap_or_default();
        let alarms = parse_alarms(&editor_text);
        let last_mtime = std::fs::metadata(&alarms_path)
            .and_then(|m| m.modified())
            .ok();

        let log_lines = std::fs::read_to_string(&log_path)
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();

        // 起動時の分は「発火済み扱い」にして、過去のアラームを遡らせない。
        let now = Local::now();
        let last_minute = format!("{:02}{:02}", now.hour(), now.minute());

        Self {
            alarms_path,
            log_path,
            alarms,
            editor_text,
            last_mtime,
            log_lines,
            show_history: false,
            fired: HashSet::new(),
            last_minute,
            popups: Vec::new(),
            next_popup_id: 0,
        }
    }

    /// エディタの内容を alarms.txt に自動保存。
    fn save_alarms(&mut self) {
        let _ = std::fs::write(&self.alarms_path, &self.editor_text);
        self.alarms = parse_alarms(&self.editor_text);
        self.last_mtime = std::fs::metadata(&self.alarms_path)
            .and_then(|m| m.modified())
            .ok();
    }

    fn save_log(&self) {
        let mut s = self.log_lines.join("\n");
        if !s.is_empty() {
            s.push('\n');
        }
        let _ = std::fs::write(&self.log_path, s);
    }

    /// アプリ外編集の検知（mtimeポーリング）。自動保存方式なので
    /// 外部の方が新しければそのままエディタにも反映する。
    fn poll_external_edit(&mut self) {
        let Ok(meta) = std::fs::metadata(&self.alarms_path) else {
            return;
        };
        let Ok(mtime) = meta.modified() else {
            return;
        };
        if Some(mtime) == self.last_mtime {
            return;
        }
        self.last_mtime = Some(mtime);
        if let Ok(content) = std::fs::read_to_string(&self.alarms_path) {
            self.alarms = parse_alarms(&content);
            self.editor_text = content;
        }
    }

    /// 分が変わった瞬間に一致アラームを発火。
    fn tick(&mut self, ctx: &egui::Context) {
        let now = Local::now();
        let cur = format!("{:02}{:02}", now.hour(), now.minute());
        if cur == self.last_minute {
            return;
        }
        self.last_minute = cur.clone();

        let today = now.format("%Y-%m-%d").to_string();
        // 日付が変わっていたら古い発火済みを掃除。
        self.fired.retain(|(d, _, _)| *d == today);

        let matching: Vec<Alarm> = self
            .alarms
            .iter()
            .filter(|a| a.hhmm == cur)
            .cloned()
            .collect();
        for a in matching {
            let key = (today.clone(), a.hhmm.clone(), a.title.clone());
            if self.fired.insert(key) {
                self.fire(ctx, &a, false);
            }
        }
    }

    /// ポップアップ表示＋音再生。is_test なら log.txt に記録しない。
    fn fire(&mut self, ctx: &egui::Context, alarm: &Alarm, is_test: bool) {
        if !is_test {
            self.log_lines = move_to_front(&self.log_lines, &alarm.to_line());
            self.save_log();
        }

        let stop = Arc::new(AtomicBool::new(false));
        sound::spawn_beeper(stop.clone());

        let pos = self.next_popup_pos(ctx);
        let id = self.next_popup_id;
        self.next_popup_id += 1;
        self.popups.push(Popup {
            id,
            hhmm: alarm.hhmm.clone(),
            title: alarm.title.clone(),
            stop,
            pos,
        });
    }

    fn next_popup_pos(&self, ctx: &egui::Context) -> egui::Pos2 {
        let monitor = ctx
            .input(|i| i.viewport().monitor_size)
            .unwrap_or(egui::vec2(1280.0, 800.0));
        let index = self.popups.len() as f32;
        let x = (monitor.x - POPUP_W - 20.0).max(0.0);
        let y = 20.0 + index * (POPUP_H + 20.0);
        egui::pos2(x, y)
    }

    fn insert_history_line(&mut self, line: &str) {
        if !self.editor_text.is_empty() && !self.editor_text.ends_with('\n') {
            self.editor_text.push('\n');
        }
        self.editor_text.push_str(line);
        self.editor_text.push('\n');
        self.save_alarms();
    }

    fn draw_popups(&mut self, ctx: &egui::Context) {
        let mut to_remove: Vec<u64> = Vec::new();
        for popup in &self.popups {
            let vid = egui::ViewportId::from_hash_of(popup.id);
            let builder = egui::ViewportBuilder::default()
                .with_title("アラーム")
                .with_inner_size([POPUP_W, POPUP_H])
                .with_position(popup.pos)
                .with_always_on_top()
                .with_resizable(false);

            let mut close = false;
            ctx.show_viewport_immediate(vid, builder, |ctx, _class| {
                let frame = egui::Frame::default()
                    .fill(egui::Color32::from_rgb(160, 20, 20))
                    .inner_margin(egui::Margin::same(12));
                egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(hhmm_to_display(&popup.hhmm))
                                .size(44.0)
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        if !popup.title.is_empty() {
                            ui.label(
                                egui::RichText::new(&popup.title)
                                    .size(20.0)
                                    .color(egui::Color32::WHITE),
                            );
                        }
                        ui.add_space(10.0);
                        if ui.button("閉じる（音停止）").clicked() {
                            close = true;
                        }
                    });
                });
                if ctx.input(|i| i.viewport().close_requested()) {
                    close = true;
                }
            });

            if close {
                popup.stop.store(true, Ordering::Relaxed);
                to_remove.push(popup.id);
            }
        }
        if !to_remove.is_empty() {
            self.popups.retain(|p| !to_remove.contains(&p.id));
        }
    }
}

impl eframe::App for AlarmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 最小化中でもティックが回るように毎フレーム再描画予約。
        ctx.request_repaint_after(Duration::from_millis(300));

        self.poll_external_edit();
        self.tick(ctx);

        let now = Local::now();
        let now_hhmm = format!("{:02}{:02}", now.hour(), now.minute());

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(now.format("%H:%M:%S").to_string())
                        .size(28.0)
                        .strong(),
                );
                ui.add_space(16.0);
                ui.label(egui::RichText::new(next_alarm_label(&self.alarms, &now_hhmm)).size(18.0));
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("履歴").clicked() {
                    self.show_history = !self.show_history;
                }
                if ui.button("テスト").clicked() {
                    let a = Alarm {
                        hhmm: now_hhmm.clone(),
                        title: "テスト".to_string(),
                    };
                    self.fire(ctx, &a, true);
                }
            });

            ui.separator();
            ui.label("alarms.txt（1行 = hhmm [タイトル]）");

            egui::ScrollArea::vertical().show(ui, |ui| {
                let resp = ui.add_sized(
                    [ui.available_width(), ui.available_height()],
                    egui::TextEdit::multiline(&mut self.editor_text)
                        .desired_rows(12)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                if resp.changed() {
                    self.save_alarms();
                }
            });
        });

        // 履歴ウィンドウ。
        if self.show_history {
            let mut open = true;
            let mut pick: Option<String> = None;
            egui::Window::new("履歴")
                .open(&mut open)
                .default_width(260.0)
                .show(ctx, |ui| {
                    ui.label("クリックで alarms.txt に挿入");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.log_lines.is_empty() {
                            ui.label("（履歴なし）");
                        }
                        for line in &self.log_lines {
                            if ui.button(line).clicked() {
                                pick = Some(line.clone());
                            }
                        }
                    });
                });
            if let Some(line) = pick {
                self.insert_history_line(&line);
            }
            self.show_history = open;
        }

        self.draw_popups(ctx);
    }
}

/// 実行ファイルの親ディレクトリ。取れなければカレントディレクトリ。
fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Alarms")
            .with_inner_size([480.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Alarms",
        options,
        Box::new(|cc| Ok(Box::new(AlarmApp::new(cc)))),
    )
}
