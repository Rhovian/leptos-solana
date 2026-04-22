//! `localStorage` helpers for remembering the last-connected wallet so
//! [`crate::context::provide_wallet_context`] can silent-reconnect on page
//! load.
//!
//! All errors are swallowed — Storage may be unavailable (Safari private
//! mode, iframes without permission), in which case the crate degrades to
//! no-persistence behavior.

use web_sys::Storage;

const KEY: &str = "leptos-solana:wallet";

pub fn last_wallet() -> Option<String> {
    storage()?.get_item(KEY).ok().flatten()
}

pub fn remember_wallet(name: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(KEY, name);
    }
}

pub fn forget_wallet() {
    if let Some(s) = storage() {
        let _ = s.remove_item(KEY);
    }
}

fn storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
