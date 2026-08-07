# Product conversation record

This file preserves the product context needed to continue the project from a
different Codex client. It is a faithful structured record rather than an
export of hidden system/tool messages.

## 2026-08-07 — Comparison workspace and resilient offline behavior

The user clarified that spot and perpetual composite CVD must be visible at the
same time, aligned like CoinAnk for divergence analysis. A detachable order-flow
workspace should stack OI candles, perpetual CVD, and spot CVD. The user also
requested that exchange disconnects preserve access to cached charts instead of
blocking the application, plus large resting-order/add/cancel alerts, repeated
absorption annotations, and POC-rejection markers on the bubble/heatmap view.

The user then selected large resting-order add/pull detection and repeated
absorption clusters as the first anomaly features to implement. The agreed
noise controls are adaptive percentiles, minimum quote notional, proximity to
the market, persistence for additions, and time-separated absorption retests.

## 2026-08-06 — Initial request

The user trades cryptocurrency and uses FlowSurface order-flow data. They asked
whether a stronger platform could add:

- TradingView-like charting and custom indicators;
- OI and CVD;
- iceberg detection;
- more meaningful POC selection;
- repeated absorption locations and the order-flow situations commonly taught
  in order-flow trading guides;
- time spent at each price;
- a separate candlestick chart with switchable resting-order and liquidation
  heatmap background layers;
- local tick downloads/cache to avoid repeatedly fetching the same history;
- an assessment of data availability and performance cost.

They initially requested research only and asked whether GitHub projects already
implemented these features.

## Research response

The research found that current FlowSurface already includes historical DOM
heatmaps, candlesticks, footprint, imbalance, naked POC, DOM, Time & Sales,
profiles, Binance trade backfill, and a bring-your-own Arrow trade server.

Flowdepth was identified as the closest existing open-source foundation. It adds
CVD, OI/trade/kline caches, gap recovery, adaptive volume bubbles, and a
probabilistic Binance USD-M iceberg/replenishment detector. Missing or incomplete
areas include a general detector framework, repeated absorption, time at price,
prospective liquidation modelling, and comprehensive custom chart tooling.

The response distinguished actual liquidation events from modelled liquidation
zones and explained that public market-by-price L2 cannot prove a native hidden
iceberg order.

## Cross-device continuity

The user asked to create a new project and see it from the Windows Codex client.
The response separated source synchronization from chat synchronization:

- source and durable project context should be stored in Git;
- Codex chat visibility depends on account/client/session support;
- local chats may not automatically appear on another computer;
- project memory documents and `AGENTS.md` provide reliable continuity.

## 2026-08-07 — Flowdepth iceberg display

The user asked how Flowdepth marks detected icebergs. The response explained:

- an upward triangle indicates a possible buy iceberg/passive bid absorption;
- a downward triangle indicates a possible sell iceberg/passive ask absorption;
- markers appear on the classic heatmap for Binance USD-M linear perpetuals;
- size reflects score and opacity reflects feed quality;
- defaults require warm-up, at least three refill cycles, score >= 70, and a
  level near touch;
- the marker is probabilistic, not proof of an exchange-native iceberg.

## 2026-08-07 — Authorization to build

The user authorized construction of a new project and confirmed that all earlier
requirements remain desired. They added two requirements:

1. While viewing a perpetual contract, spot-market CVD must be addable to the
   chart.
2. CVD must support aggregation across multiple exchanges.

They explicitly deferred TradingView drawing and indicator functionality and
requested that this conversation be stored in the project folder for Windows
Codex continuity.

## Current implementation direction

The project is named Orderflow Studio for now. It begins from the Flowdepth
source tree under GPL-3.0-or-later, with Flowdepth retained as the `upstream` Git
remote. The first implementation slice is the normalized multi-source CVD core,
followed by source selection, streaming integration, chart presentation, and
coverage diagnostics.
