//! Đọc file `.mxtsessions` / `MobaXterm.ini` (tương đương `MobaXtermImporter.cs` v1).
use uuid::Uuid;

use crate::crypto::snterm_file::ExportSessionItem;
use crate::store::UNGROUPED_VI;

pub fn is_mobaxterm_file(content: &str) -> bool {
    if content.trim().is_empty() {
        return false;
    }
    let lower = content.to_lowercase();
    lower.contains("[bookmarks") || lower.contains("#109#") || lower.contains("#106#")
}

pub fn parse(content: &str) -> Vec<ExportSessionItem> {
    let mut result = Vec::new();
    if content.trim().is_empty() {
        return result;
    }
    let mut current_group = UNGROUPED_VI.to_string();

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_group = UNGROUPED_VI.to_string();
            continue;
        }
        if let Some(rest) = strip_prefix_ci(line, "SubRep=") {
            let v = rest.trim();
            current_group = if v.is_empty() { UNGROUPED_VI.to_string() } else { v.replace('\\', "/") };
            continue;
        }
        if strip_prefix_ci(line, "ImgNum=").is_some() {
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        if eq == 0 {
            continue;
        }
        let key = line[..eq].trim();
        let val = line[eq + 1..].trim();
        let lower_val = val.to_lowercase();
        let tag_idx = lower_val.find("#109#").or_else(|| lower_val.find("#106#"));
        let Some(tag_idx) = tag_idx else { continue };
        let session_data = &val[tag_idx + 5..];
        let parts: Vec<&str> = session_data.split('%').collect();
        if parts.len() < 4 {
            continue;
        }
        let host = parts[1].trim();
        if host.is_empty() {
            continue;
        }
        let port = parts[2].trim().parse::<i64>().ok().filter(|p| *p > 0 && *p <= 65535).unwrap_or(22);
        let mut username = parts[3].trim().to_string();
        if username.is_empty() || username.eq_ignore_ascii_case("<default>") {
            username = "root".into();
        }
        let mut key_file: Option<String> = None;
        for part in parts.iter().skip(4) {
            let mut p = *part;
            if let Some(i) = p.find('`') {
                p = &p[..i];
            }
            let p = p.trim();
            if p.is_empty() {
                continue;
            }
            let lower = p.to_lowercase();
            if p.contains('\\')
                || p.contains('/')
                || lower.ends_with(".pem")
                || lower.ends_with(".ppk")
                || lower.ends_with(".key")
                || lower.ends_with("id_rsa")
                || lower.ends_with("id_ed25519")
                || std::path::Path::new(p).exists()
            {
                key_file = Some(p.trim_matches(|c| c == '"' || c == '\'').to_string());
                break;
            }
        }
        let name = if key.is_empty() { format!("{username}@{host}") } else { key.to_string() };
        result.push(ExportSessionItem {
            id: Uuid::new_v4(),
            name,
            group: current_group.clone(),
            host: host.to_string(),
            port,
            username,
            key_file_name: key_file,
            secrets: None,
            extra: serde_json::Map::new(),
        });
    }
    result
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bookmarks_with_groups_and_keys() {
        let content = "[Bookmarks_1]\nSubRep=Prod\\Web\nImgNum=41\nweb-01=#109#0%10.0.0.5%2222%ubuntu%%-1%-1%%%%%0%0%0%C:\\keys\\id_ed25519%%-1%0%0%0%%1080%%0%0%1#MobaFont%10%0%0%-1%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%0%_Std_Colors_0_%80%24%0%1%-1%<none>%%0%1%-1#0# #-1\nsftp-only=#106#0%10.0.0.6%22%<default>%%-1%-1\n[Bookmarks_2]\nvm=#109#0%host.example.com%22%root%%-1%-1\n";
        assert!(is_mobaxterm_file(content));
        let items = parse(content);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].name, "web-01");
        assert_eq!(items[0].group, "Prod/Web");
        assert_eq!(items[0].port, 2222);
        assert_eq!(items[0].key_file_name.as_deref(), Some("C:\\keys\\id_ed25519"));
        assert_eq!(items[1].username, "root");
        assert_eq!(items[1].port, 22);
        assert_eq!(items[2].group, UNGROUPED_VI);
        assert!(!is_mobaxterm_file("{\"format\":\"snterm-sessions\"}"));
    }
}
