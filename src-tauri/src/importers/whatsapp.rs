//! WhatsApp "Export chat" (from the phone): a .txt file, or a .zip with the .txt and media.
//! Android and iPhone write different line formats, in the phone's language and date order.

use super::{AttachmentSource, ParsedAttachment, ParsedChat, ParsedMessage};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

struct Line {
    date: String,
    time: String,
    ampm: Option<String>,
    rest: String,
}

fn clean(s: &str) -> String {
    s.replace(['\u{200e}', '\u{200f}', '\u{feff}'], "")
        .replace(['\u{202f}', '\u{a0}'], " ")
}

fn line_regexes() -> [Regex; 2] {
    const AMPM: &str = r"(?:\s*(?P<ampm>[AaPp]\.?\s?[Mm]\.?|SA|CH|sa|ch))?";
    [
        // iPhone: [9/11/26, 10:30:12] Name: text
        Regex::new(&format!(r"^\[(?P<date>\d{{1,4}}[./-]\d{{1,2}}[./-]\d{{1,4}}),?\s+(?P<time>\d{{1,2}}[:.]\d{{2}}(?:[:.]\d{{2}})?){AMPM}\]\s(?P<rest>.*)$")).unwrap(),
        // Android: 9/11/26, 10:30 - Name: text
        Regex::new(&format!(r"^(?P<date>\d{{1,4}}[./-]\d{{1,2}}[./-]\d{{1,4}}),?\s+(?P<time>\d{{1,2}}[:.]\d{{2}}(?:[:.]\d{{2}})?){AMPM}\s[-–]\s(?P<rest>.*)$")).unwrap(),
    ]
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Order {
    Dmy,
    Mdy,
    Ymd,
}

fn date_parts(d: &str) -> Option<(String, u32, u32)> {
    let parts: Vec<&str> = d.split(['/', '.', '-']).collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].to_string(),
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

fn detect_order(lines: &[Line]) -> Order {
    let mut first_big = false;
    let mut second_big = false;
    for l in lines {
        if let Some((a, b, _)) = date_parts(&l.date) {
            if a.len() == 4 {
                return Order::Ymd;
            }
            let a: u32 = a.parse().unwrap_or(0);
            first_big |= a > 12;
            second_big |= b > 12;
        }
    }
    if first_big {
        Order::Dmy
    } else if second_big {
        Order::Mdy
    } else {
        Order::Dmy
    }
}

fn to_ts(l: &Line, order: Order) -> Option<i64> {
    let (a, b, c) = date_parts(&l.date)?;
    let a: u32 = a.parse().ok()?;
    let (y, m, d) = match order {
        Order::Ymd => (a as i32, b, c),
        Order::Dmy => (c as i32, b, a),
        Order::Mdy => (c as i32, a, b),
    };
    let y = if y < 100 { 2000 + y } else { y };
    let t: Vec<u32> = l
        .time
        .split([':', '.'])
        .filter_map(|x| x.parse().ok())
        .collect();
    let (mut h, mi, s) = (*t.first()?, *t.get(1)?, t.get(2).copied().unwrap_or(0));
    if let Some(ap) = &l.ampm {
        let pm = matches!(ap.chars().next(), Some('p' | 'P' | 'c' | 'C'));
        if pm && h < 12 {
            h += 12;
        }
        if !pm && h == 12 {
            h = 0;
        }
    }
    let naive = chrono::NaiveDate::from_ymd_opt(y, m, d)?.and_hms_opt(h, mi, s)?;
    naive
        .and_local_timezone(chrono::Local)
        .earliest()
        .map(|t| t.timestamp_millis())
}

/// Reads the chat text and lists the media next to it.
fn load(path: &Path) -> Result<(String, String, HashMap<String, (AttachmentSource, u64)>), String> {
    let lower = path.to_string_lossy().to_lowercase();
    let mut media = HashMap::new();
    if lower.ends_with(".zip") {
        let file = std::fs::File::open(path)
            .map_err(|e| format!("Không mở được {}: {e}", path.display()))?;
        let mut zip =
            zip::ZipArchive::new(file).map_err(|e| format!("Tệp zip không hợp lệ: {e}"))?;
        let mut txt_name: Option<String> = None;
        for i in 0..zip.len() {
            let Ok(f) = zip.by_index(i) else { continue };
            let Ok(name) = f.name().map(|n| n.to_string()) else {
                continue;
            };
            if f.is_dir() {
                continue;
            }
            let base = name.rsplit('/').next().unwrap_or(&name).to_string();
            if base.to_lowercase().ends_with(".txt") && (txt_name.is_none() || base == "_chat.txt")
            {
                txt_name = Some(name.clone());
            }
            media.insert(
                base,
                (
                    AttachmentSource::Zip {
                        archive: path.to_path_buf(),
                        entry: name,
                    },
                    f.size(),
                ),
            );
        }
        let txt_name = txt_name.ok_or("Trong tệp zip không có đoạn chat (.txt).")?;
        let mut text = String::new();
        zip.by_name(&txt_name)
            .map_err(|e| e.to_string())?
            .read_to_string(&mut text)
            .map_err(|e| format!("Không đọc được đoạn chat: {e}"))?;
        let base = txt_name.rsplit('/').next().unwrap_or(&txt_name).to_string();
        media.remove(&base);
        let title_source = if base == "_chat.txt" {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            base.trim_end_matches(".txt").to_string()
        };
        Ok((text, title_source, media))
    } else {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Không đọc được {}: {e}", path.display()))?;
        if let Some(dir) = path.parent() {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let p = entry.path();
                if p.is_file() && p != path {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let name = entry.file_name().to_string_lossy().into_owned();
                    media.insert(name, (AttachmentSource::File(p), size));
                }
            }
        }
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok((text, title, media))
    }
}

