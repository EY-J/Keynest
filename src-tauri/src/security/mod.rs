mod auth;
mod auto_lock;
mod clipboard;
mod crypto;
mod locking;
mod storage;

pub(crate) use auth::{AuthError, AuthService, AuthStatus};
pub(crate) use auto_lock::AutoLockService;
#[cfg(test)]
pub(crate) use auto_lock::LockActions;
#[cfg(test)]
pub(crate) use clipboard::ClipboardPort;
pub(crate) use clipboard::{ClipboardError, ClipboardService, TauriClipboardPort};
#[cfg(test)]
pub(crate) use crypto::{CryptoError, EntropySource};
pub(crate) use crypto::{KdfParams, OsEntropy};
pub(crate) use locking::{LockCoordinator, LockError, TauriLockEventSink};
pub(crate) use storage::ProfileStore;
