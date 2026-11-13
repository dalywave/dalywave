//! Слияние страниц синхронизации: повторы, порядок `rev`, tombstones, набор устройства, закреплённое.

use chrono::Utc;
use dalywave_protocol::{
    catalog::{ArtistRole, AudioStatus, RecordType, SearchScope},
    library::{DownloadPolicy, PlaylistKind},
    sync::*,
};
use dalywave_store::{LocalSearch, Store};
use serde_json::json;
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn artist(n: u128, rev: i64, name: &str) -> SyncArtist {
    SyncArtist {
        id: id(n),
        rev,
        deezer_id: None,
        name: name.into(),
        image: None,
    }
}

fn album(n: u128, rev: i64, artist: u128, title: &str) -> SyncAlbum {
    SyncAlbum {
        id: id(n),
        rev,
        deezer_id: None,
        title: title.into(),
        artist_id: id(artist),
        record_type: RecordType::Album,
        release_date: None,
        label: Some("Label".into()),
        genres: vec!["Rap".into()],
        explicit: false,
        tracks_count: Some(2),
        duration_ms: Some(400_000),
        image: None,
        palette: None,
    }
}

fn track(n: u128, rev: i64, album: u128, artist: u128, title: &str, pos: i16) -> SyncTrack {
    SyncTrack {
        id: id(n),
        rev,
        deezer_id: None,
        title: title.into(),
        version: String::new(),
        album_id: Some(id(album)),
        disc_number: 1,
        track_position: Some(pos),
        duration_ms: 200_000,
        explicit: false,
        isrc: None,
        recording_id: Some(id(n + 500)),
        artists: vec![SyncTrackArtist {
            artist_id: id(artist),
            role: ArtistRole::Main,
        }],
    }
}

fn recording(track: u128, rev: i64, status: AudioStatus) -> SyncRecording {
    SyncRecording {
        id: id(track + 500),
        rev,
        isrc: None,
        duration_ms: 200_000,
        audio_status: status,
        primary_file_id: None,
    }
}

fn base_page() -> SyncPage {
    SyncPage {
        cursor: "10".into(),
        has_more: false,
        artists: vec![artist(1, 1, "Lowborn"), artist(2, 2, "Ёлка")],
        albums: vec![album(10, 3, 1, "Night Drive")],
        tracks: vec![
            track(100, 4, 10, 1, "Float", 1),
            track(101, 5, 10, 1, "Звезда", 2),
        ],
        recordings: vec![
            recording(100, 6, AudioStatus::Ready),
            recording(101, 7, AudioStatus::Queued),
        ],
        liked_tracks: vec![SyncLikedTrack {
            track_id: id(100),
            rev: 8,
            liked_at: Utc::now(),
        }],
        saved_albums: vec![SyncSavedAlbum {
            album_id: id(10),
            rev: 9,
            saved_at: Utc::now(),
        }],
        followed_artists: vec![SyncFollowedArtist {
            artist_id: id(1),
            rev: 10,
            policy: DownloadPolicy::default(),
            notify: true,
            followed_at: Utc::now(),
        }],
        playlists: vec![SyncPlaylist {
            id: id(20),
            rev: 11,
            title: "Дорога".into(),
            description: String::new(),
            kind: PlaylistKind::User,
            pinned: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            folder_id: None,
            position: 1024.0,
            collaborative: false,
            owner: None,
            members: Vec::new(),
            friends_can_see: true,
        }],
        playlist_items: vec![
            SyncPlaylistItem {
                id: id(30),
                rev: 12,
                playlist_id: id(20),
                track_id: id(101),
                position: 1.0,
                added_at: Utc::now(),
                added_by: None,
            },
            SyncPlaylistItem {
                id: id(31),
                rev: 13,
                playlist_id: id(20),
                track_id: id(100),
                position: 2.0,
                added_at: Utc::now(),
                added_by: None,
            },
        ],
        ..Default::default()
    }
}