fn title_from(source: &str) -> String {
    let prefixes = [
        "WhatsApp Chat with ",
        "WhatsApp Chat - ",
        "Đoạn chat WhatsApp với ",
        "Cuộc trò chuyện WhatsApp với ",
        "WhatsApp-Chat mit ",
    ];
    for p in prefixes {
        if let Some(rest) = source.strip_prefix(p) {
            return rest.trim().to_string();
        }
    }
    source.trim().to_string()
}

/// File names mentioned in a message that exist in the export.
fn attachments_in(
    text: &str,
    available: &HashMap<String, (AttachmentSource, u64)>,
    token: &Regex,
) -> Vec<String> {
    let mut found = vec![];
    for cap in token.find_iter(text) {
        let candidate = cap.as_str().trim();
        // "IMG-1.jpg (file attached)" or "<attached: 0001-PHOTO.jpg>" or a document name with spaces:
        // try the whole match, then shorter tails at word boundaries.
        let words: Vec<&str> = candidate.split(' ').collect();
        for i in 0..words.len() {
            let tail = words[i..].join(" ");
            let tail = tail.trim_start_matches(['<', '(', ':']).trim();
            if available.contains_key(tail) && !found.iter().any(|f| f == tail) {
                found.push(tail.to_string());
                break;
            }
        }
    }
    found
}

