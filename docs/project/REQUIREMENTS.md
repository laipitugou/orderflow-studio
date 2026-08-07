# Product requirements

## In scope

### Charts and layers

- Standalone candlestick chart with synchronized time and price navigation.
- Historical L2 resting-liquidity heatmap switchable behind candles.
- Actual liquidation-event heatmap/layer.
- Separately labelled modelled liquidation-zone heatmap when implemented.
- Footprint, DOM/ladder, Time & Sales, volume bubbles, and volume profiles.
- Layer visibility and data-source controls.

### Flow metrics

- Bar delta and CVD.
- Spot CVD shown alongside a perpetual-contract chart.
- Composite CVD built from multiple exchanges.
- Per-source visibility, weights, health, contribution, and coverage.
- Base-quantity and quote-notional normalization, defaulting to quote notional.
- OI, OI delta, and OI/price/delta contextual views.

### Price-level analytics

- Bar, session, visible-range, composite, developing, naked, volume, delta, and
  time POC variants.
- A confluence score combining volume, time, repeated absorption, OI changes,
  retests, and subsequent price response.
- Time at price: cumulative dwell, visit count, mean visit duration, and current
  visit duration, with disconnect/staleness truncation.

### Explainable event detection

- Possible iceberg/replenishment.
- Buy/sell absorption and repeated absorption zones.
- Exhaustion, stacked imbalance, failed/unfinished auction, trapped traders,
  CVD/delta divergence, sweeps, and spoofing-like add/pull behaviour.
- Each event stores side, price range, time range, score, evidence components,
  data quality, confirmation state, and invalidation reason.

### Data and replay

- Persist trades, book snapshots/deltas, klines, OI, liquidation events, and
  detector output locally.
- Gap detection, sequence validation, deduplication, backfill, and retention.
- Tick replay using exchange timestamps and deterministic detector execution.
- Partition high-volume data by venue/market/symbol/date and query only visible
  ranges.

### Performance

- Separate ingestion, analytics, persistence, and rendering work.
- Incremental calculations; no full-history rescan on every update.
- In-memory ring buffers for hot data and compressed disk storage for history.
- Aggregation and level-of-detail based on zoom.
- Configurable disk limits and raw-L2 retention.

## Deferred

- TradingView-compatible drawing experience.
- Pine Script compatibility or a custom indicator scripting language.
- Live order execution and account credentials.

## Data truth labels

- `Observed`: directly received exchange event.
- `Derived`: deterministic calculation from observed data, such as CVD or POC.
- `Inferred`: heuristic event such as possible iceberg or absorption.
- `Modelled`: estimated distribution such as prospective liquidation zones.

The UI must not present inferred or modelled output as observed fact.

