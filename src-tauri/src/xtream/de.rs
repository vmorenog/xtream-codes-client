//! Lenient deserializers.
//!
//! Xtream panels are wildly inconsistent about types: the same field arrives as
//! `123`, `"123"`, `""` or `null` depending on the panel software and version.
//! Deserializing strictly means the whole 50MB sync fails on one bad row, so
//! everything coming off the wire goes through these.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// `123` | `"123"` | `""` | `null` -> `Option<i64>`
pub fn flex_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    })
}

/// Same, but a missing value is an error the caller turns into a skipped row.
pub fn flex_i64_req<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    flex_i64(d)?.ok_or_else(|| serde::de::Error::custom("expected an integer id"))
}

/// `7.2` | `"7.2"` | `""` | `null` -> `Option<f64>`
pub fn flex_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    })
}

/// Anything scalar -> `Option<String>`, with empty strings normalised to `None`.
pub fn flex_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let out = match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        _ => None,
    };
    Ok(out)
}

/// `0` | `"0"` | `1` | `"1"` | `true` -> `bool`
pub fn flex_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Bool(b)) => b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(s)) => matches!(s.trim(), "1" | "true" | "yes"),
        _ => false,
    })
}

/// A list that some panels send as `[]` and others as `false` or `""`.
pub fn flex_vec<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let v = Value::deserialize(d)?;
    match v {
        Value::Array(_) => serde_json::from_value(v).map_err(serde::de::Error::custom),
        _ => Ok(Vec::new()),
    }
}

/// EPG titles and descriptions come back base64-encoded. Some panels forget.
pub fn maybe_base64(raw: &str) -> String {
    use base64::Engine;
    match base64::engine::general_purpose::STANDARD.decode(raw.trim()) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => raw.to_string(),
        },
        Err(_) => raw.to_string(),
    }
}
