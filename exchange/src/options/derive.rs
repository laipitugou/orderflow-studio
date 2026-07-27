use super::{OptionContractMatchKey, OptionRight, OptionsUnderlying};
use crate::{UnixMs, adapter};
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use thiserror::Error;

const PRODUCTION_BASE_URL: &str = "https://api.lyra.finance";
const PAGE_SIZE: u16 = 1_000;
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum DeriveMakerSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DeriveOptionInstrument {
    pub instrument_name: String,
    pub key: OptionContractMatchKey,
    pub expiration_timestamp: UnixMs,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DeriveMakerTrade {
    pub trade_id: String,
    pub key: OptionContractMatchKey,
    pub expiration_timestamp: UnixMs,
    pub timestamp: UnixMs,
    pub side: DeriveMakerSide,
    pub amount: f64,
    pub mark_price: f64,
    pub index_price: f64,
}

impl DeriveMakerTrade {
    pub fn is_semantically_valid(&self) -> bool {
        !self.trade_id.is_empty()
            && self.expiration_timestamp.as_u64() > 0
            && self.timestamp.as_u64() > 0
            && self.amount.is_finite()
            && self.amount > 0.0
            && self.mark_price.is_finite()
            && self.mark_price >= 0.0
            && self.index_price.is_finite()
            && self.index_price > 0.0
    }
}

#[derive(Debug, Error)]
pub enum DeriveError {
    #[error("failed to build Derive HTTP client: {0}")]
    Client(#[source] reqwest::Error),
    #[error("Derive HTTP request failed: {0}")]
    Request(#[source] reqwest::Error),
    #[error("Derive returned HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("invalid Derive JSON response: {0}")]
    Decode(#[source] serde_json::Error),
    #[error("Derive JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("Derive JSON-RPC response did not contain a result")]
    MissingResult,
}

#[derive(Debug, Clone)]
pub struct DeriveOptionsClient {
    client: Client,
    base_url: String,
}

impl DeriveOptionsClient {
    pub fn new(proxy: Option<&adapter::Proxy>) -> Result<Self, DeriveError> {
        Self::with_base_url(PRODUCTION_BASE_URL, proxy)
    }

    pub fn with_base_url(
        base_url: impl Into<String>,
        proxy: Option<&adapter::Proxy>,
    ) -> Result<Self, DeriveError> {
        let builder = Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT);
        let client = adapter::proxy::try_apply_proxy(builder, proxy)
            .build()
            .map_err(DeriveError::Client)?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        })
    }

    pub async fn fetch_instruments(
        &self,
        underlying: OptionsUnderlying,
    ) -> Result<Vec<DeriveOptionInstrument>, DeriveError> {
        let mut page = 1u32;
        let mut result = Vec::new();
        loop {
            let params = InstrumentsParams {
                instrument_type: "option",
                currency: underlying.as_str(),
                expired: false,
                page,
                page_size: PAGE_SIZE,
            };
            let response: InstrumentsResult =
                self.rpc("public/get_all_instruments", &params).await?;
            result.extend(
                response
                    .instruments
                    .into_iter()
                    .filter_map(|item| item.into_model(underlying)),
            );
            if page >= response.pagination.num_pages.max(1) {
                break;
            }
            page += 1;
        }
        result.sort_by(|a, b| a.instrument_name.cmp(&b.instrument_name));
        result.dedup_by(|a, b| a.instrument_name == b.instrument_name);
        Ok(result)
    }

    pub async fn fetch_trade_history(
        &self,
        underlying: OptionsUnderlying,
        from_timestamp: UnixMs,
        to_timestamp: UnixMs,
        instruments: &[DeriveOptionInstrument],
    ) -> Result<Vec<DeriveMakerTrade>, DeriveError> {
        let by_name = instruments
            .iter()
            .map(|instrument| (instrument.instrument_name.as_str(), instrument))
            .collect::<HashMap<_, _>>();
        let mut page = 1u32;
        let mut result = Vec::new();
        let mut trade_ids = HashSet::new();
        loop {
            let params = TradeHistoryParams {
                instrument_type: "option",
                currency: underlying.as_str(),
                tx_status: "settled",
                from_timestamp: from_timestamp.as_u64(),
                to_timestamp: to_timestamp.as_u64(),
                page,
                page_size: PAGE_SIZE,
            };
            let response: TradeHistoryResult =
                self.rpc("public/get_trade_history", &params).await?;
            for item in response.trades {
                let Some(instrument) = by_name.get(item.instrument_name.as_str()) else {
                    continue;
                };
                if item.tx_status != "settled" || item.liquidity_role != "maker" {
                    continue;
                }
                let Some(trade) = item.into_model(instrument) else {
                    continue;
                };
                if trade.timestamp < from_timestamp || trade.timestamp > to_timestamp {
                    continue;
                }
                if trade_ids.insert(trade.trade_id.clone()) {
                    result.push(trade);
                }
            }
            if page >= response.pagination.num_pages.max(1) {
                break;
            }
            page += 1;
        }
        result.sort_by_key(|trade| trade.timestamp);
        Ok(result)
    }

    async fn rpc<P: Serialize, T: DeserializeOwned>(
        &self,
        method: &'static str,
        params: &P,
    ) -> Result<T, DeriveError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: "flowsurface",
            method,
            params,
        };
        let response = self
            .client
            .post(format!("{}/{}", self.base_url, method))
            .json(&request)
            .send()
            .await
            .map_err(DeriveError::Request)?;
        let status = response.status();
        let body = response.text().await.map_err(DeriveError::Request)?;
        if !status.is_success() {
            return Err(DeriveError::Http {
                status: status.as_u16(),
                message: body.chars().take(256).collect(),
            });
        }
        let envelope: JsonRpcResponse<T> =
            serde_json::from_str(&body).map_err(DeriveError::Decode)?;
        if envelope.jsonrpc.as_deref() != Some("2.0") {
            return Err(DeriveError::MissingResult);
        }
        if let Some(error) = envelope.error {
            return Err(DeriveError::Rpc {
                code: error.code,
                message: error.message,
            });
        }
        envelope.result.ok_or(DeriveError::MissingResult)
    }
}

