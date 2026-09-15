use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use zeroize::Zeroizing;

use crate::security::EntropySource;

use super::{NoteInput, NotesError};

pub(super) const FORMAT_VERSION: i64 = 1;
const NONCE_LENGTH: usize = 24;
const AAD_PREFIX: &[u8] = b"keynest-note-v1:";

pub(super) struct EncryptedNote {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub(super) fn encrypt(
    input: &NoteInput,
    key: &[u8; 32],
    id: &str,
    entropy: &dyn EntropySource,
) -> Result<EncryptedNote, NotesError> {
    let mut plaintext = Zeroizing::new(Vec::new());
    serde_json::to_writer(&mut *plaintext, input).map_err(|_| NotesError::DataDamaged)?;
    let mut nonce = [0_u8; NONCE_LENGTH];
    entropy
        .fill(&mut nonce)
        .map_err(|_| NotesError::EntropyUnavailable)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| NotesError::DataDamaged)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext.as_ref(),
                aad: &associated_data(id),
            },
        )
        .map_err(|_| NotesError::DataDamaged)?;
    Ok(EncryptedNote {
        nonce: nonce.to_vec(),
        ciphertext,
    })
}

pub(super) fn decrypt(
    format_version: i64,
    nonce: &[u8],
    ciphertext: &[u8],
    key: &[u8; 32],
    id: &str,
) -> Result<NoteInput, NotesError> {
    if format_version != FORMAT_VERSION || nonce.len() != NONCE_LENGTH {
        return Err(NotesError::DataDamaged);
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| NotesError::DataDamaged)?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: &associated_data(id),
                },
            )
            .map_err(|_| NotesError::DataDamaged)?,
    );
    serde_json::from_slice::<NoteInput>(plaintext.as_ref())
        .map_err(|_| NotesError::DataDamaged)?
        .normalized()
        .map_err(|_| NotesError::DataDamaged)
}

fn associated_data(id: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity(AAD_PREFIX.len() + id.len());
    data.extend_from_slice(AAD_PREFIX);
    data.extend_from_slice(id.as_bytes());
    data
}
