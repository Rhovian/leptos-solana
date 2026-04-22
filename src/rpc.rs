//! Minimal Solana JSON-RPC client over `gloo-net`.
//!
//! We only implement the methods a client app needs to build, submit, and
//! confirm transactions — no tokio, no `solana-client`. Extend as needed.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use wasm_bindgen::JsValue;

use crate::error::{Error, Result};

/// Minimal Solana JSON-RPC client over `gloo-net::http::Request`.
///
/// Only the methods a browser client needs to build and submit
/// transactions are implemented. The private generic `call` helper is
/// trivial to extend — open a PR or fork if you need more methods.
///
/// # Example
///
/// ```ignore
/// use leptos_solana::prelude::*;
///
/// let rpc = RpcClient::devnet();
/// let lamports = rpc.get_balance(&account.address()).await?;
/// let blockhash = rpc.get_latest_blockhash(CommitmentConfig::confirmed()).await?;
/// ```
#[derive(Clone, Debug)]
pub struct RpcClient {
    endpoint: String,
}

impl RpcClient {
    /// Construct against an arbitrary RPC endpoint (Helius, QuickNode, local
    /// `solana-test-validator`, etc.).
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    /// Public mainnet-beta endpoint.
    pub fn mainnet() -> Self {
        Self::new("https://api.mainnet-beta.solana.com")
    }
    /// Public devnet endpoint.
    pub fn devnet() -> Self {
        Self::new("https://api.devnet.solana.com")
    }
    /// Public testnet endpoint.
    pub fn testnet() -> Self {
        Self::new("https://api.testnet.solana.com")
    }

    async fn call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let body_str = body.to_string();
        web_sys::console::log_3(
            &"[leptos-solana rpc] →".into(),
            &JsValue::from_str(&self.endpoint),
            &JsValue::from_str(&body_str),
        );

        let resp = Request::post(&self.endpoint)
            .header("content-type", "application/json")
            .body(body_str)
            .map_err(|e| Error::Rpc(e.to_string()))?
            .send()
            .await
            .map_err(|e| Error::Rpc(e.to_string()))?;

        // Read as text first so we can log the raw body even on parse failure.
        let text = resp.text().await.map_err(|e| Error::Rpc(e.to_string()))?;
        web_sys::console::log_2(
            &format!("[leptos-solana rpc] ← {method}").into(),
            &JsValue::from_str(&text),
        );

        let wrapped: RpcResponse<T> =
            serde_json::from_str(&text).map_err(|e| Error::Rpc(e.to_string()))?;

        match wrapped {
            RpcResponse::Ok { result, .. } => Ok(result),
            RpcResponse::Err { error, .. } => Err(Error::Rpc(format!(
                "{} ({})",
                error.message, error.code
            ))),
        }
    }

    /// Fetch a recent blockhash for inclusion in a new transaction. The
    /// wallet will typically sign against whatever blockhash you pass —
    /// use `CommitmentConfig::confirmed()` unless you have a reason not to.
    pub async fn get_latest_blockhash(&self, commitment: CommitmentConfig) -> Result<Hash> {
        #[derive(Deserialize)]
        struct Value {
            blockhash: String,
        }
        #[derive(Deserialize)]
        struct Resp {
            value: Value,
        }

        let resp: Resp = self
            .call(
                "getLatestBlockhash",
                json!([{ "commitment": commitment.commitment.to_string() }]),
            )
            .await?;
        resp.value
            .blockhash
            .parse::<Hash>()
            .map_err(|e| Error::Decode(format!("blockhash: {e}")))
    }

    /// Submit a signed, base64-encoded transaction. For the common "wallet
    /// signs and broadcasts" flow, use [`WalletContext::sign_and_send`](crate::context::WalletContext::sign_and_send)
    /// instead — this method is for cases where you already have signed bytes
    /// (e.g. from a backend) and want to broadcast them yourself.
    pub async fn send_transaction_b64(&self, signed_b64: &str) -> Result<String> {
        self.call(
            "sendTransaction",
            json!([
                signed_b64,
                { "encoding": "base64" }
            ]),
        )
        .await
    }

    /// `getBalance` in lamports. Uses `confirmed` commitment so freshly-
    /// airdropped devnet SOL shows up within a slot instead of waiting ~30s
    /// for finalization.
    pub async fn get_balance(&self, address: &str) -> Result<u64> {
        self.get_balance_with_commitment(address, CommitmentConfig::confirmed())
            .await
    }

    pub async fn get_balance_with_commitment(
        &self,
        address: &str,
        commitment: CommitmentConfig,
    ) -> Result<u64> {
        #[derive(Deserialize)]
        struct Resp {
            value: u64,
        }
        let resp: Resp = self
            .call(
                "getBalance",
                json!([address, { "commitment": commitment.commitment.to_string() }]),
            )
            .await?;
        Ok(resp.value)
    }

    /// `getSlot` - current slot the RPC node is on. Useful alongside
    /// `confirmTransaction` for freshness checks and for telling a user how
    /// far behind tip a connection is.
    pub async fn get_slot(&self) -> Result<u64> {
        self.get_slot_with_commitment(CommitmentConfig::confirmed())
            .await
    }

    pub async fn get_slot_with_commitment(&self, commitment: CommitmentConfig) -> Result<u64> {
        self.call(
            "getSlot",
            json!([{ "commitment": commitment.commitment.to_string() }]),
        )
        .await
    }

    /// `getBlockHeight` - current block height (distinct from slot; skipped
    /// slots aren't counted). Often paired with `getSlot` to diagnose fork
    /// conditions or warn the user when skip rate is high.
    pub async fn get_block_height(&self) -> Result<u64> {
        self.get_block_height_with_commitment(CommitmentConfig::confirmed())
            .await
    }

    pub async fn get_block_height_with_commitment(
        &self,
        commitment: CommitmentConfig,
    ) -> Result<u64> {
        self.call(
            "getBlockHeight",
            json!([{ "commitment": commitment.commitment.to_string() }]),
        )
        .await
    }

    /// `getMinimumBalanceForRentExemption` - lamports required to make an
    /// account of `data_len` bytes rent-exempt. Needed before creating any
    /// on-chain account.
    pub async fn get_minimum_balance_for_rent_exemption(&self, data_len: u64) -> Result<u64> {
        self.call("getMinimumBalanceForRentExemption", json!([data_len]))
            .await
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RpcResponse<T> {
    Ok {
        #[allow(dead_code)]
        jsonrpc: String,
        result: T,
    },
    Err {
        #[allow(dead_code)]
        jsonrpc: String,
        error: RpcError,
    },
}

#[derive(Deserialize, Serialize, Debug)]
struct RpcError {
    code: i64,
    message: String,
}
