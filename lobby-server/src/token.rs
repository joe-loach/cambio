use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct AccessClaim {
    /// Subject of the JWT (the user)
    pub(crate) sub: String,
    /// Time after which the JWT expires
    pub(crate) exp: u64,
    /// Time at which the JWT was issued; can be used to determine age of the JWT
    pub(crate) iat: u64,
}

#[derive(Serialize, Deserialize)]
pub struct RefreshClaim {
    /// Subject of the JWT (the user)
    pub(crate) sub: String,
    /// Time after which the JWT expires
    pub(crate) exp: u64,
    /// Time at which the JWT was issued; can be used to determine age of the JWT
    pub(crate) iat: u64,
}

pub const REFRESH_TOKEN_COOKIE: &str = "refresh_token";

pub fn encode_access_token(
    issued_at: u64,
    sub: String,
) -> Result<std::string::String, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use std::env;
    use std::sync::LazyLock;

    static ACCESS_ENCODING_KEY: LazyLock<EncodingKey> = LazyLock::new(|| {
        EncodingKey::from_secret(
            env::var("ACCESS_TOKEN_SECRET")
                .expect("Secret in config")
                .as_bytes(),
        )
    });

    const ACCESS_EXPIRY: time::Duration = time::Duration::minutes(15);

    encode(
        &Header::default(),
        &AccessClaim {
            sub,
            exp: expires_in(issued_at, ACCESS_EXPIRY),
            iat: issued_at,
        },
        &ACCESS_ENCODING_KEY,
    )
}

pub fn encode_refresh_token(
    issued_at: u64,
    sub: String,
) -> Result<std::string::String, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use std::env;
    use std::sync::LazyLock;

    static REFRESH_ENCODING_KEY: LazyLock<EncodingKey> = LazyLock::new(|| {
        EncodingKey::from_secret(
            env::var("REFRESH_TOKEN_SECRET")
                .expect("Secret in config")
                .as_bytes(),
        )
    });

    const REFRESH_EXPIRY: time::Duration = time::Duration::days(7);

    encode(
        &Header::default(),
        &RefreshClaim {
            sub,
            exp: expires_in(issued_at, REFRESH_EXPIRY),
            iat: issued_at,
        },
        &REFRESH_ENCODING_KEY,
    )
}

fn expires_in(iat: u64, dur: time::Duration) -> u64 {
    iat.saturating_add(dur.whole_seconds().unsigned_abs())
}