#[test]
fn applies_library_and_reads_it_back_in_api_types() {
    let mut store = Store::memory().unwrap();
    assert!(!store.synced().unwrap());
    let a = store.apply(&base_page()).unwrap();
    assert!(a.library);
    assert!(store.synced().unwrap());
    assert_eq!(store.cursor().unwrap().as_deref(), Some("10"));

    let liked = store.liked(None, 50).unwrap();
    assert_eq!(liked.items.len(), 1);
    let t = &liked.items[0].track;
    assert_eq!(t.title, "Float");
    assert!(t.liked);
    assert_eq!(t.audio_status, AudioStatus::Ready);
    assert_eq!(t.artists[0].name, "Lowborn");
    assert_eq!(t.album.as_ref().unwrap().title, "Night Drive");

    let album = store.album(id(10)).unwrap().unwrap();
    assert!(album.album.saved);
    assert_eq!(album.tracks.len(), 2);
    assert_eq!(album.ready_tracks, 1);
    assert_eq!(album.tracks[0].title, "Float");

    let pl = store.playlist(id(20)).unwrap().unwrap();
    assert_eq!(pl.playlist.tracks_count, 2);
    assert_eq!(pl.items[0].track.title, "Звезда", "по позиции");

    let o = store.overview().unwrap();
    assert_eq!(o.counts.liked_tracks, 1);
    assert_eq!(o.counts.saved_albums, 1);
    assert_eq!(o.pinned_playlists.len(), 1);
    assert_eq!(o.followed_artists[0].artist.name, "Lowborn");
    assert_eq!(o.followed_artists[0].tracks_total, 2);
    assert_eq!(o.followed_artists[0].tracks_on_server, 1);

    // Поиск: без регистра, «ё» = «е», по артисту
    let s = store
        .search(LocalSearch {
            q: "звезд",
            limit: 10,
        })
        .unwrap();
    assert_eq!(s.scope, SearchScope::Library);
    assert_eq!(s.tracks.len(), 1);
    let s = store
        .search(LocalSearch {
            q: "елка",
            limit: 10,
        })
        .unwrap();
    assert_eq!(s.artists.len(), 1);
    let s = store
        .search(LocalSearch {
            q: "LOWBORN",
            limit: 10,
        })
        .unwrap();
    assert_eq!(s.tracks.len(), 2, "треки артиста находятся по его имени");
    assert_eq!(s.albums.len(), 1);
}

#[test]
fn older_copy_never_overwrites_newer() {
    let mut store = Store::memory().unwrap();
    store.apply(&base_page()).unwrap();
    // Новое название приходит с большим rev, затем «опоздавшая» старая копия
    let newer = SyncPage {
        cursor: "11".into(),
        tracks: vec![track(100, 40, 10, 1, "Float (new)", 1)],
        ..Default::default()
    };
    store.apply(&newer).unwrap();
    let stale = SyncPage {
        cursor: "12".into(),
        tracks: vec![track(100, 4, 10, 1, "Float", 1)],
        ..Default::default()
    };
    let a = store.apply(&stale).unwrap();
    assert_eq!(a.rows, 0);
    assert_eq!(store.tracks(&[id(100)]).unwrap()[0].title, "Float (new)");
    // Повтор той же строки с тем же rev безопасен
    let a = store.apply(&newer).unwrap();
    assert_eq!(a.rows, 1);
    assert_eq!(store.tracks(&[id(100)]).unwrap()[0].title, "Float (new)");
}

#[test]
fn tombstones_delete_only_older_rows_and_cascade_playlists() {
    let mut store = Store::memory().unwrap();
    store.apply(&base_page()).unwrap();
    // Лайк сняли (rev 50), но строка лайка с rev 60 пришла позже — новый лайк остаётся
    let page = SyncPage {
        cursor: "20".into(),
        liked_tracks: vec![SyncLikedTrack {
            track_id: id(101),
            rev: 60,
            liked_at: Utc::now(),
        }],
        tombstones: vec![
            SyncTombstone {
                rev: 50,
                entity: SyncEntity::LikedTracks,
                key: json!({ "trackId": id(101) }),
                deleted_at: Utc::now(),
            },
            SyncTombstone {
                rev: 51,
                entity: SyncEntity::LikedTracks,
                key: json!({ "trackId": id(100) }),
                deleted_at: Utc::now(),
            },
            SyncTombstone {
                rev: 52,
                entity: SyncEntity::Playlists,
                key: json!({ "id": id(20) }),
                deleted_at: Utc::now(),
            },
        ],
        ..Default::default()
    };
    let a = store.apply(&page).unwrap();
    assert!(a.library);
    let liked = store.liked(None, 50).unwrap();
    assert_eq!(liked.items.len(), 1);
    assert_eq!(liked.items[0].track.id, id(101));
    assert!(store.playlist(id(20)).unwrap().is_none());
    assert_eq!(store.playlists().unwrap().items.len(), 0);
    let n: i64 = store.overview().unwrap().counts.playlists;
    assert_eq!(n, 0);
}

