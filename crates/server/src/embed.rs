//! Встраиваемый плеер для чужих сайтов: страница `/embed/{kind}/{id}` (трек, альбом, плейлист,
//! артист) и отрывки по 30 секунд.
//!
//! Страница собирается сервером целиком — без входа, без бандла интерфейса: обложка, название,
//! исполнитель, кнопка отрывка и ссылка «Открыть в DalyWave»; у альбома, плейлиста и артиста — список
//! первых треков. Высоту задаёт сайт (152 — карточка, 352 — со списком), ширина — любая. Тема —
//! `theme=dark|light`, язык — `lang=ru|en|uk` (по началу тега, иначе русский). Плейлист
//! открывается, только если владелец разрешил встраивание.
//!
//! Без входа звучит только отрывок: 30 с с припева, нарезанные воркером из звука на сервере
//! (`embed_clips`), а пока его нет — превью Deezer. Полный файл анонимно не отдаётся.

use std::sync::Arc;

use dalywave_protocol::{
    export::{ClipSource, EmbedClip},
    jobs::{EmbedClipPayload, kind},
    share::ShareKind,
    stream::{AudioCodec, AudioContainer, StreamQuality},
};
use serde::Serialize;
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    catalog::{
        self,
        views::{self, ImageKind, image_set},
    },
    chorus,
    deezer::{Deezer, DeezerError},
    import::audio::{self, AudioError},
    jobs::{
        self as queue, JobRow, NewJob,
        worker::{Context, Done, JobError, JobResult, Outcome},
    },
    renditions,
    share::escape,
    storage::Storage,
};

/// Длина отрывка.
pub const CLIP_MS: i64 = 30_000;
/// Сколько треков списка показывает плеер.
const LIST_MAX: i64 = 100;
/// Сколько живёт ссылка на отрывок.
pub const CLIP_TTL: std::time::Duration = std::time::Duration::from_secs(3600);
/// Нет ffmpeg на этой машине — задача ждёт, не тратя попыток.
const TOOLS_PAUSE: std::time::Duration = std::time::Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("light") => Theme::Light,
            _ => Theme::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ru,
    En,
    Uk,
}

impl Lang {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some(l) if l.starts_with("en") => Lang::En,
            Some(l) if l.starts_with("uk") => Lang::Uk,
            _ => Lang::Ru,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
            Lang::Uk => "uk",
        }
    }

    fn open(self) -> &'static str {
        match self {
            Lang::Ru => "Открыть в DalyWave",
            Lang::En => "Open in DalyWave",
            Lang::Uk => "Відкрити в DalyWave",
        }
    }

    fn preview(self) -> &'static str {
        match self {
            Lang::Ru => "Отрывок",
            Lang::En => "Preview",
            Lang::Uk => "Уривок",
        }
    }

    fn unavailable(self) -> &'static str {
        match self {
            Lang::Ru => "Отрывок недоступен",
            Lang::En => "Preview unavailable",
            Lang::Uk => "Уривок недоступний",
        }
    }

    fn soon(self) -> &'static str {
        match self {
            Lang::Ru => "Отрывок скоро будет",
            Lang::En => "Preview is on its way",
            Lang::Uk => "Уривок скоро буде",
        }
    }

    fn play(self) -> &'static str {
        match self {
            Lang::Ru => "Слушать отрывок",
            Lang::En => "Play preview",
            Lang::Uk => "Слухати уривок",
        }
    }

    fn pause(self) -> &'static str {
        match self {
            Lang::Ru => "Пауза",
            Lang::En => "Pause",
            Lang::Uk => "Пауза",
        }
    }

    fn gone(self) -> &'static str {
        match self {
            Lang::Ru => "Недоступно",
            Lang::En => "Unavailable",
            Lang::Uk => "Недоступно",
        }
    }

    fn tracks(self, n: usize) -> String {
        match self {
            Lang::En => format!("{n} {}", if n == 1 { "track" } else { "tracks" }),
            Lang::Ru => {
                let word = match (n % 10, n % 100) {
                    (1, m) if m != 11 => "трек",
                    (2..=4, m) if !(12..=14).contains(&m) => "трека",
                    _ => "треков",
                };
                format!("{n} {word}")
            }
            Lang::Uk => {
                let word = match (n % 10, n % 100) {
                    (1, m) if m != 11 => "трек",
                    (2..=4, m) if !(12..=14).contains(&m) => "треки",
                    _ => "треків",
                };
                format!("{n} {word}")
            }
        }
    }

    fn kind(self, kind: ShareKind) -> &'static str {
        match (self, kind) {
            (Lang::Ru, ShareKind::Track) => "Трек",
            (Lang::Ru, ShareKind::Album) => "Альбом",
            (Lang::Ru, ShareKind::Playlist) => "Плейлист",
            (Lang::Ru, ShareKind::Artist) => "Артист",
            (Lang::En, ShareKind::Track) => "Track",
            (Lang::En, ShareKind::Album) => "Album",
            (Lang::En, ShareKind::Playlist) => "Playlist",
            (Lang::En, ShareKind::Artist) => "Artist",
            (Lang::Uk, ShareKind::Track) => "Трек",
            (Lang::Uk, ShareKind::Album) => "Альбом",
            (Lang::Uk, ShareKind::Playlist) => "Плейлист",
            (Lang::Uk, ShareKind::Artist) => "Виконавець",
        }
    }
}

