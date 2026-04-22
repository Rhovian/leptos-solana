//! Pure-Rust [Wallet Standard](https://github.com/wallet-standard/wallet-standard)
//! bindings for [Leptos](https://leptos.dev). Discover, connect to, and sign
//! with Phantom, Backpack, Solflare, Glow, Ledger Live, and any other Wallet
//! Standard-compliant Solana wallet — no hand-written JavaScript shipped with
//! the crate, no `npm`, no wallet-adapter wrappers.
//!
//! # Quick start
//!
//! Add the crate alongside Leptos:
//!
//! ```toml
//! [dependencies]
//! leptos-solana = "0.1"
//! leptos = { version = "0.8", features = ["csr"] }
//! ```
//!
//! Install the context once at the app root, then call [`use_wallet`](prelude::use_wallet)
//! from any component:
//!
//! ```ignore
//! use leptos::prelude::*;
//! use leptos_solana::prelude::*;
//! use leptos_solana::wallet::CHAIN_MAINNET;
//!
//! #[component]
//! fn App() -> impl IntoView {
//!     provide_wallet_context(CHAIN_MAINNET);
//!     let wallet = use_wallet();
//!
//!     view! {
//!         <ul>
//!             {move || wallet.wallets.get().0.into_iter().map(|w| {
//!                 let name = w.name();
//!                 let ctx = wallet.clone();
//!                 view! {
//!                     <li><button on:click=move |_| ctx.select(w.clone())>{name}</button></li>
//!                 }
//!             }).collect_view()}
//!         </ul>
//!     }
//! }
//! ```
//!
//! Signing and submitting a transaction:
//!
//! ```ignore
//! use leptos_solana::prelude::*;
//!
//! let ix = Instruction {
//!     program_id,
//!     accounts: vec![
//!         AccountMeta::new(from, true),
//!         AccountMeta::new(to, false),
//!     ],
//!     data,
//! };
//!
//! let rpc = RpcClient::devnet();
//! let blockhash = rpc.get_latest_blockhash(CommitmentConfig::confirmed()).await?;
//!
//! let msg = Message::new_with_blockhash(&[ix], Some(&from), &blockhash);
//! let tx: VersionedTransaction = Transaction::new_unsigned(msg).into();
//!
//! let sig: Vec<u8> = wallet.sign_and_send(&tx).await?;
//! ```
//!
//! # Design notes
//!
//! - **Wallet Standard is a JS spec.** Wallets register themselves via
//!   `CustomEvent`s on `window`. The crate reaches those objects with
//!   `wasm_bindgen`/`js_sys` — no `.js` shim is written by hand and no
//!   `npm`/`esbuild` is needed. [`discovery::start`] runs the spec-compliant
//!   `app-ready` + `register-wallet` handshake.
//!
//! - **VersionedTransaction only.** The wallet-signing API is
//!   [`VersionedTransaction`](solana_transaction::versioned::VersionedTransaction)-only.
//!   Legacy `Transaction` values can still be constructed and converted via
//!   `Into` — the Legacy variant of `VersionedMessage` serializes to the
//!   exact same wire bytes, so there is no behavior change for programs
//!   that do not need address lookup tables.
//!
//! - **Tiny dep closure.** No `solana-sdk`, no `solana-client`, no `tokio`.
//!   Pure Rust transaction construction via Anza's split crates
//!   (`solana-pubkey`, `solana-message`, `solana-transaction`, `solana-instruction`),
//!   bincode v1 for wire format, `gloo-net` for JSON-RPC.
//!
//! - **Reactive state.** [`WalletContext`](context::WalletContext) exposes
//!   `wallets`/`selected`/`account`/`chain` as Leptos signals. The context's
//!   `connect`/`disconnect`/`sign_*` methods are `async` and can be called
//!   from `spawn_local`.
//!
//! - **Auto-reconnect.** On successful `connect`, the wallet name is
//!   persisted to `localStorage`; on [`provide_wallet_context`](context::provide_wallet_context),
//!   if that wallet registers during discovery it is silently reconnected
//!   without prompting.
//!
//! - **Clean teardown.** [`discovery::start`] returns a [`DiscoveryHandle`](discovery::DiscoveryHandle)
//!   that removes the event listener in `Drop`. [`provide_wallet_context`](context::provide_wallet_context)
//!   ties the handle's lifetime to the Leptos owner.
//!
//! # Comparison with the JS stack
//!
//! The JS wallet ecosystem is layered:
//! `@wallet-standard/app` → `@solana/wallet-adapter-base` → `@solana/wallet-adapter-react`
//! → `@solana/wallet-adapter-react-ui`. This crate provides the first three
//! layers for Leptos (discovery, signer primitives, and a reactive context).
//! No UI components ship with the crate.
//!
//! # Modules
//!
//! - [`context`]    Leptos `WalletContext`, signals, `connect`/`sign_*` methods.
//! - [`discovery`]  Wallet Standard event-based discovery.
//! - [`features`]   Typed wrappers over Solana feature methods.
//! - [`rpc`]        Minimal JSON-RPC client (`gloo-net`).
//! - [`storage`]    `localStorage` persistence for last-connected wallet.
//! - [`tx`]         `VersionedTransaction` bincode serialize/deserialize.
//! - [`wallet`]     `wasm_bindgen` extern types matching the Wallet Standard spec.
//! - [`error`]      Crate-wide error type.
//! - [`prelude`]    `use leptos_solana::prelude::*;` brings in everything callers need.

pub mod context;
pub mod discovery;
pub mod error;
pub mod features;
pub mod rpc;
pub mod storage;
pub mod tx;
pub mod wallet;

pub use error::{Error, Result};

/// One-stop import for common callers: `use leptos_solana::prelude::*;`.
///
/// Brings in the Leptos context functions, the wallet types, the RPC client,
/// the tx serde helpers, and every Anza `solana-*` type you'd normally
/// `use` by hand.
pub mod prelude {
    pub use crate::context::{provide_wallet_context, use_wallet, WalletContext};
    pub use crate::discovery::WalletList;
    pub use crate::error::{Error, Result};
    pub use crate::wallet::{Wallet, WalletAccount};

    pub use crate::rpc::RpcClient;
    pub use crate::tx::{deserialize, serialize};

    pub use solana_commitment_config::CommitmentConfig;
    pub use solana_hash::Hash;
    pub use solana_instruction::{AccountMeta, Instruction};
    pub use solana_message::{
        v0::Message as MessageV0, AddressLookupTableAccount, Message, VersionedMessage,
    };
    pub use solana_pubkey::Pubkey;
    pub use solana_signature::Signature;
    pub use solana_transaction::versioned::VersionedTransaction;
    pub use solana_transaction::Transaction;
}
