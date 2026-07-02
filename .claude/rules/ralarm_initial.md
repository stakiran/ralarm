# アラームアプリ 実装引き継ぎドキュメント

このファイル1つで文脈が完結するように書いてある。これを読んで Rust でアプリを実装すること。

## 1. 目的

デスクから離れても大丈夫な常駐アラームアプリ。
- 指定時刻に**音を1分間だけ**鳴らす（離席中でも鳴りっぱなしにならない）
- 同時に、時刻とタイトルを表示した**小さいが目立つウィンドウを最前面**に出す
- ウィンドウは**閉じるまで残る** → 在席なら即気づく、離席していても戻ったとき「鳴った」と分かる
- Windows / macOS 両対応

## 2. 確定仕様

### alarms.txt（アラーム定義）
- 実行ファイルと同じディレクトリに置く。1行 = 1アラーム
- 形式: `hhmm [タイトル]`（タイトルは省略可）。内部的にアラームとは `(hhmm) (title)` という一行の文字列
- 例:
  ```
  0850
  1150 買い物
  1421 1on1
  1522
  ```
- 不正行（4桁でない、時刻として無効、など）は黙って無視
- 同一の (hhmm, title) が複数行あっても1回だけ発火
- **鳴った後も行は削除しない**（翌日も同時刻に鳴る）← ユーザー確定済み
- 典型的な使い方: 毎朝このファイルを書く。アプリ内エディタから直接編集・保存できること
- アプリ外（エディタ等）で編集された場合も検知して反映する（mtimeポーリング）。ただしアプリ内エディタに未保存の変更がある間は、エディタの内容を上書きしない（発火用の内部データだけファイルから更新する）

### 発火
- 毎秒ティックし、「分が変わった瞬間」に現在 hhmm と一致するアラームを発火（起動時に過去のアラームを遡って鳴らさない）
- 同じアラームは1日1回まで（(日付, hhmm, title) の発火済みセットで管理。日付が変わったら掃除）
- 発火時の動作:
  1. log.txt に記録（下記）
  2. 最前面ポップアップを表示
  3. 音を最大60秒鳴らす（ポップアップを閉じたら即停止）

### ポップアップ
- 小さいウィンドウ（目安 300x150）、**常に最前面**（always on top）
- 赤系背景など目立つ見た目。大きな文字で `HH:MM`、その下にタイトル（あれば）
- 「閉じる（音停止）」ボタン。ウィンドウの×でも同じ
- 複数同時発火したら位置をずらして重ねる（画面右上から縦に並べる等）
- 自動では閉じない（気づけることが目的）

### log.txt（履歴）
- 発火したアラームの一行文字列 `hhmm title`（タイトルなしなら `hhmm`）を記録
- **同じ行が既にあれば削除して先頭に移動**（MRU順。よく使うものが上に集まる）
- アプリの「履歴」画面で一覧表示し、クリック（または選択→挿入）で alarms.txt エディタに1行として挿入できる

### 常駐
- メインウィンドウの×を押したら終了せず最小化（またはトレイ格納）。明示的な「終了」操作で終了
- メインウィンドウには: 現在時刻表示、次のアラーム表示（例「次: 14:21」、今日はもうなければ「次: 明日 08:50」、なければ「アラームなし」）、alarms.txt エディタ、保存 / 再読込 / 履歴 / テスト ボタン、未保存インジケータ
- テストボタン: 現在時刻+「テスト」でポップアップと音を試せる（log.txt には記録しない）

## 3. 技術選定の経緯（重要）

1. **Tkinter は却下**: ユーザーの Mac で動かなかった実績あり
2. **Electron は却下**: 重すぎる
3. **PySide6 案**: 一度実装したが、`pip install PySide6` を各マシンに入れる必要があるのが難点
4. **Rust に確定**: コード量は問題にならない（AIが書くから）。単一バイナリ配布・ランタイム不要・省メモリが決め手

### スタック
- **eframe (egui)** — GUI。マルチビューポート（immediate viewport）でポップアップ、`with_always_on_top()`
- **rodio** — 音。`SineWave` でビープ生成（音声ファイル不要）。`default-features = false` でデコーダを外して軽量化
- **chrono** — ローカル時刻

### Cargo.toml（作成済み、この内容で開始）
```toml
[package]
name = "alarms"
version = "0.1.0"
edition = "2021"

[dependencies]
eframe = "0.31"
rodio = { version = "0.19", default-features = false }
chrono = "0.4"

[profile.release]
lto = true
strip = true
```

### バージョン固定の理由と、ずらす場合の注意
- `rodio 0.19` に固定: `OutputStream::try_default()` + `Sink::try_new()` の旧APIを想定。0.20以降は `OutputStreamBuilder` 系にAPIが変わっているので、上げるなら音再生部を書き換えること
- `eframe 0.31` 想定の注意点: egui 0.32 では `fonts.font_data.insert()` が `Arc<FontData>` を要求するようになる、`Margin` の型変更（f32→i8）などの破壊的変更がある。コンパイルエラーが出たらまずバージョン差分を疑う
- eframe 0.28 以降、`run_native` のアプリ生成クロージャは `Result<Box<dyn App>, _>` を返す（`Ok(Box::new(...))`）

