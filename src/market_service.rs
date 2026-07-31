//! Application-level connectivity tracking for market-data streams.

use exchange::adapter::StreamKind;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Avoids flashing the blocking overlay for very short network hiccups while
/// still making a real disconnection visible quickly.
const OFFLINE_GRACE: Duration = Duration::from_millis(1_500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityPhase {
    Connecting,
    Online,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityTransition {
    None,
    WentOffline,
    Restored,
}

/// Aggregates all independently managed WebSocket subscriptions into one
/// application-level connection state.
///
/// FlowSurface commonly has multiple connections (depth, trades and klines,
/// potentially across several venues). A single `Connected` event must not
/// hide the offline overlay while the other required streams are still down.
#[derive(Debug)]
pub struct MarketConnectivity {
    expected: HashSet<StreamKind>,
    connected: HashSet<StreamKind>,
    phase: ConnectivityPhase,
    incomplete_since: Option<Instant>,
    last_reason: Option<String>,
    had_online_disconnect: bool,
}

impl MarketConnectivity {
    pub fn new() -> Self {
        Self {
            expected: HashSet::new(),
            connected: HashSet::new(),
            phase: ConnectivityPhase::Connecting,
            incomplete_since: None,
            last_reason: None,
            had_online_disconnect: false,
        }
    }

    /// Keeps the tracker aligned with the streams required by the active
    /// dashboard. Removed streams cannot keep the whole application offline.
    pub fn sync_expected(
        &mut self,
        streams: &[StreamKind],
        now: Instant,
    ) -> ConnectivityTransition {
        self.expected = streams.iter().copied().collect();
        self.connected
            .retain(|stream| self.expected.contains(stream));
        self.evaluate(now)
    }

    pub fn record_connected(
        &mut self,
        streams: &[StreamKind],
        now: Instant,
    ) -> ConnectivityTransition {
        // Events can arrive before the next dashboard/tick synchronization.
        // Preserve those streams as expected so startup state remains useful.
        self.expected.extend(streams.iter().copied());
        self.connected.extend(streams.iter().copied());
        self.evaluate(now)
    }

    pub fn record_disconnected(
        &mut self,
        streams: &[StreamKind],
        reason: String,
        now: Instant,
    ) -> ConnectivityTransition {
        if self.phase == ConnectivityPhase::Online {
            self.had_online_disconnect = true;
        }
        self.expected.extend(streams.iter().copied());
        for stream in streams {
            self.connected.remove(stream);
        }
        self.last_reason = Some(reason);
        self.evaluate(now)
    }

    /// Advances the grace timer even when no new WS event is received.
    pub fn tick(&mut self, now: Instant) -> ConnectivityTransition {
        self.evaluate(now)
    }

    pub fn is_online(&self) -> bool {
        self.phase == ConnectivityPhase::Online
    }

    pub fn phase(&self) -> ConnectivityPhase {
        self.phase
    }

    pub fn overlay_visible(&self) -> bool {
        self.phase == ConnectivityPhase::Offline
    }

    pub fn connected_count(&self) -> usize {
        self.connected
            .iter()
            .filter(|stream| self.expected.contains(stream))
            .count()
    }

    pub fn expected_count(&self) -> usize {
        self.expected.len()
    }

    pub fn last_reason(&self) -> Option<&str> {
        self.last_reason.as_deref()
    }

    fn evaluate(&mut self, now: Instant) -> ConnectivityTransition {
        let complete = self.expected.is_empty() || self.expected.is_subset(&self.connected);

        if complete {
            self.incomplete_since = None;
            let restored = self.phase == ConnectivityPhase::Offline || self.had_online_disconnect;
            self.phase = ConnectivityPhase::Online;
            if restored {
                self.had_online_disconnect = false;
                self.last_reason = None;
                ConnectivityTransition::Restored
            } else {
                ConnectivityTransition::None
            }
        } else {
            let since = *self.incomplete_since.get_or_insert(now);
            let grace_elapsed = now
                .checked_duration_since(since)
                .is_some_and(|elapsed| elapsed >= OFFLINE_GRACE);

            if self.phase != ConnectivityPhase::Offline && grace_elapsed {
                self.phase = ConnectivityPhase::Offline;
                ConnectivityTransition::WentOffline
            } else {
                if self.phase != ConnectivityPhase::Offline {
                    self.phase = ConnectivityPhase::Connecting;
                }
                ConnectivityTransition::None
            }
        }
    }
}

impl Default for MarketConnectivity {
    fn default() -> Self {
        Self::new()
    }
}