/// Трек в списке плеера.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: Uuid,
    pub title: String,
    pub artists: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: i32,
    pub explicit: bool,
}

/// Что показывает плеер.
#[derive(Debug, Clone)]
pub struct Card {
    pub kind: ShareKind,
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    /// Адрес обложки (наш `/api/images/…`, открыт без входа).
    pub image: Option<String>,
    /// Основной цвет обложки: лёгкий оттенок фона.
    pub tint: Option<String>,
    pub items: Vec<Item>,
    /// Ссылка «Открыть в DalyWave».
    pub url: String,
}

fn full_title(title: &str, version: &str) -> String {
    if version.trim().is_empty() {
        title.to_string()
    } else {
        format!("{title} {}", version.trim())
    }
}

fn items(tracks: Vec<dalywave_protocol::catalog::Track>) -> Vec<Item> {
    tracks
        .into_iter()
        .map(|t| Item {
            id: t.id,
            title: full_title(&t.title, &t.version),
            artists: t
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            duration_ms: t.duration_ms,
            explicit: t.explicit,
        })
        .collect()
}

/// Цвет из палитры — только `#rrggbb`: попадает в CSS страницы.
fn tint(palette: Option<Vec<String>>) -> Option<String> {
    palette?.into_iter().next().filter(|c| {
        c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
    })
}

/// Данные плеера. `None` — нет такого или плейлист не разрешён к встраиванию.
pub async fn load(
    db: &PgPool,
    deezer: &Deezer,
    public_url: &str,
    kind: ShareKind,
    id: Uuid,
    lang: Lang,
) -> Result<Option<Card>, sqlx::Error> {
    let nobody = Uuid::nil();
    let url = format!("{public_url}/{kind}/{id}");
    let card = match kind {
        ShareKind::Track => {
            let Some(t) = views::track(db, id, nobody).await? else {
                return Ok(None);
            };
            let palette = match &t.album {
                Some(a) => views::album(db, a.id, nobody)
                    .await?
                    .and_then(|a| a.palette),
                None => None,
            };
            let image = t
                .album
                .as_ref()
                .and_then(|a| a.image.as_ref())
                .map(|i| i.medium.clone());
            let list = items(vec![t]);
            let first = &list[0];
            Card {
                kind,
                id,
                title: first.title.clone(),
                subtitle: first.artists.clone(),
                image,
                tint: tint(palette),
                items: list,
                url,
            }
        }
        ShareKind::Album => {
            // Список треков альбома мог ещё не загружаться: Deezer не ответил — покажем что есть
            if let Err(e) = catalog::ensure_album(db, deezer, id).await {
                tracing::debug!(album_id = %id, error = %e, "embed: album is not loaded");
            }
            let Some(a) = views::album(db, id, nobody).await? else {
                return Ok(None);
            };
            let ids = sqlx::query_scalar!(
                "SELECT id FROM tracks WHERE album_id = $1
                 ORDER BY disc_number, track_position NULLS LAST, id LIMIT $2",
                id,
                LIST_MAX
            )
            .fetch_all(db)
            .await?;
            let year = a
                .release_date
                .map(|d| format!(" · {}", d.format("%Y")))
                .unwrap_or_default();
            Card {
                kind,
                id,
                title: a.title,
                subtitle: format!("{}{year}", a.artist.name),
                image: a.image.map(|i| i.medium),
                tint: tint(a.palette),
                items: items(views::tracks(db, &ids, nobody).await?),
                url,
            }
        }
        ShareKind::Artist => {
            let Some(a) = views::artist(db, id, nobody).await? else {
                return Ok(None);
            };
            let ids = sqlx::query_scalar!(
                "SELECT track_id FROM artist_top_tracks WHERE artist_id = $1 ORDER BY position LIMIT 10",
                id
            )
            .fetch_all(db)
            .await?;
            Card {
                kind,
                id,
                title: a.name,
                subtitle: lang.kind(kind).to_string(),
                image: a.image.map(|i| i.medium),
                tint: None,
                items: items(views::tracks(db, &ids, nobody).await?),
                url,
            }
        }
        ShareKind::Playlist => {
            let Some(p) = sqlx::query!(
                r#"SELECT p.title, p.cover_key,
                          (SELECT count(*) FROM playlist_items i WHERE i.playlist_id = p.id) AS "count!",
                          (SELECT al.id FROM playlist_items i JOIN tracks t ON t.id = i.track_id
                           JOIN albums al ON al.id = t.album_id
                           WHERE i.playlist_id = p.id ORDER BY i.position LIMIT 1) AS first_album
                   FROM playlists p WHERE p.id = $1 AND p.embeddable"#,
                id
            )
            .fetch_optional(db)
            .await?
            else {
                return Ok(None);
            };
            let ids = sqlx::query_scalar!(
                "SELECT track_id FROM playlist_items WHERE playlist_id = $1 ORDER BY position, id LIMIT $2",
                id,
                LIST_MAX
            )
            .fetch_all(db)
            .await?;
            let first = match p.first_album {
                Some(a) => views::album(db, a, nobody).await?,
                None => None,
            };
            let own = image_set(ImageKind::Playlists, id, p.cover_key.as_deref(), None);
            let image = own
                .or_else(|| first.as_ref().and_then(|a| a.image.clone()))
                .map(|i| i.medium);
            Card {
                kind,
                id,
                title: p.title,
                subtitle: format!(
                    "{} · {}",
                    lang.kind(kind),
                    lang.tracks(usize::try_from(p.count).unwrap_or(0))
                ),
                image,
                tint: tint(first.and_then(|a| a.palette)),
                items: items(views::tracks(db, &ids, nobody).await?),
                url,
            }
        }
    };
    Ok(Some(card))
}

