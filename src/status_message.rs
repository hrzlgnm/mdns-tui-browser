// Copyright 2026 hrzlgnm
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Messages shown over the UI until the next key press dismisses them.

/// A message shown over the UI until the next key press dismisses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessage {
    /// The text shown to the user.
    pub text: String,
    /// Whether dismissing it with `m` opens the daemon details popup.
    pub offers_details: bool,
}

impl StatusMessage {
    /// A message offering details on `m`, with a hint saying so appended
    /// to `text`.
    pub fn with_details(text: &str) -> Self {
        Self {
            text: format!("{text}; press m for details"),
            offers_details: true,
        }
    }
}

impl From<String> for StatusMessage {
    fn from(text: String) -> Self {
        Self {
            text,
            offers_details: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_details_appends_hint_and_offers_details() {
        assert_eq!(
            StatusMessage::with_details("mDNS: trouble"),
            StatusMessage {
                text: "mDNS: trouble; press m for details".to_string(),
                offers_details: true,
            }
        );
    }

    #[test]
    fn test_from_string_keeps_text_without_details() {
        assert_eq!(
            StatusMessage::from("plain".to_string()),
            StatusMessage {
                text: "plain".to_string(),
                offers_details: false,
            }
        );
    }
}
