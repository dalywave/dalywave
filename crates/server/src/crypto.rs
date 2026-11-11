//! Случайные токены, хеши и PKCE.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn b64(data: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

pub fn b64_decode(data: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(data.trim_end_matches('=')).ok()
}

/// Случайные байты из системного генератора. Его отказ — поломка окружения: запрос получит 500.
pub fn random_bytes<const N: usize>() -> Result<[u8; N], getrandom::Error> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf)?;
    Ok(buf)
}

/// 256 бит в base64url: коды, state, nonce, verifier и токены сессий.
pub fn random_token() -> Result<String, getrandom::Error> {
    Ok(b64(random_bytes::<32>()?))
}

pub fn sha256(data: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(data.as_ref()).into()
}

/// PKCE S256: base64url(sha256(verifier)).
pub fn pkce_challenge(verifier: &str) -> String {
    b64(sha256(verifier))
}

/// verifier и challenge по RFC 7636: 43–128 символов из unreserved.
pub fn valid_pkce(value: &str) -> bool {
    (43..=128).contains(&value.len())
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~'))
}

/// Сравнение секретов за постоянное время. Хеши выравнивают длину, поэтому не утекает и она.
pub fn secrets_match(a: &str, b: &str) -> bool {
    sha256(a).ct_eq(&sha256(b)).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_rfc7636_example() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert!(valid_pkce("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"));
        assert!(!valid_pkce("short"));
        assert!(!valid_pkce(&"a".repeat(43).replace('a', "+")));
    }

    #[test]
    fn tokens_are_random_and_compare_safely() {
        let (a, b) = (random_token().unwrap(), random_token().unwrap());
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert!(secrets_match(&a, &a.clone()));
        assert!(!secrets_match(&a, &b));
        assert!(!secrets_match(&a, ""));
    }
}
