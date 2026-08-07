# Orderflow Studio agent guide

This repository is a new GPL-3.0-or-later project derived from Flowdepth and
Flowsurface. Preserve their copyright and attribution.

## Product direction

- Build a native, local-first crypto order-flow workstation.
- Treat exchange data, inferred events, and modelled estimates as distinct data
  classes in both code and UI.
- Never label probabilistic iceberg, absorption, spoofing, or liquidation-model
  output as confirmed exchange facts.
- Normalize multi-venue quantities before aggregation. The default comparison
  unit is quote notional; never add contracts and base quantity directly.
- Historical data must be gap-aware, deduplicated, locally cached, and loaded by
  visible range rather than fully loaded at startup.
- Drawing tools and user-scripted indicators are deferred until the market-data
  and order-flow analysis layers are stable.

## Engineering rules

- Keep pure analytics in `data/src/orderflow`; do not couple detectors to iced.
- Put exchange-specific parsing and sequencing in `exchange` adapters.
- Add deterministic unit tests for aggregation and detection rules.
- Use exchange timestamps for correlation, with receive time only as a tie-breaker.
- Maintain backward-compatible serde defaults for saved layouts.
- Run `cargo fmt --all -- --check`, `cargo check --workspace --locked`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`, and
  `cargo test --workspace --locked` before shipping.

## Project memory

Read these before making product-level changes:

- `docs/project/PROJECT_BRIEF.md`
- `docs/project/REQUIREMENTS.md`
- `docs/project/DECISIONS.md`
- `docs/project/CONVERSATION.md`

