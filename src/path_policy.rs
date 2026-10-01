use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::ConnectorError;

/// Limits plans to paths that remain relative in Unix and Windows consumers.
pub(crate) fn portable_relative(field: &'static str, value: &str) -> Result<(), ConnectorError> {
    if value.is_empty() || value.len() > 512 || value.contains('\\') {
        return Err(ConnectorError::new(
            field,
            "must be a bounded portable relative path",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ConnectorError::new(
            field,
            "must contain only normal relative path components",
        ));
    }
    Ok(())
}

pub(crate) fn existing_file(
    root: &Path,
    field: &'static str,
    value: &str,
) -> Result<PathBuf, ConnectorError> {
    let path = root.join(value);
    reject_symlink_components(root, &path, field)?;
    let canonical = path
        .canonicalize()
        .map_err(|_| ConnectorError::new(field, "file does not exist"))?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(ConnectorError::new(
            field,
            "must resolve to a regular file inside the root",
        ));
    }
    Ok(canonical)
}

pub(crate) fn output_file(
    root: &Path,
    field: &'static str,
    value: &str,
) -> Result<PathBuf, ConnectorError> {
    let path = root.join(value);
    let parent = path
        .parent()
        .ok_or_else(|| ConnectorError::new(field, "must have a parent directory"))?;
    reject_symlink_components(root, parent, field)?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| ConnectorError::new(field, "parent directory does not exist"))?;
    if !canonical_parent.starts_with(root) {
        return Err(ConnectorError::new(
            field,
            "parent must remain inside the root",
        ));
    }
    if let Ok(metadata) = fs::symlink_metadata(&path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(ConnectorError::new(
            field,
            "existing output must be a regular non-symlink file",
        ));
    }
    let name = path
        .file_name()
        .ok_or_else(|| ConnectorError::new(field, "must name an output file"))?;
    Ok(canonical_parent.join(name))
}

fn reject_symlink_components(
    root: &Path,
    path: &Path,
    field: &'static str,
) -> Result<(), ConnectorError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| ConnectorError::new(field, "must remain inside the root"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        if let Ok(metadata) = fs::symlink_metadata(&current)
            && metadata.file_type().is_symlink()
        {
            return Err(ConnectorError::new(
                field,
                "symlink components are forbidden",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::portable_relative;

    #[test]
    fn rejects_escape_and_platform_specific_paths() {
        assert!(portable_relative("path", "../outside").is_err());
        assert!(portable_relative("path", r"state\exports").is_err());
        assert!(portable_relative("path", "/absolute").is_err());
    }
}
