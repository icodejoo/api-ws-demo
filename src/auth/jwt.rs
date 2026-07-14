use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iat: u64,
    pub exp: u64,
}

#[derive(Serialize)]
struct Header<'a> {
    alg: &'a str,
    typ: &'a str,
}

#[derive(Debug)]
pub enum JwtError {
    Malformed,
    BadSignature,
    Expired,
}

pub fn encode(claims: &Claims, secret: &[u8]) -> String {
    let header = Header {
        alg: "HS256",
        typ: "JWT",
    };
    let header_b64 = B64.encode(serde_json::to_vec(&header).expect("header serializes"));
    let claims_b64 = B64.encode(serde_json::to_vec(claims).expect("claims serialize"));
    let signing_input = format!("{header_b64}.{claims_b64}");
    let sig = sign(signing_input.as_bytes(), secret);
    format!("{signing_input}.{}", B64.encode(sig))
}

pub fn decode(token: &str, secret: &[u8]) -> Result<Claims, JwtError> {
    let mut parts = token.split('.');
    let (Some(h), Some(p), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(JwtError::Malformed);
    };

    let signing_input = format!("{h}.{p}");
    let expected_sig = sign(signing_input.as_bytes(), secret);
    let given_sig = B64.decode(s).map_err(|_| JwtError::Malformed)?;
    if given_sig != expected_sig {
        return Err(JwtError::BadSignature);
    }

    let claims_bytes = B64.decode(p).map_err(|_| JwtError::Malformed)?;
    let claims: Claims = serde_json::from_slice(&claims_bytes).map_err(|_| JwtError::Malformed)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after unix epoch")
        .as_secs();
    if claims.exp < now {
        return Err(JwtError::Expired);
    }

    Ok(claims)
}

fn sign(data: &[u8], secret: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn round_trip_valid_token() {
        let secret = b"test-secret";
        let claims = Claims {
            sub: "alice".into(),
            iat: now(),
            exp: now() + 900,
        };
        let token = encode(&claims, secret);
        let decoded = decode(&token, secret).unwrap();
        assert_eq!(decoded.sub, "alice");
    }

    #[test]
    fn rejects_bad_signature() {
        let claims = Claims {
            sub: "alice".into(),
            iat: now(),
            exp: now() + 900,
        };
        let token = encode(&claims, b"secret-a");
        assert!(matches!(
            decode(&token, b"secret-b"),
            Err(JwtError::BadSignature)
        ));
    }

    #[test]
    fn rejects_expired_token() {
        let claims = Claims {
            sub: "alice".into(),
            iat: now() - 100,
            exp: now() - 1,
        };
        let secret = b"test-secret";
        let token = encode(&claims, secret);
        assert!(matches!(decode(&token, secret), Err(JwtError::Expired)));
    }
}
