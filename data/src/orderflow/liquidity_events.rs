//! Replayable L2 add/pull detection and repeated-absorption clustering.

use exchange::{
    TickerInfo, UnixMs,
    adapter::MarketKind,
    depth::Depth,
    unit::{Qty, price::Price},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LiquiditySide {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiquidityEventKind {
    LargeAdd,
    LargePull,
    RepeatedAbsorption,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiquidityEvent {
    pub kind: LiquidityEventKind,
    pub side: LiquiditySide,
    pub price: Price,
    pub first_seen: UnixMs,
    pub confirmed_at: UnixMs,
    pub quote_notional: f64,
    pub score: u8,
    pub test_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LiquidityEventConfig {
    pub enabled: bool,
    pub min_quote_notional: f64,
    pub adaptive_percentile: f64,
    pub warmup_changes: usize,
    pub sample_window: usize,
    pub max_distance_bps: f64,
    pub add_persistence_ms: u64,
    pub absorption_cluster_ticks: u16,
    pub absorption_retest_ms: u64,
    pub absorption_required_tests: u16,
    pub retention_ms: u64,
}

impl Default for LiquidityEventConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_quote_notional: 250_000.0,
            adaptive_percentile: 0.98,
            warmup_changes: 32,
            sample_window: 512,
            max_distance_bps: 50.0,
            add_persistence_ms: 750,
            absorption_cluster_ticks: 3,
            absorption_retest_ms: 1_500,
            absorption_required_tests: 2,
            retention_ms: 30 * 60 * 1_000,
        }
    }
}

#[derive(Debug, Clone)]
struct PendingAdd {
    side: LiquiditySide,
    price: Price,
    first_seen: UnixMs,
    notional: f64,
}

#[derive(Debug, Clone)]
struct AbsorptionCluster {
    side: LiquiditySide,
    price: Price,
    first_seen: UnixMs,
    last_test: UnixMs,
    total_notional: f64,
    tests: u16,
    last_emitted_tests: u16,
}

pub struct LiquidityEventDetector {
    config: LiquidityEventConfig,
    ticker: TickerInfo,
    tick_size: exchange::unit::PriceStep,
    previous: Option<Depth>,
    samples: VecDeque<f64>,
    pending_adds: HashMap<(LiquiditySide, Price), PendingAdd>,
    absorption: Vec<AbsorptionCluster>,
    events: VecDeque<LiquidityEvent>,
}

impl LiquidityEventDetector {
    pub fn new(
        config: LiquidityEventConfig,
        ticker: TickerInfo,
        tick_size: exchange::unit::PriceStep,
    ) -> Self {
        Self {
            config,
            ticker,
            tick_size,
            previous: None,
            samples: VecDeque::new(),
            pending_adds: HashMap::new(),
            absorption: Vec::new(),
            events: VecDeque::new(),
        }
    }

    pub fn set_config(&mut self, config: LiquidityEventConfig) {
        self.config = config;
    }
    pub fn events(&self) -> &VecDeque<LiquidityEvent> {
        &self.events
    }

    fn notional(&self, qty: Qty, price: Price) -> f64 {
        let size_is_quote =
            exchange::unit::qty::volume_size_unit() == exchange::unit::qty::SizeUnit::Quote;
        match self.ticker.market_type() {
            MarketKind::InversePerps => qty.to_f64(),
            _ if size_is_quote => qty.to_f64(),
            _ => qty.to_f64() * price.to_f64(),
        }
    }

    pub fn observe_absorption_qty(
        &mut self,
        side: LiquiditySide,
        price: Price,
        qty: Qty,
        now: UnixMs,
    ) -> Option<LiquidityEvent> {
        self.observe_absorption(side, price, self.notional(qty, price), now)
    }

    fn threshold(&self) -> f64 {
        if self.samples.len() < self.config.warmup_changes {
            return self.config.min_quote_notional;
        }
        let mut values = self
            .samples
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        values.sort_by(f64::total_cmp);
        let index = ((values.len().saturating_sub(1)) as f64
            * self.config.adaptive_percentile.clamp(0.5, 0.999))
        .round() as usize;
        self.config.min_quote_notional.max(values[index])
    }

    fn near_mid(&self, price: Price, mid: Price) -> bool {
        if mid.to_f64() <= 0.0 {
            return false;
        }
        ((price.to_f64() - mid.to_f64()).abs() / mid.to_f64()) * 10_000.0
            <= self.config.max_distance_bps
    }

    fn observe_side(
        &mut self,
        side: LiquiditySide,
        old: &BTreeMap<Price, Qty>,
        new: &BTreeMap<Price, Qty>,
        mid: Price,
        now: UnixMs,
    ) {
        for (&price, &new_qty) in new {
            if !self.near_mid(price, mid) {
                continue;
            }
            let old_qty = old.get(&price).copied().unwrap_or(Qty::ZERO);
            if new_qty <= old_qty {
                continue;
            }
            let notional = self.notional(new_qty - old_qty, price);
            self.push_sample(notional);
            if notional >= self.threshold() {
                self.pending_adds
                    .entry((side, price))
                    .or_insert(PendingAdd {
                        side,
                        price,
                        first_seen: now,
                        notional,
                    });
            }
        }
        for (&price, &old_qty) in old {
            if !self.near_mid(price, mid) {
                continue;
            }
            let new_qty = new.get(&price).copied().unwrap_or(Qty::ZERO);
            if old_qty <= new_qty {
                continue;
            }
            let notional = self.notional(old_qty - new_qty, price);
            self.push_sample(notional);
            if notional >= self.threshold() {
                self.events.push_back(LiquidityEvent {
                    kind: LiquidityEventKind::LargePull,
                    side,
                    price,
                    first_seen: now,
                    confirmed_at: now,
                    quote_notional: notional,
                    score: self.score(notional),
                    test_count: 1,
                });
            }
        }
    }

    fn push_sample(&mut self, value: f64) {
        if value.is_finite() && value > 0.0 {
            self.samples.push_back(value);
        }
        while self.samples.len() > self.config.sample_window.max(1) {
            self.samples.pop_front();
        }
    }

    fn score(&self, value: f64) -> u8 {
        let threshold = self.threshold().max(1.0);
        (55.0 + 20.0 * (value / threshold).log2().max(0.0)).clamp(0.0, 100.0) as u8
    }

    pub fn observe_depth(&mut self, depth: &Depth, now: UnixMs) -> Vec<LiquidityEvent> {
        if !self.config.enabled {
            self.previous = Some(depth.clone());
            return vec![];
        }
        let before = self.events.len();
        if let (Some(previous), Some(mid)) = (self.previous.clone(), depth.mid_price()) {
            self.observe_side(LiquiditySide::Bid, &previous.bids, &depth.bids, mid, now);
            self.observe_side(LiquiditySide::Ask, &previous.asks, &depth.asks, mid, now);
            let keys = self.pending_adds.keys().copied().collect::<Vec<_>>();
            for key @ (side, price) in keys {
                let current = match side {
                    LiquiditySide::Bid => depth.bids.get(&price),
                    LiquiditySide::Ask => depth.asks.get(&price),
                }
                .copied()
                .unwrap_or(Qty::ZERO);
                let Some(candidate) = self.pending_adds.get(&key).cloned() else {
                    continue;
                };
                if current.is_zero() || self.notional(current, price) < candidate.notional * 0.5 {
                    self.pending_adds.remove(&key);
                    continue;
                }
                if now.saturating_diff(candidate.first_seen) >= self.config.add_persistence_ms {
                    self.events.push_back(LiquidityEvent {
                        kind: LiquidityEventKind::LargeAdd,
                        side: candidate.side,
                        price: candidate.price,
                        first_seen: candidate.first_seen,
                        confirmed_at: now,
                        quote_notional: candidate.notional,
                        score: self.score(candidate.notional),
                        test_count: 1,
                    });
                    self.pending_adds.remove(&key);
                }
            }
        }
        self.previous = Some(depth.clone());
        self.prune(now);
        self.events
            .iter()
            .skip(before.min(self.events.len()))
            .cloned()
            .collect()
    }

    /// Feed a separately confirmed absorption hit (execution plus failed price
    /// progress). Nearby hits are clustered; time separation prevents one burst
    /// from being counted as multiple tests.
    pub fn observe_absorption(
        &mut self,
        side: LiquiditySide,
        price: Price,
        quote_notional: f64,
        now: UnixMs,
    ) -> Option<LiquidityEvent> {
        if !self.config.enabled || !quote_notional.is_finite() || quote_notional <= 0.0 {
            return None;
        }
        let tolerance =
            self.tick_size.to_f64_lossy() * f64::from(self.config.absorption_cluster_ticks);
        let cluster = self.absorption.iter_mut().find(|cluster| {
            cluster.side == side && (cluster.price.to_f64() - price.to_f64()).abs() <= tolerance
        });
        let cluster = match cluster {
            Some(cluster) => cluster,
            None => {
                self.absorption.push(AbsorptionCluster {
                    side,
                    price,
                    first_seen: now,
                    last_test: now,
                    total_notional: 0.0,
                    tests: 0,
                    last_emitted_tests: 0,
                });
                self.absorption.last_mut().unwrap()
            }
        };
        if cluster.tests == 0
            || now.saturating_diff(cluster.last_test) >= self.config.absorption_retest_ms
        {
            cluster.tests = cluster.tests.saturating_add(1);
            cluster.last_test = now;
        }
        cluster.total_notional += quote_notional;
        if cluster.tests < self.config.absorption_required_tests
            || cluster.tests <= cluster.last_emitted_tests
        {
            return None;
        }
        let event = LiquidityEvent {
            kind: LiquidityEventKind::RepeatedAbsorption,
            side,
            price: cluster.price,
            first_seen: cluster.first_seen,
            confirmed_at: now,
            quote_notional: cluster.total_notional,
            score: (60 + cluster.tests.saturating_sub(2).min(4) * 10).min(100) as u8,
            test_count: cluster.tests,
        };
        cluster.last_emitted_tests = cluster.tests;
        self.events.push_back(event.clone());
        Some(event)
    }

    fn prune(&mut self, now: UnixMs) {
        let cutoff = now.saturating_sub(self.config.retention_ms);
        while self
            .events
            .front()
            .is_some_and(|event| event.confirmed_at < cutoff)
        {
            self.events.pop_front();
        }
        self.absorption
            .retain(|cluster| cluster.last_test >= cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exchange::{
        Ticker,
        adapter::Exchange,
        unit::{MinQtySize, MinTicksize, PriceStep},
    };

    fn detector() -> LiquidityEventDetector {
        let ticker = TickerInfo {
            ticker: Ticker::new("BTCUSDT", Exchange::BinanceLinear),
            min_ticksize: MinTicksize::from(0.1),
            min_qty: MinQtySize::from(0.001),
            contract_size: None,
        };
        let config = LiquidityEventConfig {
            enabled: true,
            min_quote_notional: 10_000.0,
            warmup_changes: 100,
            add_persistence_ms: 500,
            ..LiquidityEventConfig::default()
        };
        LiquidityEventDetector::new(config, ticker, PriceStep::from(ticker.min_ticksize))
    }

    fn book(bid_qty: f64) -> Depth {
        let mut depth = Depth::default();
        depth
            .bids
            .insert(Price::from_f64(99.9), Qty::from_f64(bid_qty));
        depth
            .asks
            .insert(Price::from_f64(100.1), Qty::from_f64(1.0));
        depth
    }

    #[test]
    fn large_add_requires_persistence_and_pull_is_observed() {
        let mut detector = detector();
        detector.observe_depth(&book(1.0), UnixMs::new(0));
        assert!(
            detector
                .observe_depth(&book(201.0), UnixMs::new(100))
                .is_empty()
        );
        let events = detector.observe_depth(&book(201.0), UnixMs::new(700));
        assert_eq!(events[0].kind, LiquidityEventKind::LargeAdd);
        let events = detector.observe_depth(&book(1.0), UnixMs::new(800));
        assert_eq!(events[0].kind, LiquidityEventKind::LargePull);
    }

    #[test]
    fn absorption_burst_needs_separate_retest() {
        let mut detector = detector();
        let price = Price::from_f64(100.0);
        assert!(
            detector
                .observe_absorption(LiquiditySide::Bid, price, 20_000.0, UnixMs::new(0))
                .is_none()
        );
        assert!(
            detector
                .observe_absorption(LiquiditySide::Bid, price, 20_000.0, UnixMs::new(500))
                .is_none()
        );
        let event = detector
            .observe_absorption(LiquiditySide::Bid, price, 20_000.0, UnixMs::new(2_000))
            .unwrap();
        assert_eq!(event.test_count, 2);
        assert_eq!(event.kind, LiquidityEventKind::RepeatedAbsorption);
    }
}
