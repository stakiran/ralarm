//! ビープ音再生。音声ファイル不要（SineWave生成）。
//! ポップアップごとに1スレッド。stopフラグで即停止。音声デバイスが無くてもpanicしない。

use rodio::source::{SineWave, Source};
use rodio::{OutputStream, Sink};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// stop が true になるか60秒経過するまでビープを鳴らし続けるスレッドを起動する。
pub fn spawn_beeper(stop: Arc<AtomicBool>) {
    thread::spawn(move || {
        beep_loop(&stop);
    });
}

fn beep_loop(stop: &Arc<AtomicBool>) {
    // デバイスが取れなければ黙って諦める（ウィンドウだけ出る）。
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(v) => v,
        Err(_) => return,
    };

    let start = Instant::now();
    while !stop.load(Ordering::Relaxed) && start.elapsed() < Duration::from_secs(60) {
        let sink = match Sink::try_new(&handle) {
            Ok(s) => s,
            Err(_) => return,
        };
        let source = SineWave::new(880.0)
            .take_duration(Duration::from_secs_f32(0.4))
            .amplify(0.20);
        sink.append(source);

        // 鳴っている間、100ms刻みで停止フラグを見ながら約0.7秒待つ。
        let wait_start = Instant::now();
        while wait_start.elapsed() < Duration::from_millis(700) {
            if stop.load(Ordering::Relaxed) {
                sink.stop();
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
