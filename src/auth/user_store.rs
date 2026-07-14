#[derive(Clone)]
pub struct User {
    pub username: String,
    pub salt: [u8; 16],
    pub password_hash: [u8; 32],
}
