//! Pure, replayable multi-source CVD aggregation.
//!
//! Exchange adapters are responsible for producing both base quantity and quote
//! notional. Keeping that normalization at the adapter boundary prevents this
//! engine from adding contracts, coins, and dollars as if they were equivalent.

use exchange::{UnixMs, adapter::Exchange};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Which market family should feed a CVD panel independently of the main chart.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum CvdSourceMode {
    /// Preserve legacy behaviour: use the trades belonging to the main chart.
    #[default]
    Chart,
    /// Use matching spot trades, even when the main chart shows a perpetual.
    MatchingSpot,
    /// Combine enabled spot venues for the matching base asset.
    CompositeSpot,
    /// Combine enabled perpetual venues for the matching base asset.
    CompositePerpetual,
    /// Use a separately persisted list of source instruments.
    Custom,
}

impl CvdSourceMode {
    pub const ALL: [Self; 5] = [
        Self::Chart,
        Self::MatchingSpot,
        Self::CompositeSpot,
        Self::CompositePerpetual,
        Self::Custom,
    ];
}

impl std::fmt::Display for CvdSourceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Chart => "Chart market",
            Self::MatchingSpot => "Matching spot",
            Self::CompositeSpot => "Composite spot",
            Self::CompositePerpetual => "Composite perpetual",
            Self::Custom => "Custom sources",
        })
    }
}

/// Common unit used before values from different markets or venues are added.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum CvdAggregationUnit {
    /// Quote value (normally USD/USDT-equivalent). Safe default across contracts
    /// and spot feeds once adapters have applied their contract specifications.
    #[default]
    QuoteNotional,
    /// Base asset quantity. Only valid for compatible instruments sharing a base.
    BaseQuantity,
}

impl CvdAggregationUnit {
    pub const ALL: [Self; 2] = [Self::QuoteNotional, Self::BaseQuantity];
}

impl std::fmt::Display for CvdAggregationUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::QuoteNotional => "Quote notional",
            Self::BaseQuantity => "Base quantity",
        })
    }
}

/// Stable identity for one exchange instrument feeding the composite.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct CvdSourceId {
    pub exchange: Exchange,
    pub symbol: String,
}

impl CvdSourceId {
    pub fn new(exchange: Exchange, symbol: impl Into<String>) -> Self {
        Self {
            exchange,
            symbol: symbol.into(),
        }
    }
}

/// Per-source policy. Weight is intentionally explicit; volume-share weighting
/// can otherwise cause a venue's influence to change silently during outages.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct CvdSourceConfig {
    pub id: CvdSourceId,
    pub enabled: bool,
    pub weight: f64,
}

impl Default for CvdSourceConfig {
    fn default() -> Self {
        Self {
            id: CvdSourceId::new(Exchange::BinanceSpot, "BTCUSDT"),
            enabled: true,
            weight: 1.0,
        }
    }
}

/// A trade normalized by its adapter. Both quantities describe the same trade.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedCvdTrade {
    pub source: CvdSourceId,
    pub time: UnixMs,
    pub is_sell: bool,
    pub base_quantity: f64,
    pub quote_notional: f64,
}

/// Directional totals for one time bucket.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CvdBucket {
    pub buy: f64,
    pub sell: f64,
    pub trade_count: u64,
}

impl CvdBucket {
    pub fn delta(self) -> f64 {
        self.buy - self.sell
    }

    fn add(&mut self, is_sell: bool, value: f64) {
        if is_sell {
            self.sell += value;
        } else {
            self.buy += value;
        }
        self.trade_count = self.trade_count.saturating_add(1);
    }
}

/// Composite output plus enough metadata for coverage and contribution UI.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct CompositeCvdBucket {
    pub total: CvdBucket,
    pub by_source: HashMap<CvdSourceId, CvdBucket>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IngestOutcome {
    Accepted { bucket_start: UnixMs },
    UnknownSource,
    DisabledSource,
    InvalidValue,
}

/// Incremental aggregator used identically by live ingestion and replay.
pub struct CvdAggregator {
    interval_ms: u64,
    unit: CvdAggregationUnit,
    sources: HashMap<CvdSourceId, CvdSourceConfig>,
    buckets: BTreeMap<UnixMs, CompositeCvdBucket>,
    last_event: HashMap<CvdSourceId, UnixMs>,
}

impl CvdAggregator {
    pub fn new(interval_ms: u64, unit: CvdAggregationUnit) -> Self {
        assert!(interval_ms > 0, "CVD interval must be non-zero");
        Self {
            interval_ms,
            unit,
            sources: HashMap::new(),
            buckets: BTreeMap::new(),
            last_event: HashMap::new(),
        }
    }

    pub fn upsert_source(&mut self, source: CvdSourceConfig) {
        self.sources.insert(source.id.clone(), source);
    }

    pub fn remove_source(&mut self, id: &CvdSourceId) {
        self.sources.remove(id);
        self.last_event.remove(id);
    }

