//! Serde cho DateTime theo đúng kiểu .NET System.Text.Json ghi ra:
//! `2026-09-28T14:30:00.1234567Z` (7 chữ số lẻ, hậu tố Z). Đọc chấp nhận mọi dạng ISO 8601.
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{self, Deserialize, Deserializer, Serializer};

pub fn format_dotnet(dt: &DateTime<Utc>) -> String {
    let ticks = dt.timestamp_subsec_nanos() / 100;
    format!("{}.{:07}Z", dt.format("%Y-%m-%dT%H:%M:%S"), ticks)
}

pub fn parse_dotnet(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // .NET có thể ghi tới 7 chữ số lẻ + offset; chrono rfc3339 xử lý được. Dạng không có Z/offset:
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(DateTime::<Utc>::from_naive_utc_and_offset(n, Utc));
        }
    }
    None
}

pub mod required {
    use super::*;
    pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format_dotnet(dt))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let v: Option<String> = Option::deserialize(d)?;
        match v {
            Some(s) => parse_dotnet(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid datetime: {s}"))),
            None => Ok(Utc::now()),
        }
    }
}

pub mod optional {
    use super::*;
    pub fn serialize<S: Serializer>(dt: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
        match dt {
            Some(d) => s.serialize_str(&format_dotnet(d)),
            None => s.serialize_none(),
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
        let v: Option<String> = Option::deserialize(d)?;
        Ok(v.and_then(|s| parse_dotnet(&s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_dotnet_format() {
        let s = "2026-09-28T14:30:00.1234567Z";
        let dt = parse_dotnet(s).unwrap();
        assert_eq!(format_dotnet(&dt), s);
        assert!(parse_dotnet("2026-09-28T14:30:00+07:00").is_some());
        assert!(parse_dotnet("2026-09-28T14:30:00").is_some());
        assert!(parse_dotnet("2026-09-28T14:30:00.123Z").is_some());
    }
}
