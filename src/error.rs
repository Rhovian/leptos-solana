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
        // Wallets conventionally throw `{ code: 4001, message: "User rejected" }`.
        if let Some(obj) = v.dyn_ref::<js_sys::Object>() {
            if let Ok(code) = js_sys::Reflect::get(obj, &"code".into()) {
                if code.as_f64() == Some(4001.0) {
                    return Error::UserRejected;
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
