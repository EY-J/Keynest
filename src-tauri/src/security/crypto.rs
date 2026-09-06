use argon2::{Algorithm, Argon2, Block, Params, Version};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub(crate) const PROFILE_AAD: &[u8] = b"keynest-profile-v1";
pub(crate) const RECOVERY_AAD: &[u8] = b"keynest-recovery-wrap-v1";
const SALT_LENGTH: usize = 16;
const VAULT_KEY_LENGTH: usize = 32;
const NONCE_LENGTH: usize = 24;
const WRAPPED_KEY_LENGTH: usize = VAULT_KEY_LENGTH + 16;
const RECOVERY_KEY_ENTROPY_LENGTH: usize = 16;
const RECOVERY_KEY_PREFIX: &str = "KN-R1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParams {
    pub(crate) const fn production() -> Self {
        // RFC 9106 section 4, second recommended (memory-constrained) option.
        // Fixed policy, not hardware calibration. See docs/security/argon2-policy.md.
        Self {
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 4,
        }
    }

    fn validate_production(self) -> Result<(), CryptoError> {
        // Read policy is deliberately independent of new-wrapper defaults.
        // Bound both memory and total pass work before Argon2 allocates memory.
        if !(65_536..=131_072).contains(&self.memory_kib)
            || !(3..=6).contains(&self.iterations)
            || !(1..=4).contains(&self.parallelism)
            || u64::from(self.memory_kib) * u64::from(self.iterations) > 393_216
        {
            return Err(CryptoError::InvalidParameters);
        }
        Ok(())
    }

    pub(crate) fn validate(self) -> Result<(), CryptoError> {
        // This exact inexpensive fixture is absent from non-test builds. No
        // persisted flag, environment variable, or IPC input can enable it.
        #[cfg(test)]
        if self == Self::testing() {
            return Ok(());
        }
        self.validate_production()
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

#[derive(Zeroize, ZeroizeOnDrop)]
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
#[serde(deny_unknown_fields)]
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
    #[error("the recovery key format is invalid")]
    InvalidRecoveryKey,
}

pub(crate) fn wrap_new_vault_key(
    password: &str,
    params: KdfParams,
    entropy: &dyn EntropySource,
) -> Result<(WrappedVaultKey, VaultKey), CryptoError> {
    let mut vault_key = Zeroizing::new([0_u8; VAULT_KEY_LENGTH]);
    entropy.fill(vault_key.as_mut())?;
    let wrapped_key = wrap_existing_vault_key(password, &vault_key, params, entropy)?;

    Ok((wrapped_key, VaultKey(vault_key)))
}

pub(crate) fn wrap_existing_vault_key(
    password: &str,
    vault_key: &[u8; VAULT_KEY_LENGTH],
    params: KdfParams,
    entropy: &dyn EntropySource,
) -> Result<WrappedVaultKey, CryptoError> {
    wrap_existing_vault_key_with_aad(password, vault_key, params, entropy, PROFILE_AAD)
}

pub(crate) fn generate_recovery_key(
    entropy: &dyn EntropySource,
) -> Result<Zeroizing<String>, CryptoError> {
    let mut bytes = Zeroizing::new([0_u8; RECOVERY_KEY_ENTROPY_LENGTH]);
    entropy.fill(bytes.as_mut())?;
    let mut encoded = Zeroizing::new(String::with_capacity(5 + 1 + (8 * 5) - 1));
    encoded.push_str(RECOVERY_KEY_PREFIX);
    for (index, byte) in bytes.iter().enumerate() {
        if index % 2 == 0 {
            encoded.push('-');
        }
        use std::fmt::Write as _;
        write!(encoded, "{byte:02X}").map_err(|_| CryptoError::InvalidParameters)?;
    }
    Ok(encoded)
}

