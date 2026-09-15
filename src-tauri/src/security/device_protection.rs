use thiserror::Error;
use zeroize::Zeroizing;

#[cfg(windows)]
const DEVICE_PIN_ENTROPY: &[u8] = b"KeyNest device PIN secret v1";

pub(crate) trait DeviceProtector: Send + Sync {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, DeviceProtectionError>;
    fn unprotect(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, DeviceProtectionError>;
}

#[derive(Debug, Error)]
pub(crate) enum DeviceProtectionError {
    #[error("Windows device protection is unavailable")]
    Unavailable,
}

pub(crate) struct OsDeviceProtector;

#[cfg(windows)]
mod windows {
    use std::ptr;

    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };

    use super::*;

    impl DeviceProtector for OsDeviceProtector {
        fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, DeviceProtectionError> {
            Ok(crypt(plaintext, true)?.to_vec())
        }

        fn unprotect(
            &self,
            ciphertext: &[u8],
        ) -> Result<Zeroizing<Vec<u8>>, DeviceProtectionError> {
            crypt(ciphertext, false)
        }
    }

    fn crypt(input: &[u8], protect: bool) -> Result<Zeroizing<Vec<u8>>, DeviceProtectionError> {
        let input_len =
            u32::try_from(input.len()).map_err(|_| DeviceProtectionError::Unavailable)?;
        let entropy_len = u32::try_from(DEVICE_PIN_ENTROPY.len())
            .map_err(|_| DeviceProtectionError::Unavailable)?;
        let input_blob = CRYPT_INTEGER_BLOB {
            cbData: input_len,
            pbData: input.as_ptr().cast_mut(),
        };
        let entropy_blob = CRYPT_INTEGER_BLOB {
            cbData: entropy_len,
            pbData: DEVICE_PIN_ENTROPY.as_ptr().cast_mut(),
        };
        let mut output_blob = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };

        // DPAPI allocates output with LocalAlloc. UI is forbidden so this call can
        // never present an OS credential prompt behind KeyNest.
        let succeeded = unsafe {
            if protect {
                CryptProtectData(
                    &input_blob,
                    ptr::null(),
                    &entropy_blob,
                    ptr::null(),
                    ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output_blob,
                )
            } else {
                CryptUnprotectData(
                    &input_blob,
                    ptr::null_mut(),
                    &entropy_blob,
                    ptr::null(),
                    ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output_blob,
                )
            }
        };
        if succeeded == 0 || output_blob.pbData.is_null() {
            if !output_blob.pbData.is_null() {
                unsafe {
                    let _ = LocalFree(output_blob.pbData.cast());
                }
            }
            return Err(DeviceProtectionError::Unavailable);
        }

        let output = unsafe {
            let slice = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
            let copied = Zeroizing::new(slice.to_vec());
            if !protect {
                std::ptr::write_bytes(output_blob.pbData, 0, output_blob.cbData as usize);
            }
            let _ = LocalFree(output_blob.pbData.cast());
            copied
        };
        Ok(output)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_dpapi_round_trip_is_non_plaintext_and_user_scoped() {
        let protector = OsDeviceProtector;
        let plaintext = b"fixture device secret";
        let protected = protector.protect(plaintext).unwrap();
        assert_ne!(protected, plaintext);
        assert_eq!(
            protector.unprotect(&protected).unwrap().as_slice(),
            plaintext
        );
    }
}

#[cfg(not(windows))]
impl DeviceProtector for OsDeviceProtector {
    fn protect(&self, _plaintext: &[u8]) -> Result<Vec<u8>, DeviceProtectionError> {
        Err(DeviceProtectionError::Unavailable)
    }

    fn unprotect(&self, _ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, DeviceProtectionError> {
        Err(DeviceProtectionError::Unavailable)
    }
}