#[test]
fn device_set_and_pins_belong_to_this_device() {
    let mut store = Store::memory().unwrap();
    let me = id(7);
    let other = id(8);
    store.set_device_id(Some(me)).unwrap();
    let mut page = base_page();
    page.device_sets = vec![
        SyncDeviceSetItem {
            track_id: id(100),
            rev: 20,
            device_id: me,
            kind: "favorite".into(),
            score: 0.9,
        },
        SyncDeviceSetItem {
            track_id: id(101),
            rev: 21,
            device_id: other,
            kind: "fresh".into(),
            score: 0.8,
        },
    ];
    page.device_downloads = vec![
        SyncDeviceDownload {
            id: id(40),
            rev: 22,
            device_id: me,
            kind: "album".into(),
            ref_id: Some(id(10)),
        },
        SyncDeviceDownload {
            id: id(41),
            rev: 23,
            device_id: other,
            kind: "liked".into(),
            ref_id: None,
        },
    ];
    let a = store.apply(&page).unwrap();
    assert!(a.reserve && a.pins);
    let reserve = store.reserve().unwrap();
    assert_eq!(reserve.len(), 1, "набор другого устройства не наш");
    let pins = store.pins().unwrap();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].kind, "album");
    assert_eq!(pins[0].server_id, Some(id(40)));
    assert_eq!(store.pinned_tracks().unwrap(), vec![id(100), id(101)]);

    // Чужой tombstone набора не трогает наш, свой — удаляет
    let page = SyncPage {
        cursor: "30".into(),
        tombstones: vec![
            SyncTombstone {
                rev: 70,
                entity: SyncEntity::DeviceSets,
                key: json!({ "deviceId": other, "trackId": id(100) }),
                deleted_at: Utc::now(),
            },
            SyncTombstone {
                rev: 71,
                entity: SyncEntity::DeviceDownloads,
                key: json!({ "id": id(40), "deviceId": me }),
                deleted_at: Utc::now(),
            },
        ],
        ..Default::default()
    };
    store.apply(&page).unwrap();
    assert_eq!(store.reserve().unwrap().len(), 1);
    assert!(store.pins().unwrap().is_empty(), "откреплено на сервере");
    let page = SyncPage {
        cursor: "31".into(),
        tombstones: vec![SyncTombstone {
            rev: 72,
            entity: SyncEntity::DeviceSets,
            key: json!({ "deviceId": me, "trackId": id(100) }),
            deleted_at: Utc::now(),
        }],
        ..Default::default()
    };
    let a = store.apply(&page).unwrap();
    assert!(a.reserve);
    assert!(store.reserve().unwrap().is_empty());
}

#[test]
fn local_pins_wait_for_the_server() {
    let mut store = Store::memory().unwrap();
    store.set_device_id(Some(id(7))).unwrap();
    store.apply(&base_page()).unwrap();
    store.add_pin("playlist", Some(id(20))).unwrap();
    store.add_pin("liked", None).unwrap();
    assert_eq!(store.pending_pins().unwrap().len(), 2);
    assert_eq!(store.pinned_tracks().unwrap(), vec![id(101), id(100)]);
    store.confirm_pin("playlist", Some(id(20)), id(90)).unwrap();
    assert_eq!(store.pending_pins().unwrap().len(), 1);
    // Открепили без сети: неотправленное забыто, отправленное ждёт удаления на сервере
    store.remove_pin("liked", None).unwrap();
    store.remove_pin("playlist", Some(id(20))).unwrap();
    assert!(store.pins().unwrap().is_empty());
    let pending = store.pending_pins().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].pending.as_deref(), Some("remove"));
    // Синхронизация, пришедшая до отправки, не возвращает откреплённое
    let page = SyncPage {
        cursor: "40".into(),
        device_downloads: vec![SyncDeviceDownload {
            id: id(90),
            rev: 80,
            device_id: id(7),
            kind: "playlist".into(),
            ref_id: Some(id(20)),
        }],
        ..Default::default()
    };
    store.apply(&page).unwrap();
    assert!(store.pins().unwrap().is_empty());
    store.forget_pin("playlist", Some(id(20))).unwrap();
    assert!(store.pending_pins().unwrap().is_empty());
}

