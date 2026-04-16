// SPDX-License-Identifier: GPL-2.0-or-later

use std::borrow::Cow;

use reedline::{Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus};

/// Custom prompt that displays `(qemu) ` with no indicator or right segment.
pub(crate) struct HmpPrompt;

impl Prompt for HmpPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("(qemu) ")
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("::: ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let prefix = match history_search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!(
            "({}reverse-search: {}) ",
            prefix, history_search.term
        ))
    }
}
