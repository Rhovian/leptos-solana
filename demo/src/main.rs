//! Minimal `leptos-solana` demo.
//!
//! Discovers Wallet Standard Solana wallets (Phantom / Backpack / Solflare /
//! Glow / …), connects, signs a message, and does a 0.0001 SOL self-transfer
//! on devnet — all in pure Rust, no hand-written JS.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_solana::prelude::*;
use leptos_solana::wallet::CHAIN_DEVNET;

const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0u8; 32]);
const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    provide_wallet_context(CHAIN_DEVNET);
    let status_signal = RwSignal::new(String::new());
    provide_context(StatusSignal(status_signal));

    view! {
        <h1>"leptos-solana"</h1>
        <p>
            "Pure-Rust Wallet Standard demo. Chain: "<strong>"devnet"</strong>
            ". Make sure your wallet is on devnet before signing & sending."
        </p>

        <Picker />
        <Connect />
        <SignMessage />
        <SendSol />
        <div class="status">{move || status_signal.get()}</div>
    }
}

// Reactive status line shared across sections. Stored in a Leptos context
// so each action can push a line without prop-drilling.
#[derive(Clone, Copy)]
struct StatusSignal(RwSignal<String>);

fn status() -> StatusSignal {
    use_context::<StatusSignal>().expect("StatusSignal missing")
}

#[component]
fn Picker() -> impl IntoView {
    let wallet = use_wallet();
    let wallet_for_when = wallet.clone();

    view! {
        <section>
            <h3>"1. Pick a wallet"</h3>
            <Show
                when=move || !wallet_for_when.wallets.get().is_empty()
                fallback=|| view! {
                    <p>"No Wallet Standard wallets detected. Install Phantom, Backpack, or Solflare."</p>
                }
            >
                {
                    let wallet = wallet.clone();
                    move || {
                        let wallet = wallet.clone();
                        let selected_name = wallet
                            .selected
                            .get()
                            .map(|s| s.name())
                            .unwrap_or_default();
                        wallet.wallets.get().0.into_iter().map(move |w| {
                            let name = w.name();
                            let icon = w.icon();
                            let supports = w.supports_solana();
                            let is_selected = selected_name == name;
                            let wallet_ctx = wallet.clone();
                            let class = if is_selected { "wallet-btn selected" } else { "wallet-btn" };
                            view! {
                                <button
                                    class=class
                                    on:click=move |_| wallet_ctx.select(w.clone())
                                    disabled=!supports
                                    style="margin-right: 8px; margin-bottom: 8px;"
                                >
                                    <img src=icon />
                                    <span>{name}</span>
                                </button>
                            }
                        }).collect_view()
                    }
                }
            </Show>
        </section>
    }
}

#[component]
fn Connect() -> impl IntoView {
    let wallet = use_wallet();
    let status = status();

    let has_selection = {
        let w = wallet.clone();
        move || w.selected.get().is_some()
    };
    let has_account = {
        let w = wallet.clone();
        move || w.account.get().is_some()
    };
    let address = {
        let w = wallet.clone();
        move || w.account.get().map(|a| a.address()).unwrap_or_default()
    };

    let connect_click = {
        let w = wallet.clone();
        move |_| {
            let w = w.clone();
            status.0.set("connecting…".into());
            spawn_local(async move {
                match w.connect().await {
                    Ok(()) => status.0.set("connected".into()),
                    Err(e) => status.0.set(format!("connect failed: {e}")),
                }
            });
        }
    };

    let disconnect_click = {
        let w = wallet.clone();
        move |_| {
            let w = w.clone();
            spawn_local(async move {
                let _ = w.disconnect().await;
                status.0.set("disconnected".into());
            });
        }
    };

    view! {
        <section>
            <h3>"2. Connect"</h3>
            <Show
                when=has_account.clone()
                fallback=move || {
                    let connect_click = connect_click.clone();
                    let has_selection = has_selection.clone();
                    view! {
                        <button on:click=connect_click disabled=move || !has_selection()>
                            "Connect"
                        </button>
                    }
                }
            >
                <div>
                    <strong>"Account: "</strong>
                    <code>{address.clone()}</code>
                </div>
                <button on:click=disconnect_click.clone() style="margin-top: 8px;">
                    "Disconnect"
                </button>
            </Show>
        </section>
    }
}

