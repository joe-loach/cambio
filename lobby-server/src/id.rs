use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct Id(String);

pub const VALID_ID_CHARS: [char; 9] = ['1', '2', '3', '4', '5', '6', '7', '8', '9'];

impl Id {
    pub const LEN: usize = 6;
    /// Create a new random [`Id`].
    ///
    /// The [`Id`] will not be sequential and will consist of only VALID_CHARS.
    pub fn new() -> Self {
        // crate a random string from VALID_CHARS, len() == LEN
        let game_id = nanoid::format(nanoid::rngs::default, &VALID_ID_CHARS, Self::LEN);

        Self(game_id)
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    InvalidLength(usize),
    InvalidChars,
}

impl std::str::FromStr for Id {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check length
        if s.len() != Id::LEN {
            return Err(ParseError::InvalidLength(s.len()));
        }

        // Check if all characters are valid id characters
        if !s.chars().all(|c| VALID_ID_CHARS.contains(&c)) {
            return Err(ParseError::InvalidChars);
        }

        Ok(Id(s.to_string()))
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;

        s.parse().map_err(|err| match err {
            ParseError::InvalidLength(len) => serde::de::Error::invalid_length(
                len,
                &format!("expected Id of length {}", Id::LEN).as_str(),
            ),
            ParseError::InvalidChars => serde::de::Error::custom("Invalid characters in Id"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_id() {
        let id = Id::new();

        assert_eq!(id.0.len(), Id::LEN);
    }

    #[test]
    fn id_parse() {
        let s = String::from("456789");
        let _ = s
            .parse::<Id>()
            .expect("length of 6 and all chars are valid");

        let s = String::from("000000");
        let err = s.parse::<Id>().expect_err("0 is not a valid char");
        assert_eq!(err, ParseError::InvalidChars);

        let s = String::from("");
        let err = s.parse::<Id>().expect_err("s is empty");
        assert_eq!(err, ParseError::InvalidLength(0));

        let s = String::from("12345");
        let err = s.parse::<Id>().expect_err("s is too short");
        assert_eq!(err, ParseError::InvalidLength(5));

        let s = String::from("1234567");
        let err = s.parse::<Id>().expect_err("s is too long");
        assert_eq!(err, ParseError::InvalidLength(7));
    }
}
