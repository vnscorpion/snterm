//! Thuật toán gộp thuần (không I/O) — Phần 2 mục 4.2:
//! bản `updatedAt` mới hơn thắng; tombstone đấu với bản ghi theo mốc thời gian; gộp VM thêm tay trùng nhau.
use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::store::Tombstone;
use super::vault::{SyncRecord, SyncState};

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeReport {
    pub add_local: Vec<String>,
    pub update_local: Vec<String>,
    pub delete_local: Vec<String>,
    pub to_remote: usize,
    pub merged_duplicates: Vec<String>,
}

impl MergeReport {
    pub fn local_changes(&self) -> usize {
        self.add_local.len() + self.update_local.len() + self.delete_local.len()
    }
}

pub struct MergeOutput {
    pub merged: SyncState,
    pub report: MergeReport,
    pub local_changed: bool,
    pub remote_changed: bool,
}

fn label(r: &SyncRecord) -> String {
    if r.name.trim().is_empty() { format!("{}@{}", r.username, r.host) } else { r.name.clone() }
}

pub fn merge(local: &SyncState, remote: &SyncState, now: DateTime<Utc>, keep_days: i64) -> MergeOutput {
    // Tombstone: lấy mốc mới nhất mỗi id.
    let mut tomb: HashMap<Uuid, DateTime<Utc>> = HashMap::new();
    for t in local.tombstones.iter().chain(remote.tombstones.iter()) {
        let e = tomb.entry(t.id).or_insert(t.deleted_at);
        if t.deleted_at > *e {
            *e = t.deleted_at;
        }
    }
    let lmap: HashMap<Uuid, &SyncRecord> = local.records.iter().map(|r| (r.id, r)).collect();
    let rmap: HashMap<Uuid, &SyncRecord> = remote.records.iter().map(|r| (r.id, r)).collect();
    let mut ids: Vec<Uuid> = lmap.keys().chain(rmap.keys()).copied().collect();
    ids.sort();
    ids.dedup();

    let mut alive: BTreeMap<Uuid, SyncRecord> = BTreeMap::new();
    for id in ids {
        let cand: SyncRecord = match (lmap.get(&id), rmap.get(&id)) {
            (Some(l), Some(r)) => {
                if r.updated_at > l.updated_at {
                    (*r).clone()
                } else {
                    (*l).clone()
                }
            }
            (Some(l), None) => (*l).clone(),
            (None, Some(r)) => (*r).clone(),
            (None, None) => continue,
        };
        if let Some(d) = tomb.get(&id) {
            if *d >= cand.updated_at {
                continue; // đã xóa, tombstone mới hơn
            }
            tomb.remove(&id); // bản ghi sống lại
        }
        alive.insert(id, cand);
    }

    // Gộp VM trùng (khác id, cùng host+port+user+tên): giữ id có createdAt cũ hơn, nội dung theo updatedAt mới hơn.
    let mut by_key: HashMap<String, Vec<Uuid>> = HashMap::new();
    for r in alive.values() {
        by_key.entry(r.dedupe_key()).or_default().push(r.id);
    }
    let mut merged_dups = Vec::new();
    for (_, ids) in by_key.into_iter().filter(|(_, v)| v.len() > 1) {
        let recs: Vec<SyncRecord> = ids.iter().map(|i| alive[i].clone()).collect();
        let keeper = recs.iter().min_by_key(|r| (r.created_at, r.id)).unwrap();
        let newest = recs.iter().max_by_key(|r| (r.updated_at, r.id)).unwrap();
        let mut winner = newest.clone();
        winner.id = keeper.id;
        winner.created_at = keeper.created_at;
        for r in &recs {
            if r.id != keeper.id {
                alive.remove(&r.id);
                tomb.insert(r.id, now);
                merged_dups.push(label(r));
            }
        }
        alive.insert(keeper.id, winner);
    }

    let cutoff = now - Duration::days(keep_days);
    let mut tombstones: Vec<Tombstone> = tomb.into_iter().filter(|(_, d)| *d >= cutoff).map(|(id, d)| Tombstone { id, deleted_at: d }).collect();
    tombstones.sort_by_key(|t| t.id);
    let merged = SyncState { records: alive.into_values().collect(), tombstones };

    let mut report = MergeReport { merged_duplicates: merged_dups, ..Default::default() };
    for m in &merged.records {
        match lmap.get(&m.id) {
            None => report.add_local.push(label(m)),
            Some(l) if !l.same_content(m) => report.update_local.push(label(m)),
            _ => {}
        }
    }
    for l in &local.records {
        if !merged.records.iter().any(|m| m.id == l.id) {
            report.delete_local.push(label(l));
        }
    }
    let local_changed = report.local_changes() > 0 || !same_tombstones(&local.tombstones, &merged.tombstones);
    let remote_changed = !same_records(&remote.records, &merged.records) || !same_tombstones(&remote.tombstones, &merged.tombstones);
    report.to_remote = merged
        .records
        .iter()
        .filter(|m| match rmap.get(&m.id) {
            None => true,
            Some(r) => !r.same_content(m),
        })
        .count()
        + remote.records.iter().filter(|r| !merged.records.iter().any(|m| m.id == r.id)).count();
    MergeOutput { merged, report, local_changed, remote_changed }
}

fn same_records(a: &[SyncRecord], b: &[SyncRecord]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let bm: HashMap<Uuid, &SyncRecord> = b.iter().map(|r| (r.id, r)).collect();
    a.iter().all(|x| bm.get(&x.id).map(|y| x.same_content(y)).unwrap_or(false))
}

