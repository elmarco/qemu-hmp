// SPDX-License-Identifier: GPL-2.0-or-later

use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};

/// Syntax highlighter for HMP input lines.
///
/// Colours the command name blue, keyval keys (before `=`) grey, and
/// everything else in the default terminal colour.
pub(crate) struct HmpHighlighter;

impl Highlighter for HmpHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let cmd_style = Style::new().fg(Color::Blue);
        let key_style = Style::new().fg(Color::DarkGray);
        let default_style = Style::new();

        let mut styled = StyledText::new();

        // Split into command and the rest.
        let (cmd, rest) = match line.find(' ') {
            Some(idx) => (&line[..idx], &line[idx..]),
            None => (line, ""),
        };

        styled.push((cmd_style, cmd.to_string()));

        if rest.is_empty() {
            return styled;
        }

        // For "info <sub>", colour the subcommand blue too.
        if cmd == "info" || cmd == "help" {
            let trimmed = rest.trim_start();
            let leading = &rest[..rest.len() - trimmed.len()];
            styled.push((default_style, leading.to_string()));
            let (sub, after) = match trimmed.find(' ') {
                Some(idx) => (&trimmed[..idx], &trimmed[idx..]),
                None => (trimmed, ""),
            };
            styled.push((cmd_style, sub.to_string()));
            if !after.is_empty() {
                highlight_args(&mut styled, after, &key_style, &default_style);
            }
            return styled;
        }

        highlight_args(&mut styled, rest, &key_style, &default_style);
        styled
    }
}

/// Highlight argument text: keyval keys (before `=`) in `key_style`,
/// everything else in `default_style`.
fn highlight_args(styled: &mut StyledText, text: &str, key_style: &Style, default_style: &Style) {
    // Walk through the text character by character, splitting on commas
    // and equals signs to colour keys differently.
    let mut pos = 0;
    let bytes = text.as_bytes();
    while pos < bytes.len() {
        // Find the next '=' to detect a key.
        let remaining = &text[pos..];
        // Look for key=value pattern. A key ends at '=' and starts after
        // whitespace or comma.
        if let Some(eq) = remaining.find('=') {
            // Check there's no comma before the '=' (the key is one token).
            let before_eq = &remaining[..eq];
            if !before_eq.contains(',') {
                // Everything before the key start (whitespace/commas) is default.
                let key_start = before_eq.rfind([',', ' ']).map(|i| i + 1).unwrap_or(0);
                if key_start > 0 {
                    styled.push((*default_style, remaining[..key_start].to_string()));
                }
                // The key itself.
                styled.push((*key_style, remaining[key_start..eq].to_string()));
                // The '=' and value until next comma or end.
                let after_eq = &remaining[eq..];
                let val_end = after_eq.find(',').unwrap_or(after_eq.len());
                styled.push((*default_style, after_eq[..val_end].to_string()));
                pos += eq + val_end;
            } else {
                // Comma before '=' — output up to the comma as default,
                // then continue parsing.
                let comma = before_eq.find(',').unwrap();
                styled.push((*default_style, remaining[..=comma].to_string()));
                pos += comma + 1;
            }
        } else {
            // No '=' remaining — output the rest as default.
            styled.push((*default_style, remaining.to_string()));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract (style_fg_color, text) pairs from a StyledText for assertions.
    fn styled_segments(st: &StyledText) -> Vec<(Option<Color>, &str)> {
        st.buffer
            .iter()
            .map(|(style, text)| (style.foreground, text.as_str()))
            .collect()
    }

    #[test]
    fn highlight_command_only() {
        let h = HmpHighlighter;
        let st = h.highlight("quit", 4);
        let segs = styled_segments(&st);
        assert_eq!(segs, vec![(Some(Color::Blue), "quit")]);
    }

    #[test]
    fn highlight_command_with_args() {
        let h = HmpHighlighter;
        let st = h.highlight("qom-get /machine type", 21);
        let segs = styled_segments(&st);
        assert_eq!(segs[0], (Some(Color::Blue), "qom-get"));
        // Rest is default-styled arguments
        assert_eq!(segs[1].1, " /machine type");
    }

    #[test]
    fn highlight_info_subcommand() {
        let h = HmpHighlighter;
        let st = h.highlight("info version", 12);
        let segs = styled_segments(&st);
        assert_eq!(segs[0], (Some(Color::Blue), "info"));
        // space
        assert_eq!(segs[1].1, " ");
        // subcommand in blue
        assert_eq!(segs[2], (Some(Color::Blue), "version"));
    }

    #[test]
    fn highlight_keyval() {
        let h = HmpHighlighter;
        let st = h.highlight("object_add rng-random,id=foo", 28);
        let segs = styled_segments(&st);
        assert_eq!(segs[0], (Some(Color::Blue), "object_add"));
        // " rng-random," is default
        // "id" is grey (DarkGray)
        let has_grey_key = segs
            .iter()
            .any(|(c, t)| *c == Some(Color::DarkGray) && *t == "id");
        assert!(has_grey_key, "expected 'id' in DarkGray, got: {segs:?}");
        // "=foo" is default
        let has_eq_val = segs.iter().any(|(_, t)| *t == "=foo");
        assert!(has_eq_val, "expected '=foo' segment, got: {segs:?}");
    }

    #[test]
    fn highlight_multiple_keyvals() {
        let h = HmpHighlighter;
        let st = h.highlight("object_add t,a=1,b=2", 20);
        let segs = styled_segments(&st);
        let grey_keys: Vec<&str> = segs
            .iter()
            .filter(|(c, _)| *c == Some(Color::DarkGray))
            .map(|(_, t)| *t)
            .collect();
        assert_eq!(grey_keys, vec!["a", "b"]);
    }
}
