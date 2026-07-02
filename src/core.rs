//! アラームのコアロジック（GUIやOS依存なし・テスト対象）。

/// 1件のアラーム。内部表現は `(hhmm, title)`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alarm {
    pub hhmm: String,
    pub title: String,
}

impl Alarm {
    /// log.txt / 履歴で使う一行文字列。タイトルなしなら `hhmm` のみ。
    pub fn to_line(&self) -> String {
        if self.title.is_empty() {
            self.hhmm.clone()
        } else {
            format!("{} {}", self.hhmm, self.title)
        }
    }
}

/// "0850" のような4桁文字列が有効な時刻か。hh:00-23, mm:00-59。
pub fn is_valid_hhmm(s: &str) -> bool {
    if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let hh: u32 = s[0..2].parse().unwrap_or(99);
    let mm: u32 = s[2..4].parse().unwrap_or(99);
    hh <= 23 && mm <= 59
}

/// "0850" -> "08:50"
pub fn hhmm_to_display(hhmm: &str) -> String {
    if hhmm.len() == 4 {
        format!("{}:{}", &hhmm[0..2], &hhmm[2..4])
    } else {
        hhmm.to_string()
    }
}

/// alarms.txt の内容をパース。
/// - 1行 = `hhmm [タイトル]`
/// - 不正行は無視
/// - 同一 (hhmm, title) は最初の1件だけ（順序保持）
pub fn parse_alarms(content: &str) -> Vec<Alarm> {
    let mut out: Vec<Alarm> = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let (hhmm, title) = match line.split_once(char::is_whitespace) {
            Some((h, t)) => (h, t.trim().to_string()),
            None => (line, String::new()),
        };
        if !is_valid_hhmm(hhmm) {
            continue;
        }
        let alarm = Alarm {
            hhmm: hhmm.to_string(),
            title,
        };
        if !out.contains(&alarm) {
            out.push(alarm);
        }
    }
    out
}

/// log.txt の move-to-front（MRU）。
/// entry を先頭へ、既存の同一行は除去、空行も除去。
pub fn move_to_front(existing: &[String], entry: &str) -> Vec<String> {
    let entry = entry.trim();
    let mut out = vec![entry.to_string()];
    for line in existing {
        let line = line.trim();
        if line.is_empty() || line == entry {
            continue;
        }
        out.push(line.to_string());
    }
    out
}

/// メインウィンドウ用の「次のアラーム」表示文字列。
/// - 今日これから鳴る最小 hhmm があれば「次: HH:MM」
/// - なければ全体最小で「次: 明日 HH:MM」
/// - アラームがなければ「アラームなし」
pub fn next_alarm_label(alarms: &[Alarm], now_hhmm: &str) -> String {
    if alarms.is_empty() {
        return "アラームなし".to_string();
    }
    let upcoming = alarms
        .iter()
        .map(|a| a.hhmm.as_str())
        .filter(|h| *h > now_hhmm)
        .min();
    if let Some(h) = upcoming {
        return format!("次: {}", hhmm_to_display(h));
    }
    let earliest = alarms.iter().map(|a| a.hhmm.as_str()).min().unwrap();
    format!("次: 明日 {}", hhmm_to_display(earliest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(hhmm: &str, title: &str) -> Alarm {
        Alarm {
            hhmm: hhmm.to_string(),
            title: title.to_string(),
        }
    }

    #[test]
    fn test_parse_alarms() {
        let input = "0850\n1150 買い物\n1421 1on1\n1522\n\nbad line\n2570\n1150 買い物\n0850 朝会\n";
        let expected = vec![
            a("0850", ""),
            a("1150", "買い物"),
            a("1421", "1on1"),
            a("1522", ""),
            a("0850", "朝会"),
        ];
        assert_eq!(parse_alarms(input), expected);
    }

    #[test]
    fn test_is_valid_hhmm() {
        assert!(is_valid_hhmm("0000"));
        assert!(is_valid_hhmm("2359"));
        assert!(!is_valid_hhmm("2570")); // 分が無効
        assert!(!is_valid_hhmm("2400")); // 時が無効
        assert!(!is_valid_hhmm("850")); // 桁不足
        assert!(!is_valid_hhmm("08a0")); // 非数字
    }

    #[test]
    fn test_move_to_front() {
        let existing: Vec<String> = vec!["1150 買い物", "0850", "1421 1on1"]
            .into_iter()
            .map(String::from)
            .collect();
        let after = move_to_front(&existing, "1421 1on1");
        assert_eq!(after, vec!["1421 1on1", "1150 買い物", "0850"]);

        let after2 = move_to_front(&after, "0930 新規");
        assert_eq!(after2[0], "0930 新規");
        assert_eq!(after2, vec!["0930 新規", "1421 1on1", "1150 買い物", "0850"]);
    }

    #[test]
    fn test_move_to_front_removes_blanks() {
        let existing: Vec<String> = vec!["1150 買い物", "", "0850"]
            .into_iter()
            .map(String::from)
            .collect();
        let after = move_to_front(&existing, "0850");
        assert_eq!(after, vec!["0850", "1150 買い物"]);
    }

    #[test]
    fn test_next_alarm_label() {
        let alarms = vec![a("0850", ""), a("1421", "1on1"), a("1522", "")];
        assert_eq!(next_alarm_label(&alarms, "1000"), "次: 14:21");
        assert_eq!(next_alarm_label(&alarms, "1600"), "次: 明日 08:50");
        assert_eq!(next_alarm_label(&[], "1000"), "アラームなし");
    }
}