#[test]
fn features_and_plays_feed_the_offline_wave() {
    let mut store = Store::memory().unwrap();
    let mut page = base_page();
    page.track_features = vec![SyncTrackFeatures {
        recording_id: id(600),
        rev: 30,
        energy: Some(0.8),
        valence: Some(0.4),
        danceability: Some(0.6),
        bpm: Some(128.0),
        key_camelot: Some("8A".into()),
        voice: Some(0.9),
        language: None,
        approachability: Some(0.5),
        relaxed: None,
        party: None,
    }];
    store.apply(&page).unwrap();
    store
        .queue_file(&dalywave_store::NewFile {
            track_id: id(100),
            recording_id: id(600),
            file_id: id(700),
            path: "a/b.m4a".into(),
            size_bytes: 1000,
            sha256: None,
            codec: "opus".into(),
            container: "mp4".into(),
            duration_ms: 200_000,
            loudness_lufs: Some(-9.0),
            true_peak_db: None,
            waveform: None,
        })
        .unwrap();
    assert!(
        store.playable_features().unwrap().is_empty(),
        "недокачанное не играет"
    );
    store
        .set_file_state(id(100), dalywave_store::FileState::Ready, 1000, None)
        .unwrap();
    store
        .record_play(&dalywave_store::PlayRecord {
            track_id: id(100),
            at: Utc::now(),
            played_ms: 5000,
            duration_ms: 200_000,
            skipped: true,
            wave: true,
        })
        .unwrap();
    let f = store.playable_features().unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].energy, Some(0.8));
    assert_eq!(f[0].skips_30d, 1);
    assert!(f[0].liked);
    assert_eq!(f[0].artist_names, vec!["Lowborn".to_string()]);
    assert!(!f[0].blocked);
    // «Без нецензурного»: explicit не играет и в волне без сети
    let mut explicit = track(100, 900, 10, 1, "Devil", 1);
    explicit.explicit = true;
    store
        .apply(&SyncPage {
            cursor: "9".into(),
            tracks: vec![explicit],
            settings: Some(dalywave_protocol::settings::SyncSettings {
                rev: 901,
                settings: dalywave_protocol::settings::UserSettings {
                    hide_explicit: true,
                    ..Default::default()
                },
            }),
            ..Default::default()
        })
        .unwrap();
    assert!(store.playable_features().unwrap()[0].blocked);
    let u = store.usage().unwrap();
    assert_eq!(u.cache, 1000);
    store.add_pin("track", Some(id(100))).unwrap();
    let u = store.usage().unwrap();
    assert_eq!((u.pinned, u.cache), (1000, 0));
}

#[test]
fn wave_presets_follow_rev_and_tombstones() {
    let mut store = Store::memory().unwrap();
    let preset = |n: u128, rev: i64, name: &str, position: f64| SyncWavePreset {
        id: id(n),
        rev,
        name: name.into(),
        icon: "laptop".into(),
        settings: dalywave_protocol::wave::WaveSettings {
            mood: dalywave_protocol::wave::WaveMood::Calm,
            ..Default::default()
        },
        position,
        created_at: Utc::now(),
    };
    let page = SyncPage {
        cursor: "5".into(),
        wave_presets: vec![preset(1, 3, "Работа", 2.0), preset(2, 4, "Ночь", 1.0)],
        ..Default::default()
    };
    store.apply(&page).unwrap();
    let names: Vec<String> = store
        .wave_presets()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["Ночь", "Работа"]);
    assert_eq!(
        store.wave_presets().unwrap()[0].settings.mood,
        dalywave_protocol::wave::WaveMood::Calm
    );
    // Старая копия не затирает переименование; tombstone удаляет только более старое
    let page = SyncPage {
        cursor: "6".into(),
        wave_presets: vec![preset(1, 9, "Фокус", 2.0), preset(2, 2, "Старое", 1.0)],
        tombstones: vec![SyncTombstone {
            rev: 10,
            entity: SyncEntity::WavePresets,
            key: json!({ "id": id(2) }),
            deleted_at: Utc::now(),
        }],
        ..Default::default()
    };
    store.apply(&page).unwrap();
    let names: Vec<String> = store
        .wave_presets()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["Фокус"]);
}