## 4. 実装上の設計メモ

- **ファイル配置**: alarms.txt / log.txt は `std::env::current_exe()` の親ディレクトリ。取得失敗時はカレントディレクトリにフォールバック。alarms.txt がなければ空で自動作成
- **日本語フォント**: egui 標準フォントは CJK 非対応なので、起動時にOS標準フォントを読み込んで `FontDefinitions` に追加する（families の末尾に push してフォールバックとして使う。先頭に挿すと英数の見た目が変わる）。候補を順に `fs::read` して最初に成功したものを使う:
  - Windows: `C:\Windows\Fonts\meiryo.ttc` → `YuGothM.ttc` → `msgothic.ttc`
  - macOS: `/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc` → `/System/Library/Fonts/Hiragino Sans GB.ttc` → `/System/Library/Fonts/Supplemental/Osaka.ttf`
  - .ttc は face 0 が使われる想定（ttf-parser がコレクション対応）。全滅したらデフォルトフォントのまま（英数のみ表示）でクラッシュしないこと
- **音**: ポップアップごとにスレッドを1本立て、`Arc<AtomicBool>` の停止フラグを共有。ループ: 880Hz サイン波 0.35〜0.5秒 → 100ms刻みで停止フラグを見ながら計 ~0.7秒待つ、を経過60秒まで繰り返す。音声デバイスが取れなくても panic しない（音なしでウィンドウだけ出る）
- **ポップアップ**: `ctx.show_viewport_immediate(ViewportId::from_hash_of(id), builder, ...)`。builder は `with_always_on_top()`, `with_inner_size`, `with_position`, `with_resizable(false)`。位置は**生成時に一度だけ計算して保持**する（毎フレーム builder に違う値を渡すとユーザーがドラッグできなくなる）。モニタサイズは `ctx.input(|i| i.viewport().monitor_size)`、取れなければ 1280x800 と仮定。子ビューポート内で `close_requested()` を見て閉じ処理（音停止＋リストから除去）
- **常駐（×で最小化）**: メインの update 内で `ctx.input(|i| i.viewport().close_requested())` を検知したら、終了フラグが立っていない限り `ViewportCommand::CancelClose` + `ViewportCommand::Minimized(true)` を送る。「終了」ボタンでフラグを立てて `ViewportCommand::Close`
- **再描画**: `ctx.request_repaint_after(Duration::from_millis(300))` を毎フレーム呼び、最小化中でもティックが回るようにする
- **Windowsでコンソールを出さない**: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
- **履歴挿入**: egui の TextEdit でカーソル位置挿入は面倒なので、エディタ末尾に改行付きで追記する方式でよい

## 5. コアロジックの仕様（テスト済み・この通りに）

以下は Python 版で実際にユニットテストを通した期待値。Rust 版でも `#[cfg(test)]` テストとして同じケースを入れること。

### parse_alarms
入力:
```
0850
1150 買い物
1421 1on1
1522

bad line
2570
1150 買い物
0850 朝会
```
期待出力（順序保持・重複除去・不正行無視）:
```
[("0850",""), ("1150","買い物"), ("1421","1on1"), ("1522",""), ("0850","朝会")]
```
ポイント: `2570` は分が無効なので捨てる。`0850` と `0850 朝会` は別アラーム。`1150 買い物` の2回目は捨てる。hh は 00–23、mm は 00–59 のみ有効。タイトルは最初の空白以降を trim。

### record_log（MRU move-to-front）
log.txt が
```
1150 買い物
0850
1421 1on1
```
のとき `record_log("1421 1on1")` 後は
```
1421 1on1
1150 買い物
0850
```
続けて `record_log("0930 新規")` 後の先頭行は `0930 新規`。空行は保存時に除去。末尾は改行1つ。

### next_alarm 表示
- 今日これから鳴る最小の hhmm があれば「次: HH:MM」
- なければ全体の最小で「次: 明日 HH:MM」
- アラームがなければ「アラームなし」

## 6. 未検証事項（正直な申告）

- 開発サンドボックスのネットワークが全遮断だったため、**Rust コードは一切コンパイル・実行されていない**。crate のAPI詳細は記憶ベースなので、細かいコンパイルエラーは出る前提で直すこと（特に §3 のバージョン注意点）
- ポップアップの always-on-top / マルチビューポートの実挙動（特に macOS）は要実機確認
- .ttc フォント読み込みが egui/ab_glyph で通るかは要確認。ダメなら .ttf 候補に切り替えるか Noto Sans JP を `include_bytes!` で埋め込む

## 7. ビルド・配布

- 各OSでネイティブビルド（クロスコンパイルはしない）: `cargo build --release` → `target/release/alarms(.exe)`
- バイナリを好きな場所に置く。alarms.txt / log.txt はその隣にできる
- 自動起動: Windows は `shell:startup` にショートカット、macOS はログイン項目に追加
- ロジックのテスト: `cargo test`

## 8. やらないこと / 将来オプション

- スヌーズ機能（仕様外）
- システムトレイ常駐（tray-icon crate。winit のイベントループとの統合が面倒なので初版は「×で最小化」方式。将来検討）
- Linux 対応は必須ではない（動けばラッキー程度。音は ALSA、ビルドに libasound2-dev + pkg-config が要る）
