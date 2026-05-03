//! Minimal Solana JSON-RPC client over `gloo-net`.
//!
//! We only implement the methods a client app needs to build, submit, and
//! confirm transactions — no tokio, no `solana-client`. Extend as needed.

use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
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
            RpcResponse::Err { error, .. } => {
                Err(Error::Rpc(format!("{} ({})", error.message, error.code)))
            }
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

    /// `getAccountInfo` - read arbitrary account data. Returns `None` if the
    /// account does not exist on-chain.
    ///
    /// Uses `confirmed` commitment by default; for finalized reads call
    /// `get_account_info_with_commitment` and pass `CommitmentConfig::finalized()`.
    pub async fn get_account_info(&self, address: &str) -> Result<Option<AccountInfo>> {
        self.get_account_info_with_commitment(address, CommitmentConfig::confirmed())
            .await
    }

    pub async fn get_account_info_with_commitment(
        &self,
        address: &str,
        commitment: CommitmentConfig,
    ) -> Result<Option<AccountInfo>> {
        #[derive(Deserialize)]
        struct Resp {
            value: Option<AccountInfoRaw>,
        }
        let resp: Resp = self
            .call(
                "getAccountInfo",
                json!([
                    address,
                    {
                        "commitment": commitment.commitment.to_string(),
                        "encoding": "base64",
                    }
                ]),
            )
            .await?;
        match resp.value {
            None => Ok(None),
            Some(raw) => Ok(Some(account_info_from_raw(raw)?)),
        }
    }

    /// `getMultipleAccounts` - batch variant of `getAccountInfo`. Saves
    /// round-trips when fetching N known account addresses. The returned
    /// vector is parallel to `addresses` and uses `None` for any address
    /// that does not exist on-chain.
    pub async fn get_multiple_accounts(
        &self,
        addresses: &[&str],
    ) -> Result<Vec<Option<AccountInfo>>> {
        self.get_multiple_accounts_with_commitment(addresses, CommitmentConfig::confirmed())
            .await
    }

    pub async fn get_multiple_accounts_with_commitment(
        &self,
        addresses: &[&str],
        commitment: CommitmentConfig,
    ) -> Result<Vec<Option<AccountInfo>>> {
        #[derive(Deserialize)]
        struct Resp {
            value: Vec<Option<AccountInfoRaw>>,
        }
        let resp: Resp = self
            .call(
                "getMultipleAccounts",
                json!([
                    addresses,
                    {
                        "commitment": commitment.commitment.to_string(),
                        "encoding": "base64",
                    }
                ]),
            )
            .await?;
        resp.value
            .into_iter()
            .map(|opt| match opt {
                None => Ok(None),
                Some(raw) => Ok(Some(account_info_from_raw(raw)?)),
            })
            .collect()
    }

    /// `getTokenAccountBalance` - balance of a single SPL token account.
    ///
    /// Returns `amount` as a base-10 string in token-side units, plus
    /// `decimals` and the pre-formatted `ui_amount_string` for display.
    /// Use `ui_amount_string` rather than `ui_amount` for rendering - `f64`
    /// introduces precision drift on small fractional units.
    ///
    /// Uses `confirmed` commitment by default; for finalized reads call
    /// `get_token_account_balance_with_commitment`.
    pub async fn get_token_account_balance(&self, address: &str) -> Result<TokenAccountBalance> {
        self.get_token_account_balance_with_commitment(address, CommitmentConfig::confirmed())
            .await
    }

    pub async fn get_token_account_balance_with_commitment(
        &self,
        address: &str,
        commitment: CommitmentConfig,
    ) -> Result<TokenAccountBalance> {
        #[derive(Deserialize)]
        struct Resp {
            value: TokenAccountBalance,
        }
        let resp: Resp = self
            .call(
                "getTokenAccountBalance",
                json!([address, { "commitment": commitment.commitment.to_string() }]),
            )
            .await?;
        Ok(resp.value)
    }

    /// `getTransaction` - fetch a landed transaction with execution logs.
    /// Returns `None` if the signature is unknown or has not yet landed.
    ///
    /// Useful for debugging: log messages, fee, and a top-level err for
    /// failed transactions are all included on the returned struct.
    ///
    /// Uses `confirmed` commitment by default; for finalized reads call
    /// `get_transaction_with_commitment`.
    pub async fn get_transaction(&self, signature: &str) -> Result<Option<TransactionStatus>> {
        self.get_transaction_with_commitment(signature, CommitmentConfig::confirmed())
            .await
    }

    pub async fn get_transaction_with_commitment(
        &self,
        signature: &str,
        commitment: CommitmentConfig,
    ) -> Result<Option<TransactionStatus>> {
        self.call(
            "getTransaction",
            json!([
                signature,
                {
                    "commitment": commitment.commitment.to_string(),
                    "encoding": "json",
                    "maxSupportedTransactionVersion": 0,
                }
            ]),
        )
        .await
    }

    /// `requestAirdrop` - dev/test convenience: ask the cluster to credit
    /// `lamports` to `address`. Returns the resulting transaction signature.
    /// Mainnet rejects this call; this is for `devnet` / `testnet` /
    /// `solana-test-validator` only.
    pub async fn request_airdrop(&self, address: &str, lamports: u64) -> Result<String> {
        self.call("requestAirdrop", json!([address, lamports]))
            .await
    }
}

