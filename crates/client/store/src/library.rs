//! Медиатека из локальной базы — в тех же типах, что отдаёт сервер: экраны не знают, откуда данные.

use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, NaiveDate, Utc};
use dalywave_protocol::{
    catalog::{
        AlbumDetail, AlbumRef, AlbumSummary, ArtistDetail, ArtistRef, ArtistRole, ArtistSummary,
        AudioStatus, DeezerKind, FollowState, ImageSet, RecordType, Releases, SearchResults,
        SearchScope, SearchTop, Track, TrackArtist,
    },
    library::{
        DownloadPolicy, FollowedArtist, FollowedArtistPage, LibraryCounts, LibraryOverview,
        LikedTrack, LikedTrackPage, PlaylistDetail, PlaylistFolder, PlaylistItem, PlaylistKind,
        PlaylistList, PlaylistSummary, SavedAlbum, SavedAlbumPage,
    },
    social::{PersonRef, PlaylistAccess},
};
use dalywave_textmatch::{Query, Top};
use rusqlite::{OptionalExtension, Row, functions::FunctionFlags, params};
use uuid::Uuid;

use crate::{Store, StoreResult, normalize};

/// Найденное локальным поиском: сходство с запросом (с артистами и по одному названию) и своё ли.
struct Scored {
    id: Uuid,
    name: String,
    score: f32,
    by_name: f32,
    mine: bool,
}

/// Сходство ступенями по 0,02: внутри ступени решает своё и название.
fn step(score: f32) -> i32 {
    (score * 50.0).round() as i32
}

/// Что искать в локальной базе.
#[derive(Debug, Clone, Copy)]
pub struct LocalSearch<'a> {
    pub q: &'a str,
    pub limit: usize,
}

fn uuid(s: String) -> Uuid {
    s.parse().unwrap_or_default()
}

fn time(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_default()
}

fn image(s: Option<String>) -> Option<ImageSet> {
    s.and_then(|s| serde_json::from_str(&s).ok())
}

const TRACK_SQL: &str = "
SELECT t.id, t.deezer_id, t.title, t.version, t.duration_ms, t.explicit, t.disc_number, t.track_position,
       t.recording_id, t.isrc, r.audio_status, al.id, al.title, al.image,
       EXISTS (SELECT 1 FROM liked_tracks l WHERE l.track_id = t.id)
FROM tracks t
LEFT JOIN recordings r ON r.id = t.recording_id
LEFT JOIN albums al ON al.id = t.album_id
WHERE t.id = ?1";

const ALBUM_SQL: &str = "
SELECT al.id, al.deezer_id, al.title, al.artist_id, ar.name, al.record_type, al.release_date, al.explicit,
       al.tracks_count, al.duration_ms, al.image, al.palette,
       EXISTS (SELECT 1 FROM saved_albums s WHERE s.album_id = al.id)
FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id
WHERE al.id = ?1";

fn album_row(r: &Row) -> rusqlite::Result<AlbumSummary> {
    Ok(AlbumSummary {
        id: uuid(r.get(0)?),
        deezer_id: r.get(1)?,
        title: r.get(2)?,
        artist: ArtistRef {
            id: uuid(r.get(3)?),
            name: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        },
        record_type: r.get::<_, String>(5)?.parse().unwrap_or(RecordType::Album),
        release_date: r
            .get::<_, Option<String>>(6)?
            .and_then(|d| d.parse::<NaiveDate>().ok()),
        explicit: r.get(7)?,
        tracks_count: r.get(8)?,
        duration_ms: r.get(9)?,
        image: image(r.get(10)?),
        palette: r
            .get::<_, Option<String>>(11)?
            .and_then(|s| serde_json::from_str(&s).ok()),
        saved: r.get(12)?,
    })
}

