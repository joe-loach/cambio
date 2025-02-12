use serde::{Deserialize, Serialize};

/// A JWT token claim
#[derive(Serialize, Deserialize)]
pub struct TokenClaim {
    /// Subject of the JWT (the user)
    pub(crate) sub: String,
    /// Time after which the JWT expires
    pub(crate) exp: u64,
    /// Time at which the JWT was issued; can be used to determine age of the JWT
    pub(crate) iat: u64,
}