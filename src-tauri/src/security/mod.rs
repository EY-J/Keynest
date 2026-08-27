mod auth;
mod auto_lock;
mod clipboard;
mod crypto;
mod locking;
mod operation;
mod storage;

pub(crate) use auth::{AuthError, AuthService, AuthStatus};
pub(crate) use auto_lock::AutoLockService;
#[cfg(test)]
pub(crate) use auto_lock::LockActions;
#[cfg(test)]
pub(crate) use clipboard::ClipboardPort;
pub(crate) use clipboard::{ClipboardError, ClipboardService, TauriClipboardPort};
#[cfg(test)]
pub(crate) use crypto::CryptoError;
pub(crate) use crypto::{EntropySource, KdfParams, OsEntropy};
#[cfg(test)]
pub(crate) use locking::LockEventSink;
pub(crate) use locking::{LockCoordinator, LockError, TauriLockEventSink};
pub(crate) use operation::{SecurityOperationGate, SecurityOperationGuard};
pub(crate) use storage::ProfileStore;
