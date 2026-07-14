use hmac::{Hmac, KeyInit, Mac};
use rand::Rng;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Fast keyed hash (HMAC-SHA256), deliberately not argon2/bcrypt: this is an
/// ephemeral test server on a CPU-constrained free instance, not a system
/// storing real user credentials, so deliberate stretching would only cost
/// CPU for no real security benefit here.
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);
    salt
}

pub fn hash_password(password: &str, salt: &[u8; 16]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(salt).expect("hmac accepts any key length");
    mac.update(password.as_bytes());
    mac.finalize().into_bytes().into()
}

pub fn verify_password(password: &str, salt: &[u8; 16], expected: &[u8; 32]) -> bool {
    hash_password(password, salt) == *expected
}