// ---------- страница ----------

const CSS: &str = r#"
*{box-sizing:border-box}
html,body{margin:0;height:100%;background:transparent;overflow:hidden}
body{font:14px/1.35 ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;-webkit-font-smoothing:antialiased;-webkit-text-size-adjust:100%}
.dark{--bg:#14171d;--t1:#f3f5f9;--t2:rgba(232,236,244,.68);--t3:rgba(232,236,244,.56);--track:rgba(232,236,244,.16);--fill:rgba(255,255,255,.06);--fill2:rgba(255,255,255,.1);--edge:rgba(255,255,255,.07);--hair:rgba(255,255,255,.08);--on:#08090c;--k:22%}
.light{--bg:#ffffff;--t1:#101218;--t2:rgba(16,18,24,.7);--t3:rgba(16,18,24,.6);--track:rgba(16,18,24,.14);--fill:rgba(16,18,24,.045);--fill2:rgba(16,18,24,.08);--edge:rgba(16,18,24,.1);--hair:rgba(16,18,24,.08);--on:#ffffff;--k:9%}
.card{position:fixed;inset:0;display:flex;flex-direction:column;overflow:hidden;border-radius:12px;background:var(--bg);color:var(--t1);box-shadow:inset 0 0 0 1px var(--edge)}
.card[style]{background:color-mix(in srgb,var(--tint) var(--k),var(--bg))}
a{color:inherit;text-decoration:none}
.head{position:relative;display:flex;gap:14px;padding:14px;height:min(100%,152px);flex:none}
.cover{flex:none;height:100%;aspect-ratio:1;border-radius:8px;object-fit:cover;background:var(--fill2)}
.round{border-radius:50%}
.info{display:flex;flex:1;flex-direction:column;min-width:0;padding-top:2px}
.title{display:block;max-width:calc(100% - 28px);overflow:hidden;font-size:17px;font-weight:650;letter-spacing:-.01em;white-space:nowrap;text-overflow:ellipsis}
.title:hover{text-decoration:underline;text-underline-offset:2px}
.sub{overflow:hidden;margin-top:2px;font-size:13px;color:var(--t2);white-space:nowrap;text-overflow:ellipsis}
.brand{position:absolute;top:12px;right:12px;display:flex;align-items:center;gap:6px;height:24px;padding:0 2px;border-radius:999px;font-size:12px;font-weight:550;color:var(--t2)}
.brand:hover{color:var(--t1)}
.brand span{display:none}
.mark{width:20px;height:20px;flex:none}
.ctl{display:flex;align-items:center;gap:12px;margin-top:auto}
.play{display:grid;flex:none;width:40px;height:40px;place-items:center;padding:0;border:0;border-radius:50%;background:var(--t1);color:var(--on);cursor:pointer;transition:transform .12s cubic-bezier(.16,1,.3,1)}
.play:hover{transform:scale(1.05)}
.play:active{transform:scale(.96)}
.play:focus-visible,.row:focus-visible,.title:focus-visible,.brand:focus-visible{outline:2px solid var(--t1);outline-offset:2px}
.play svg{width:18px;height:18px}
.play .i-pause,.is-playing .play .i-play{display:none}
.is-playing .play .i-pause{display:block}
.is-loading .play{opacity:.7}
.bar{position:relative;flex:1;height:3px;overflow:hidden;border-radius:2px;background:var(--track)}
.bar i{position:absolute;inset:0;background:var(--t1);transform-origin:0 50%;transform:scaleX(0)}
.time{flex:none;min-width:34px;font-size:12px;font-variant-numeric:tabular-nums;color:var(--t3);text-align:right}
.time.note{min-width:0;max-width:45%;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}
.list{display:none;flex:1;min-height:0;margin:0;padding:4px 6px 8px;overflow-y:auto;list-style:none;border-top:1px solid var(--hair);scrollbar-width:thin;scrollbar-color:var(--fill2) transparent}
.row{display:grid;grid-template-columns:28px 1fr auto;align-items:center;gap:10px;width:100%;min-height:44px;padding:4px 10px 4px 6px;border:0;border-radius:6px;background:none;color:inherit;font:inherit;text-align:left;cursor:pointer}
.row:hover{background:var(--fill)}
.n{font-size:13px;font-variant-numeric:tabular-nums;color:var(--t3);text-align:center}
.names{display:flex;flex-direction:column;min-width:0}
.names b,.names span{overflow:hidden;white-space:nowrap;text-overflow:ellipsis}
.names b{font-weight:500}
.names span{font-size:12px;color:var(--t3)}
.e{display:inline-grid;font-style:normal;place-items:center;width:15px;height:15px;margin-left:6px;border-radius:3px;background:var(--fill2);font-size:9px;font-weight:700;vertical-align:1px;color:var(--t2)}
.d{font-size:12px;font-variant-numeric:tabular-nums;color:var(--t3)}
.row.on b{color:var(--t1)}
.row.on .n{color:var(--t1)}
.row.off{opacity:.45}
.eq{display:none;align-items:flex-end;justify-content:center;gap:2px;height:12px}
.eq i{width:3px;height:100%;border-radius:1px;background:var(--t1);animation:eq 1s ease-in-out infinite}
.eq i:nth-child(2){animation-delay:-.4s}
.eq i:nth-child(3){animation-delay:-.7s}
@keyframes eq{0%,100%{transform:scaleY(.3)}50%{transform:scaleY(1)}}
.eq i{transform-origin:50% 100%}
.is-playing .row.on .eq{display:flex}
.is-playing .row.on .num{display:none}
.empty{display:flex;flex:1;align-items:center;justify-content:center;gap:10px;padding:16px;font-size:13px;color:var(--t3);text-align:center}
@media (max-width:419px){.time .w{display:none}}
@media (min-width:420px){.brand span{display:inline}.title{max-width:calc(100% - 150px)}}
@media (max-width:300px){.cover{display:none}}
@media (min-height:240px){
  .list{display:block}
  .single .head{height:100%}
  .single .cover{height:auto;width:min(46%,calc(100vh - 28px));align-self:flex-start}
  .single .title{font-size:22px;white-space:normal;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical}
  .single .info{padding-top:6px}
}
@media (prefers-reduced-motion:reduce){.eq i{animation:none}.play{transition:none}}
"#;

const SCRIPT: &str = r#"
(function(){
var d=JSON.parse(document.getElementById('dw-data').textContent);
var card=document.querySelector('.card');
var bar=document.querySelector('.bar i');
var time=document.querySelector('.time');
var play=document.querySelector('.play');
var rows=[].slice.call(document.querySelectorAll('.row'));
var audio=new Audio();
audio.preload='none';
var cur=-1,len=d.clip/1000,raf=0;
function fmt(s){s=Math.max(0,Math.round(s));return Math.floor(s/60)+':'+String(s%60).padStart(2,'0')}
function idle(){var w=document.createElement('span');w.className='w';w.textContent=d.t.preview+' ';time.replaceChildren(w,fmt(len));time.classList.add('note')}
function note(t){time.textContent=t;time.classList.add('note')}
function state(s){card.classList.toggle('is-playing',s==='playing');card.classList.toggle('is-loading',s==='loading');play.setAttribute('aria-label',s==='playing'?d.t.pause:d.t.play)}
function mark(i){rows.forEach(function(r,k){r.classList.toggle('on',k===i);r.setAttribute('aria-current',k===i?'true':'false')})}
function tick(){var total=Math.min(audio.duration||len,len)||len;var p=Math.min(1,audio.currentTime/total);bar.style.transform='scaleX('+p+')';time.classList.remove('note');time.textContent=fmt(total-audio.currentTime);if(!audio.paused)raf=requestAnimationFrame(tick)}
function stop(){cancelAnimationFrame(raf);state('idle')}
function start(i){
  if(i<0||i>=d.items.length)return;
  if(i===cur&&audio.src){if(audio.paused){audio.play().catch(function(){})}else{audio.pause()}return}
  cur=i;mark(i);state('loading');audio.pause();bar.style.transform='scaleX(0)';
  fetch('/api/embed/tracks/'+d.items[i].id+'/clip').then(function(r){
    if(r.status===200)return r.json();
    throw r.status;
  }).then(function(j){if(cur!==i)return;audio.src=j.url;return audio.play()}).catch(function(e){
    if(cur!==i)return;
    state('idle');audio.removeAttribute('src');
    if(rows[i])rows[i].classList.add('off');
    note(e===503?d.t.soon:d.t.unavailable);
  });
}
audio.addEventListener('playing',function(){state('playing');tick()});
audio.addEventListener('pause',stop);
audio.addEventListener('ended',function(){stop();bar.style.transform='scaleX(0)';if(cur+1<d.items.length&&rows.length){start(cur+1)}else{cur=-1;mark(-1);idle()}});
play.addEventListener('click',function(){start(cur<0?0:cur)});
rows.forEach(function(r,k){r.addEventListener('click',function(){start(k)})});
idle();
})();
"#;

/// Знак DalyWave: волна на градиенте «Сумерек».
const MARK: &str = r##"<svg class="mark" viewBox="0 0 100 100" aria-hidden="true"><defs><linearGradient id="dwg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#4B21FF"/><stop offset=".4" stop-color="#B01CF0"/><stop offset=".74" stop-color="#FF3D7A"/><stop offset="1" stop-color="#FF9A3C"/></linearGradient></defs><rect width="100" height="100" rx="28" fill="url(#dwg)"/><path d="M22 38C24 43 30 64 34 65C38 66 46 46 50 46C54 46 62 66 66 65C70 64 76 43 78 38" fill="none" stroke="#fff" stroke-width="8.5" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;
const PLAY: &str = r#"<svg class="i-play" viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M240 128a15.7 15.7 0 0 1-7.6 13.5L88.3 229.7a16 16 0 0 1-16.2.3A15.9 15.9 0 0 1 64 216.1V39.9a15.9 15.9 0 0 1 8.1-13.8 16 16 0 0 1 16.2.3l144.1 88.1A15.7 15.7 0 0 1 240 128Z"/></svg>"#;
const PAUSE: &str = r#"<svg class="i-pause" viewBox="0 0 256 256" fill="currentColor" aria-hidden="true"><path d="M216 48v160a16 16 0 0 1-16 16h-40a16 16 0 0 1-16-16V48a16 16 0 0 1 16-16h40a16 16 0 0 1 16 16ZM96 32H56a16 16 0 0 0-16 16v160a16 16 0 0 0 16 16h40a16 16 0 0 0 16-16V48a16 16 0 0 0-16-16Z"/></svg>"#;

fn duration(ms: i32) -> String {
    let s = (ms.max(0) + 500) / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// JSON внутри `<script>`: `<` экранируется, чтобы строка с `</script>` не закрыла тег.
fn script_json(v: &serde_json::Value) -> String {
    v.to_string().replace('<', "\\u003c")
}

/// Страница плеера. `nonce` — для встроенного скрипта (CSP).
pub fn render(card: &Card, theme: Theme, lang: Lang, nonce: &str) -> String {
    let theme_class = match theme {
        Theme::Dark => "dark",
        Theme::Light => "light",
    };
    let single = card.kind == ShareKind::Track;
    let tint = card
        .tint
        .as_ref()
        .map(|c| format!(" style=\"--tint:{c}\""))
        .unwrap_or_default();
    let round = if card.kind == ShareKind::Artist {
        " round"
    } else {
        ""
    };
    let cover = match &card.image {
        Some(src) => format!(
            "<img class=\"cover{round}\" src=\"{}\" alt=\"\" width=\"124\" height=\"124\">",
            escape(src)
        ),
        None => format!("<span class=\"cover{round}\"></span>"),
    };
    let url = escape(&card.url);
    let mut list = String::new();
    if !single {
        list.push_str("<ol class=\"list\">");
        let show_artists = card.kind != ShareKind::Album;
        for (i, it) in card.items.iter().enumerate() {
            let explicit = if it.explicit {
                "<i class=\"e\" aria-label=\"E\">E</i>"
            } else {
                ""
            };
            let artists = if show_artists {
                format!("<span>{}</span>", escape(&it.artists))
            } else {
                String::new()
            };
            list.push_str(&format!(
                "<li><button class=\"row\" type=\"button\"><span class=\"n\"><span class=\"num\">{}</span><span class=\"eq\" aria-hidden=\"true\"><i></i><i></i><i></i></span></span><span class=\"names\"><b>{}{explicit}</b>{artists}</span><span class=\"d\">{}</span></button></li>",
                i + 1,
                escape(&it.title),
                duration(it.duration_ms),
            ));
        }
        list.push_str("</ol>");
    }
    let data = json!({
        "items": card.items,
        "clip": CLIP_MS,
        "t": {
            "preview": lang.preview(),
            "unavailable": lang.unavailable(),
            "soon": lang.soon(),
            "play": lang.play(),
            "pause": lang.pause(),
        },
    });
    let disabled = if card.items.is_empty() {
        " disabled"
    } else {
        ""
    };
    format!(
        "<!doctype html><html lang=\"{lang_code}\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"robots\" content=\"noindex\">\
<title>{title} — DalyWave</title><style>{CSS}</style></head>\
<body><main class=\"card {theme_class}{single_class}\"{tint}>\
<div class=\"head\">{cover}<div class=\"info\">\
<a class=\"title\" href=\"{url}\" target=\"_blank\" rel=\"noopener\">{title}</a>\
<div class=\"sub\">{subtitle}</div>\
<div class=\"ctl\"><button class=\"play\" type=\"button\" aria-label=\"{play_label}\"{disabled}>{PLAY}{PAUSE}</button>\
<div class=\"bar\" aria-hidden=\"true\"><i></i></div><span class=\"time\" aria-live=\"polite\"></span></div>\
</div><a class=\"brand\" href=\"{url}\" target=\"_blank\" rel=\"noopener\" aria-label=\"{open}\">{MARK}<span>{open}</span></a></div>\
{list}</main>\
<script type=\"application/json\" id=\"dw-data\">{data}</script>\
<script nonce=\"{nonce}\">{SCRIPT}</script></body></html>",
        lang_code = lang.code(),
        title = escape(&card.title),
        subtitle = escape(&card.subtitle),
        single_class = if single { " single" } else { "" },
        play_label = lang.play(),
        open = lang.open(),
        data = script_json(&data),
    )
}

/// Плеера нет: удалён, не найден или плейлист закрыт для встраивания.
pub fn render_gone(theme: Theme, lang: Lang) -> String {
    let theme_class = match theme {
        Theme::Dark => "dark",
        Theme::Light => "light",
    };
    format!(
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"robots\" content=\"noindex\">\
<title>DalyWave</title><style>{CSS}</style></head><body><main class=\"card {theme_class}\">\
<div class=\"empty\">{MARK}<span>{}</span></div></main></body></html>",
        lang.code(),
        lang.gone(),
    )
}

/// CSP страницы: встраивать можно куда угодно (в том числе в окна приложений Tauri), скрипт —
/// только свой с `nonce`, сеть — только наш API, звук — по подписанной ссылке хранилища или Deezer.
pub fn csp(nonce: &str) -> String {
    format!(
        "default-src 'none'; img-src 'self' https: data:; media-src *; style-src 'unsafe-inline'; \
         script-src 'nonce-{nonce}'; connect-src 'self'; frame-ancestors * tauri:; base-uri 'none'; \
         form-action 'none'"
    )
}

// ---------- отрывки ----------

/// Чем ответить на запрос отрывка.
#[derive(Debug)]
pub enum ClipAnswer {
    Ready(EmbedClip),
    /// Отрывок с припева нарезается, а превью Deezer у трека нет.
    Preparing,
    NotFound,
}

pub fn clip_key(track_id: Uuid, start_ms: i64) -> String {
    format!("embed/{track_id}/{start_ms}.m4a")
}

/// Поставить нарезку отрывка в очередь (одна задача на трек).
pub async fn request_clip(conn: &mut PgConnection, track_id: Uuid) -> Result<(), sqlx::Error> {
    queue::enqueue(
        conn,
        NewJob::new(
            kind::EMBED_CLIP,
            renditions::LANE,
            EmbedClipPayload { track_id },
        )
        .dedupe(format!("embed_clip:{track_id}"))
        .max_attempts(3),
    )
    .await?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ClipError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("хранилище не подписало ссылку: {0}")]
    Storage(String),
    #[error("Deezer не ответил: {0}")]
    Deezer(DeezerError),
}

/// Отрывок трека: готовый с припева, иначе превью Deezer (и нарезка встаёт в очередь, если звук
/// на сервере есть).
pub async fn clip(
    db: &PgPool,
    storage: &Storage,
    deezer: &Deezer,
    track_id: Uuid,
) -> Result<ClipAnswer, ClipError> {
    let ready = sqlx::query!(
        "SELECT storage_key, length_ms FROM embed_clips WHERE track_id = $1",
        track_id
    )
    .fetch_optional(db)
    .await?;
    if let Some(c) = ready {
        let url = storage
            .presign_get(&c.storage_key, CLIP_TTL)
            .await
            .map_err(|e| ClipError::Storage(e.to_string()))?;
        return Ok(ClipAnswer::Ready(EmbedClip {
            url: url.to_string(),
            source: ClipSource::Chorus,
            length_ms: i64::from(c.length_ms),
        }));
    }
    let Some(t) = sqlx::query!(
        r#"SELECT t.deezer_id, r.audio_status AS "audio_status?"
           FROM tracks t LEFT JOIN recordings r ON r.id = t.recording_id WHERE t.id = $1"#,
        track_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(ClipAnswer::NotFound);
    };
    let on_server = t.audio_status.as_deref() == Some("ready");
    if on_server {
        let mut conn = db.acquire().await?;
        request_clip(&mut conn, track_id).await?;
    }
    let preview = match t.deezer_id {
        Some(d) => match deezer.track(d).await {
            Ok(dz) => dz.preview.filter(|p| p.starts_with("https://")),
            Err(DeezerError::NotFound) => None,
            Err(e) if on_server => {
                tracing::warn!(%track_id, error = %e, "deezer preview lookup failed");
                None
            }
            Err(e) => return Err(ClipError::Deezer(e)),
        },
        None => None,
    };
    Ok(match preview {
        Some(url) => ClipAnswer::Ready(EmbedClip {
            url,
            source: ClipSource::Preview,
            length_ms: CLIP_MS,
        }),
        None if on_server => ClipAnswer::Preparing,
        None => ClipAnswer::NotFound,
    })
}

/// Задача `embed_clip`: скачать файл записи, вырезать 30 с с припева, положить в хранилище.
pub async fn run_clip(ctx: Arc<Context>, job: JobRow) -> JobResult {
    let p: EmbedClipPayload = job.payload()?;
    let done = sqlx::query_scalar!(
        "SELECT track_id FROM embed_clips WHERE track_id = $1",
        p.track_id
    )
    .fetch_optional(&ctx.db)
    .await?;
    if done.is_some() {
        return Ok(json!({ "skipped": "exists" }).into());
    }
    let recording =
        sqlx::query_scalar!("SELECT recording_id FROM tracks WHERE id = $1", p.track_id)
            .fetch_optional(&ctx.db)
            .await?
            .flatten();
    let picked = match recording {
        Some(r) => match renditions::pick(&ctx.db, r, StreamQuality::Normal).await? {
            Some(renditions::ApplePick::Ready(file)) => Some(file),
            // Копию для своей музыки делают — отрывок подождёт её
            Some(renditions::ApplePick::Preparing) => {
                return Err(JobError::retry("копия файла ещё готовится"));
            }
            None => None,
        },
        None => None,
    };
    let Some(picked) = picked else {
        return Ok(json!({ "skipped": "not_ready" }).into());
    };
    // Место припева считается по длине трека в каталоге; файл может быть чуть короче
    let file_ms = i64::from(picked.file.duration_ms);
    let start_ms = chorus::starts(&ctx.db, &[p.track_id], CLIP_MS)
        .await?
        .remove(&p.track_id)
        .map(|(at, _)| at)
        .unwrap_or(0)
        .min(file_ms - CLIP_MS)
        .max(0);
    let length_ms = CLIP_MS.min(file_ms - start_ms).max(1000);
    let dir = tempfile::tempdir().map_err(|e| JobError::retry(format!("временная папка: {e}")))?;
    let input = dir.path().join("source");
    let output = dir.path().join("clip.m4a");
    match ctx
        .storage
        .download_to(&picked.file.storage_key, &input)
        .await
    {
        Ok(_) => {}
        Err(crate::storage::StorageError::NotFound) => {
            return Err(JobError::permanent("файла записи нет в хранилище"));
        }
        Err(e) => return Err(e.into()),
    }
    match audio::clip_aac(&input, &output, start_ms, length_ms).await {
        Ok(()) => {}
        Err(AudioError::Missing(tool)) => {
            let msg = format!("на сервере нет {tool}");
            ctx.cooldowns.pause(kind::EMBED_CLIP, TOOLS_PAUSE);
            let worker = job.locked_by.clone().unwrap_or_default();
            queue::postpone(&ctx.db, job.id, &worker, &msg, TOOLS_PAUSE).await?;
            return Ok(Done::Settled(Outcome::Retry(msg)));
        }
        Err(AudioError::Unreadable(e)) => return Err(JobError::permanent(e)),
        Err(AudioError::Failed(e)) => return Err(JobError::retry(e)),
    }
    let key = clip_key(p.track_id, start_ms);
    let size = ctx
        .storage
        .put_file(&key, &output, &AudioContainer::Mp4.mime(AudioCodec::Aac))
        .await? as i64;
    let inserted = sqlx::query_scalar!(
        "INSERT INTO embed_clips (track_id, source_file_id, storage_key, start_ms, length_ms, size_bytes)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (track_id) DO NOTHING
         RETURNING track_id",
        p.track_id,
        picked.file.id,
        key,
        start_ms as i32,
        length_ms as i32,
        size,
    )
    .fetch_optional(&ctx.db)
    .await;
    match inserted {
        Ok(_) => Ok(json!({ "clip": key, "startMs": start_ms, "bytes": size }).into()),
        // Файл записи удалили, пока шла нарезка
        Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => {
            let _ = ctx.storage.delete(&key).await;
            Ok(json!({ "skipped": "source_gone" }).into())
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(kind: ShareKind) -> Card {
        Card {
            kind,
            id: Uuid::nil(),
            title: "Rock & <Roll>".into(),
            subtitle: "A \"B\"".into(),
            image: Some("/api/images/albums/x/500.jpg?v=1".into()),
            tint: Some("#336699".into()),
            items: vec![Item {
                id: Uuid::nil(),
                title: "</script><script>alert(1)</script>".into(),
                artists: "Ann".into(),
                duration_ms: 185_400,
                explicit: true,
            }],
            url: "https://wave.daly.rest/album/00000000-0000-0000-0000-000000000000".into(),
        }
    }

    #[test]
    fn page_escapes_text_and_script_data() {
        let html = render(&card(ShareKind::Album), Theme::Light, Lang::Ru, "abc");
        assert!(html.contains("Rock &amp; &lt;Roll&gt;"));
        assert!(html.contains("A &quot;B&quot;"));
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("\\u003c/script>"));
        assert!(html.contains("class=\"card light\" style=\"--tint:#336699\""));
        assert!(html.contains("<script nonce=\"abc\">"));
        assert!(html.contains("Открыть в DalyWave"));
        assert!(html.contains("3:05"));
        let html = render(&card(ShareKind::Track), Theme::Dark, Lang::En, "abc");
        assert!(html.contains("card dark single"));
        assert!(!html.contains("class=\"list\""));
        assert!(html.contains("Open in DalyWave"));
    }

    #[test]
    fn tints_are_plain_colors_only() {
        assert_eq!(tint(Some(vec!["#a1b2c3".into()])), Some("#a1b2c3".into()));
        assert_eq!(tint(Some(vec!["red;}".into()])), None);
        assert_eq!(tint(Some(vec!["#12345g".into()])), None);
        assert_eq!(tint(None), None);
    }

    #[test]
    fn plural_tracks() {
        assert_eq!(Lang::Ru.tracks(1), "1 трек");
        assert_eq!(Lang::Ru.tracks(3), "3 трека");
        assert_eq!(Lang::Ru.tracks(11), "11 треков");
        assert_eq!(Lang::Ru.tracks(22), "22 трека");
        assert_eq!(Lang::Ru.tracks(25), "25 треков");
        assert_eq!(Lang::En.tracks(1), "1 track");
        assert_eq!(Lang::Uk.tracks(1), "1 трек");
        assert_eq!(Lang::Uk.tracks(3), "3 треки");
        assert_eq!(Lang::Uk.tracks(11), "11 треків");
        assert_eq!(Lang::Uk.tracks(21), "21 трек");
        assert_eq!(Lang::Uk.tracks(22), "22 треки");
        assert_eq!(Lang::Uk.tracks(25), "25 треків");
    }

    #[test]
    fn language_from_query() {
        assert_eq!(Lang::parse(None), Lang::Ru);
        assert_eq!(Lang::parse(Some("ru")), Lang::Ru);
        assert_eq!(Lang::parse(Some("en-GB")), Lang::En);
        assert_eq!(Lang::parse(Some("uk")), Lang::Uk);
        assert_eq!(Lang::parse(Some("uk-UA")), Lang::Uk);
        assert_eq!(Lang::parse(Some("de")), Lang::Ru);
        let html = render(&card(ShareKind::Album), Theme::Dark, Lang::Uk, "abc");
        assert!(html.contains("<html lang=\"uk\">"));
        assert!(html.contains("Відкрити в DalyWave"));
        assert!(render_gone(Theme::Dark, Lang::Uk).contains("Недоступно"));
    }

    #[test]
    fn csp_allows_framing_and_only_own_script() {
        let c = csp("n1");
        assert!(c.contains("frame-ancestors * tauri:"));
        assert!(c.contains("script-src 'nonce-n1'"));
        assert!(c.contains("connect-src 'self'"));
    }
}
