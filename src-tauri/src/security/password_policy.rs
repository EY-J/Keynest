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

// Deterministic counterpart of shared/security/masterPasswordPolicy.ts. Shared
// test vectors guard the acceptance boundary; this is not measured entropy.
fn accepts_master_password(password: &str) -> bool {
    let characters = zeroize::Zeroizing::new(password.chars().collect::<Vec<_>>());
    let character_count = characters.len();
    if character_count < MINIMUM_PASSWORD_CHARACTERS {
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
    let common_match = [
        "password", "passw0rd", "letmein", "qwerty", "admin", "welcome", "iloveyou", "abc123",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern));
    if common_match {
        penalty += 30.0;
    }
    let lower_characters = zeroize::Zeroizing::new(lower.chars().collect::<Vec<_>>());
    if has_sequential_pattern(&lower_characters) {
        penalty += 20.0;
    }
    let keyboard_pattern = ["qwert", "asdf", "zxcv"]
        .iter()
        .any(|pattern| lower.contains(pattern));
    if keyboard_pattern {
        penalty += 30.0;
    }
    if characters.windows(3).any(|w| w[0] == w[1] && w[1] == w[2]) {
        penalty += 24.0;
    }
    let repeated_chunk = has_repeated_chunk(&characters);
    if repeated_chunk {
        penalty += 24.0;
    }
    if has_date_pattern(password.as_bytes()) {
        penalty += 15.0;
    }
    let unique_character_count = characters
        .iter()
        .enumerate()
        .filter(|(index, character)| !characters[..*index].contains(character))
        .count();
    let low_diversity =
        character_count >= 8 && unique_character_count as f64 / (character_count as f64) < 0.35;
    if low_diversity {
        penalty += 30.0;
    }
    let critical_pattern = low_diversity
        || repeated_chunk
        || (common_match && character_count <= 16)
        || (keyboard_pattern && character_count <= 16);

    !critical_pattern && character_count as f64 * pool.max(1.0).log2() - penalty >= 55.0
}

fn has_sequential_pattern(characters: &[char]) -> bool {
    characters.windows(4).any(|window| {
        let all_letters = window.iter().all(char::is_ascii_lowercase);
        let all_digits = window.iter().all(char::is_ascii_digit);
        if !all_letters && !all_digits {
            return false;
        }
        let step = window[1] as i32 - window[0] as i32;
        step.abs() == 1
            && window
                .windows(2)
                .all(|pair| pair[1] as i32 - pair[0] as i32 == step)
    })
}

fn has_repeated_chunk(characters: &[char]) -> bool {
    for chunk_length in 2..=usize::min(6, characters.len() / 2) {
        for start in 0..=characters.len() - chunk_length * 2 {
            if characters[start..start + chunk_length]
                == characters[start + chunk_length..start + chunk_length * 2]
            {
                return true;
            }
        }
    }
    false
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
