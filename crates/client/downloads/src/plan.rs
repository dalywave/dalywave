//! Что держать на устройстве и что качать в пределах места.

use std::collections::{HashMap, HashSet};

use dalywave_store::{FileRecord, FileState};
use uuid::Uuid;

use crate::{ESTIMATED_TRACK_BYTES, MAX_ATTEMPTS};

#[derive(Debug, Clone, Copy)]
pub struct PlanInput<'a> {
    /// Закреплённые треки в порядке закрепления.
    pub pinned: &'a [Uuid],
    /// Умный запас в порядке важности.
    pub reserve: &'a [Uuid],
    /// Все файлы на устройстве (и недокачанные).
    pub files: &'a [FileRecord],
    /// Предел места, байт.
    pub limit: u64,
    /// Качать ли умный запас (выключен — только закреплённое).
    pub reserve_enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Что качать, по порядку.
    pub fetch: Vec<Uuid>,
    /// Что удалить, по порядку: кэш (давно не игравшее первым), затем хвост запаса.
    pub evict: Vec<Uuid>,
    /// Закреплённое, которое не помещается в предел.
    pub over_limit: Vec<Uuid>,
    /// Сколько займёт всё, что остаётся.
    pub planned_bytes: u64,
}

fn size_of(f: Option<&FileRecord>) -> u64 {
    match f {
        Some(f) if f.size_bytes > 0 => f.size_bytes,
        _ => ESTIMATED_TRACK_BYTES,
    }
}

fn fetchable(f: Option<&FileRecord>) -> bool {
    match f {
        None => true,
        Some(f) => {
            f.state != FileState::Ready
                && !(f.state == FileState::Failed && f.attempts >= MAX_ATTEMPTS)
        }
    }
}

