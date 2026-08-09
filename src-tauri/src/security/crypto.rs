use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroizing;

pub(crate) const PROFILE_AAD: &[u8] = b"keynest-profile-v1";
const SALT_LENGTH: usize = 16;
const VAULT_KEY_LENGTH: usize = 32;
const NONCE_LENGTH: usize = 24;
const WRAPPED_KEY_LENGTH: usize = VAULT_KEY_LENGTH + 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParams {
    pub(crate) const fn production() -> Self {
        Self {
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 4,
        }
    }

    #[cfg(test)]
    pub(crate) const fn testing() -> Self {
        Self {
            memory_kib: 32,
            iterations: 1,
            parallelism: 1,
        }
    }
}

pub(crate) struct VaultKey(Zeroizing<[u8; VAULT_KEY_LENGTH]>);

impl VaultKey {
    pub(crate) fn expose(&self) -> &[u8; VAULT_KEY_LENGTH] {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn expose_for_test(&self) -> &[u8; VAULT_KEY_LENGTH] {
        self.expose()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WrappedVaultKey {
    pub params: KdfParams,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

pub(crate) trait EntropySource: Send + Sync {
    fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError>;
}

pub(crate) struct OsEntropy;

impl EntropySource for OsEntropy {
    fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
        getrandom::fill(destination).map_err(|_| CryptoError::EntropyUnavailable)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum CryptoError {
    #[error("the cryptographic parameters are invalid")]
    InvalidParameters,
    #[error("the encrypted profile contains invalid key material")]
    InvalidMetadata,
    #[error("secure random data is unavailable")]
    EntropyUnavailable,
    #[error("the encrypted key could not be authenticated")]
    AuthenticationFailed,
}

pub(crate) fn wrap_new_vault_key(
    password: &str,
    params: KdfParams,
    entropy: &dyn EntropySource,
) -> Result<(WrappedVaultKey, VaultKey), CryptoError> {
    let mut salt = [0_u8; SALT_LENGTH];
    let mut nonce = [0_u8; NONCE_LENGTH];
    let mut vault_key = Zeroizing::new([0_u8; VAULT_KEY_LENGTH]);
    entropy.fill(&mut salt)?;
    entropy.fill(vault_key.as_mut())?;
    entropy.fill(&mut nonce)?;

    let wrapping_key = derive_wrapping_key(password, &salt, params)?;
    let cipher = XChaCha20Poly1305::new_from_slice(wrapping_key.as_ref())
        .map_err(|_| CryptoError::InvalidParameters)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: vault_key.as_ref(),
                aad: PROFILE_AAD,
            },
        )
        .map_err(|_| CryptoError::AuthenticationFailed)?;

    Ok((
        WrappedVaultKey {
            params,
            salt: STANDARD.encode(salt),
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
        },
        VaultKey(vault_key),
    ))
}

pub(crate) fn unwrap_vault_key(
    password: &str,
    wrapped: &WrappedVaultKey,
) -> Result<VaultKey, CryptoError> {
    let salt = decode_exact::<SALT_LENGTH>(&wrapped.salt)?;
    let nonce = decode_exact::<NONCE_LENGTH>(&wrapped.nonce)?;
    let ciphertext = decode_exact::<WRAPPED_KEY_LENGTH>(&wrapped.ciphertext)?;
    let wrapping_key = derive_wrapping_key(password, &salt, wrapped.params)?;
    let cipher = XChaCha20Poly1305::new_from_slice(wrapping_key.as_ref())
        .map_err(|_| CryptoError::InvalidParameters)?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: PROFILE_AAD,
                },
            )
            .map_err(|_| CryptoError::AuthenticationFailed)?,
    );
    let key: [u8; VAULT_KEY_LENGTH] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::InvalidMetadata)?;

    Ok(VaultKey(Zeroizing::new(key)))
}

fn derive_wrapping_key(
    password: &str,
    salt: &[u8; SALT_LENGTH],
    params: KdfParams,
) -> Result<Zeroizing<[u8; VAULT_KEY_LENGTH]>, CryptoError> {
    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(VAULT_KEY_LENGTH),
    )
    .map_err(|_| CryptoError::InvalidParameters)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut output = Zeroizing::new([0_u8; VAULT_KEY_LENGTH]);
    argon2
        .hash_password_into(password.as_bytes(), salt, output.as_mut())
        .map_err(|_| CryptoError::InvalidParameters)?;
    Ok(output)
}

fn decode_exact<const N: usize>(encoded: &str) -> Result<[u8; N], CryptoError> {
    STANDARD
        .decode(encoded)
        .map_err(|_| CryptoError::InvalidMetadata)?
        .try_into()
        .map_err(|_| CryptoError::InvalidMetadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedEntropy;

    impl EntropySource for FixedEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), CryptoError> {
            let length = destination.len() as u8;
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = length.wrapping_add(index as u8);
            }
            Ok(())
        }
    }

    #[test]
    fn correct_password_unwraps_the_generated_vault_key() {
        let entropy = FixedEntropy;
        let (wrapped, original) =
            wrap_new_vault_key("a secure master password", KdfParams::testing(), &entropy).unwrap();

        let unlocked = unwrap_vault_key("a secure master password", &wrapped).unwrap();

        assert_eq!(unlocked.expose_for_test(), original.expose_for_test());
    }

    #[test]
    fn incorrect_password_cannot_unwrap_the_vault_key() {
        let entropy = FixedEntropy;
        let (wrapped, _) =
            wrap_new_vault_key("a secure master password", KdfParams::testing(), &entropy).unwrap();

        assert!(matches!(
            unwrap_vault_key("the wrong master password", &wrapped),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn tampered_ciphertext_fails_authentication() {
        let entropy = FixedEntropy;
        let (mut wrapped, _) =
            wrap_new_vault_key("a secure master password", KdfParams::testing(), &entropy).unwrap();
        let mut ciphertext = STANDARD.decode(&wrapped.ciphertext).unwrap();
        ciphertext[0] ^= 1;
        wrapped.ciphertext = STANDARD.encode(ciphertext);

        assert!(matches!(
            unwrap_vault_key("a secure master password", &wrapped),
            Err(CryptoError::AuthenticationFailed)
        ));
    }
}