impl Store {
    /// Треки по id в том же порядке; неизвестные пропускаются.
    pub fn tracks(&self, ids: &[Uuid]) -> StoreResult<Vec<Track>> {
        let conn = self.conn();
        let mut st = conn.prepare_cached(TRACK_SQL)?;
        let mut artists = conn.prepare_cached(
            "SELECT ta.artist_id, a.name, ta.role FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
             WHERE ta.track_id = ?1 ORDER BY ta.position",
        )?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let id = id.to_string();
            let row = st
                .query_row([&id], |r| {
                    let album_id: Option<String> = r.get(11)?;
                    Ok(Track {
                        id: uuid(r.get(0)?),
                        deezer_id: r.get(1)?,
                        title: r.get(2)?,
                        version: r.get(3)?,
                        duration_ms: r.get(4)?,
                        explicit: r.get(5)?,
                        disc_number: r.get(6)?,
                        track_position: r.get(7)?,
                        recording_id: r.get::<_, Option<String>>(8)?.map(uuid),
                        isrc: r.get(9)?,
                        audio_status: r
                            .get::<_, Option<String>>(10)?
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(AudioStatus::None),
                        album: match album_id {
                            Some(a) => Some(AlbumRef {
                                id: uuid(a),
                                title: r.get(12)?,
                                image: image(r.get(13)?),
                            }),
                            None => None,
                        },
                        artists: Vec::new(),
                        liked: r.get(14)?,
                        clean_id: None,
                    })
                })
                .optional()?;
            let Some(mut track) = row else { continue };
            track.artists = artists
                .query_map([&id], |r| {
                    Ok(TrackArtist {
                        id: uuid(r.get(0)?),
                        name: r.get(1)?,
                        role: r.get::<_, String>(2)?.parse().unwrap_or(ArtistRole::Main),
                    })
                })?
                .collect::<Result<_, _>>()?;
            out.push(track);
        }
        Ok(out)
    }

    fn ids(&self, sql: &str, p: impl rusqlite::Params) -> StoreResult<Vec<Uuid>> {
        let mut st = self.conn().prepare_cached(sql)?;
        let ids = st
            .query_map(p, |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids.into_iter().map(uuid).collect())
    }

    /// Любимые треки, новые первыми. Курсор — смещение.
    pub fn liked(&self, cursor: Option<&str>, limit: usize) -> StoreResult<LikedTrackPage> {
        let offset: i64 = cursor.and_then(|c| c.parse().ok()).unwrap_or(0);
        let mut st = self.conn().prepare_cached(
            "SELECT track_id, liked_at FROM liked_tracks ORDER BY liked_at DESC, track_id LIMIT ?1 OFFSET ?2",
        )?;
        let rows: Vec<(String, String)> = st
            .query_map(params![limit as i64 + 1, offset], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<Result<_, _>>()?;
        let more = rows.len() > limit;
        let rows: Vec<_> = rows.into_iter().take(limit).collect();
        let ids: Vec<Uuid> = rows.iter().map(|(id, _)| uuid(id.clone())).collect();
        let tracks = self.tracks(&ids)?;
        let items = rows
            .iter()
            .filter_map(|(id, at)| {
                let track = tracks.iter().find(|t| t.id.to_string() == *id)?.clone();
                Some(LikedTrack {
                    track,
                    liked_at: time(at),
                })
            })
            .collect();
        Ok(LikedTrackPage {
            items,
            next_cursor: more.then(|| (offset + limit as i64).to_string()),
        })
    }

    pub fn album_summary(&self, id: Uuid) -> StoreResult<Option<AlbumSummary>> {
        Ok(self
            .conn()
            .prepare_cached(ALBUM_SQL)?
            .query_row([id.to_string()], album_row)
            .optional()?)
    }

    pub fn saved_albums(&self) -> StoreResult<SavedAlbumPage> {
        let mut st = self
            .conn()
            .prepare_cached("SELECT album_id, saved_at FROM saved_albums ORDER BY saved_at DESC")?;
        let rows: Vec<(String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut items = Vec::with_capacity(rows.len());
        for (id, at) in rows {
            if let Some(album) = self.album_summary(uuid(id))? {
                items.push(SavedAlbum {
                    album,
                    saved_at: time(&at),
                });
            }
        }
        Ok(SavedAlbumPage {
            items,
            next_cursor: None,
        })
    }

    pub fn artist_summary(&self, id: Uuid) -> StoreResult<Option<ArtistSummary>> {
        Ok(self
            .conn()
            .prepare_cached(
                "SELECT a.id, a.deezer_id, a.name, a.image,
                        (SELECT count(*) FROM albums al WHERE al.artist_id = a.id),
                        EXISTS (SELECT 1 FROM followed_artists f WHERE f.artist_id = a.id)
                 FROM artists a WHERE a.id = ?1",
            )?
            .query_row([id.to_string()], |r| {
                Ok(ArtistSummary {
                    id: uuid(r.get(0)?),
                    deezer_id: r.get(1)?,
                    name: r.get(2)?,
                    image: image(r.get(3)?),
                    fans: None,
                    albums_count: Some(r.get(4)?),
                    followed: r.get(5)?,
                })
            })
            .optional()?)
    }

    /// Треки артиста в базе и сколько из них на сервере.
    fn artist_counts(&self, id: Uuid) -> StoreResult<(i64, i64)> {
        Ok(self
            .conn()
            .prepare_cached(
                "SELECT count(DISTINCT t.id),
                    count(DISTINCT CASE WHEN r.audio_status = 'ready' THEN t.id END)
             FROM track_artists ta JOIN tracks t ON t.id = ta.track_id
             LEFT JOIN recordings r ON r.id = t.recording_id
             WHERE ta.artist_id = ?1",
            )?
            .query_row([id.to_string()], |r| Ok((r.get(1)?, r.get(0)?)))?)
    }

    fn followed_rows(&self, limit: Option<usize>) -> StoreResult<Vec<FollowedArtist>> {
        let mut st = self.conn().prepare_cached(
            "SELECT artist_id, policy, notify, followed_at FROM followed_artists
             ORDER BY followed_at DESC LIMIT ?1",
        )?;
        let rows: Vec<(String, String, bool, String)> = st
            .query_map([limit.map_or(-1, |l| l as i64)], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<Result<_, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, policy, notify, at) in rows {
            let id = uuid(id);
            let Some(artist) = self.artist_summary(id)? else {
                continue;
            };
            let (ready, total) = self.artist_counts(id)?;
            out.push(FollowedArtist {
                artist,
                policy: serde_json::from_str::<DownloadPolicy>(&policy).unwrap_or_default(),
                notify,
                followed_at: time(&at),
                tracks_on_server: ready,
                tracks_total: total,
            });
        }
        Ok(out)
    }

    pub fn followed(&self) -> StoreResult<FollowedArtistPage> {
        Ok(FollowedArtistPage {
            items: self.followed_rows(None)?,
            next_cursor: None,
        })
    }

    fn playlist_summary(&self, id: Uuid) -> StoreResult<Option<PlaylistSummary>> {
        let conn = self.conn();
        let base = conn
            .prepare_cached(
                "SELECT p.id, p.title, p.description, p.kind, p.pinned, p.created_at, p.updated_at,
                        (SELECT count(*) FROM playlist_items i WHERE i.playlist_id = p.id),
                        (SELECT coalesce(sum(t.duration_ms), 0) FROM playlist_items i JOIN tracks t ON t.id = i.track_id
                         WHERE i.playlist_id = p.id),
                        p.folder_id, p.position, p.collaborative, p.owner, p.members, p.friends_can_see
                 FROM playlists p WHERE p.id = ?1",
            )?
            .query_row([id.to_string()], |r| {
                Ok(PlaylistSummary {
                    id: uuid(r.get(0)?),
                    title: r.get(1)?,
                    description: r.get(2)?,
                    kind: r.get::<_, String>(3)?.parse().unwrap_or(PlaylistKind::User),
                    pinned: r.get(4)?,
                    created_at: time(&r.get::<_, String>(5)?),
                    updated_at: time(&r.get::<_, String>(6)?),
                    tracks_count: r.get(7)?,
                    duration_ms: r.get(8)?,
                    covers: Vec::new(),
                    image: None,
                    folder_id: r.get::<_, Option<String>>(9)?.and_then(|f| f.parse().ok()),
                    position: r.get(10)?,
                    played_at: None,
                    generator: None,
                    collaborative: r.get(11)?,
                    owner: r
                        .get::<_, Option<String>>(12)?
                        .and_then(|o| serde_json::from_str::<PersonRef>(&o).ok()),
                    members: serde_json::from_str(&r.get::<_, String>(13)?).unwrap_or_default(),
                    friends_can_see: r.get(14)?,
                    access: PlaylistAccess::Owner,
                })
            })
            .optional()?;
        let Some(mut p) = base else { return Ok(None) };
        // В локальной базе только своё и совместные, где человек участник
        if p.owner.is_some() {
            p.access = PlaylistAccess::Member;
        }
        let mut st = conn.prepare_cached(
            "SELECT al.id, al.image FROM playlist_items i JOIN tracks t ON t.id = i.track_id
             JOIN albums al ON al.id = t.album_id
             WHERE i.playlist_id = ?1 AND al.image IS NOT NULL ORDER BY i.position LIMIT 40",
        )?;
        let mut seen = HashSet::new();
        for row in st.query_map([id.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (album, img) = row?;
            if p.covers.len() < 4
                && seen.insert(album)
                && let Some(img) = image(Some(img))
            {
                p.covers.push(img);
            }
        }
        Ok(Some(p))
    }

    pub fn playlists(&self) -> StoreResult<PlaylistList> {
        let ids = self.ids(
            "SELECT id FROM playlists ORDER BY pinned DESC, updated_at DESC",
            [],
        )?;
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            items.extend(self.playlist_summary(id)?);
        }
        let conn = self.conn();
        let mut st = conn.prepare_cached(
            "SELECT id, title, position, collapsed, created_at, updated_at FROM playlist_folders
             ORDER BY position, id",
        )?;
        let folders = st
            .query_map([], |r| {
                Ok(PlaylistFolder {
                    id: uuid(r.get(0)?),
                    title: r.get(1)?,
                    position: r.get(2)?,
                    collapsed: r.get(3)?,
                    created_at: time(&r.get::<_, String>(4)?),
                    updated_at: time(&r.get::<_, String>(5)?),
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(PlaylistList { items, folders })
    }

    pub fn playlist(&self, id: Uuid) -> StoreResult<Option<PlaylistDetail>> {
        let Some(playlist) = self.playlist_summary(id)? else {
            return Ok(None);
        };
        let mut st = self.conn().prepare_cached(
            "SELECT id, track_id, position, added_at, added_by FROM playlist_items WHERE playlist_id = ?1
             ORDER BY position",
        )?;
        let rows: Vec<(String, String, f64, String, Option<String>)> = st
            .query_map([id.to_string()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?;
        let ids: Vec<Uuid> = rows.iter().map(|r| uuid(r.1.clone())).collect();
        let tracks = self.tracks(&ids)?;
        let items = rows
            .into_iter()
            .filter_map(|(item, track, position, added, by)| {
                let track = tracks.iter().find(|t| t.id.to_string() == track)?.clone();
                Some(PlaylistItem {
                    id: uuid(item),
                    position,
                    added_at: time(&added),
                    track,
                    added_by: by.and_then(|b| b.parse().ok()),
                })
            })
            .collect();
        Ok(Some(PlaylistDetail { playlist, items }))
    }

    pub fn overview(&self) -> StoreResult<LibraryOverview> {
        let conn = self.conn();
        let count = |sql: &str| -> StoreResult<i64> { Ok(conn.query_row(sql, [], |r| r.get(0))?) };
        let counts = LibraryCounts {
            liked_tracks: count("SELECT count(*) FROM liked_tracks")?,
            saved_albums: count("SELECT count(*) FROM saved_albums")?,
            followed_artists: count("SELECT count(*) FROM followed_artists")?,
            playlists: count("SELECT count(*) FROM playlists")?,
            ready_tracks: count(
                "SELECT count(*) FROM tracks t JOIN recordings r ON r.id = t.recording_id
                 WHERE r.audio_status = 'ready' AND (t.id IN (SELECT track_id FROM liked_tracks)
                   OR t.album_id IN (SELECT album_id FROM saved_albums)
                   OR t.id IN (SELECT track_id FROM playlist_items))",
            )?,
        };
        let pinned = self.ids(
            "SELECT id FROM playlists WHERE pinned ORDER BY updated_at DESC",
            [],
        )?;
        let mut pinned_playlists = Vec::new();
        for id in pinned {
            pinned_playlists.extend(self.playlist_summary(id)?);
        }
        Ok(LibraryOverview {
            counts,
            pinned_playlists,
            followed_artists: self.followed_rows(Some(50))?,
        })
    }

    /// Страница альбома, если его треки есть в базе.
    pub fn album(&self, id: Uuid) -> StoreResult<Option<AlbumDetail>> {
        let Some(album) = self.album_summary(id)? else {
            return Ok(None);
        };
        let ids = self.ids(
            "SELECT id FROM tracks WHERE album_id = ?1 ORDER BY disc_number, track_position, title",
            [id.to_string()],
        )?;
        if ids.is_empty() {
            return Ok(None);
        }
        let tracks = self.tracks(&ids)?;
        let (label, genres): (Option<String>, String) = self.conn().query_row(
            "SELECT label, genres FROM albums WHERE id = ?1",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(Some(AlbumDetail {
            total_duration_ms: tracks.iter().map(|t| i64::from(t.duration_ms)).sum(),
            ready_tracks: tracks
                .iter()
                .filter(|t| t.audio_status == AudioStatus::Ready)
                .count() as i64,
            album,
            upc: None,
            label,
            genres: serde_json::from_str(&genres).unwrap_or_default(),
            fans: None,
            tracks,
        }))
    }

    /// Страница артиста из того, что есть в базе: любимое и прослушанное — первым, релизы, подписка.
    pub fn artist(&self, id: Uuid) -> StoreResult<Option<ArtistDetail>> {
        let Some(artist) = self.artist_summary(id)? else {
            return Ok(None);
        };
        let top = self.ids(
            "SELECT t.id FROM track_artists ta JOIN tracks t ON t.id = ta.track_id
             WHERE ta.artist_id = ?1
             ORDER BY EXISTS (SELECT 1 FROM liked_tracks l WHERE l.track_id = t.id) DESC,
                      (SELECT count(*) FROM plays p WHERE p.track_id = t.id) DESC, t.title
             LIMIT 10",
            [id.to_string()],
        )?;
        let album_ids = self.ids(
            "SELECT id FROM albums WHERE artist_id = ?1 ORDER BY release_date DESC",
            [id.to_string()],
        )?;
        let mut releases = Releases::default();
        for a in album_ids {
            if let Some(s) = self.album_summary(a)? {
                match s.record_type {
                    RecordType::Album => releases.albums.push(s),
                    RecordType::Ep => releases.eps.push(s),
                    RecordType::Single => releases.singles.push(s),
                    RecordType::Compile => releases.compilations.push(s),
                }
            }
        }
        let follow = self
            .conn()
            .query_row(
                "SELECT policy, notify, followed_at FROM followed_artists WHERE artist_id = ?1",
                [id.to_string()],
                |r| {
                    Ok(FollowState {
                        policy: serde_json::from_str(&r.get::<_, String>(0)?).unwrap_or_default(),
                        notify: r.get(1)?,
                        followed_at: time(&r.get::<_, String>(2)?),
                    })
                },
            )
            .optional()?;
        let (ready, total) = self.artist_counts(id)?;
        Ok(Some(ArtistDetail {
            artist,
            top_tracks: self.tracks(&top)?,
            releases,
            related: Vec::new(),
            follow,
            tracks_on_server: ready,
            tracks_total: total,
        }))
    }

    /// Поиск по медиатеке и всему, что есть в базе — как поиск по медиатеке на сервере: без регистра и
    /// диакритики, по куску слова, с опечаткой, в другой раскладке и по звучанию («капсайз» — Capsize).
    /// Порядок — по сходству с запросом, при равном — любимое и сохранённое выше.
    pub fn search(&self, s: LocalSearch) -> StoreResult<SearchResults> {
        let mut out = SearchResults {
            scope: SearchScope::Library,
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            top: None,
            suggestion: None,
        };
        if normalize(s.q).is_empty() {
            return Ok(out);
        }
        let query = Arc::new(Query::new(s.q));
        // Отбор — по готовым колонкам латиницы и звучания, той же проверкой, что и на сервере
        let filter = Arc::clone(&query);
        self.conn().create_scalar_function(
            "search_hit",
            2,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            move |ctx| {
                let latin = ctx.get::<String>(0)?;
                let sound = ctx.get::<String>(1)?;
                Ok(filter.hits(&latin, &sound))
            },
        )?;
        let found = self.search_scored(&query);
        self.conn().remove_function("search_hit", 2)?;
        let (tracks, albums, artists) = found?;

        let pick = |mut list: Vec<Scored>| {
            list.sort_by(|a, b| {
                step(b.score)
                    .cmp(&step(a.score))
                    .then(b.mine.cmp(&a.mine))
                    .then(a.name.cmp(&b.name))
            });
            list.truncate(s.limit);
            list
        };
        let titles: Vec<(String, f32)> =
            tracks.iter().map(|t| (t.name.clone(), t.by_name)).collect();
        let (tracks, albums, artists) = (pick(tracks), pick(albums), pick(artists));
        let named = |list: &[Scored]| -> Vec<(Uuid, String, f32)> {
            list.iter()
                .map(|x| (x.id, x.name.clone(), x.by_name))
                .collect()
        };
        let (top, suggestion) = query.verdict(&named(&artists), &named(&albums), &titles);
        out.top = top.map(|t| match t {
            Top::Artist(id) => SearchTop {
                kind: DeezerKind::Artist,
                id,
            },
            Top::Album(id) => SearchTop {
                kind: DeezerKind::Album,
                id,
            },
        });
        out.suggestion = suggestion;
        out.tracks = self.tracks(&tracks.iter().map(|t| t.id).collect::<Vec<_>>())?;
        for a in albums {
            out.albums.extend(self.album_summary(a.id)?);
        }
        for a in artists {
            out.artists.extend(self.artist_summary(a.id)?);
        }
        Ok(out)
    }

    /// Треки, альбомы и артисты, прошедшие отбор `search_hit`, с оценкой сходства.
    fn search_scored(&self, q: &Query) -> StoreResult<(Vec<Scored>, Vec<Scored>, Vec<Scored>)> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT t.id, trim(t.title || ' ' || t.version),
                    COALESCE((SELECT group_concat(a.name, char(31) ORDER BY ta.position)
                              FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                              WHERE ta.track_id = t.id), ''),
                    EXISTS (SELECT 1 FROM liked_tracks l WHERE l.track_id = t.id)
             FROM tracks t
             WHERE search_hit(
                 t.latin || ' ' || COALESCE((SELECT group_concat(a.latin, ' ') FROM track_artists ta
                                             JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id), ''),
                 t.sound || ' ' || COALESCE((SELECT group_concat(a.sound, ' ') FROM track_artists ta
                                             JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id), ''))",
        )?;
        let tracks = st
            .query_map([], |r| {
                let title: String = r.get(1)?;
                let names: String = r.get(2)?;
                let names: Vec<&str> = names.split('\u{1f}').filter(|n| !n.is_empty()).collect();
                Ok(Scored {
                    id: uuid(r.get(0)?),
                    score: q.score_track(&title, &names),
                    by_name: q.score(&title),
                    mine: r.get(3)?,
                    name: title,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut st = conn.prepare(
            "SELECT al.id, al.title, COALESCE(ar.name, ''),
                    EXISTS (SELECT 1 FROM saved_albums s WHERE s.album_id = al.id)
             FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id
             WHERE search_hit(al.latin || ' ' || COALESCE(ar.latin, ''), al.sound || ' ' || COALESCE(ar.sound, ''))",
        )?;
        let albums = st
            .query_map([], |r| {
                let title: String = r.get(1)?;
                let artist: String = r.get(2)?;
                Ok(Scored {
                    id: uuid(r.get(0)?),
                    score: q.score_track(&title, &[artist]),
                    by_name: q.score(&title),
                    mine: r.get(3)?,
                    name: title,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut st = conn.prepare(
            "SELECT a.id, a.name, EXISTS (SELECT 1 FROM followed_artists f WHERE f.artist_id = a.id)
             FROM artists a WHERE search_hit(a.latin, a.sound)",
        )?;
        let artists = st
            .query_map([], |r| {
                let name: String = r.get(1)?;
                let score = q.score(&name);
                Ok(Scored {
                    id: uuid(r.get(0)?),
                    score,
                    by_name: score,
                    mine: r.get(2)?,
                    name,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((tracks, albums, artists))
    }

    /// Трек «не рекомендовать» или его артист.
    pub fn blocked_track(&self, id: Uuid) -> StoreResult<bool> {
        Ok(self.conn().prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM blocked_items WHERE kind = 'track' AND ref_id = ?1)
                 OR EXISTS (SELECT 1 FROM blocked_items b JOIN track_artists ta ON ta.artist_id = b.ref_id
                            WHERE b.kind = 'artist' AND ta.track_id = ?1)",
        )?
        .query_row([id.to_string()], |r| r.get(0))?)
    }
}
