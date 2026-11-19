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
    /// A message whose text tells the user to press `m` for details.
    pub fn with_details(text: String) -> Self {
        Self {
            text,
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
