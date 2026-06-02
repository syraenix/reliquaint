//! Spawn `$EDITOR` (with `$VISUAL` then `nano`/`vim`/`vi` fallbacks)
//! on a temp file seeded with the draft TOML, and return the edited
//! content. The user's edits round-trip through the v0.1 parser before
//! the wizard accepts them.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("no editor found: set $EDITOR, $VISUAL, or install nano/vim/vi")]
    NoneAvailable,

    #[error("editor {0} exited with non-zero status")]
    NonZeroExit(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve the editor command to spawn. Order: `$EDITOR`, `$VISUAL`,
/// then the first of `nano`, `vim`, `vi` present on `$PATH`. Returns
/// `None` if no editor can be found.
pub fn resolve_editor() -> Option<String> {
    for var in &["EDITOR", "VISUAL"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    for candidate in &["nano", "vim", "vi"] {
        if which_on_path(candidate) {
            return Some(candidate.to_string());
        }
    }
    None
}

fn which_on_path(name: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path) {
        if dir.join(name).is_file() {
            return true;
        }
    }
    false
}

/// Open the resolved editor on `initial` (rendered as a `.toml` temp
/// file so syntax-aware editors can help) and return the user's edited
/// content. Caller validates the result.
pub fn edit_string(initial: &str) -> Result<String, EditorError> {
    let editor = resolve_editor().ok_or(EditorError::NoneAvailable)?;
    let mut tmp = tempfile::Builder::new()
        .prefix("reliquaint-draft-")
        .suffix(".toml")
        .tempfile()?;
    tmp.write_all(initial.as_bytes())?;
    tmp.flush()?;
    let path: PathBuf = tmp.path().to_path_buf();

    let mut parts = shell_split(&editor);
    let program = parts.first().cloned().ok_or(EditorError::NoneAvailable)?;
    let args: Vec<String> = parts.drain(1..).collect();
    let status = Command::new(&program).args(args).arg(&path).status()?;
    if !status.success() {
        return Err(EditorError::NonZeroExit(editor));
    }
    Ok(std::fs::read_to_string(&path)?)
}

/// Lightweight shell-style split: respects ASCII spaces only. Good
/// enough for `$EDITOR="emacs -nw"` and similar; users with anything
/// fancier can write a wrapper script.
fn shell_split(s: &str) -> Vec<String> {
    s.split_ascii_whitespace().map(|t| t.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn resolve_editor_prefers_env() {
        let saved = std::env::var("EDITOR").ok();
        std::env::set_var("EDITOR", "my-favourite-editor");
        assert_eq!(resolve_editor(), Some("my-favourite-editor".to_string()));
        match saved {
            Some(v) => std::env::set_var("EDITOR", v),
            None => std::env::remove_var("EDITOR"),
        }
    }

    #[test]
    #[serial]
    fn resolve_editor_empty_env_falls_through_to_fallbacks() {
        let saved_editor = std::env::var("EDITOR").ok();
        let saved_visual = std::env::var("VISUAL").ok();
        std::env::set_var("EDITOR", "");
        std::env::set_var("VISUAL", "");
        // We don't assert a specific fallback — depends on the test env.
        let _ = resolve_editor();
        match saved_editor {
            Some(v) => std::env::set_var("EDITOR", v),
            None => std::env::remove_var("EDITOR"),
        }
        match saved_visual {
            Some(v) => std::env::set_var("VISUAL", v),
            None => std::env::remove_var("VISUAL"),
        }
    }

    #[test]
    fn shell_split_handles_multiword_command() {
        assert_eq!(
            shell_split("emacs -nw"),
            vec!["emacs".to_string(), "-nw".to_string()],
        );
    }
}
