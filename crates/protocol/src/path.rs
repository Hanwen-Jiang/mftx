use std::path::{Component, Path};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("path is empty")]
    Empty,
    #[error("absolute paths are not allowed")]
    Absolute,
    #[error("parent-directory traversal is not allowed")]
    Traversal,
    #[error("path contains an unsupported component")]
    UnsupportedComponent,
}

pub fn clean_relative_path(input: impl AsRef<str>) -> Result<String, PathError> {
    let raw = input.as_ref().replace('\\', "/");
    if raw.trim().is_empty() {
        return Err(PathError::Empty);
    }

    let path = Path::new(&raw);
    if path.is_absolute() || raw.starts_with('/') {
        return Err(PathError::Absolute);
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let text = value
                    .to_str()
                    .ok_or(PathError::UnsupportedComponent)?
                    .trim();
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            }
            Component::CurDir => {}
            Component::ParentDir => return Err(PathError::Traversal),
            Component::RootDir | Component::Prefix(_) => return Err(PathError::Absolute),
        }
    }

    if parts.is_empty() {
        return Err(PathError::Empty);
    }

    Ok(parts.join("/"))
}
