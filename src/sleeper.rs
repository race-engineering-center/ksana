use std::time::{Duration, Instant};

use super::traits::Sleeper;

#[derive(Default)]
pub struct AdaptiveSleeper {}

impl Sleeper for AdaptiveSleeper {
    fn sleep_ms(&self, ms: u64) {
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(ms - 1));
        while start.elapsed().as_millis() < ms as u128 {
            std::hint::spin_loop();
        }
    }
}

#[allow(dead_code)]
#[derive(Default)]
pub struct SimpleSleeper {}

impl Sleeper for SimpleSleeper {
    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}