#[derive(Serialize)]
struct JsonRpcRequest<'a, P> {
    jsonrpc: &'static str,
    id: &'static str,
    method: &'static str,
    params: &'a P,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    jsonrpc: Option<String>,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Serialize)]
struct InstrumentsParams<'a> {
    instrument_type: &'static str,
    currency: &'a str,
    expired: bool,
    page: u32,
    page_size: u16,
}

#[derive(Deserialize)]
struct InstrumentsResult {
    instruments: Vec<InstrumentDto>,
    pagination: Pagination,
}

#[derive(Deserialize)]
struct Pagination {
    num_pages: u32,
}

#[derive(Deserialize)]
struct InstrumentDto {
    instrument_name: String,
    instrument_type: String,
    base_currency: String,
    is_active: bool,
    option_details: Option<OptionDetailsDto>,
}

#[derive(Deserialize)]
struct OptionDetailsDto {
    expiry: u64,
    strike: String,
    option_type: String,
}

impl InstrumentDto {
    fn into_model(self, underlying: OptionsUnderlying) -> Option<DeriveOptionInstrument> {
        if !self.is_active
            || self.instrument_type != "option"
            || self.base_currency != underlying.as_str()
        {
            return None;
        }
        let details = self.option_details?;
        let expiration_timestamp = UnixMs::new(details.expiry.checked_mul(1_000)?);
        if expiration_timestamp <= UnixMs::now() {
            return None;
        }
        let strike = parse_finite(&details.strike)?;
        let right = match details.option_type.as_str() {
            "C" | "call" => OptionRight::Call,
            "P" | "put" => OptionRight::Put,
            _ => return None,
        };
        let key = OptionContractMatchKey::new(underlying, expiration_timestamp, strike, right)?;
        Some(DeriveOptionInstrument {
            instrument_name: self.instrument_name,
            key,
            expiration_timestamp,
        })
    }
}

#[derive(Serialize)]
struct TradeHistoryParams<'a> {
    instrument_type: &'static str,
    currency: &'a str,
    tx_status: &'static str,
    from_timestamp: u64,
    to_timestamp: u64,
    page: u32,
    page_size: u16,
}

#[derive(Deserialize)]
struct TradeHistoryResult {
    trades: Vec<TradeDto>,
    pagination: Pagination,
}

