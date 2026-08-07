# Delivery roadmap

## Phase 1 — Multi-source flow foundation

- [x] Create independent project memory and retain upstream attribution.
- [x] Define CVD source modes for chart, matching spot, composite spot,
  composite perpetual, and custom source sets.
- [x] Implement deterministic multi-source CVD aggregation with source weights,
  quote/base normalization boundary, contribution metadata, stale-source health,
  and bounded retention.
- [x] Add persisted CVD source/unit controls to the indicator settings.
- [ ] Resolve matching instruments across exchange symbol conventions.
- [ ] Register extra trade streams independently of the main chart.
- [ ] Feed composite buckets into the CVD panel and render source/coverage labels.
- [ ] Cache composite buckets with calculation-version invalidation.

## Phase 2 — Price-level context

- [x] Implement a pure event-time Time-at-Price accumulator with disconnect caps.
- [ ] Render dwell heatmap/profile and visit statistics.
- [ ] Implement developing, composite, time, delta, naked, and confluence POCs.
- [x] Add a detachable OI-candle, perpetual-CVD, spot-CVD comparison workspace.
- [ ] Add OI delta and divergence annotations to the synchronized context panels.

## Phase 3 — Explainable event framework

- [ ] Generalize Flowdepth's iceberg event schema.
- [x] Add adaptive large resting-liquidity add/pull detection and markers.
- [x] Add repeated-absorption clustering from separately confirmed absorption hits.
- [ ] Generalize absorption confirmation beyond Binance Linear replenishment evidence.
- [ ] Add exhaustion, stacked imbalance, auction completion, trapped trader,
  divergence, sweep, and spoofing-like detectors.
- [ ] Add score breakdown, data-quality state, confirmation, and invalidation UI.

## Phase 4 — Liquidation and book layers

- [ ] Store and render exchange-observed liquidation events.
- [ ] Extend durable local storage to raw L2 snapshot/deltas and liquidation data.
- [ ] Add deterministic record/replay for every detector.
- [ ] Implement separately labelled modelled liquidation zones only after model
  assumptions and validation are documented.

## Phase 5 — Performance and Windows delivery

- [ ] Visible-range queries and multi-resolution heatmap tiles.
- [ ] Disk quotas, per-data-class retention, compaction, and cache inspection UI.
- [ ] Windows CI builds, signed release strategy, installer, and migration tests.

## Deferred

- TradingView-style drawing tools.
- User-authored indicator scripting/Pine compatibility.
