//! Interactive prompts used by the CLI wizard. Wrapped in a
//! `Prompter` trait so the orchestrator can be tested without driving
//! a real terminal — the production impl uses `dialoguer`, the test
//! impl reads scripted answers.

use std::collections::VecDeque;

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("prompt cancelled")]
    Cancelled,
    #[error("scripted prompter ran out of answers")]
    OutOfAnswers,
    #[error("prompt io: {0}")]
    Io(#[from] std::io::Error),
}

pub trait Prompter {
    /// Yes/no confirmation. `default` is the value that ENTER picks.
    fn confirm(&mut self, prompt: &str, default: bool) -> Result<bool, PromptError>;

    /// Pick one option from a list. Returns the chosen index.
    fn select(&mut self, prompt: &str, options: &[String]) -> Result<usize, PromptError>;
}

/// Production prompter backed by `dialoguer`. Uses the colorful theme
/// when stderr is a TTY, plain theme otherwise.
pub struct DialoguerPrompter;

impl Prompter for DialoguerPrompter {
    fn confirm(&mut self, prompt: &str, default: bool) -> Result<bool, PromptError> {
        dialoguer::Confirm::new()
            .with_prompt(prompt)
            .default(default)
            .interact()
            .map_err(map_dialoguer_err)
    }

    fn select(&mut self, prompt: &str, options: &[String]) -> Result<usize, PromptError> {
        dialoguer::Select::new()
            .with_prompt(prompt)
            .items(options)
            .default(0)
            .interact()
            .map_err(map_dialoguer_err)
    }
}

fn map_dialoguer_err(e: dialoguer::Error) -> PromptError {
    match e {
        dialoguer::Error::IO(io) => PromptError::Io(io),
    }
}

/// Auto-accepting prompter: answers `yes` to every confirm and picks
/// option 0 from every select. Used by `--yes` / scripted invocations.
pub struct AutoAccept;

impl Prompter for AutoAccept {
    fn confirm(&mut self, _prompt: &str, _default: bool) -> Result<bool, PromptError> {
        Ok(true)
    }

    fn select(&mut self, _prompt: &str, _options: &[String]) -> Result<usize, PromptError> {
        Ok(0)
    }
}

/// Scripted prompter that pops scripted answers in order. Used by
/// unit tests of the wizard orchestrator.
pub struct ScriptedPrompter {
    confirms: VecDeque<bool>,
    selects: VecDeque<usize>,
}

impl ScriptedPrompter {
    pub fn new(confirms: Vec<bool>, selects: Vec<usize>) -> Self {
        Self {
            confirms: confirms.into(),
            selects: selects.into(),
        }
    }
}

impl Prompter for ScriptedPrompter {
    fn confirm(&mut self, _prompt: &str, _default: bool) -> Result<bool, PromptError> {
        self.confirms.pop_front().ok_or(PromptError::OutOfAnswers)
    }

    fn select(&mut self, _prompt: &str, _options: &[String]) -> Result<usize, PromptError> {
        self.selects.pop_front().ok_or(PromptError::OutOfAnswers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_prompter_returns_answers_in_order() {
        let mut p = ScriptedPrompter::new(vec![true, false], vec![2, 0]);
        assert!(p.confirm("?", false).unwrap());
        assert!(!p.confirm("?", true).unwrap());
        assert_eq!(p.select("?", &[]).unwrap(), 2);
        assert_eq!(p.select("?", &[]).unwrap(), 0);
    }

    #[test]
    fn scripted_prompter_runs_out_cleanly() {
        let mut p = ScriptedPrompter::new(vec![], vec![]);
        assert!(matches!(
            p.confirm("?", true).unwrap_err(),
            PromptError::OutOfAnswers
        ));
    }
}
