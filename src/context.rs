//! Leptos context + signals for wallet state.
//!
//! Call [`provide_wallet_context`] once at the app root. Downstream
//! components read the connected wallet/account via [`use_wallet`] and
//! invoke `connect` / `disconnect` / `sign_*` through the returned handle.

use leptos::prelude::*;
use leptos::task::spawn_local;
use solana_transaction::versioned::VersionedTransaction;

use crate::discovery::{self, DiscoveryHandle, WalletList};
use crate::error::Result;
use crate::wallet::{Wallet, WalletAccount};
use crate::{features, storage, tx};

/// Reactive wallet state + async actions. Obtain one by calling
/// [`provide_wallet_context`] at the app root and [`use_wallet`] in any
/// descendant component; the type is `Clone` and cheap to copy into
/// closures (all signals are `Arc`-backed).
///
/// # Signals
///
/// - [`wallets`](Self::wallets) — every Wallet Standard wallet that has
///   registered so far. Updated as wallets load.
/// - [`selected`](Self::selected) — the wallet the user picked. `None`
///   until [`select`](Self::select) is called.
/// - [`account`](Self::account) — the first Solana account exposed after
///   [`connect`](Self::connect) succeeds.
/// - [`chain`](Self::chain) — the chain id passed to the wallet on
///   [`sign_transaction`](Self::sign_transaction) and
///   [`sign_and_send`](Self::sign_and_send). Use constants from
///   [`crate::wallet`].
#[derive(Clone)]
pub struct WalletContext {
    /// All Wallet Standard wallets that have registered so far. The list
    /// grows as late-loading wallets dispatch `wallet-standard:register-wallet`.
    pub wallets: ReadSignal<WalletList>,
    #[allow(dead_code)]
    set_wallets: WriteSignal<WalletList>,

    /// The user-chosen wallet. `None` until [`WalletContext::select`] is called.
    pub selected: ReadSignal<Option<Wallet>>,
    set_selected: WriteSignal<Option<Wallet>>,

    /// The first Solana account the wallet exposed after a successful
    /// [`WalletContext::connect`]. Multi-chain wallets (Backpack,
    /// Phantom-EVM) may return accounts from other chains; this signal
    /// only ever holds one that advertises a `solana:*` chain.
    pub account: ReadSignal<Option<WalletAccount>>,
    set_account: WriteSignal<Option<WalletAccount>>,

    /// Chain identifier passed to the wallet on transaction signing.
    /// Wallet Standard spec strings are `"solana:mainnet"` / `"solana:devnet"`
    /// / `"solana:testnet"` / `"solana:localnet"`. See
    /// [`crate::wallet::CHAIN_MAINNET`] etc. for constants.
    pub chain: ReadSignal<String>,
    set_chain: WriteSignal<String>,
}

impl WalletContext {
    /// Record which wallet the user picked from [`Self::wallets`]. Does
    /// not prompt the wallet — call [`Self::connect`] after to actually
    /// authorize.
    pub fn select(&self, wallet: Wallet) {
        self.set_selected.set(Some(wallet));
    }

    /// Switch the chain passed to the wallet on subsequent signing calls.
    /// Pass one of the `CHAIN_*` constants from [`crate::wallet`].
    pub fn set_chain(&self, chain: impl Into<String>) {
        self.set_chain.set(chain.into());
    }

    /// Prompt the [selected](Self::selected) wallet to connect and expose
    /// its Solana accounts. On success, [`account`](Self::account) holds
    /// the first account that advertises a `solana:*` chain and the wallet
    /// name is persisted to `localStorage` for future auto-reconnect.
    ///
    /// # Errors
    ///
    /// - [`Error::NoAccount`](crate::Error::NoAccount) if no wallet is
    ///   selected or the wallet returned no Solana accounts.
    /// - [`Error::UserRejected`](crate::Error::UserRejected) if the user
    ///   declined the connect prompt.
    /// - [`Error::Js`](crate::Error::Js) wrapping any other wallet-side error.
    pub async fn connect(&self) -> Result<()> {
        let wallet = self
            .selected
            .get_untracked()
            .ok_or(crate::Error::NoAccount)?;
        let accounts = features::connect(&wallet, false).await?;
        let picked = pick_solana_account(accounts).ok_or(crate::Error::NoAccount)?;
        self.set_account.set(Some(picked));
        storage::remember_wallet(&wallet.name());
        Ok(())
    }

    /// Disconnect the selected wallet and clear the persisted name.
    /// Subsequent calls to [`provide_wallet_context`] will not auto-reconnect.
    pub async fn disconnect(&self) -> Result<()> {
        if let Some(wallet) = self.selected.get_untracked() {
            features::disconnect(&wallet).await?;
        }
        self.set_account.set(None);
        storage::forget_wallet();
        Ok(())
    }

    /// Sign arbitrary message bytes via `solana:signMessage`. Returns the
    /// raw 64-byte ed25519 signature; encode with `bs58` for display.
    pub async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>> {
        let (wallet, account) = self.wallet_account()?;
        features::sign_message(&wallet, &account, message).await
    }

    /// Sign a [`VersionedTransaction`] without submitting. Returns the
    /// fully-signed transaction.
    pub async fn sign_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<VersionedTransaction> {
        let bytes = tx::serialize(transaction)?;
        let signed = self.sign_transaction_raw(&bytes).await?;
        tx::deserialize(&signed)
    }

