//! Transaction serialization helpers.
//!
//! The crate is VersionedTransaction-only. Legacy `Transaction` still
//! works for construction (and `VersionedTransaction: From<Transaction>`
//! is implemented upstream), but the wallet-signing API accepts and
//! returns [`VersionedTransaction`].
//!
//! Wire format is bincode v1 with Solana's short-vec framing for all
//! count fields; `solana-transaction`'s serde impl handles both.

use solana_transaction::versioned::VersionedTransaction;

use crate::error::{Error, Result};

/// Serialize a [`VersionedTransaction`] for the wallet's
/// `solana:signTransaction` / `solana:signAndSendTransaction` features.
pub fn serialize(tx: &VersionedTransaction) -> Result<Vec<u8>> {
    bincode::serialize(tx).map_err(|e| Error::Serialize(e.to_string()))
}

/// Deserialize a signed [`VersionedTransaction`] returned by a wallet.
pub fn deserialize(bytes: &[u8]) -> Result<VersionedTransaction> {
    bincode::deserialize(bytes).map_err(|e| Error::Serialize(e.to_string()))
}
