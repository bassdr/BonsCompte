//! Bounded string types for safe deserialization
//!
//! These types limit string allocation size during serde deserialization
//! to prevent memory exhaustion attacks from malicious input.

use serde::{de, Deserialize, Deserializer};
use std::fmt;

/// Maximum lengths for various string fields
pub const MAX_USERNAME_LENGTH: usize = 50;
pub const MAX_DISPLAY_NAME_LENGTH: usize = 100;
pub const MAX_PROJECT_NAME_LENGTH: usize = 100;
pub const MAX_PROJECT_DESCRIPTION_LENGTH: usize = 500;
pub const MAX_PARTICIPANT_NAME_LENGTH: usize = 100;
pub const MAX_WARNING_HORIZON_LENGTH: usize = 30;
pub const MAX_SHORT_STRING_LENGTH: usize = 50;

/// A string that is bounded to a maximum length during deserialization.
/// This prevents memory exhaustion attacks from malicious input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedString<const N: usize>(String);

impl<const N: usize> BoundedString<N> {
    /// Creates a new bounded string, returning None if the input exceeds the limit
    pub fn new(s: String) -> Option<Self> {
        if s.len() <= N {
            Some(Self(s))
        } else {
            None
        }
    }

    /// Returns the inner string
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Returns a reference to the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the length of the string
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the string is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Trims whitespace and returns the result, or None if empty after trimming
    pub fn trim_non_empty(&self) -> Option<&str> {
        let trimmed = self.0.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }
}

impl<const N: usize> AsRef<str> for BoundedString<N> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<const N: usize> std::ops::Deref for BoundedString<N> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> fmt::Display for BoundedString<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de, const N: usize> Deserialize<'de> for BoundedString<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.len() > N {
            Err(de::Error::custom(format!(
                "string exceeds maximum length of {} bytes",
                N
            )))
        } else {
            Ok(BoundedString(s))
        }
    }
}

// Type aliases for common use cases
pub type Username = BoundedString<MAX_USERNAME_LENGTH>;
pub type DisplayName = BoundedString<MAX_DISPLAY_NAME_LENGTH>;
pub type ProjectName = BoundedString<MAX_PROJECT_NAME_LENGTH>;
pub type ProjectDescription = BoundedString<MAX_PROJECT_DESCRIPTION_LENGTH>;
pub type ParticipantName = BoundedString<MAX_PARTICIPANT_NAME_LENGTH>;
pub type WarningHorizon = BoundedString<MAX_WARNING_HORIZON_LENGTH>;
pub type ShortString = BoundedString<MAX_SHORT_STRING_LENGTH>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounded_string_within_limit() {
        let s: BoundedString<10> = serde_json::from_str("\"hello\"").unwrap();
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn test_bounded_string_at_limit() {
        let s: BoundedString<5> = serde_json::from_str("\"hello\"").unwrap();
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn test_bounded_string_exceeds_limit() {
        let result: Result<BoundedString<4>, _> = serde_json::from_str("\"hello\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_bounded_string_empty() {
        let s: BoundedString<10> = serde_json::from_str("\"\"").unwrap();
        assert!(s.is_empty());
    }
}
