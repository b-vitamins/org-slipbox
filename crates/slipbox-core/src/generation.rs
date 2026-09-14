//! Caller-supplied identity for a source generation, not proof of publication or readiness.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::source_identity::SourceId;

const MAX_GENERATION_CHARS: usize = 64;

/// A generation identity: 1 to 64 characters of `A-Za-z0-9`, `.`, `-` or `_`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GenerationId(String);

impl GenerationId {
    pub fn parse(value: &str) -> Result<Self, GenerationIdError> {
        let found = value.chars().count();
        if found == 0 {
            return Err(GenerationIdError::Empty);
        }
        if found > MAX_GENERATION_CHARS {
            return Err(GenerationIdError::Length { found });
        }
        if !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-' || byte == b'_'
        }) {
            return Err(GenerationIdError::Charset);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GenerationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<String> for GenerationId {
    type Error = GenerationIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<GenerationId> for String {
    fn from(value: GenerationId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationIdError {
    Empty,
    Length { found: usize },
    Charset,
}

impl fmt::Display for GenerationIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a generation identity is not empty"),
            Self::Length { found } => write!(
                formatter,
                "a generation identity is at most {MAX_GENERATION_CHARS} characters long, not {found}"
            ),
            Self::Charset => formatter.write_str(
                "a generation identity carries only alphanumerics, dots, dashes and underscores",
            ),
        }
    }
}

impl Error for GenerationIdError {}

/// The source and generation that a bound reader answers for.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GenerationBinding {
    pub source: SourceId,
    pub generation: GenerationId,
}

impl GenerationBinding {
    #[must_use]
    pub fn new(source: SourceId, generation: GenerationId) -> Self {
        Self { source, generation }
    }
}

impl fmt::Display for GenerationBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.source, self.generation)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::source_identity::SOURCE_ID_ENTROPY_BYTES;

    #[test]
    fn a_generation_identity_accepts_the_shapes_a_fixture_name_takes() {
        for value in ["1", "fixture-01", "2026.09.14_a", "A", &"g".repeat(64)] {
            let parsed = GenerationId::parse(value).expect("a plain generation identity");
            assert_eq!(parsed.as_str(), value);
            assert_eq!(parsed.to_string(), value);
        }
    }

    #[test]
    fn a_generation_identity_refuses_empty_long_and_foreign_characters() {
        assert_eq!(GenerationId::parse(""), Err(GenerationIdError::Empty));
        assert_eq!(
            GenerationId::parse(&"g".repeat(65)),
            Err(GenerationIdError::Length { found: 65 })
        );
        for value in ["fixture 01", "fixture/01", "généra", "a\0b"] {
            assert_eq!(GenerationId::parse(value), Err(GenerationIdError::Charset));
        }
    }

    #[test]
    fn a_generation_identity_travels_as_a_validated_string() {
        let id: GenerationId =
            serde_json::from_value(json!("fixture-01")).expect("a plain generation identity");
        assert_eq!(
            serde_json::to_value(&id).expect("a generation identity serializes"),
            json!("fixture-01")
        );
        serde_json::from_value::<GenerationId>(json!("fixture 01"))
            .expect_err("a generation identity with a space is refused on the wire");
    }

    #[test]
    fn a_binding_names_both_halves_on_the_wire() {
        let binding = GenerationBinding::new(
            SourceId::mint([7; SOURCE_ID_ENTROPY_BYTES]),
            GenerationId::parse("fixture-01").expect("a plain generation identity"),
        );

        assert_eq!(
            serde_json::to_value(&binding).expect("a binding serializes"),
            json!({ "source": binding.source.as_str(), "generation": "fixture-01" })
        );
        assert_eq!(
            binding.to_string(),
            format!("{}@fixture-01", binding.source)
        );
    }
}
