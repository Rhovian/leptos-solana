//! Typed wrappers over Solana Wallet Standard feature methods.
//!
//! A feature lives at `wallet.features["solana:signTransaction"]` (etc.) as
//! an object with a method and metadata. Here we reach in via reflection,
//! build the expected input object, call the method, and decode the output.
//!
//! All feature methods are async — they return `Promise<Output>`. We wrap
//! them in `JsFuture` and expose plain `async fn`s.

use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;

use crate::error::{Error, Result};
use crate::wallet::{
    Wallet, WalletAccount, FEATURE_CONNECT, FEATURE_DISCONNECT,
    FEATURE_SIGN_AND_SEND_TRANSACTION, FEATURE_SIGN_MESSAGE, FEATURE_SIGN_TRANSACTION,
};

fn feature(wallet: &Wallet, id: &'static str) -> Result<JsValue> {
    let features = wallet.features();
    let feat = Reflect::get(&features, &JsValue::from_str(id))
        .map_err(|_| Error::MissingFeature(id))?;
    if feat.is_undefined() || feat.is_null() {
        return Err(Error::MissingFeature(id));
    }
    Ok(feat)
}

fn method(feature: &JsValue, name: &str) -> Result<Function> {
    Reflect::get(feature, &JsValue::from_str(name))?
        .dyn_into::<Function>()
        .map_err(|_| Error::Decode(format!("feature method `{name}` not a function")))
}

fn set(obj: &Object, key: &str, value: &JsValue) -> Result<()> {
    Reflect::set(obj, &JsValue::from_str(key), value)?;
    Ok(())
}

async fn call_promise(method: &Function, this: &JsValue, arg: &JsValue) -> Result<JsValue> {
    let called = method.call1(this, arg).map_err(|e| {
        web_sys::console::error_2(&"[leptos-solana] wallet call threw:".into(), &e);
        Error::from(e)
    })?;
    let promise: Promise = called
        .dyn_into()
        .map_err(|_| Error::Decode("feature method did not return a Promise".into()))?;
    JsFuture::from(promise).await.map_err(|e| {
        web_sys::console::error_2(&"[leptos-solana] wallet promise rejected:".into(), &e);
        Error::from(e)
    })
}

// ────────────────────────── standard:connect ──────────────────────────

/// Connect to the wallet, prompting the user for approval. Returns the
/// accounts the wallet chose to expose.
pub async fn connect(wallet: &Wallet, silent: bool) -> Result<Vec<WalletAccount>> {
    let feat = feature(wallet, FEATURE_CONNECT)?;
    let connect = method(&feat, "connect")?;

    let input = Object::new();
    if silent {
        set(&input, "silent", &JsValue::from_bool(true))?;
    }

    let output = call_promise(&connect, &feat, &input).await?;
    let accounts_js = Reflect::get(&output, &"accounts".into())?;
    let arr: Array = accounts_js
        .dyn_into()
        .map_err(|_| Error::Decode("connect output missing `accounts` array".into()))?;
    Ok(arr
        .iter()
        .map(|v| v.unchecked_into::<WalletAccount>())
        .collect())
}

// ──────────────────────── standard:disconnect ─────────────────────────

pub async fn disconnect(wallet: &Wallet) -> Result<()> {
    let feat = feature(wallet, FEATURE_DISCONNECT)?;
    let disconnect = method(&feat, "disconnect")?;
    let promise: Promise = disconnect
        .call0(&feat)?
        .dyn_into()
        .map_err(|_| Error::Decode("disconnect did not return Promise".into()))?;
    JsFuture::from(promise).await?;
    Ok(())
}

// ─────────────────────── solana:signMessage ───────────────────────────

/// Sign an arbitrary message. Returns the 64-byte signature.
pub async fn sign_message(
    wallet: &Wallet,
    account: &WalletAccount,
    message: &[u8],
) -> Result<Vec<u8>> {
    let feat = feature(wallet, FEATURE_SIGN_MESSAGE)?;
    let sign = method(&feat, "signMessage")?;

    let input = Object::new();
    set(&input, "account", account.unchecked_ref())?;
    set(&input, "message", Uint8Array::from(message).as_ref())?;

    // solana:signMessage(...inputs) is variadic — pass ONE input, get back
    // a one-element array of outputs.
    let output = call_promise(&sign, &feat, &input).await?;
    let out_arr: Array = output
        .dyn_into()
        .map_err(|_| Error::Decode("signMessage did not return array".into()))?;
    let first = out_arr.get(0);
    let sig_js = Reflect::get(&first, &"signature".into())?;
    let sig: Uint8Array = sig_js
        .dyn_into()
        .map_err(|_| Error::Decode("signMessage output missing signature".into()))?;
    Ok(sig.to_vec())
}

// ─────────────────────── solana:signTransaction ────────────────────────

/// Sign a serialized transaction (bincode v1 wire format). Returns the
/// signed transaction bytes.
pub async fn sign_transaction(
    wallet: &Wallet,
    account: &WalletAccount,
    chain: &str,
    transaction: &[u8],
) -> Result<Vec<u8>> {
    let feat = feature(wallet, FEATURE_SIGN_TRANSACTION)?;
    let sign = method(&feat, "signTransaction")?;

    let input = Object::new();
    set(&input, "account", account.unchecked_ref())?;
    set(&input, "chain", &JsValue::from_str(chain))?;
    set(&input, "transaction", Uint8Array::from(transaction).as_ref())?;

    let output = call_promise(&sign, &feat, &input).await?;
    let out_arr: Array = output
        .dyn_into()
        .map_err(|_| Error::Decode("signTransaction did not return array".into()))?;
    let first = out_arr.get(0);
    let signed_js = Reflect::get(&first, &"signedTransaction".into())?;
    let signed: Uint8Array = signed_js
        .dyn_into()
        .map_err(|_| Error::Decode("signTransaction output missing signedTransaction".into()))?;
    Ok(signed.to_vec())
}

// ──────────────────── solana:signAndSendTransaction ────────────────────

/// Sign *and* submit a transaction through the wallet's own RPC. Returns
/// the transaction signature (64 bytes).
pub async fn sign_and_send_transaction(
    wallet: &Wallet,
    account: &WalletAccount,
    chain: &str,
    transaction: &[u8],
) -> Result<Vec<u8>> {
    let feat = feature(wallet, FEATURE_SIGN_AND_SEND_TRANSACTION)?;
    let send = method(&feat, "signAndSendTransaction")?;

    let hex: String = transaction.iter().map(|b| format!("{b:02x}")).collect();
    web_sys::console::log_4(
        &"[leptos-solana] signAndSendTransaction chain=".into(),
        &JsValue::from_str(chain),
        &format!("len={} tx=", transaction.len()).into(),
        &JsValue::from_str(&hex),
    );

    let input = Object::new();
    set(&input, "account", account.unchecked_ref())?;
    set(&input, "chain", &JsValue::from_str(chain))?;
    set(&input, "transaction", Uint8Array::from(transaction).as_ref())?;

    let output = call_promise(&send, &feat, &input).await?;
    let out_arr: Array = output
        .dyn_into()
        .map_err(|_| Error::Decode("signAndSendTransaction did not return array".into()))?;
    let first = out_arr.get(0);
    let sig_js = Reflect::get(&first, &"signature".into())?;
    let sig: Uint8Array = sig_js
        .dyn_into()
        .map_err(|_| Error::Decode("signAndSendTransaction output missing signature".into()))?;
    Ok(sig.to_vec())
}
