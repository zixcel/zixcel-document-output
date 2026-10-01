use std::fmt;

/// Structured validation failure for CLI and library callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorError {
    pub field: &'static str,
    pub message: String,
}

impl ConnectorError {
    pub(crate) fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ConnectorError {}

pub(crate) fn identifier(field: &'static str, value: &str) -> Result<(), ConnectorError> {
    if value.is_empty()
        || value.len() > 96
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(ConnectorError::new(
            field,
            "must be a lowercase ASCII identifier",
        ));
    }
    Ok(())
}

pub(crate) fn reject_secrets(source: &str) -> Result<(), ConnectorError> {
    if source.len() > 1_048_576 {
        return Err(ConnectorError::new("config", "configuration exceeds 1 MiB"));
    }
    let value: toml::Value = toml::from_str(source)
        .map_err(|_| ConnectorError::new("config", "configuration is not valid TOML"))?;
    reject_value(&value, 0)
}

fn reject_value(value: &toml::Value, depth: usize) -> Result<(), ConnectorError> {
    if depth > 32 {
        return Err(ConnectorError::new(
            "config",
            "configuration nesting exceeds 32 levels",
        ));
    }
    match value {
        toml::Value::Table(table) => {
            for (key, nested) in table {
                let key: String = key
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .flat_map(char::to_lowercase)
                    .collect();
                if matches!(
                    key.as_str(),
                    "password"
                        | "token"
                        | "accesstoken"
                        | "refreshtoken"
                        | "clientsecret"
                        | "privatekey"
                        | "apikey"
                        | "credential"
                        | "credentials"
                        | "secret"
                ) {
                    return Err(ConnectorError::new(
                        "config",
                        "embedded credential fields are forbidden; use secret_ref",
                    ));
                }
                reject_value(nested, depth + 1)?;
            }
        }
        toml::Value::Array(items) => {
            for item in items {
                reject_value(item, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