#[component]
fn SignMessage() -> impl IntoView {
    let wallet = use_wallet();
    let status = status();

    let (message, set_message) = signal("hello from leptos-solana".to_string());
    let (signature, set_signature) = signal(String::new());

    let has_account = {
        let w = wallet.clone();
        move || w.account.get().is_some()
    };

    let sign_click = {
        let w = wallet.clone();
        move |_| {
            let w = w.clone();
            let msg = message.get_untracked();
            status.0.set("waiting for wallet signature…".into());
            spawn_local(async move {
                match w.sign_message(msg.as_bytes()).await {
                    Ok(sig) => {
                        set_signature.set(bs58::encode(&sig).into_string());
                        status.0.set("message signed".into());
                    }
                    Err(e) => status.0.set(format!("sign failed: {e}")),
                }
            });
        }
    };

    view! {
        <section>
            <h3>"3. Sign a message"</h3>
            <input
                type="text"
                prop:value=message
                on:input=move |ev| set_message.set(event_target_value(&ev))
            />
            <button
                on:click=sign_click
                disabled=move || !has_account()
                style="margin-top: 8px;"
            >
                "Sign"
            </button>
            <Show when=move || !signature.get().is_empty()>
                <div style="margin-top: 8px;">
                    <strong>"Signature (bs58): "</strong>
                    <code>{move || signature.get()}</code>
                </div>
            </Show>
        </section>
    }
}

#[component]
fn SendSol() -> impl IntoView {
    let wallet = use_wallet();
    let status = status();
    let rpc = RpcClient::devnet();

    let (tx_sig, set_tx_sig) = signal(String::new());
    let (balance, set_balance) = signal(None::<u64>);

    let has_account = {
        let w = wallet.clone();
        move || w.account.get().is_some()
    };

    let refresh_balance = {
        let w = wallet.clone();
        let rpc = rpc.clone();
        move |_| {
            let Some(account) = w.account.get_untracked() else { return };
            let addr = account.address();
            let rpc = rpc.clone();
            spawn_local(async move {
                match rpc.get_balance(&addr).await {
                    Ok(lamports) => set_balance.set(Some(lamports)),
                    Err(e) => status.0.set(format!("balance rpc failed: {e}")),
                }
            });
        }
    };

    let send_click = {
        let w = wallet.clone();
        let rpc = rpc.clone();
        move |_| {
            let w = w.clone();
            let rpc = rpc.clone();
            let Some(account) = w.account.get_untracked() else { return };
            status.0.set("building transaction…".into());
            spawn_local(async move {
                let from: Pubkey = match account.address().parse() {
                    Ok(pk) => pk,
                    Err(e) => {
                        status.0.set(format!("bad address: {e}"));
                        return;
                    }
                };
                let ix = system_transfer(&from, &from, LAMPORTS_PER_SOL / 10_000);
                let blockhash = match rpc
                    .get_latest_blockhash(CommitmentConfig::confirmed())
                    .await
                {
                    Ok(bh) => bh,
                    Err(e) => {
                        status.0.set(format!("blockhash rpc failed: {e}"));
                        return;
                    }
                };
                let msg = Message::new_with_blockhash(&[ix], Some(&from), &blockhash);
                let tx: VersionedTransaction = Transaction::new_unsigned(msg).into();

                status.0.set("waiting for wallet to sign & send…".into());
                match w.sign_and_send(&tx).await {
                    Ok(sig_bytes) => {
                        let sig = bs58::encode(&sig_bytes).into_string();
                        set_tx_sig.set(sig.clone());
                        status.0.set(format!("sent: {sig}"));
                    }
                    Err(e) => status.0.set(format!("send failed: {e}")),
                }
            });
        }
    };

    view! {
        <section>
            <h3>"4. Self-transfer 0.0001 SOL"</h3>
            <p style="color: #888; font-size: 0.9em;">
                "Builds SystemProgram::transfer, serializes via bincode v1, "
                "hands the tx to the wallet for sign + submit."
            </p>
            <button on:click=refresh_balance disabled=move || !has_account()>
                "Refresh balance"
            </button>
            <Show when=move || balance.get().is_some()>
                {move || balance.get().map(|lamports| {
                    let sol = lamports as f64 / LAMPORTS_PER_SOL as f64;
                    view! { <span style="margin-left: 12px;">{format!("{sol:.6} SOL")}</span> }
                })}
            </Show>
            <div style="margin-top: 12px;">
                <button on:click=send_click disabled=move || !has_account()>
                    "Sign & send"
                </button>
            </div>
            <Show when=move || !tx_sig.get().is_empty()>
                <div style="margin-top: 8px;">
                    <strong>"Tx: "</strong>
                    <code>{move || tx_sig.get()}</code>
                    {move || {
                        let sig = tx_sig.get();
                        view! {
                            <a
                                href=format!("https://explorer.solana.com/tx/{sig}?cluster=devnet")
                                target="_blank"
                                rel="noopener"
                                style="margin-left: 8px;"
                            >
                                "Explorer →"
                            </a>
                        }
                    }}
                </div>
            </Show>
        </section>
    }
}

/// Build a `SystemProgram::transfer` instruction by hand (discriminator 2,
/// little-endian u64 lamports). Avoids pulling in `solana-system-interface`.
fn system_transfer(from: &Pubkey, to: &Pubkey, lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: SYSTEM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*from, true),
            AccountMeta::new(*to, false),
        ],
        data,
    }
}