pub fn plan(input: PlanInput) -> Plan {
    let files: HashMap<Uuid, &FileRecord> = input.files.iter().map(|f| (f.track_id, f)).collect();
    let mut out = Plan::default();
    let mut used = 0u64;
    let mut keep: HashSet<Uuid> = HashSet::new();

    for id in input.pinned {
        if !keep.insert(*id) {
            continue;
        }
        let f = files.get(id).copied();
        let size = size_of(f);
        if used + size > input.limit {
            keep.remove(id);
            out.over_limit.push(*id);
            continue;
        }
        used += size;
        if fetchable(f) {
            out.fetch.push(*id);
        }
    }

    let mut reserve_dropped = Vec::new();
    for id in input.reserve {
        if keep.contains(id) {
            continue;
        }
        let f = files.get(id).copied();
        let size = size_of(f);
        if !input.reserve_enabled || used + size > input.limit {
            reserve_dropped.push(*id);
            continue;
        }
        keep.insert(*id);
        used += size;
        if fetchable(f) {
            out.fetch.push(*id);
        }
    }

    // Кэш: скачанное вне закреплённого и запаса — самое свежее по прослушиванию держится, пока есть место
    let reserve_set: HashSet<Uuid> = input.reserve.iter().copied().collect();
    let mut cache: Vec<&FileRecord> = input
        .files
        .iter()
        .filter(|f| !keep.contains(&f.track_id) && !reserve_set.contains(&f.track_id))
        .collect();
    cache.sort_by_key(|f| std::cmp::Reverse(f.last_played_at.or(f.downloaded_at)));
    let mut cache_evict = Vec::new();
    for f in cache {
        // Недокачанное вне плана — сразу: продолжать его незачем
        if f.state != FileState::Ready {
            cache_evict.push(f);
            continue;
        }
        if used + f.size_bytes <= input.limit {
            used += f.size_bytes;
            keep.insert(f.track_id);
        } else {
            cache_evict.push(f);
        }
    }
    cache_evict.sort_by_key(|f| f.last_played_at.or(f.downloaded_at));
    out.evict
        .extend(cache_evict.into_iter().map(|f| f.track_id));
    // Хвост запаса — с конца: наименее важное уходит первым
    for id in reserve_dropped.into_iter().rev() {
        if files.contains_key(&id) {
            out.evict.push(id);
        }
    }
    out.planned_bytes = used;
    out
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::*;

    const MB: u64 = 1024 * 1024;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn file(n: u128, size: u64, state: FileState, played_hours_ago: Option<i64>) -> FileRecord {
        FileRecord {
            track_id: id(n),
            recording_id: None,
            file_id: None,
            path: format!("{n}.m4a"),
            size_bytes: size,
            sha256: None,
            codec: None,
            container: None,
            duration_ms: None,
            loudness_lufs: None,
            true_peak_db: None,
            waveform: None,
            state,
            received: 0,
            error: None,
            attempts: 0,
            downloaded_at: Some(Utc::now() - Duration::days(10)),
            last_played_at: played_hours_ago.map(|h| Utc::now() - Duration::hours(h)),
        }
    }

    #[test]
    fn pinned_first_then_reserve_within_limit() {
        let files = vec![file(1, 4 * MB, FileState::Ready, None)];
        let p = plan(PlanInput {
            pinned: &[id(1), id(2)],
            reserve: &[id(10), id(11), id(12)],
            files: &files,
            limit: 4 * MB + 2 * ESTIMATED_TRACK_BYTES,
            reserve_enabled: true,
        });
        // Закреплённый 1 уже есть, 2 — качать; из запаса помещается один
        assert_eq!(p.fetch, vec![id(2), id(10)]);
        assert!(p.over_limit.is_empty());
        assert!(p.evict.is_empty());
    }

    #[test]
    fn eviction_order_cache_oldest_first_then_reserve_tail_never_pinned() {
        let files = vec![
            file(1, 5 * MB, FileState::Ready, None),       // закреплён
            file(10, 5 * MB, FileState::Ready, None),      // запас, важный
            file(11, 5 * MB, FileState::Ready, None),      // запас, хвост
            file(20, 5 * MB, FileState::Ready, Some(1)),   // кэш, играл час назад
            file(21, 5 * MB, FileState::Ready, Some(100)), // кэш, давно
            file(22, 5 * MB, FileState::Ready, Some(50)),  // кэш
        ];
        // Места на закреплённое, один трек запаса и один кэш
        let p = plan(PlanInput {
            pinned: &[id(1)],
            reserve: &[id(10), id(11)],
            files: &files,
            limit: 20 * MB,
            reserve_enabled: true,
        });
        assert!(p.fetch.is_empty());
        // Запас важнее кэша: 1, 10, 11 остаются; из кэша — только самый свежий (20), давнее — первым вон
        assert_eq!(p.evict, vec![id(21), id(22)]);

        // Предел меньше: уходит и хвост запаса — после кэша
        let p = plan(PlanInput {
            pinned: &[id(1)],
            reserve: &[id(10), id(11)],
            files: &files,
            limit: 10 * MB,
            reserve_enabled: true,
        });
        assert_eq!(p.evict, vec![id(21), id(22), id(20), id(11)]);

        // Закреплённое больше предела: не удаляется, лишнее просто не качается
        let p = plan(PlanInput {
            pinned: &[id(1), id(2), id(3)],
            reserve: &[],
            files: &files,
            limit: 5 * MB + ESTIMATED_TRACK_BYTES,
            reserve_enabled: true,
        });
        assert_eq!(p.fetch, vec![id(2)]);
        assert_eq!(p.over_limit, vec![id(3)]);
        assert!(!p.evict.contains(&id(1)));
    }

    #[test]
    fn reserve_off_keeps_only_pinned_and_cache() {
        let files = vec![
            file(10, 5 * MB, FileState::Ready, None),
            file(20, 5 * MB, FileState::Ready, Some(2)),
        ];
        let p = plan(PlanInput {
            pinned: &[],
            reserve: &[id(10), id(11)],
            files: &files,
            limit: 100 * MB,
            reserve_enabled: false,
        });
        assert!(p.fetch.is_empty());
        assert_eq!(p.evict, vec![id(10)]);
    }

    #[test]
    fn failed_too_often_is_not_retried_and_partial_outside_plan_goes() {
        let mut broken = file(2, 0, FileState::Failed, None);
        broken.attempts = MAX_ATTEMPTS;
        let mut partial = file(30, 5 * MB, FileState::Downloading, None);
        partial.received = MB;
        let files = vec![broken, partial];
        let p = plan(PlanInput {
            pinned: &[id(2), id(3)],
            reserve: &[],
            files: &files,
            limit: 100 * MB,
            reserve_enabled: true,
        });
        assert_eq!(p.fetch, vec![id(3)]);
        assert_eq!(p.evict, vec![id(30)]);
    }
}
