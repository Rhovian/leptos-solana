//! Wallet Standard discovery via CustomEvents on `window`.
//!
//! Spec (<https://github.com/wallet-standard/wallet-standard>):
//!
//! ```text
//!   App                                    Wallet
//!   ─────────────────────────────────────  ─────────────────────────────
//!   dispatch wallet-standard:app-ready  →  listens, calls api.register(w)
//!                       { detail: api }
//!
//!   listens for wallet-standard:        ←  dispatch wallet-standard:
//!     register-wallet                       register-wallet
//!                                            { detail: (api) => api.register(w) }
//!   invokes detail(api) → api.register(w)
//! ```
//!
//! `api = { register: fn(wallet) -> fn() }`. We ignore the unregister
//! return. Most wallets (Phantom, Backpack, Solflare, Glow, ...) call
//! `api.register(wallet)` with a single wallet — we only handle that case.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, Object, Reflect};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{CustomEvent, CustomEventInit, Window};

use crate::error::{Error, Result};
use crate::wallet::Wallet;

#[derive(Clone, Debug, Default)]
pub struct WalletList(pub Vec<Wallet>);

impl WalletList {
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn solana_only(self) -> Self {
        WalletList(self.0.into_iter().filter(|w| w.supports_solana()).collect())
    }
    pub fn find_by_name(&self, name: &str) -> Option<&Wallet> {
        self.0.iter().find(|w| w.name() == name)
    }
}

/// Owns the JS closures backing an active discovery subscription.
/// Dropping removes the `register-wallet` event listener and lets the
/// register closure be garbage-collected.
pub struct DiscoveryHandle {
    window: Window,
    // Held so they outlive the event listener; dropped in order.
    _register: Closure<dyn FnMut(JsValue)>,
    listener: Closure<dyn FnMut(JsValue)>,
}

impl Drop for DiscoveryHandle {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            "wallet-standard:register-wallet",
            self.listener.as_ref().unchecked_ref(),
        );
    }
}

/// Start Wallet Standard discovery. Calls `on_wallet` once per wallet —
/// both wallets already loaded (caught via `app-ready`) and wallets that
/// load later (caught via `register-wallet`).
///
/// Keep the returned [`DiscoveryHandle`] alive for as long as you want to
/// receive events; drop it to unsubscribe.
pub fn start<F>(on_wallet: F) -> Result<DiscoveryHandle>
where
    F: FnMut(Wallet) + 'static,
{
    let window = web_sys::window().ok_or_else(|| Error::Js("no window".into()))?;
    let on_wallet = Rc::new(RefCell::new(on_wallet));

    // `register(wallet)` — wallets call this to hand us a Wallet object.
    // Variadic in the spec (`register(w1, w2, ...)`) but virtually all
    // wallets pass one at a time; we only read the first arg.
    let register = {
        let on_wallet = on_wallet.clone();
        Closure::<dyn FnMut(JsValue)>::new(move |wallet: JsValue| {
            if !wallet.is_undefined() && !wallet.is_null() {
                (on_wallet.borrow_mut())(wallet.unchecked_into::<Wallet>());
            }
        })
    };

    // api = { register }
    let api = Object::new();
    Reflect::set(&api, &"register".into(), register.as_ref().unchecked_ref())?;

    // 1. Dispatch app-ready so already-loaded wallets register with us.
    let init = CustomEventInit::new();
    init.set_detail(&api);
    let event = CustomEvent::new_with_event_init_dict("wallet-standard:app-ready", &init)?;
    window.dispatch_event(&event)?;

    // 2. Subscribe to register-wallet so wallets loading later can register.
    //    Each event's `detail` is a callback `(api) => api.register(wallet)`.
    let listener = {
        let api = api.clone();
        Closure::<dyn FnMut(JsValue)>::new(move |ev: JsValue| {
            let Some(ev) = ev.dyn_ref::<CustomEvent>() else {
                return;
            };
            if let Ok(callback) = ev.detail().dyn_into::<Function>() {
                let _ = callback.call1(&JsValue::NULL, &api);
            }
        })
    };
    window.add_event_listener_with_callback(
        "wallet-standard:register-wallet",
        listener.as_ref().unchecked_ref(),
    )?;

    Ok(DiscoveryHandle {
        window,
        _register: register,
        listener,
    })
}
