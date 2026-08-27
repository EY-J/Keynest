use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::security::EntropySource;

use super::{VaultError, VaultRecordInput};

pub(super) const FORMAT_VERSION: i64 = 1;
const NONCE_LENGTH: usize = 24;
const AAD_PREFIX: &[u8] = b"keynest-vault-record-v1:";

#[derive(Deserialize, Serialize, Zeroize, ZeroizeOnDrop)]
struct CredentialPayload {
    name: String,
    username: String,
    password: String,
    website: Option<String>,
    category: String,
    tags: Vec<String>,
}

impl From<&VaultRecordInput> for CredentialPayload {
    fn from(input: &VaultRecordInput) -> Self {
        Self {
            name: input.name.clone(),
            username: input.username.clone(),
            password: input.password.clone(),
            website: input.website.clone(),
            category: input.category.clone(),
            tags: input.tags.clone(),
        }
    }
}

impl CredentialPayload {
    fn to_input(&self) -> VaultRecordInput {
        VaultRecordInput {
            name: self.name.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            website: self.website.clone(),
            category: self.category.clone(),
            tags: self.tags.clone(),
        }
    }
}

pub(super) struct EncryptedPayload {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub(super) fn encrypt(
    input: &VaultRecordInput,
    vault_key: &[u8; 32],
    id: &str,
    entropy: &dyn EntropySource,
) -> Result<EncryptedPayload, VaultError> {
    let payload = CredentialPayload::from(input);
    let plaintext =
        Zeroizing::new(serde_json::to_vec(&payload).map_err(|_| VaultError::DataDamaged)?);
    let mut nonce = [0_u8; NONCE_LENGTH];
    entropy
        .fill(&mut nonce)
        .map_err(|_| VaultError::EntropyUnavailable)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(vault_key).map_err(|_| VaultError::DataDamaged)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext.as_ref(),
                aad: &associated_data(id),
            },
        )
        .map_err(|_| VaultError::DataDamaged)?;

    Ok(EncryptedPayload {
        nonce: nonce.to_vec(),
        ciphertext,
    })
}

pub(super) fn decrypt(
    format_version: i64,
    nonce: &[u8],
    ciphertext: &[u8],
    vault_key: &[u8; 32],
    id: &str,
) -> Result<VaultRecordInput, VaultError> {
    if format_version != FORMAT_VERSION || nonce.len() != NONCE_LENGTH {
        return Err(VaultError::DataDamaged);
    }
    let cipher =
        XChaCha20Poly1305::new_from_slice(vault_key).map_err(|_| VaultError::DataDamaged)?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: &associated_data(id),
                },
            )
            .map_err(|_| VaultError::DataDamaged)?,
    );
    let payload: CredentialPayload =
        serde_json::from_slice(plaintext.as_ref()).map_err(|_| VaultError::DataDamaged)?;

    payload
        .to_input()
        .normalized()
        .map_err(|_| VaultError::DataDamaged)
}

fn associated_data(id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(AAD_PREFIX.len() + id.len());
    aad.extend_from_slice(AAD_PREFIX);
    aad.extend_from_slice(id.as_bytes());
    aad
}
