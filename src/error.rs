use thiserror::Error;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("wallet standard not available on this page (navigator.wallets missing)")]
    WalletsUnavailable,

    #[error("wallet does not expose feature `{0}`")]
    MissingFeature(&'static str),

    #[error("wallet has no connected account")]
    NoAccount,

    #[error("wallet does not support chain `{0}`")]
    UnsupportedChain(String),

    #[error("user rejected the request")]
    UserRejected,

    /// Wallet is locked or has not been authorized for this dApp yet.
    /// Surfaced as JSON-RPC code `4100`. The recommended UX is to prompt
    /// the user to unlock / connect the wallet before retrying.
    #[error("wallet is locked or unauthorized; prompt user to connect")]
    WalletLocked,

    /// Wallet reports the requested method or feature is not implemented.
    /// Surfaced as JSON-RPC code `4200`.
    #[error("wallet does not support method `{0}`")]
    UnsupportedMethod(String),

    /// Wallet reports it is disconnected entirely. Surfaced as JSON-RPC
    /// code `4900`. Distinct from `WrongChain` (`4901`), which means the
    /// wallet is connected but on the wrong network.
    #[error("wallet is disconnected")]
    WalletDisconnected,

    /// Wallet is connected but on a different chain than the dApp expects.
    /// Surfaced as JSON-RPC code `4901`. `expected` is the chain id the
    /// dApp asked for; `got` is what the wallet reported, when available.
    #[error("wrong chain: expected `{expected}`, got `{got:?}`")]
    WrongChain {
        expected: String,
        got: Option<String>,
    },

    /// Wallet or RPC reports the account does not have enough SOL to cover
    /// the fee + transfer. Recognised from substring matches on the
    /// canonical RPC errors (Solana validators don't ship a numeric code
    /// for this).
    #[error("insufficient funds: account does not have enough SOL for fees or transfer")]
    InsufficientFunds,

    /// Wallet's preflight returned `Blockhash not found`, which usually
    /// indicates a chain/cluster mismatch (the blockhash was fetched from
    /// a cluster the wallet is not currently on) or that the blockhash
    /// expired between fetch and submit.
    #[error("blockhash not found: likely chain/cluster mismatch or expired blockhash")]
    BlockhashNotFound,

    /// Wallet's preflight transaction simulation failed. `logs` holds the
    /// program-instruction logs the validator emitted (often the most
    /// useful piece for debugging); `err` holds the raw error string the
    /// validator returned.
    #[error("transaction simulation failed: {err}")]
    SimulationFailed { logs: Vec<String>, err: String },

    #[error("js interop: {0}")]
    Js(String),

    #[error("decode: {0}")]
    Decode(String),

    #[error("rpc: {0}")]
    Rpc(String),

    #[error("serialize: {0}")]
    Serialize(String),
}

impl From<wasm_bindgen::JsValue> for Error {
    fn from(v: wasm_bindgen::JsValue) -> Self {
        // Wallets conventionally throw `{ code: <number>, message: <string> }`.
        // Dispatch on the JSON-RPC error code first; fall back to substring
        // matches on `message` for transaction-time errors that don't carry
        // a canonical code (Solana RPC: "Blockhash not found", "insufficient
        // funds", "Transaction simulation failed").
        if let Some(obj) = v.dyn_ref::<js_sys::Object>() {
            let code = js_sys::Reflect::get(obj, &"code".into())
                .ok()
                .and_then(|c| c.as_f64());
            let msg = js_sys::Reflect::get(obj, &"message".into())
                .ok()
                .and_then(|m| m.as_string())
                .unwrap_or_default();

            // Wallet-Standard / EIP-1193 style numeric codes.
            match code {
                Some(4001.0) => return Error::UserRejected,
                Some(4100.0) => return Error::WalletLocked,
                Some(4200.0) => {
                    let method = if msg.is_empty() { "unknown".to_string() } else { msg.clone() };
                    return Error::UnsupportedMethod(method);
                }
                Some(4900.0) => return Error::WalletDisconnected,
                Some(4901.0) => {
                    return Error::WrongChain {
                        expected: String::new(),
                        got: None,
                    };
                }
                _ => {}
            }

            // Substring dispatch on `message` for chain-side / RPC errors
            // that bubble through the wallet without a numeric code.
            if !msg.is_empty() {
                let lower = msg.to_ascii_lowercase();
                if lower.contains("blockhash not found") {
                    return Error::BlockhashNotFound;
                }
                if lower.contains("insufficient funds")
                    || lower.contains(
                        "attempt to debit an account but found no record of a prior credit",
                    )
                {
                    return Error::InsufficientFunds;
                }
                if lower.contains("transaction simulation failed") {
                    let logs = js_sys::Reflect::get(obj, &"logs".into())
                        .ok()
                        .and_then(|js| {
                            js_sys::Array::from(&js)
                                .iter()
                                .map(|v| v.as_string())
                                .collect::<Option<Vec<_>>>()
                        })
                        .unwrap_or_default();
                    return Error::SimulationFailed { logs, err: msg };
                }
            }
        }

        // JS Error objects don't expose `message` as enumerable, so
        // `JSON.stringify(err)` yields `{}`. Pull the real fields via Reflect.
        if let Some(obj) = v.dyn_ref::<js_sys::Object>() {
            let msg = js_sys::Reflect::get(obj, &"message".into())
                .ok()
                .and_then(|m| m.as_string())
                .filter(|s| !s.is_empty());
            let name = js_sys::Reflect::get(obj, &"name".into())
                .ok()
                .and_then(|n| n.as_string())
                .filter(|s| !s.is_empty());
            let code = js_sys::Reflect::get(obj, &"code".into())
                .ok()
                .and_then(|c| c.as_f64());

            if msg.is_some() || name.is_some() || code.is_some() {
                let mut s = String::new();
                if let Some(n) = name {
                    s.push_str(&n);
                }
                if let Some(c) = code {
                    if !s.is_empty() {
                        s.push(' ');
                    }
                    s.push_str(&format!("({c:.0})"));
                }
                if let Some(m) = msg {
                    if !s.is_empty() {
                        s.push_str(": ");
                    }
                    s.push_str(&m);
                }
                return Error::Js(s);
            }
        }

        Error::Js(
            v.as_string()
                .or_else(|| js_sys::JSON::stringify(&v).ok().and_then(|s| s.as_string()))
                .unwrap_or_else(|| "unknown js error".into()),
        )
    }
}

use wasm_bindgen::JsCast;
