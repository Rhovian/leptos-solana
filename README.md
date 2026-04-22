# leptos-solana

Pure-Rust [Wallet Standard](https://github.com/wallet-standard/wallet-standard)
bindings for [Leptos](https://leptos.dev). Discover, connect to, and sign with
Phantom, Backpack, Solflare, Glow, Ledger Live, and any other Wallet
Standard-compliant Solana wallet — no hand-written JavaScript shipped with the
crate, no `npm`, no wallet-adapter wrappers.

```toml
[dependencies]
leptos-solana = "0.1"
leptos = { version = "0.8", features = ["csr"] }
```

## Status

Early. Works for mainstream single-signer flows: connect, sign message,
sign + send (legacy or versioned) transactions. API may still change
between `0.1.x` releases. See [Roadmap](#roadmap) for what is deliberately
missing.

## Why

The JS Solana wallet stack is layered:
`@wallet-standard/app` → `@solana/wallet-adapter-base` → `@solana/wallet-adapter-react`
→ `@solana/wallet-adapter-react-ui`. Using any of it from a Leptos (or
generally pure-Rust) app means shipping `privy.js` / `wallet-adapter.js`
bundles, running `esbuild` as a pre-build step, and bridging every call back
into Rust via `wasm-bindgen` `extern` blocks.

`leptos-solana` replaces the first three layers with a Rust library. Wallets
are still JavaScript objects living in the browser — that's how they're
delivered — but the crate reaches them with `js-sys`/`web-sys` the same way
wasm-bindgen reaches `fetch` or `localStorage`. No `.js` shim is written by
hand, no `package.json` required. UI is not provided; build it yourself with
Leptos components (see [`demo/`](./demo)).

## Features

- **Spec-compliant discovery.** Runs the Wallet Standard event handshake
  (`wallet-standard:app-ready` + `wallet-standard:register-wallet`). Every
  wallet that registers shows up in a reactive `WalletList` signal.
- **Typed Solana feature wrappers.** `standard:connect`, `standard:disconnect`,
  `solana:signMessage`, `solana:signTransaction`, `solana:signAndSendTransaction`.
- **VersionedTransaction-first.** The wallet-signing API is versioned-only;
  Address Lookup Tables work. Legacy `Transaction` values convert via
  `Into` with identical wire format.
- **Reactive Leptos context.** Signals for `wallets` / `selected` / `account` /
  `chain`; async methods for `connect` / `sign_*` / `disconnect`.
- **Auto-reconnect.** Remembers the last wallet via `localStorage` and
  silent-connects on page load.
- **Minimal JSON-RPC.** `getLatestBlockhash`, `getBalance`, `sendTransaction`
  over `gloo-net`. No `solana-client`, no `tokio`.
- **Clean teardown.** `discovery::start` returns a handle that removes the
  event listener on `Drop`; the context ties its lifetime to the Leptos owner.
- **Tiny dep closure.** Built on Anza's split Solana crates
  (`solana-pubkey`, `solana-hash`, `solana-instruction`, `solana-message`,
  `solana-transaction`, `solana-signature`, `solana-commitment-config`).
  No `solana-sdk` umbrella.

## Quick start

```rust
use leptos::prelude::*;
use leptos_solana::prelude::*;
use leptos_solana::wallet::CHAIN_MAINNET;

#[component]
fn App() -> impl IntoView {
    provide_wallet_context(CHAIN_MAINNET);
    let wallet = use_wallet();

    view! {
        <ul>
            {move || wallet.wallets.get().0.into_iter().map(|w| {
                let name = w.name();
                let ctx = wallet.clone();
                view! {
                    <li><button on:click=move |_| ctx.select(w.clone())>{name}</button></li>
                }
            }).collect_view()}
        </ul>
    }
}
```

### Signing and submitting a transaction

```rust
use leptos_solana::prelude::*;

// Build any instruction — here, SystemProgram::transfer by hand (disc 2, u64 LE).
let mut data = Vec::with_capacity(12);
data.extend_from_slice(&2u32.to_le_bytes());
data.extend_from_slice(&lamports.to_le_bytes());

let ix = Instruction {
    program_id: Pubkey::new_from_array([0u8; 32]), // System Program
    accounts: vec![
        AccountMeta::new(from, true),
        AccountMeta::new(to, false),
    ],
    data,
};

let rpc = RpcClient::devnet();
let blockhash = rpc.get_latest_blockhash(CommitmentConfig::confirmed()).await?;

let msg = Message::new_with_blockhash(&[ix], Some(&from), &blockhash);
let tx: VersionedTransaction = Transaction::new_unsigned(msg).into();

let sig: Vec<u8> = wallet.sign_and_send(&tx).await?;
```

### Just signing (no submit)

```rust
let signed: VersionedTransaction = wallet.sign_transaction(&tx).await?;
```

### Raw bytes escape hatch

If you already have pre-serialized transaction bytes (e.g. from a backend):

```rust
let signed_bytes: Vec<u8> = wallet.sign_transaction_raw(&tx_bytes).await?;
let submit_sig: Vec<u8> = wallet.sign_and_send_raw(&tx_bytes).await?;
```

### Sign-in message

```rust
let sig: Vec<u8> = wallet.sign_message(b"Welcome to my dApp").await?;
```

## Demo

The repo includes a runnable demo in [`demo/`](./demo) — wallet picker,
message signing, and a 0.0001 SOL self-transfer on devnet.

```sh
cd demo && trunk serve
# then open http://127.0.0.1:3001
```

## Roadmap

Not yet implemented; contributions welcome:

- `signAllTransactions` (bulk sign)
- `confirmTransaction` / signature status polling
- `simulateTransaction` (preflight)
- Sign-In with Solana (`solana:signIn`)
- Richer RPC surface (`getAccountInfo`, `getTokenAccountsByOwner`, `requestAirdrop`, …)
- Error code taxonomy beyond `UserRejected` (wallet locked, wrong chain, insufficient funds, …)
- Optional feature-gated Anchor discriminator helpers

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](./LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.
