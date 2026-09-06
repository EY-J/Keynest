use super::auth::AuthError;

const MINIMUM_PASSWORD_CHARACTERS: usize = 12;

// Only newly established passwords are validated. Existing profiles must remain
// unlockable, even when their passwords predate this policy.
pub(super) fn validate_master_password(password: &str) -> Result<(), AuthError> {
    if password.chars().count() < MINIMUM_PASSWORD_CHARACTERS {
        return Err(AuthError::PasswordTooShort);
    }
    if !accepts_master_password(password) {
        return Err(AuthError::PasswordTooWeak);
    }
    Ok(())
}

// Deterministic counterpart of shared/security/masterPasswordPolicy.ts. Preserve the
// Vault heuristic (including UTF-16 length and ASCII character classes). Shared
// test vectors guard the acceptance boundary; this is not measured entropy.
fn accepts_master_password(password: &str) -> bool {
    if password.chars().count() < MINIMUM_PASSWORD_CHARACTERS {
        return false;
    }
    let mut pool: f64 = 0.0;
    if password.bytes().any(|c| c.is_ascii_lowercase()) {
        pool += 26.0;
    }
    if password.bytes().any(|c| c.is_ascii_uppercase()) {
        pool += 26.0;
    }
    if password.bytes().any(|c| c.is_ascii_digit()) {
        pool += 10.0;
    }
    if password.chars().any(|c| !c.is_ascii_alphanumeric()) {
        pool += 32.0;
    }
    let lower = zeroize::Zeroizing::new(password.to_lowercase());
    let mut penalty = 0.0;
    if [
        "password", "passw0rd", "letmein", "qwerty", "admin", "welcome", "iloveyou", "abc123",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
    {
        penalty += 30.0;
    }
    if [
        "0123", "1234", "2345", "3456", "4567", "5678", "6789", "9876", "8765", "7654", "6543",
        "5432", "4321",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
    {
        penalty += 12.0;
    }
    if ["qwert", "asdf", "zxcv"]
        .iter()
        .any(|pattern| lower.contains(pattern))
    {
        penalty += 18.0;
    }
    let units = zeroize::Zeroizing::new(password.encode_utf16().collect::<Vec<_>>());
    if units
        .windows(3)
        .any(|w| w[0] == w[1] && w[1] == w[2] && ![10, 13, 0x2028, 0x2029].contains(&w[0]))
    {
        penalty += 12.0;
    }
    if has_date_pattern(password.as_bytes()) {
        penalty += 10.0;
    }
    // Fair (Good) starts at 40; Strong starts at 55 in the existing analyzer.
    units.len() as f64 * pool.max(1.0).log2() - penalty >= 40.0
}

fn word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn has_date_pattern(bytes: &[u8]) -> bool {
    if bytes
        .windows(4)
        .any(|w| (&w[..2] == b"19" || &w[..2] == b"20") && w[2..].iter().all(u8::is_ascii_digit))
    {
        return true;
    }
    // Equivalent to JS \b\d{1,2}[/-]\d{1,2}(?:[/-]\d{2,4})?\b,
    // including its ASCII word boundaries and optional-year backtracking.
    for start in 0..bytes.len() {
        if start > 0 && word(bytes[start - 1]) {
            continue;
        }
        for month_len in 1..=2 {
            let separator = start + month_len;
            if separator >= bytes.len()
                || !bytes[start..separator].iter().all(u8::is_ascii_digit)
                || !matches!(bytes[separator], b'/' | b'-')
            {
                continue;
            }
            for day_len in 1..=2 {
                let end = separator + 1 + day_len;
                if end > bytes.len() || !bytes[separator + 1..end].iter().all(u8::is_ascii_digit) {
                    continue;
                }
                if end == bytes.len() || !word(bytes[end]) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_frontend_policy_vectors() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/master-password-policy.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(
                accepts_master_password(case["password"].as_str().unwrap()),
                case["strength"].as_str().unwrap() != "Weak",
                "case {}",
                case["id"]
            );
        }
    }
}