pub(crate) fn normalize_recovery_key(recovery_key: &str) -> Result<Zeroizing<String>, CryptoError> {
    let compact = Zeroizing::new(
        recovery_key
            .chars()
            .filter(|character| !character.is_ascii_whitespace() && *character != '-')
            .collect::<String>(),
    );
    if !compact.is_ascii() || compact.len() != 4 + (RECOVERY_KEY_ENTROPY_LENGTH * 2) {
        return Err(CryptoError::InvalidRecoveryKey);
    }
    let upper = Zeroizing::new(compact.to_ascii_uppercase());
    if !upper.starts_with("KNR1") || !upper[4..].bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CryptoError::InvalidRecoveryKey);
    }

    let mut canonical = Zeroizing::new(String::with_capacity(45));
    canonical.push_str(RECOVERY_KEY_PREFIX);
    for (index, chunk) in upper[4..].as_bytes().chunks(4).enumerate() {
        debug_assert_eq!(chunk.len(), 4);
        if index < 8 {
            canonical.push('-');
        }
        canonical
            .push_str(std::str::from_utf8(chunk).map_err(|_| CryptoError::InvalidRecoveryKey)?);
    }
    Ok(canonical)
}

pub(crate) fn wrap_recovery_vault_key(
    recovery_key: &str,
    vault_key: &[u8; VAULT_KEY_LENGTH],
    params: KdfParams,
    entropy: &dyn EntropySource,
) -> Result<WrappedVaultKey, CryptoError> {
    let normalized = normalize_recovery_key(recovery_key)?;
    wrap_existing_vault_key_with_aad(&normalized, vault_key, params, entropy, RECOVERY_AAD)
}

fn wrap_existing_vault_key_with_aad(
    secret: &str,
    vault_key: &[u8; VAULT_KEY_LENGTH],
    params: KdfParams,
    entropy: &dyn EntropySource,
    aad: &[u8],
) -> Result<WrappedVaultKey, CryptoError> {
    let mut salt = [0_u8; SALT_LENGTH];
    let mut nonce = [0_u8; NONCE_LENGTH];
    entropy.fill(&mut salt)?;
    entropy.fill(&mut nonce)?;

    let wrapping_key = derive_wrapping_key(secret, &salt, params)?;
    let cipher = XChaCha20Poly1305::new_from_slice(wrapping_key.as_ref())
        .map_err(|_| CryptoError::InvalidParameters)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: vault_key,
                aad,
            },
        )
        .map_err(|_| CryptoError::AuthenticationFailed)?;

    Ok(WrappedVaultKey {
        params,
        salt: STANDARD.encode(salt),
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
}

pub(crate) fn unwrap_vault_key(
    password: &str,
    wrapped: &WrappedVaultKey,
) -> Result<VaultKey, CryptoError> {
    unwrap_vault_key_with_aad(password, wrapped, PROFILE_AAD)
}

pub(crate) fn unwrap_recovery_vault_key(
    recovery_key: &str,
    wrapped: &WrappedVaultKey,
) -> Result<VaultKey, CryptoError> {
    let normalized = normalize_recovery_key(recovery_key)?;
    unwrap_vault_key_with_aad(&normalized, wrapped, RECOVERY_AAD)
}

fn unwrap_vault_key_with_aad(
    secret: &str,
    wrapped: &WrappedVaultKey,
    aad: &[u8],
) -> Result<VaultKey, CryptoError> {
    let salt = decode_exact::<SALT_LENGTH>(&wrapped.salt)?;
    let nonce = decode_exact::<NONCE_LENGTH>(&wrapped.nonce)?;
    let ciphertext = decode_exact::<WRAPPED_KEY_LENGTH>(&wrapped.ciphertext)?;
    let wrapping_key = derive_wrapping_key(secret, &salt, wrapped.params)?;
    let cipher = XChaCha20Poly1305::new_from_slice(wrapping_key.as_ref())
        .map_err(|_| CryptoError::InvalidParameters)?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad,
                },
            )
            .map_err(|_| CryptoError::AuthenticationFailed)?,
    );
    if plaintext.len() != VAULT_KEY_LENGTH {
        return Err(CryptoError::InvalidMetadata);
    }
    let mut key = Zeroizing::new([0_u8; VAULT_KEY_LENGTH]);
    key.copy_from_slice(&plaintext);
    Ok(VaultKey(key))
}

