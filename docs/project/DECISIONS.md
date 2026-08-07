# Architecture decisions

## ADR-001: Start from Flowdepth

Status: accepted.

Flowdepth already supplies the Rust/iced desktop shell, supported exchange
adapters, charts, persistent cache, CVD, OI, and a documented Binance iceberg
detector. Reusing it materially reduces sequencing and rendering risk while
preserving GPL attribution.

## ADR-002: Composite CVD uses normalized notional

Status: accepted.

The default multi-venue CVD unit is quote notional (`price * base quantity`, or
the already-normalized quote quantity for inverse contracts). Raw contracts,
base quantity, and quote value must not be summed without conversion. A base
quantity mode may be offered only when all selected sources share a compatible
base asset and quantity definition.

## ADR-003: CVD has independent source selection

Status: accepted.

The market shown by the main chart does not dictate the CVD source. A perpetual
chart may display:

- its own perpetual CVD;
- matching spot CVD;
- composite spot CVD;
- composite perpetual CVD; or
- multiple separately labelled CVD series.

## ADR-004: Pure order-flow analytics

Status: accepted.

Aggregation and event detectors live in the data layer and receive normalized
events. They do not depend on iced, allowing deterministic testing and replay.

## ADR-005: No chart scripting in the first phase

Status: accepted.

Drawing tools and custom indicators are postponed while ingestion, persistence,
coverage, aggregation, and detector semantics are stabilized.