    /// Sign and submit a [`VersionedTransaction`] through the wallet's own
    /// RPC. Returns the 64-byte transaction signature (bs58 for display).
    pub async fn sign_and_send(&self, transaction: &VersionedTransaction) -> Result<Vec<u8>> {
        let bytes = tx::serialize(transaction)?;
        self.sign_and_send_raw(&bytes).await
    }

    /// Escape hatch: sign pre-serialized transaction bytes.
    pub async fn sign_transaction_raw(&self, tx_bytes: &[u8]) -> Result<Vec<u8>> {
        let (wallet, account) = self.wallet_account()?;
        let chain = self.chain.get_untracked();
        features::sign_transaction(&wallet, &account, &chain, tx_bytes).await
    }

    /// Escape hatch: sign + send pre-serialized transaction bytes.
    pub async fn sign_and_send_raw(&self, tx_bytes: &[u8]) -> Result<Vec<u8>> {
        let (wallet, account) = self.wallet_account()?;
        let chain = self.chain.get_untracked();
        features::sign_and_send_transaction(&wallet, &account, &chain, tx_bytes).await
    }

    fn wallet_account(&self) -> Result<(Wallet, WalletAccount)> {
        let wallet = self
            .selected
            .get_untracked()
            .ok_or(crate::Error::NoAccount)?;
        let account = self
            .account
            .get_untracked()
            .ok_or(crate::Error::NoAccount)?;
        Ok((wallet, account))
    }
}

/// Pick the first account that advertises a `solana:*` chain. Multi-chain
/// wallets (Backpack, Phantom-EVM, etc.) can return mixed accounts; taking
/// the first without filtering gives you an Ethereum address for a Solana
/// balance call, which fails silently with zero.
fn pick_solana_account(accounts: Vec<WalletAccount>) -> Option<WalletAccount> {
    accounts.into_iter().find(|a| {
        a.chains()
            .iter()
            .filter_map(|c| c.as_string())
            .any(|c| c.starts_with("solana:"))
    })
}

/// Install a [`WalletContext`] into the current Leptos owner and start
/// Wallet Standard discovery. Call once at the app root; descendants read
/// the context with [`use_wallet`].
///
/// This function:
///
/// 1. Creates the `wallets` / `selected` / `account` / `chain` signals.
/// 2. Dispatches `wallet-standard:app-ready` and subscribes to
///    `wallet-standard:register-wallet` events — wallets that register
///    either way populate the `wallets` signal.
/// 3. If a previously-connected wallet name is in `localStorage` and that
///    wallet registers during discovery, silent-reconnects to it
///    (`standard:connect` with `silent: true` — no user prompt).
/// 4. Stashes the discovery handle in a [`StoredValue`] so the event
///    listener is removed when the Leptos owner drops.
///
/// # Example
///
/// ```ignore
/// use leptos_solana::prelude::*;
/// use leptos_solana::wallet::CHAIN_MAINNET;
///
/// #[component]
/// fn App() -> impl IntoView {
///     provide_wallet_context(CHAIN_MAINNET);
///     // ... children can now call use_wallet()
///     view! { <Root /> }
/// }
/// ```
pub fn provide_wallet_context(default_chain: &str) -> WalletContext {
    let (wallets, set_wallets) = signal(WalletList::default());
    let (selected, set_selected) = signal(None::<Wallet>);
    let (account, set_account) = signal(None::<WalletAccount>);
    let (chain, set_chain) = signal(default_chain.to_string());

    let last = storage::last_wallet();

    let handle = discovery::start(move |wallet| {
        let name = wallet.name();
        let already_known = wallets.with_untracked(|list: &WalletList| {
            list.0.iter().any(|w| w.name() == name)
        });
        if !already_known {
            set_wallets.update(|list| list.0.push(wallet.clone()));
        }

        // Silent-reconnect if this is the remembered wallet and we don't
        // already have an account (guards against re-registration).
        if Some(&name) == last.as_ref() && account.get_untracked().is_none() {
            set_selected.set(Some(wallet.clone()));
            let wallet = wallet.clone();
            spawn_local(async move {
                if let Ok(accounts) = features::connect(&wallet, true).await {
                    if let Some(a) = pick_solana_account(accounts) {
                        set_account.set(Some(a));
                    }
                }
            });
        }
    })
    .ok();

    // Keep the discovery handle alive for the owner's lifetime, then drop
    // it (which removes the event listener).
    if let Some(h) = handle {
        let _ = StoredValue::new(HandleCell(Some(h)));
    }

    let ctx = WalletContext {
        wallets,
        set_wallets,
        selected,
        set_selected,
        account,
        set_account,
        chain,
        set_chain,
    };
    provide_context(ctx.clone());
    ctx
}

/// Newtype so we can satisfy Leptos's `Send + Sync` bound on stored
/// values. wasm32 without atomics treats `!Send` types as Send because
/// there's only one thread; this wrapper makes that explicit.
struct HandleCell(#[allow(dead_code)] Option<DiscoveryHandle>);

// SAFETY: leptos-solana targets single-threaded wasm; there is no other
// thread that could observe the handle concurrently.
unsafe impl Send for HandleCell {}
unsafe impl Sync for HandleCell {}

/// Read the [`WalletContext`] previously installed by [`provide_wallet_context`].
///
/// # Panics
///
/// Panics if [`provide_wallet_context`] was not called in an ancestor of
/// the current component.
pub fn use_wallet() -> WalletContext {
    use_context::<WalletContext>()
        .expect("WalletContext missing — call provide_wallet_context at the app root")
}