#[derive(Deserialize)]
struct TradeDto {
    trade_id: String,
    instrument_name: String,
    timestamp: u64,
    direction: String,
    liquidity_role: String,
    trade_amount: String,
    mark_price: String,
    index_price: String,
    tx_status: String,
}

impl TradeDto {
    fn into_model(self, instrument: &DeriveOptionInstrument) -> Option<DeriveMakerTrade> {
        let side = match self.direction.as_str() {
            "buy" => DeriveMakerSide::Buy,
            "sell" => DeriveMakerSide::Sell,
            _ => return None,
        };
        let amount = parse_finite(&self.trade_amount)?;
        let mark_price = parse_finite(&self.mark_price)?;
        let index_price = parse_finite(&self.index_price)?;
        if self.trade_id.is_empty()
            || self.timestamp == 0
            || amount <= 0.0
            || mark_price < 0.0
            || index_price <= 0.0
        {
            return None;
        }
        Some(DeriveMakerTrade {
            trade_id: self.trade_id,
            key: instrument.key,
            expiration_timestamp: instrument.expiration_timestamp,
            timestamp: UnixMs::new(self.timestamp),
            side,
            amount,
            mark_price,
            index_price,
        })
    }
}

fn parse_finite(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::io::{Read, Write};

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/derive_option_history.json"
        ))
        .expect("derive fixture")
    }

    #[test]
    fn option_details_are_the_authoritative_match_key() {
        let page: JsonRpcResponse<InstrumentsResult> =
            serde_json::from_value(fixture()["instruments_page_1"].clone()).expect("page");
        let instrument = page.result.expect("result").instruments.remove(0);
        let model = instrument
            .into_model(OptionsUnderlying::Btc)
            .expect("valid option");
        assert_eq!(model.key.right, OptionRight::Call);
        assert_eq!(model.key.strike_cents, 10_000_001);
        assert_eq!(model.key.expiry_utc_day, 22_095);
        assert_eq!(model.expiration_timestamp, UnixMs::new(1_909_008_000_000));
    }

    #[tokio::test]
    async fn paginates_and_keeps_only_unique_settled_maker_trades() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let fixture = fixture();
        let pages = [
            fixture["instruments_page_1"].to_string(),
            fixture["instruments_page_2"].to_string(),
            fixture["trades_page_1"].to_string(),
            fixture["trades_page_2"].to_string(),
        ];
        let server = std::thread::spawn(move || {
            for (index, body) in pages.into_iter().enumerate() {
                let (mut stream, _) = listener.accept().expect("connection");
                let mut request = [0u8; 16_384];
                let count = stream.read(&mut request).expect("request");
                let request = String::from_utf8_lossy(&request[..count]);
                let expected_method = if index < 2 {
                    "public/get_all_instruments"
                } else {
                    "public/get_trade_history"
                };
                assert!(request.contains(expected_method));
                assert!(request.contains("\"page_size\":1000"));
                assert!(request.contains(&format!("\"page\":{}", index % 2 + 1)));
                assert!(request.contains("\"instrument_type\":\"option\""));
                assert!(request.contains("\"currency\":\"BTC\""));
                if index >= 2 {
                    assert!(request.contains("\"tx_status\":\"settled\""));
                    assert!(request.contains("\"from_timestamp\":1800000000000"));
                    assert!(request.contains("\"to_timestamp\":1800000010000"));
                } else {
                    assert!(request.contains("\"expired\":false"));
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).expect("response");
            }
        });
        let client =
            DeriveOptionsClient::with_base_url(format!("http://{address}"), None).expect("client");
        let instruments = client
            .fetch_instruments(OptionsUnderlying::Btc)
            .await
            .expect("instruments");
        assert_eq!(instruments.len(), 2);
        assert_eq!(instruments[1].key.right, OptionRight::Put);
        let trades = client
            .fetch_trade_history(
                OptionsUnderlying::Btc,
                UnixMs::new(1_800_000_000_000),
                UnixMs::new(1_800_000_010_000),
                &instruments,
            )
            .await
            .expect("trades");
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].side, DeriveMakerSide::Buy);
        assert_eq!(trades[1].side, DeriveMakerSide::Sell);
        let persisted = serde_json::to_string(&trades).expect("serialize");
        for forbidden in ["wallet", "subaccount", "realized", "fee", "tx_hash"] {
            assert!(!persisted.contains(forbidden));
        }
        server.join().expect("server");
    }
}