    pub fn ingest(&mut self, trade: NormalizedCvdTrade) -> IngestOutcome {
        let Some(source) = self.sources.get(&trade.source) else {
            return IngestOutcome::UnknownSource;
        };
        if !source.enabled {
            return IngestOutcome::DisabledSource;
        }

        let raw_value = match self.unit {
            CvdAggregationUnit::QuoteNotional => trade.quote_notional,
            CvdAggregationUnit::BaseQuantity => trade.base_quantity,
        };
        let value = raw_value * source.weight;
        if !value.is_finite() || value < 0.0 || !source.weight.is_finite() || source.weight < 0.0 {
            return IngestOutcome::InvalidValue;
        }

        let bucket_time = (trade.time.as_u64() / self.interval_ms) * self.interval_ms;
        let bucket_start = UnixMs::new(bucket_time);
        let bucket = self.buckets.entry(bucket_start).or_default();
        bucket.total.add(trade.is_sell, value);
        bucket
            .by_source
            .entry(trade.source.clone())
            .or_default()
            .add(trade.is_sell, value);
        self.last_event.insert(trade.source, trade.time);

        IngestOutcome::Accepted { bucket_start }
    }

    pub fn buckets(&self) -> &BTreeMap<UnixMs, CompositeCvdBucket> {
        &self.buckets
    }

    pub fn source_is_stale(&self, id: &CvdSourceId, now: UnixMs, stale_after_ms: u64) -> bool {
        self.last_event
            .get(id)
            .is_none_or(|last| now.as_u64().saturating_sub(last.as_u64()) > stale_after_ms)
    }

    pub fn clear_before(&mut self, cutoff: UnixMs) {
        self.buckets = self.buckets.split_off(&cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade(
        source: &CvdSourceId,
        time: u64,
        is_sell: bool,
        base: f64,
        quote: f64,
    ) -> NormalizedCvdTrade {
        NormalizedCvdTrade {
            source: source.clone(),
            time: UnixMs::new(time),
            is_sell,
            base_quantity: base,
            quote_notional: quote,
        }
    }

    #[test]
    fn combines_spot_venues_in_quote_notional() {
        let binance = CvdSourceId::new(Exchange::BinanceSpot, "BTCUSDT");
        let bybit = CvdSourceId::new(Exchange::BybitSpot, "BTCUSDT");
        let mut aggregate = CvdAggregator::new(60_000, CvdAggregationUnit::QuoteNotional);
        aggregate.upsert_source(CvdSourceConfig {
            id: binance.clone(),
            enabled: true,
            weight: 1.0,
        });
        aggregate.upsert_source(CvdSourceConfig {
            id: bybit.clone(),
            enabled: true,
            weight: 1.0,
        });

        aggregate.ingest(trade(&binance, 1_000, false, 0.1, 6_000.0));
        aggregate.ingest(trade(&bybit, 2_000, true, 0.05, 3_000.0));

        let bucket = aggregate.buckets().get(&UnixMs::new(0)).unwrap();
        assert_eq!(bucket.total.buy, 6_000.0);
        assert_eq!(bucket.total.sell, 3_000.0);
        assert_eq!(bucket.total.delta(), 3_000.0);
        assert_eq!(bucket.by_source.len(), 2);
    }

    #[test]
    fn supports_weighting_and_base_quantity_mode() {
        let source = CvdSourceId::new(Exchange::BinanceSpot, "ETHUSDT");
        let mut aggregate = CvdAggregator::new(1_000, CvdAggregationUnit::BaseQuantity);
        aggregate.upsert_source(CvdSourceConfig {
            id: source.clone(),
            enabled: true,
            weight: 0.5,
        });
        aggregate.ingest(trade(&source, 1_250, false, 4.0, 12_000.0));

        let bucket = aggregate.buckets().get(&UnixMs::new(1_000)).unwrap();
        assert_eq!(bucket.total.buy, 2.0);
    }

    #[test]
    fn rejects_unknown_disabled_and_invalid_sources() {
        let known = CvdSourceId::new(Exchange::OkexSpot, "BTC-USDT");
        let unknown = CvdSourceId::new(Exchange::MexcSpot, "BTCUSDT");
        let mut aggregate = CvdAggregator::new(1_000, CvdAggregationUnit::QuoteNotional);
        aggregate.upsert_source(CvdSourceConfig {
            id: known.clone(),
            enabled: false,
            weight: 1.0,
        });

        assert_eq!(
            aggregate.ingest(trade(&unknown, 0, false, 1.0, 1.0)),
            IngestOutcome::UnknownSource
        );
        assert_eq!(
            aggregate.ingest(trade(&known, 0, false, 1.0, 1.0)),
            IngestOutcome::DisabledSource
        );

        aggregate.upsert_source(CvdSourceConfig {
            id: known.clone(),
            enabled: true,
            weight: -1.0,
        });
        assert_eq!(
            aggregate.ingest(trade(&known, 0, false, 1.0, 1.0)),
            IngestOutcome::InvalidValue
        );
    }

    #[test]
    fn reports_stale_sources_and_prunes_old_buckets() {
        let source = CvdSourceId::new(Exchange::BinanceLinear, "BTCUSDT");
        let mut aggregate = CvdAggregator::new(1_000, CvdAggregationUnit::QuoteNotional);
        aggregate.upsert_source(CvdSourceConfig {
            id: source.clone(),
            enabled: true,
            weight: 1.0,
        });
        aggregate.ingest(trade(&source, 1_000, false, 1.0, 10.0));
        aggregate.ingest(trade(&source, 2_000, false, 1.0, 10.0));

        assert!(!aggregate.source_is_stale(&source, UnixMs::new(2_500), 2_000));
        assert!(aggregate.source_is_stale(&source, UnixMs::new(4_001), 2_000));
        aggregate.clear_before(UnixMs::new(2_000));
        assert_eq!(aggregate.buckets().len(), 1);
        assert!(aggregate.buckets().contains_key(&UnixMs::new(2_000)));
    }
}
