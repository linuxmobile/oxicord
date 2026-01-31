//! Serde utilities for Oxicord.

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serializer};
use std::fmt;

/// Module to handle deserialization of Snowflake IDs that might be strings or numbers.
pub mod string_to_u64 {
    use super::{Deserializer, Serializer, Visitor, de, fmt};

    /// Serializes a u64 as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the serializer fails.
    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    /// Deserializes a u64 from a string or number.
    /// Handles negative string representations by treating them as i64 bit patterns.
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not a string or integer, or if parsing fails.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringOrIntVisitor;

        impl Visitor<'_> for StringOrIntVisitor {
            type Value = u64;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or integer representing a snowflake ID")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value)
            }

            #[allow(clippy::cast_sign_loss)]
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value as u64)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.starts_with('-') {
                    // Handle negative string representation of u64 (interpreted as i64)
                    let val_i64 = value.parse::<i64>().map_err(de::Error::custom)?;
                    #[allow(clippy::cast_sign_loss)]
                    Ok(val_i64 as u64)
                } else {
                    value.parse::<u64>().map_err(de::Error::custom)
                }
            }
        }

        deserializer.deserialize_any(StringOrIntVisitor)
    }

    /// Module to handle deserialization of optional Snowflake IDs.
    pub mod option {
        use super::{Deserializer, Serializer, Visitor, de, fmt};

        /// Serializes an optional u64 as a string.
        ///
        /// # Errors
        ///
        /// Returns an error if the serializer fails.
        pub fn serialize<S>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match value {
                Some(v) => super::serialize(v, serializer),
                None => serializer.serialize_none(),
            }
        }

        /// Deserializes an optional u64 from a string or number.
        ///
        /// # Errors
        ///
        /// Returns an error if deserialization fails.
        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct OptionVisitor;

            impl<'de> Visitor<'de> for OptionVisitor {
                type Value = Option<u64>;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("optional snowflake ID")
                }

                fn visit_none<E>(self) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(None)
                }

                fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    super::deserialize(deserializer).map(Some)
                }
            }
            deserializer.deserialize_option(OptionVisitor)
        }
    }
}

/// Module to handle deserialization of a vector of Snowflake IDs.
pub mod vec_string_to_u64 {
    use super::{Deserialize, Deserializer, Serializer, Visitor, fmt};
    use serde::de::SeqAccess;

    /// Serializes a Vec<u64> as a list of strings.
    ///
    /// # Errors
    ///
    /// Returns an error if the serializer fails.
    #[allow(clippy::ptr_arg)]
    pub fn serialize<S>(value: &Vec<u64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(value.iter().map(ToString::to_string))
    }

    /// Deserializes a Vec<u64> from a list of strings or numbers.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VecVisitor;

        impl<'de> Visitor<'de> for VecVisitor {
            type Value = Vec<u64>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of snowflake IDs")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                #[derive(Deserialize)]
                #[serde(transparent)]
                struct Id(#[serde(with = "super::string_to_u64")] u64);

                let mut vec = Vec::new();

                while let Some(Id(val)) = seq.next_element()? {
                    vec.push(val);
                }
                Ok(vec)
            }
        }

        deserializer.deserialize_seq(VecVisitor)
    }
}