pub fn parse(
    path: &Path,
    self_name: &str,
    sink: &mut dyn FnMut(ParsedChat) -> Result<(), String>,
) -> Result<(), String> {
    let (raw, title_source, available) = load(path)?;
    let regs = line_regexes();
    let mut lines: Vec<Line> = vec![];
    for row in raw.lines() {
        let row = clean(row);
        let mut matched = false;
        for re in &regs {
            if let Some(c) = re.captures(&row) {
                lines.push(Line {
                    date: c["date"].to_string(),
                    time: c["time"].to_string(),
                    ampm: c.name("ampm").map(|m| m.as_str().to_string()),
                    rest: c["rest"].to_string(),
                });
                matched = true;
                break;
            }
        }
        if !matched {
            if let Some(last) = lines.last_mut() {
                last.rest.push('\n');
                last.rest.push_str(&row);
            }
        }
    }
    if lines.is_empty() {
        return Err("Không nhận ra định dạng đoạn chat WhatsApp.".into());
    }
    let order = detect_order(&lines);
    let token = Regex::new(r"[^\n<>/\\:]+?\.[A-Za-z0-9]{2,5}\b").unwrap();
    let omitted = [
        "<Media omitted>",
        "<Đã bỏ qua tệp phương tiện>",
        "<Medien ausgeschlossen>",
        "image omitted",
        "video omitted",
    ];
    let title = title_from(&title_source);
    let mut senders = HashSet::new();
    let mut occurrences: HashMap<String, u32> = HashMap::new();
    let mut messages = vec![];
    for l in &lines {
        let Some(ts) = to_ts(l, order) else { continue };
        let (sender, mut text) = match l.rest.split_once(": ") {
            Some((s, t)) if !s.contains('\n') && s.chars().count() <= 80 => {
                (Some(s.trim().to_string()), t.to_string())
            }
            _ => (None, l.rest.clone()),
        };
        if let Some(s) = &sender {
            senders.insert(s.clone());
        }
        let names = attachments_in(&text, &available, &token);
        for n in &names {
            for pattern in [
                format!("<attached: {n}>"),
                format!("{n} (file attached)"),
                format!("{n} (tệp đính kèm)"),
                format!("{n} (tệp được đính kèm)"),
                n.clone(),
            ] {
                text = text.replace(&pattern, "");
            }
        }
        let had_omitted = omitted.iter().any(|o| text.contains(o));
        for o in omitted {
            text = text.replace(o, "");
        }
        let text = text.trim().to_string();
        let attachments: Vec<ParsedAttachment> = names
            .iter()
            .filter_map(|n| available.get(n).map(|(src, size)| (n, src, size)))
            .map(|(n, src, size)| ParsedAttachment {
                name: n.clone(),
                kind: crate::rules::categorize(&crate::util::ext_of(n), "")
                    .as_str()
                    .into(),
                size: *size,
                source: src.clone(),
            })
            .collect();
        let base = format!("{ts}|{}|{}", sender.as_deref().unwrap_or(""), text);
        let n = occurrences.entry(base.clone()).or_insert(0);
        *n += 1;
        let kind = if sender.is_none() {
            "service"
        } else if attachments.is_empty() && !had_omitted {
            "text"
        } else if text.is_empty() {
            "media"
        } else {
            "text+media"
        };
        messages.push(ParsedMessage {
            key: format!(
                "wa:{}",
                &crate::util::sha256_hex(format!("{base}|{n}").as_bytes())[..24]
            ),
            outgoing: if self_name.is_empty() {
                None
            } else {
                sender.as_deref().map(|s| s == self_name)
            },
            sender,
            ts,
            text,
            kind: kind.into(),
            reply_to: None,
            attachments,
        });
    }
    let is_group = senders.len() > 2;
    sink(ParsedChat {
        key: format!("export:{title}"),
        title,
        is_group,
        messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn collect(path: &Path, me: &str) -> ParsedChat {
        let mut out = None;
        parse(path, me, &mut |c| {
            out = Some(c);
            Ok(())
        })
        .unwrap();
        out.unwrap()
    }

    #[test]
    fn android_txt_with_media_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("IMG-20261109-WA0001.jpg"), b"jpg").unwrap();
        let txt = "09/11/2026, 10:30 - Tin nhắn được mã hóa đầu cuối.\n\
                   09/11/2026, 10:31 - Lan: Chào anh\n\
                   dòng thứ hai\n\
                   09/11/2026, 10:32 - Minh: IMG-20261109-WA0001.jpg (file attached)\n\
                   13/11/2026, 08:00 - Lan: <Media omitted>\n";
        let p = dir.path().join("WhatsApp Chat with Lan.txt");
        std::fs::write(&p, txt).unwrap();
        let c = collect(&p, "Minh");
        assert_eq!(c.title, "Lan");
        assert_eq!(c.messages.len(), 4);
        assert_eq!(c.messages[0].kind, "service");
        assert_eq!(c.messages[1].text, "Chào anh\ndòng thứ hai");
        assert_eq!(c.messages[1].outgoing, Some(false));
        assert_eq!(c.messages[2].outgoing, Some(true));
        assert_eq!(c.messages[2].attachments.len(), 1);
        assert_eq!(c.messages[2].text, "");
        assert_eq!(c.messages[3].kind, "media");
        // 13/11 means day-first; 09/11 is then 9 November.
        let d = chrono::DateTime::from_timestamp_millis(c.messages[1].ts)
            .unwrap()
            .with_timezone(&chrono::Local);
        assert_eq!(d.format("%d/%m").to_string(), "09/11");
    }

    #[test]
    fn iphone_zip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("WhatsApp Chat - Gia đình.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("_chat.txt", opts).unwrap();
        zip.write_all("[11/9/26, 10:30:12 PM] Bố: Ảnh nè\n\u{200e}[11/9/26, 10:31:00 PM] Bố: \u{200e}<attached: 00000005-PHOTO-2026-11-09-22-31-00.jpg>\n[11/14/26, 9:00:00 AM] Mẹ: ok\n[11/14/26, 9:01:00 AM] An: 👍\n".as_bytes()).unwrap();
        zip.start_file("00000005-PHOTO-2026-11-09-22-31-00.jpg", opts)
            .unwrap();
        zip.write_all(b"jpg").unwrap();
        zip.finish().unwrap();
        let c = collect(&p, "");
        assert_eq!(c.title, "Gia đình");
        assert!(c.is_group);
        assert_eq!(c.messages[1].attachments.len(), 1);
        assert!(matches!(
            c.messages[1].attachments[0].source,
            AttachmentSource::Zip { .. }
        ));
        let d = chrono::DateTime::from_timestamp_millis(c.messages[0].ts)
            .unwrap()
            .with_timezone(&chrono::Local);
        assert_eq!(
            d.format("%m-%d %H:%M").to_string(),
            "11-09 22:30",
            "month-first because 14 can't be a month"
        );
        // Keys are stable across imports of the same export.
        let again = collect(&p, "");
        assert_eq!(c.messages[2].key, again.messages[2].key);
    }
}