fn derive_wrapping_key(
    password: &str,
    salt: &[u8; SALT_LENGTH],
    params: KdfParams,
) -> Result<Zeroizing<[u8; VAULT_KEY_LENGTH]>, CryptoError> {
    params.validate()?;
    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(VAULT_KEY_LENGTH),
    )
    .map_err(|_| CryptoError::InvalidParameters)?;
    // The convenience Argon2 API allocates an ordinary Vec<Block>. Own the workspace
    // so it is wiped on success, errors and unwinding, as well as the derived output.
    let mut memory = Zeroizing::new(vec![Block::default(); argon_params.block_count()]);
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut output = Zeroizing::new([0_u8; VAULT_KEY_LENGTH]);
    argon2
        .hash_password_into_with_memory(
            password.as_bytes(),
            salt,
            output.as_mut(),
            memory.as_mut_slice(),
        )
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
    #[test]
    fn key_and_argon_workspace_have_zeroizing_owners() {
        fn assert_wipes_on_drop<T: zeroize::Zeroize + zeroize::ZeroizeOnDrop>() {}
        assert_wipes_on_drop::<super::VaultKey>();
        assert_wipes_on_drop::<zeroize::Zeroizing<Vec<argon2::Block>>>();
        let mut key = super::VaultKey(zeroize::Zeroizing::new([42; 32]));
        zeroize::Zeroize::zeroize(&mut key);
        assert_eq!(key.expose(), &[0; 32]);
    }

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
    fn production_kdf_is_explicit_and_resource_bounds_are_independent_of_defaults() {
        assert_eq!(
            KdfParams::production(),
            KdfParams {
                memory_kib: 65_536,
                iterations: 3,
                parallelism: 4,
            }
        );
        assert!(KdfParams::testing().validate_production().is_err());
        for params in [
            KdfParams::production(),
            KdfParams {
                memory_kib: 131_072,
                iterations: 3,
                parallelism: 4,
            },
            KdfParams {
                memory_kib: 65_536,
                iterations: 6,
                parallelism: 1,
            },
        ] {
            assert!(params.validate_production().is_ok());
        }
    }

    #[test]
    fn invalid_kdf_costs_fail_before_derivation_including_direct_crypto_calls() {
        let params = KdfParams::production();
        let mut invalid = Vec::new();
        for memory_kib in [0, 1, 65_535, 131_073, u32::MAX] {
            invalid.push(KdfParams {
                memory_kib,
                ..params
            });
        }
        for iterations in [0, 1, 2, 7, u32::MAX] {
            invalid.push(KdfParams {
                iterations,
                ..params
            });
        }
        for parallelism in [0, 5, u32::MAX] {
            invalid.push(KdfParams {
                parallelism,
                ..params
            });
        }
        invalid.push(KdfParams {
            memory_kib: 131_072,
            iterations: 4,
            ..params
        });
        let (valid_wrapper, _) =
            wrap_new_vault_key("fixture password", KdfParams::testing(), &FixedEntropy).unwrap();
        for params in invalid {
            assert_eq!(params.validate(), Err(CryptoError::InvalidParameters));
            assert!(matches!(
                derive_wrapping_key("fixture password", &[0; 16], params),
                Err(CryptoError::InvalidParameters)
            ));
            let mut wrapped = valid_wrapper.clone();
            wrapped.params = params;
            assert!(matches!(
                unwrap_vault_key("fixture password", &wrapped),
                Err(CryptoError::InvalidParameters)
            ));
            assert!(matches!(
                wrap_existing_vault_key("fixture password", &[0; 32], params, &FixedEntropy),
                Err(CryptoError::InvalidParameters)
            ));
        }
    }

    #[test]
    fn each_wrapper_gets_a_fresh_os_random_128_bit_salt() {
        let params = KdfParams::testing();
        let (first, key) = wrap_new_vault_key("fixture password", params, &OsEntropy).unwrap();
        let second =
            wrap_existing_vault_key("fixture password", key.expose(), params, &OsEntropy).unwrap();
        let recovery = generate_recovery_key(&OsEntropy).unwrap();
        let third = wrap_recovery_vault_key(&recovery, key.expose(), params, &OsEntropy).unwrap();
        let salts = [&first.salt, &second.salt, &third.salt];
        for salt in salts {
            assert_eq!(STANDARD.decode(salt).unwrap().len(), 16);
        }
        assert_ne!(first.salt, second.salt);
        assert_ne!(first.salt, third.salt);
        assert_ne!(second.salt, third.salt);
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

    #[test]
    fn password_change_rewraps_existing_vault_key_without_changing_it() {
        let entropy = FixedEntropy;
        let (_, vault_key) =
            wrap_new_vault_key("old secure master password", KdfParams::testing(), &entropy)
                .unwrap();

        let wrapped = wrap_existing_vault_key(
            "new secure master password",
            vault_key.expose(),
            KdfParams::testing(),
            &entropy,
        )
        .unwrap();
        let unwrapped = unwrap_vault_key("new secure master password", &wrapped).unwrap();

        assert_eq!(unwrapped.expose_for_test(), vault_key.expose_for_test());
    }

    #[test]
    fn generated_recovery_key_has_128_bits_in_a_versioned_readable_format() {
        let key = generate_recovery_key(&FixedEntropy).unwrap();

        assert_eq!(
            key.as_str(),
            "KN-R1-1011-1213-1415-1617-1819-1A1B-1C1D-1E1F"
        );
        assert_eq!(normalize_recovery_key(&key).unwrap().as_str(), key.as_str());
        assert_eq!(
            normalize_recovery_key(" kn-r1 1011-1213 1415-1617-1819-1a1b-1c1d-1e1f ")
                .unwrap()
                .as_str(),
            key.as_str()
        );
    }

    #[test]
    fn malformed_recovery_keys_are_rejected_without_truncating_entropy() {
        for invalid in [
            "KN-R1-ABCD",
            "KN-R2-1011-1213-1415-1617-1819-1A1B-1C1D-1E1F",
            "KN-R1-1011-1213-1415-1617-1819-1A1B-1C1D-1E1G",
            "KN-R1-1011-1213-1415-1617-1819-1A1B-1C1D-1E1F00",
        ] {
            assert_eq!(
                normalize_recovery_key(invalid),
                Err(CryptoError::InvalidRecoveryKey)
            );
        }
    }

    #[test]
    fn recovery_wrapper_uses_the_same_vault_key_and_an_independent_aad_domain() {
        let (_, vault_key) = wrap_new_vault_key(
            "a secure master password",
            KdfParams::testing(),
            &FixedEntropy,
        )
        .unwrap();
        let recovery_key = generate_recovery_key(&FixedEntropy).unwrap();
        let wrapped = wrap_recovery_vault_key(
            &recovery_key,
            vault_key.expose(),
            KdfParams::testing(),
            &FixedEntropy,
        )
        .unwrap();

        assert_eq!(
            unwrap_recovery_vault_key(&recovery_key, &wrapped)
                .unwrap()
                .expose_for_test(),
            vault_key.expose_for_test()
        );
        assert!(matches!(
            unwrap_vault_key(&recovery_key, &wrapped),
            Err(CryptoError::AuthenticationFailed)
        ));
        // Even identical input secrets and test salt/nonce cannot cross AAD domains.
        let master = wrap_existing_vault_key(
            &recovery_key,
            vault_key.expose(),
            KdfParams::testing(),
            &FixedEntropy,
        )
        .unwrap();
        assert!(matches!(
            unwrap_recovery_vault_key(&recovery_key, &master),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    #[test]
    fn wrong_or_tampered_recovery_wrapper_fails_authentication() {
        let (_, vault_key) = wrap_new_vault_key(
            "a secure master password",
            KdfParams::testing(),
            &FixedEntropy,
        )
        .unwrap();
        let recovery_key = generate_recovery_key(&FixedEntropy).unwrap();
        let wrapped = wrap_recovery_vault_key(
            &recovery_key,
            vault_key.expose(),
            KdfParams::testing(),
            &FixedEntropy,
        )
        .unwrap();
        let wrong = "KN-R1-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF-FFFF";
        assert!(matches!(
            unwrap_recovery_vault_key(wrong, &wrapped),
            Err(CryptoError::AuthenticationFailed)
        ));

        for field in ["salt", "nonce", "ciphertext"] {
            let mut tampered = wrapped.clone();
            let encoded = match field {
                "salt" => &mut tampered.salt,
                "nonce" => &mut tampered.nonce,
                _ => &mut tampered.ciphertext,
            };
            let mut bytes = STANDARD.decode(&*encoded).unwrap();
            bytes[0] ^= 1;
            *encoded = STANDARD.encode(bytes);
            assert!(matches!(
                unwrap_recovery_vault_key(&recovery_key, &tampered),
                Err(CryptoError::AuthenticationFailed)
            ));
        }
    }
}