#[test]
fn folders_and_settings_follow_rev_and_tombstones() {
    let mut store = Store::memory().unwrap();
    let playlist = |n: u128, rev: i64, folder: Option<u128>, position: f64| SyncPlaylist {
        id: id(n),
        rev,
        title: format!("P{n}"),
        description: String::new(),
        kind: PlaylistKind::User,
        pinned: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        folder_id: folder.map(id),
        position,
        collaborative: false,
        owner: None,
        members: Vec::new(),
        friends_can_see: true,
    };
    let folder = |rev: i64, title: &str, collapsed: bool| SyncPlaylistFolder {
        id: id(70),
        rev,
        title: title.into(),
        position: 512.0,
        collapsed,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let settings = |rev: i64, hide: bool| dalywave_protocol::settings::SyncSettings {
        rev,
        settings: dalywave_protocol::settings::UserSettings {
            hide_explicit: hide,
            ..Default::default()
        },
    };
    assert!(!store.settings().unwrap().hide_explicit, "по умолчанию");
    let page = SyncPage {
        cursor: "5".into(),
        playlists: vec![
            playlist(1, 3, Some(70), 1024.0),
            playlist(2, 4, None, 2048.0),
        ],
        playlist_folders: vec![folder(5, "Бег", true)],
        settings: Some(settings(6, true)),
        ..Default::default()
    };
    store.apply(&page).unwrap();
    let list = store.playlists().unwrap();
    assert_eq!(list.folders.len(), 1);
    assert_eq!(
        (list.folders[0].title.as_str(), list.folders[0].collapsed),
        ("Бег", true)
    );
    let p1 = list.items.iter().find(|p| p.id == id(1)).unwrap();
    assert_eq!((p1.folder_id, p1.position), (Some(id(70)), 1024.0));
    assert!(store.settings().unwrap().hide_explicit);

    // Старые копии не затирают новое; удаление папки выпускает её плейлисты
    let page = SyncPage {
        cursor: "6".into(),
        playlist_folders: vec![folder(2, "Старое", false)],
        settings: Some(settings(1, false)),
        tombstones: vec![SyncTombstone {
            rev: 10,
            entity: SyncEntity::PlaylistFolders,
            key: json!({ "id": id(70) }),
            deleted_at: Utc::now(),
        }],
        ..Default::default()
    };
    store.apply(&page).unwrap();
    let list = store.playlists().unwrap();
    assert!(list.folders.is_empty());
    assert!(list.items.iter().all(|p| p.folder_id.is_none()));
    assert!(store.settings().unwrap().hide_explicit);
}

#[test]
fn shared_playlist_keeps_owner_members_and_who_added() {
    use dalywave_protocol::social::{PersonRef, PlaylistAccess};
    let mut store = Store::memory().unwrap();
    store.apply(&base_page()).unwrap();
    let masha = PersonRef {
        id: id(900),
        name: "Маша".into(),
        avatar_url: Some("https://daly.rest/a/masha.png".into()),
    };
    let me = PersonRef {
        id: id(901),
        name: "Я".into(),
        avatar_url: None,
    };
    // Совместный плейлист Маши, где я участник: её трек и мой
    let page = SyncPage {
        cursor: "30".into(),
        playlists: vec![SyncPlaylist {
            id: id(40),
            rev: 70,
            title: "Вечер".into(),
            description: String::new(),
            kind: PlaylistKind::User,
            pinned: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            folder_id: None,
            position: -1024.0,
            collaborative: true,
            owner: Some(masha.clone()),
            members: vec![me.clone()],
            friends_can_see: true,
        }],
        playlist_items: vec![
            SyncPlaylistItem {
                id: id(41),
                rev: 71,
                playlist_id: id(40),
                track_id: id(100),
                position: 1.0,
                added_at: Utc::now(),
                added_by: Some(masha.id),
            },
            SyncPlaylistItem {
                id: id(42),
                rev: 72,
                playlist_id: id(40),
                track_id: id(101),
                position: 2.0,
                added_at: Utc::now(),
                added_by: Some(me.id),
            },
        ],
        ..Default::default()
    };
    store.apply(&page).unwrap();
    let pl = store.playlist(id(40)).unwrap().unwrap();
    assert!(pl.playlist.collaborative);
    assert_eq!(pl.playlist.access, PlaylistAccess::Member);
    assert_eq!(pl.playlist.owner.as_ref(), Some(&masha));
    assert_eq!(pl.playlist.members, vec![me.clone()]);
    assert_eq!(pl.items[0].added_by, Some(masha.id));
    assert_eq!(pl.items[1].added_by, Some(me.id));
    // Свой плейлист — свой
    let own = store.playlist(id(20)).unwrap().unwrap();
    assert_eq!(own.playlist.access, PlaylistAccess::Owner);
    assert!(own.playlist.owner.is_none());

    // Меня убрали из плейлиста: tombstone плейлиста удаляет и его позиции
    let gone = SyncPage {
        cursor: "31".into(),
        tombstones: vec![SyncTombstone {
            rev: 80,
            entity: SyncEntity::Playlists,
            key: json!({ "id": id(40) }),
            deleted_at: Utc::now(),
        }],
        ..Default::default()
    };
    store.apply(&gone).unwrap();
    assert!(store.playlist(id(40)).unwrap().is_none());
}