fn same_tombstones(a: &[Tombstone], b: &[Tombstone]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let bm: HashMap<Uuid, DateTime<Utc>> = b.iter().map(|t| (t.id, t.deleted_at)).collect();
    a.iter().all(|t| bm.get(&t.id).map(|d| d.timestamp() == t.deleted_at.timestamp()).unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str, host: &str, upd: DateTime<Utc>) -> SyncRecord {
        SyncRecord {
            id: Uuid::new_v4(),
            name: name.into(),
            group: "Dev".into(),
            host: host.into(),
            port: 22,
            username: "root".into(),
            key_file_name: None,
            key_hash: None,
            key_content: None,
            password: Some("pw".into()),
            passphrase: None,
            created_at: upd,
            updated_at: upd,
        }
    }
    fn st(recs: Vec<SyncRecord>, tombs: Vec<Tombstone>) -> SyncState {
        SyncState { records: recs, tombstones: tombs }
    }
    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-01T00:00:00Z").unwrap().with_timezone(&Utc)
    }

    #[test]
    fn add_each_way_and_update_newer_wins() {
        let now = t0() + Duration::hours(5);
        let a = rec("a", "1.1.1.1", t0());
        let mut b = rec("b", "2.2.2.2", t0());
        let out = merge(&st(vec![a.clone()], vec![]), &st(vec![b.clone()], vec![]), now, 180);
        assert_eq!(out.merged.records.len(), 2);
        assert_eq!(out.report.add_local, vec!["b"]);
        assert_eq!(out.report.to_remote, 1);
        assert!(out.local_changed && out.remote_changed);
        // B sửa ở máy kia (mới hơn) → thắng
        let mut b_new = b.clone();
        b_new.name = "b-renamed".into();
        b_new.updated_at = t0() + Duration::hours(1);
        b.updated_at = t0();
        let out = merge(&st(vec![b.clone()], vec![]), &st(vec![b_new.clone()], vec![]), now, 180);
        assert_eq!(out.merged.records[0].name, "b-renamed");
        assert_eq!(out.report.update_local, vec!["b-renamed"]);
        assert!(!out.remote_changed);
        // Bằng nhau → giữ local, không đổi gì
        let out = merge(&st(vec![b_new.clone()], vec![]), &st(vec![b_new.clone()], vec![]), now, 180);
        assert!(!out.local_changed && !out.remote_changed);
    }

    #[test]
    fn delete_vs_edit_by_timestamp() {
        let now = t0() + Duration::days(1);
        let a = rec("a", "1.1.1.1", t0());
        // Xóa ở local (tombstone sau updatedAt) → mất ở remote
        let tomb = Tombstone { id: a.id, deleted_at: t0() + Duration::hours(1) };
        let out = merge(&st(vec![], vec![tomb.clone()]), &st(vec![a.clone()], vec![]), now, 180);
        assert!(out.merged.records.is_empty());
        assert_eq!(out.merged.tombstones.len(), 1);
        assert!(out.remote_changed);
        // Remote sửa sau khi local xóa → VM sống lại
        let mut a2 = a.clone();
        a2.updated_at = t0() + Duration::hours(2);
        let out = merge(&st(vec![], vec![tomb]), &st(vec![a2], vec![]), now, 180);
        assert_eq!(out.merged.records.len(), 1);
        assert!(out.merged.tombstones.is_empty());
        assert_eq!(out.report.add_local.len(), 1);
    }

    #[test]
    fn duplicates_added_on_two_machines_are_merged_once() {
        let now = t0() + Duration::days(1);
        let mut x = rec("web", "10.0.0.1", t0());
        x.created_at = t0() - Duration::days(2);
        let mut y = rec("web", "10.0.0.1", t0() + Duration::hours(1));
        y.password = Some("newer".into());
        let out = merge(&st(vec![x.clone()], vec![]), &st(vec![y.clone()], vec![]), now, 180);
        assert_eq!(out.merged.records.len(), 1);
        let m = &out.merged.records[0];
        assert_eq!(m.id, x.id, "giữ id có createdAt cũ hơn");
        assert_eq!(m.password.as_deref(), Some("newer"), "nội dung theo updatedAt mới hơn");
        assert!(out.merged.tombstones.iter().any(|t| t.id == y.id));
        assert_eq!(out.report.merged_duplicates.len(), 1);
        // Chu kỳ 2: cả hai bên đã giống → ổn định
        let out2 = merge(&out.merged, &out.merged, now, 180);
        assert!(!out2.local_changed && !out2.remote_changed);
    }

    #[test]
    fn tombstones_expire() {
        let now = t0();
        let old = Tombstone { id: Uuid::new_v4(), deleted_at: t0() - Duration::days(200) };
        let fresh = Tombstone { id: Uuid::new_v4(), deleted_at: t0() - Duration::days(10) };
        let out = merge(&st(vec![], vec![old]), &st(vec![], vec![fresh.clone()]), now, 180);
        assert_eq!(out.merged.tombstones.len(), 1);
        assert_eq!(out.merged.tombstones[0].id, fresh.id);
    }

    #[test]
    fn vietnamese_names_preserved() {
        let now = t0();
        let a = rec("Máy chủ Ắ Ằ Ợ", "h", t0());
        let out = merge(&st(vec![], vec![]), &st(vec![a], vec![]), now, 180);
        assert_eq!(out.merged.records[0].name, "Máy chủ Ắ Ằ Ợ");
        assert_eq!(out.report.add_local, vec!["Máy chủ Ắ Ằ Ợ"]);
    }
}
