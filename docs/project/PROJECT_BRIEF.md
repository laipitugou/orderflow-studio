# Orderflow Studio project brief

## Objective

Build a Windows-capable, native crypto order-flow workstation that extends the
Flowdepth/Flowsurface foundation with multi-venue analytics, durable local tick
storage, explainable event detection, and clear chart annotations.

The initial implementation focuses on market data and order-flow analytics.
TradingView-style drawing tools and user-authored indicators are intentionally
deferred.

## First delivery slice

1. Preserve Flowdepth's heatmap, footprint, DOM, CVD, OI, cache, bubbles, GEX,
   and Binance replenishment detector.
2. Add a reusable multi-source CVD aggregation core.
3. Allow a perpetual chart to select spot-market CVD sources.
4. Allow multiple exchanges to contribute to one composite CVD.
5. Normalize aggregation to quote notional by default and expose coverage and
   source contribution metadata.
6. Establish project memory so Codex on Windows can resume work from the repo.

## Product principles

- Local-first and replayable.
- Exchange facts and inferred signals are visually distinguishable.
- Every detector has an explainable score and configurable thresholds.
- Missing data produces coverage warnings, never silently fabricated continuity.
- Rendering work is bounded by the visible time and price range.

## Foundation and license

The source tree began from Flowdepth, itself derived from Flowsurface. The
project remains GPL-3.0-or-later. Upstream history and attribution must remain
available. The local Git remote named `upstream` tracks Flowdepth.

