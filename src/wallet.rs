//! wasm-bindgen extern types matching the Wallet Standard spec.
//!
//! Spec: <https://github.com/wallet-standard/wallet-standard/tree/master/packages/core/base>
//!
//! The Wallet Standard defines a `Wallet` interface that is a plain JS object
//! with fields (version, name, icon, chains, accounts, features). `features`
//! is a record keyed by identifier strings like `"standard:connect"` or
//! `"solana:signTransaction"`; each value is itself an object with methods.
//!
//! We declare these as opaque wasm-bindgen externs and reach into them via
//! [`js_sys::Reflect`] in [`crate::features`].

use js_sys::{Array, Object, Uint8Array};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// A Wallet Standard wallet (`Wallet` interface).
    #[wasm_bindgen(extends = Object)]
    #[derive(Clone, Debug)]
    pub type Wallet;

    #[wasm_bindgen(method, getter)]
    pub fn version(this: &Wallet) -> String;

    #[wasm_bindgen(method, getter)]
    pub fn name(this: &Wallet) -> String;

    #[wasm_bindgen(method, getter)]
    pub fn icon(this: &Wallet) -> String;

    /// Array of chain identifier strings, e.g. `"solana:mainnet"`.
    #[wasm_bindgen(method, getter)]
    pub fn chains(this: &Wallet) -> Array;

    /// Array of currently-connected [`WalletAccount`]s.
    #[wasm_bindgen(method, getter)]
    pub fn accounts(this: &Wallet) -> Array;

    /// Record of feature-id → feature-object.
    #[wasm_bindgen(method, getter)]
    pub fn features(this: &Wallet) -> Object;

    /// A Wallet Standard account (`WalletAccount` interface).
    #[wasm_bindgen(extends = Object)]
    #[derive(Clone, Debug)]
    pub type WalletAccount;

    /// Raw public-key bytes (32 bytes for Solana).
    #[wasm_bindgen(method, getter)]
    pub fn address(this: &WalletAccount) -> String;

    #[wasm_bindgen(method, getter, js_name = publicKey)]
    pub fn public_key(this: &WalletAccount) -> Uint8Array;

    #[wasm_bindgen(method, getter)]
    pub fn chains(this: &WalletAccount) -> Array;

    #[wasm_bindgen(method, getter)]
    pub fn features(this: &WalletAccount) -> Array;
}

// Chain identifier strings used on Solana wallets. Pass these to
// `solana:signTransaction` / `solana:signAndSendTransaction` inputs.
pub const CHAIN_MAINNET: &str = "solana:mainnet";
pub const CHAIN_DEVNET: &str = "solana:devnet";
pub const CHAIN_TESTNET: &str = "solana:testnet";
pub const CHAIN_LOCALNET: &str = "solana:localnet";

// Feature identifiers.
pub const FEATURE_CONNECT: &str = "standard:connect";
pub const FEATURE_DISCONNECT: &str = "standard:disconnect";
pub const FEATURE_EVENTS: &str = "standard:events";
pub const FEATURE_SIGN_MESSAGE: &str = "solana:signMessage";
pub const FEATURE_SIGN_TRANSACTION: &str = "solana:signTransaction";
pub const FEATURE_SIGN_AND_SEND_TRANSACTION: &str = "solana:signAndSendTransaction";
pub const FEATURE_SIGN_IN: &str = "solana:signIn";

impl Wallet {
    /// True if this wallet advertises support for any Solana chain.
    pub fn supports_solana(&self) -> bool {
        self.chains()
            .iter()
            .filter_map(|v| v.as_string())
            .any(|s| s.starts_with("solana:"))
    }
}