/// Decoded account state returned by `getAccountInfo` / `getMultipleAccounts`.
#[derive(Clone, Debug)]
pub struct AccountInfo {
    /// Raw account data, base64-decoded into bytes.
    pub data: Vec<u8>,
    /// Owning program.
    pub owner: Pubkey,
    /// Account balance in lamports.
    pub lamports: u64,
    /// True if the account is an executable program.
    pub executable: bool,
    /// Epoch in which the account next owes rent.
    pub rent_epoch: u64,
}

/// Token account balance returned by `getTokenAccountBalance`.
///
/// `amount` is the raw integer balance as a base-10 string (token-side
/// representation; `u64::from_str` to parse). `ui_amount_string` is the
/// canonical decimal string for display - prefer it over `ui_amount` for
/// rendering since `f64` introduces precision drift on small fractional
/// units. `ui_amount` is `Option<f64>` because the Solana RPC returns
/// `null` for tokens whose balance overflows `f64`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TokenAccountBalance {
    pub amount: String,
    pub decimals: u8,
    #[serde(rename = "uiAmount", default)]
    pub ui_amount: Option<f64>,
    #[serde(rename = "uiAmountString")]
    pub ui_amount_string: String,
}

/// Subset of `getTransaction` response useful for debugging - slot,
/// timing, fee, log messages, and a top-level error if execution failed.
/// The full `transaction` and `inner_instructions` are intentionally
/// omitted to keep this struct small; if you need them, query the RPC
/// directly via `call`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TransactionStatus {
    pub slot: u64,
    #[serde(rename = "blockTime", default)]
    pub block_time: Option<i64>,
    #[serde(default)]
    pub meta: Option<TransactionMeta>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TransactionMeta {
    /// `None` on success, `Some(serde_json::Value)` describing the
    /// failure on error. Solana's err shape varies (InstructionError,
    /// InsufficientFundsForRent, etc.), so we surface raw JSON.
    #[serde(default)]
    pub err: Option<serde_json::Value>,
    pub fee: u64,
    #[serde(rename = "logMessages", default)]
    pub log_messages: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct AccountInfoRaw {
    /// Tuple of `[base64_string, "base64"]` when encoding=base64.
    data: (String, String),
    owner: String,
    lamports: u64,
    executable: bool,
    /// Solana wire format uses camelCase; the field arrives as `rentEpoch`.
    #[serde(rename = "rentEpoch", default)]
    rent_epoch: u64,
}

fn account_info_from_raw(raw: AccountInfoRaw) -> Result<AccountInfo> {
    let (data_b64, _encoding) = raw.data;
    let data = BASE64_STANDARD
        .decode(data_b64.as_bytes())
        .map_err(|e| Error::Decode(format!("account data: {e}")))?;
    let owner =
        Pubkey::from_str(&raw.owner).map_err(|e| Error::Decode(format!("account owner: {e}")))?;
    Ok(AccountInfo {
        data,
        owner,
        lamports: raw.lamports,
        executable: raw.executable,
        rent_epoch: raw.rent_epoch,
    })
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
