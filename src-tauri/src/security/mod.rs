mod auth;
mod auto_lock;
mod clipboard;
mod crypto;
mod locking;
mod storage;

pub(crate) use auth::{AuthError, AuthService, AuthStatus, LockOutcome};
pub(crate) use auto_lock::AutoLockService;
#[cfg(test)]
pub(crate) use auto_lock::LockActions;
pub(crate) use clipboard::{ClipboardService, TauriClipboardPort};
#[cfg(test)]
pub(crate) use crypto::{CryptoError, EntropySource};
pub(crate) use crypto::{KdfParams, OsEntropy};
pub(crate) use locking::{LockCoordinator, LockError, TauriLockEventSink};
pub(crate) use storage::ProfileStore;
