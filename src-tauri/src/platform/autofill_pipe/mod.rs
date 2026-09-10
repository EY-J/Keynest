mod client;
mod io;
mod security;
mod server;

pub use client::{forward_autofill_request, PipeError};
pub(crate) use server::PipeServer;

use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(750);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_millis(250);
const RECEIPT_TIMEOUT: Duration = Duration::from_millis(250);
const RECEIPT: u8 = 0x06;

#[cfg(test)]
mod tests;
