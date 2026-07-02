# ralarm

A tiny resident alarm app. Rings at specified times with a beep (max 60s) and an always-on-top popup window that stays until you close it — so you notice the alarm even if you were away from your desk.

Single binary, no runtime required. Windows / macOS.

## Usage

1. Run `ralarm.exe` (or `ralarm` on macOS).
2. Write alarms in the in-app editor (auto-saved to `alarms.txt` next to the binary):

   ```
   0850
   1150 shopping
   1421 1on1
   ```

   One line = one alarm: `hhmm [title]` (title optional). Invalid lines are silently ignored.

3. At the specified time: a beep sounds (up to 60s) and a red always-on-top popup appears. Close the popup to stop the sound.

Notes:

- Alarm lines are not removed after firing — they fire again the next day. Each alarm fires at most once per day.
- `alarms.txt` can also be edited externally; changes are picked up automatically (mtime polling).
- Fired alarms are logged to `log.txt` in MRU order. The History window lets you re-insert past alarms into the editor.
- The Test button fires a popup + sound immediately (not logged).

## Build

```
cargo build --release   # -> target/release/ralarm(.exe)
cargo test              # core logic tests
```

## Stack

- [eframe / egui](https://github.com/emilk/egui) — GUI, multi-viewport popups with always-on-top
- [rodio](https://github.com/RustAudio/rodio) — beep via generated sine wave (no audio files)
- [chrono](https://github.com/chronotope/chrono) — local time

## Code layout

| File | Role |
|---|---|
| `src/core.rs` | Pure logic: alarm parsing, MRU log, next-alarm label (unit tested) |
| `src/sound.rs` | Beep thread with stop flag; never panics without an audio device |
| `src/fonts.rs` | Loads an OS-standard CJK font as fallback |
| `src/main.rs` | eframe app: main window, tick/fire loop, popups, history |

## License

MIT
