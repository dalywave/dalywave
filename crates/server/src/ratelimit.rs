use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

type Buckets = HashMap<(&'static str, IpAddr), (Instant, u32)>;

/// Лимитер с фиксированным окном в памяти процесса. API работает в одном экземпляре, этого хватает.
#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<Buckets>,
}

impl RateLimiter {
    /// `true`, если запрос укладывается в лимит; запрос при этом учитывается.
    pub fn check(&self, bucket: &'static str, ip: IpAddr, max: u32, window: Duration) -> bool {
        let now = Instant::now();
        let mut map = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() > 10_000 {
            map.retain(|_, (start, _)| now.duration_since(*start) < window);
        }
        let entry = map.entry((bucket, ip)).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            *entry = (now, 0);
        }
        if entry.1 >= max {
            return false;
        }
        entry.1 += 1;
        true
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
