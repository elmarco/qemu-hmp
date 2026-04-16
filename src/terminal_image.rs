// SPDX-License-Identifier: GPL-2.0-or-later

//! Inline image display using the Kitty graphics protocol.
//!
//! Supported by Ghostty, Kitty, WezTerm, foot, and other modern
//! terminal emulators.  Falls back silently when stdout is not a
//! terminal or the image cannot be read.

use std::io::{IsTerminal, Write};

use base64::Engine;

/// Maximum base64 payload per escape sequence chunk.
const CHUNK_SIZE: usize = 4096;

/// Display an image file inline in the terminal.
///
/// Does nothing when stdout is not a terminal, the file cannot be
/// read, or the format is not recognized (PNG or PPM P6).
pub fn display_image_inline(path: &str) {
    if !std::io::stdout().is_terminal() {
        return;
    }

    let Ok(data) = std::fs::read(path) else {
        return;
    };

    if data.starts_with(b"\x89PNG") {
        display_png(&data);
    } else if data.starts_with(b"P6") {
        display_ppm(&data);
    }
}

/// Transmit a PNG image using the Kitty graphics protocol (f=100).
fn display_png(data: &[u8]) {
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    write_kitty_chunks(&b64, "a=T,f=100");
}

/// Parse a PPM P6 file and transmit as raw RGB (f=24).
fn display_ppm(data: &[u8]) {
    let Some((width, height, pixel_offset)) = parse_ppm_header(data) else {
        return;
    };
    let pixels = &data[pixel_offset..];
    let b64 = base64::engine::general_purpose::STANDARD.encode(pixels);
    write_kitty_chunks(&b64, &format!("a=T,f=24,s={width},v={height}"));
}

/// Write base64-encoded image data using the Kitty graphics protocol,
/// chunking as required.
fn write_kitty_chunks(b64: &str, first_control: &str) {
    let mut stdout = std::io::stdout().lock();
    let bytes = b64.as_bytes();

    if bytes.len() <= CHUNK_SIZE {
        write!(stdout, "\x1b_G{first_control};").ok();
        stdout.write_all(bytes).ok();
        write!(stdout, "\x1b\\").ok();
    } else {
        let total = bytes.chunks(CHUNK_SIZE).count();
        for (i, chunk) in bytes.chunks(CHUNK_SIZE).enumerate() {
            let last = i == total - 1;
            if i == 0 {
                write!(stdout, "\x1b_G{first_control},m=1;").ok();
            } else {
                write!(stdout, "\x1b_Gm={};", if last { 0 } else { 1 }).ok();
            }
            stdout.write_all(chunk).ok();
            write!(stdout, "\x1b\\").ok();
        }
    }

    writeln!(stdout).ok();
    stdout.flush().ok();
}

/// Parse a PPM P6 header, returning (width, height, pixel_data_offset).
fn parse_ppm_header(data: &[u8]) -> Option<(u32, u32, usize)> {
    if !data.starts_with(b"P6") {
        return None;
    }
    let mut pos = 2;

    loop {
        // Skip whitespace
        while pos < data.len() && data[pos].is_ascii_whitespace() {
            pos += 1;
        }
        // Skip comment lines
        if pos < data.len() && data[pos] == b'#' {
            while pos < data.len() && data[pos] != b'\n' {
                pos += 1;
            }
        } else {
            break;
        }
    }

    // Width
    let start = pos;
    while pos < data.len() && data[pos].is_ascii_digit() {
        pos += 1;
    }
    let width: u32 = std::str::from_utf8(&data[start..pos]).ok()?.parse().ok()?;

    // Skip whitespace
    while pos < data.len() && data[pos].is_ascii_whitespace() {
        pos += 1;
    }

    // Height
    let start = pos;
    while pos < data.len() && data[pos].is_ascii_digit() {
        pos += 1;
    }
    let height: u32 = std::str::from_utf8(&data[start..pos]).ok()?.parse().ok()?;

    // Skip whitespace
    while pos < data.len() && data[pos].is_ascii_whitespace() {
        pos += 1;
    }

    // Maxval (consumed but not used)
    while pos < data.len() && data[pos].is_ascii_digit() {
        pos += 1;
    }

    // Exactly one whitespace byte separates maxval from pixel data
    if pos < data.len() && data[pos].is_ascii_whitespace() {
        pos += 1;
    }

    Some((width, height, pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ppm_basic() {
        let mut data = b"P6\n4 3\n255\n".to_vec();
        data.extend_from_slice(&[0u8; 4 * 3 * 3]); // 4x3 RGB
        let (w, h, off) = parse_ppm_header(&data).unwrap();
        assert_eq!(w, 4);
        assert_eq!(h, 3);
        assert_eq!(off, b"P6\n4 3\n255\n".len());
    }

    #[test]
    fn parse_ppm_with_comment() {
        let header = b"P6\n# a comment\n2 2\n255\n";
        let mut data = header.to_vec();
        data.extend_from_slice(&[0u8; 2 * 2 * 3]);
        let (w, h, off) = parse_ppm_header(&data).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(off, header.len());
    }

    #[test]
    fn parse_ppm_invalid() {
        assert!(parse_ppm_header(b"P5\n1 1\n255\n").is_none());
        assert!(parse_ppm_header(b"").is_none());
    }
}
