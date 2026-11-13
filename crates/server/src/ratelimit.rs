use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use uuid::Uuid;

type Buckets = HashMap<(&'static str, IpAddr), (Instant, u32)>;
type KeyedBuckets = HashMap<(&'static str, Uuid), (Instant, u32)>;
type TextBuckets = HashMap<(&'static str, String), (Instant, u32)>;

/// Лимитер с фиксированным окном в памяти процесса. API работает в одном экземпляре, этого хватает.
#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<Buckets>,
    /// Лимиты по id (гость «Слушать вместе»): у гостей за одним Wi-Fi общий IP
    keyed: Mutex<KeyedBuckets>,
    /// Лимиты по строке: вход своим аккаунтом — по паре «адрес + логин»
    text: Mutex<TextBuckets>,
}

fn hit<K: std::hash::Hash + Eq>(
    map: &Mutex<HashMap<K, (Instant, u32)>>,
    key: K,
    max: u32,
    window: Duration,
) -> bool {
    let now = Instant::now();
    let mut map = map.lock().unwrap_or_else(|e| e.into_inner());
    if map.len() > 10_000 {
        map.retain(|_, (start, _)| now.duration_since(*start) < window);
    }
    let entry = map.entry(key).or_insert((now, 0));
    if now.duration_since(entry.0) >= window {
        *entry = (now, 0);
    }
    if entry.1 >= max {
        return false;
    }
    entry.1 += 1;
    true
}

impl RateLimiter {
    /// `true`, если запрос укладывается в лимит; запрос при этом учитывается.
    pub fn check(&self, bucket: &'static str, ip: IpAddr, max: u32, window: Duration) -> bool {
        hit(&self.buckets, (bucket, ip), max, window)
    }

    /// То же, но лимит — на id (а не на адрес).
    pub fn check_id(&self, bucket: &'static str, id: Uuid, max: u32, window: Duration) -> bool {
        hit(&self.keyed, (bucket, id), max, window)
    }

    /// То же, но лимит — на строку, например `адрес|логин`.
    pub fn check_key(&self, bucket: &'static str, key: &str, max: u32, window: Duration) -> bool {
        hit(&self.text, (bucket, key.to_string()), max, window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_limit_per_ip() {
        let rl = RateLimiter::default();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        for _ in 0..3 {
            assert!(rl.check("t", ip, 3, Duration::from_secs(60)));
        }
        assert!(!rl.check("t", ip, 3, Duration::from_secs(60)));
        assert!(rl.check("other", ip, 3, Duration::from_secs(60)));
        let other: IpAddr = "10.0.0.2".parse().unwrap();
        assert!(rl.check("t", other, 3, Duration::from_secs(60)));
    }
}
