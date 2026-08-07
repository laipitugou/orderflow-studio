//! Event-time price dwell accounting.
//!
//! The tracker attributes the interval between two coherent market events to
//! the prior price level. Long gaps are capped so disconnects are not mistaken
//! for market acceptance.

use exchange::UnixMs;
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PriceDwell {
    pub total_ms: u64,
    pub visits: u64,
    pub completed_visits: u64,
    pub longest_visit_ms: u64,
    pub current_visit_ms: u64,
}

impl PriceDwell {
    pub fn mean_completed_visit_ms(self) -> Option<f64> {
        (self.completed_visits > 0)
            .then(|| (self.total_ms - self.current_visit_ms) as f64 / self.completed_visits as f64)
    }
}

/// Price is represented as an integer grouped tick supplied by the chart.
pub struct TimeAtPriceTracker {
    max_interval_ms: u64,
    levels: BTreeMap<i64, PriceDwell>,
    current: Option<CurrentVisit>,
}

#[derive(Debug, Clone, Copy)]
struct CurrentVisit {
    level: i64,
    last_event: UnixMs,
    elapsed_ms: u64,
}

impl TimeAtPriceTracker {
    pub fn new(max_interval_ms: u64) -> Self {
        assert!(
            max_interval_ms > 0,
            "time-at-price gap cap must be non-zero"
        );
        Self {
            max_interval_ms,
            levels: BTreeMap::new(),
            current: None,
        }
    }

    pub fn update(&mut self, level: i64, time: UnixMs) {
        if let Some(mut current) = self.current.take() {
            if time < current.last_event {
                self.current = Some(current);
                return;
            }

            let elapsed = time
                .as_u64()
                .saturating_sub(current.last_event.as_u64())
                .min(self.max_interval_ms);
            current.elapsed_ms = current.elapsed_ms.saturating_add(elapsed);
            let dwell = self.levels.entry(current.level).or_default();
            dwell.total_ms = dwell.total_ms.saturating_add(elapsed);
            dwell.current_visit_ms = current.elapsed_ms;

            if current.level == level {
                current.last_event = time;
                self.current = Some(current);
                return;
            }

            dwell.completed_visits = dwell.completed_visits.saturating_add(1);
            dwell.longest_visit_ms = dwell.longest_visit_ms.max(current.elapsed_ms);
            dwell.current_visit_ms = 0;
        }

        let dwell = self.levels.entry(level).or_default();
        dwell.visits = dwell.visits.saturating_add(1);
        self.current = Some(CurrentVisit {
            level,
            last_event: time,
            elapsed_ms: 0,
        });
    }

    /// Break continuity after a sequence gap, reconnect, or stale feed. No time
    /// after the last coherent event is attributed to the market.
    pub fn invalidate_continuity(&mut self) {
        if let Some(current) = self.current.take() {
            let dwell = self.levels.entry(current.level).or_default();
            dwell.completed_visits = dwell.completed_visits.saturating_add(1);
            dwell.longest_visit_ms = dwell.longest_visit_ms.max(current.elapsed_ms);
            dwell.current_visit_ms = 0;
        }
    }

    pub fn levels(&self) -> &BTreeMap<i64, PriceDwell> {
        &self.levels
    }

    pub fn current_level(&self) -> Option<i64> {
        self.current.map(|visit| visit.level)
    }

    pub fn clear(&mut self) {
        self.levels.clear();
        self.current = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_visits_and_dwell_using_event_time() {
        let mut tracker = TimeAtPriceTracker::new(5_000);
        tracker.update(100, UnixMs::new(1_000));
        tracker.update(100, UnixMs::new(2_000));
        tracker.update(101, UnixMs::new(3_500));
        tracker.update(100, UnixMs::new(4_000));

        let first = tracker.levels().get(&100).unwrap();
        assert_eq!(first.total_ms, 2_500);
        assert_eq!(first.visits, 2);
        assert_eq!(first.completed_visits, 1);
        assert_eq!(first.longest_visit_ms, 2_500);
        assert_eq!(first.mean_completed_visit_ms(), Some(2_500.0));
    }

    #[test]
    fn caps_stale_intervals_and_can_break_continuity() {
        let mut tracker = TimeAtPriceTracker::new(2_000);
        tracker.update(100, UnixMs::new(1_000));
        tracker.update(100, UnixMs::new(20_000));
        assert_eq!(tracker.levels().get(&100).unwrap().total_ms, 2_000);

        tracker.invalidate_continuity();
        tracker.update(100, UnixMs::new(50_000));
        assert_eq!(tracker.levels().get(&100).unwrap().visits, 2);
    }

    #[test]
    fn ignores_out_of_order_updates() {
        let mut tracker = TimeAtPriceTracker::new(2_000);
        tracker.update(100, UnixMs::new(2_000));
        tracker.update(101, UnixMs::new(1_000));
        assert_eq!(tracker.current_level(), Some(100));
        assert!(!tracker.levels().contains_key(&101));
    }
}
