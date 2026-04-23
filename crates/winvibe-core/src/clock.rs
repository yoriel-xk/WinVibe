use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// --- Traits ---

pub trait MonotonicClock: Send + Sync {
    fn now_ms(&self) -> u64;
}

pub trait WallClock: Send + Sync {
    fn now(&self) -> time::OffsetDateTime;
}

// --- 生产实现 ---

pub struct RealMonotonicClock {
    epoch: std::time::Instant,
}

impl RealMonotonicClock {
    pub fn new() -> Self {
        Self { epoch: std::time::Instant::now() }
    }
}

impl Default for RealMonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MonotonicClock for RealMonotonicClock {
    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }
}

pub struct RealWallClock;

impl WallClock for RealWallClock {
    fn now(&self) -> time::OffsetDateTime {
        time::OffsetDateTime::now_utc()
    }
}

// --- 测试用 fake ---

pub struct FakeMonotonicClock {
    ms: AtomicU64,
}

impl FakeMonotonicClock {
    pub fn new(start_ms: u64) -> Self {
        Self { ms: AtomicU64::new(start_ms) }
    }

    pub fn advance(&self, d: Duration) {
        self.ms.fetch_add(d.as_millis() as u64, Ordering::Relaxed);
    }

    pub fn set(&self, ms: u64) {
        self.ms.store(ms, Ordering::Relaxed);
    }
}

impl Default for FakeMonotonicClock {
    fn default() -> Self {
        Self::new(0)
    }
}

impl MonotonicClock for FakeMonotonicClock {
    fn now_ms(&self) -> u64 {
        self.ms.load(Ordering::Relaxed)
    }
}

pub struct FakeWallClock {
    inner: std::sync::Mutex<time::OffsetDateTime>,
}

impl FakeWallClock {
    pub fn new(start: time::OffsetDateTime) -> Self {
        Self { inner: std::sync::Mutex::new(start) }
    }

    pub fn advance(&self, d: Duration) {
        let mut guard = self.inner.lock().unwrap();
        *guard = *guard + d;
    }
}

impl Default for FakeWallClock {
    fn default() -> Self {
        Self::new(time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap())
    }
}

impl WallClock for FakeWallClock {
    fn now(&self) -> time::OffsetDateTime {
        *self.inner.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fake_mono_advances() {
        let clock = FakeMonotonicClock::new(1000);
        assert_eq!(clock.now_ms(), 1000);
        clock.advance(Duration::from_secs(5));
        assert_eq!(clock.now_ms(), 6000);
    }

    #[test]
    fn fake_wall_advances() {
        let clock = FakeWallClock::new(
            time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        );
        let t0 = clock.now();
        clock.advance(Duration::from_secs(60));
        let t1 = clock.now();
        assert_eq!((t1 - t0).whole_seconds(), 60);
    }
}
