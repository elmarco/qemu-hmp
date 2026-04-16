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
        if line.trim_start().starts_with('{') {
            return highlight_json(line);
        }

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

/// JSON highlighting state machine.
///
/// Tracks whether we are inside a string that is a key (before `:`) or a
/// value, and assigns colours accordingly.
fn highlight_json(line: &str) -> StyledText {
    let bracket_style = Style::new().fg(Color::White);
    let key_style = Style::new().fg(Color::Cyan);
    let string_style = Style::new().fg(Color::Green);
    let number_style = Style::new().fg(Color::Yellow);
    let bool_style = Style::new().fg(Color::Magenta);
    let default_style = Style::new();

    let mut styled = StyledText::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        match ch {
            '{' | '}' | '[' | ']' => {
                styled.push((bracket_style, ch.to_string()));
                i += 1;
            }
            ':' | ',' => {
                styled.push((default_style, ch.to_string()));
                i += 1;
            }
            '"' => {
                // Collect the full string including quotes.
                let start = i;
                i += 1; // skip opening quote
                while i < len {
                    if chars[i] == '\\' {
                        i += 2; // skip escape sequence
                    } else if chars[i] == '"' {
                        i += 1; // skip closing quote
                        break;
                    } else {
                        i += 1;
                    }
                }
                let s: String = chars[start..i].iter().collect();

                // Determine if this string is a key: scan forward past
                // whitespace for a ':'.
                let mut j = i;
                while j < len && chars[j].is_whitespace() {
                    j += 1;
                }
                let is_key = j < len && chars[j] == ':';
                styled.push((if is_key { key_style } else { string_style }, s));
            }
            _ if ch.is_ascii_digit() || ch == '-' => {
                let start = i;
                i += 1;
                while i < len
                    && (chars[i].is_ascii_digit()
                        || chars[i] == '.'
                        || chars[i] == 'e'
                        || chars[i] == 'E'
                        || chars[i] == '+'
                        || chars[i] == '-')
                {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                styled.push((number_style, s));
            }
            _ if ch.is_ascii_alphabetic() => {
                let start = i;
                while i < len && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                let style = match s.as_str() {
                    "true" | "false" | "null" => bool_style,
                    _ => default_style,
                };
                styled.push((style, s));
            }
            _ => {
                // Whitespace and other characters.
                styled.push((default_style, ch.to_string()));
                i += 1;
            }
        }
    }

    styled
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

    #[test]
    fn highlight_json_keys_and_values() {
        let h = HmpHighlighter;
        let st = h.highlight(r#"{"execute": "query-version"}"#, 0);
        let segs = styled_segments(&st);
        // "execute" is a key → Cyan
        let has_key = segs
            .iter()
            .any(|(c, t)| *c == Some(Color::Cyan) && *t == r#""execute""#);
        assert!(has_key, "expected key in Cyan, got: {segs:?}");
        // "query-version" is a string value → Green
        let has_val = segs
            .iter()
            .any(|(c, t)| *c == Some(Color::Green) && *t == r#""query-version""#);
        assert!(has_val, "expected string value in Green, got: {segs:?}");
    }

    #[test]
    fn highlight_json_brackets() {
        let h = HmpHighlighter;
        let st = h.highlight("{}", 0);
        let segs = styled_segments(&st);
        assert_eq!(segs[0], (Some(Color::White), "{"));
        assert_eq!(segs[1], (Some(Color::White), "}"));
    }

    #[test]
    fn highlight_json_number() {
        let h = HmpHighlighter;
        let st = h.highlight(r#"{"count": 42}"#, 0);
        let segs = styled_segments(&st);
        let has_num = segs
            .iter()
            .any(|(c, t)| *c == Some(Color::Yellow) && *t == "42");
        assert!(has_num, "expected number in Yellow, got: {segs:?}");
    }

    #[test]
    fn highlight_json_bool_null() {
        let h = HmpHighlighter;
        let st = h.highlight(r#"{"a": true, "b": null}"#, 0);
        let segs = styled_segments(&st);
        let has_true = segs
            .iter()
            .any(|(c, t)| *c == Some(Color::Magenta) && *t == "true");
        assert!(has_true, "expected true in Magenta, got: {segs:?}");
        let has_null = segs
            .iter()
            .any(|(c, t)| *c == Some(Color::Magenta) && *t == "null");
        assert!(has_null, "expected null in Magenta, got: {segs:?}");
    }

    #[test]
    fn highlight_json_multiline() {
        let h = HmpHighlighter;
        let st = h.highlight("{\n  \"execute\": \"query-version\"\n}", 0);
        let segs = styled_segments(&st);
        let has_key = segs
            .iter()
            .any(|(c, t)| *c == Some(Color::Cyan) && *t == r#""execute""#);
        assert!(
            has_key,
            "expected key in Cyan in multiline JSON, got: {segs:?}"
        );
    }

    #[test]
    fn highlight_json_nested() {
        let h = HmpHighlighter;
        let st = h.highlight(r#"{"execute": "x", "arguments": {"driver": "e1000"}}"#, 0);
        let segs = styled_segments(&st);
        let cyan_keys: Vec<&str> = segs
            .iter()
            .filter(|(c, _)| *c == Some(Color::Cyan))
            .map(|(_, t)| *t)
            .collect();
        assert_eq!(
            cyan_keys,
            vec![r#""execute""#, r#""arguments""#, r#""driver""#]
        );
    }
}
