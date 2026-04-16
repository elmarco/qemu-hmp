// SPDX-License-Identifier: GPL-2.0-or-later

//! Expression evaluator matching QEMU's `get_expr()` in monitor/hmp.c.
//!
//! Supports:
//! - Arithmetic: `+`, `-`, `*`, `/`, `%`
//! - Bitwise: `&`, `|`, `^`
//! - Unary: `+`, `-`, `~`
//! - Parentheses: `(expr)`
//! - Character literals: `'c'`
//! - Register references: `$name` (resolved via QMP)
//! - Numeric literals: decimal, `0x` hex, `0` octal (C `strtoull` semantics)
//!
//! Operator precedence (lowest to highest):
//! 1. `+`, `-`  (expr_sum)
//! 2. `&`, `|`, `^`  (expr_logic)
//! 3. `*`, `/`, `%`  (expr_prod)
//! 4. unary `+`, `-`, `~`, atoms  (expr_unary)

use serde::{Deserialize, Serialize};

use crate::commands::CmdError;
use crate::qmp::QmpConnection;

// x-query-cpu-register is not yet in the qapi-rs crate, define it locally.

#[derive(Debug, Deserialize)]
pub struct CpuRegisterInfo {
    pub value: i64,
}

#[derive(Debug, Serialize)]
#[allow(non_camel_case_types)]
pub struct x_query_cpu_register {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<i64>,
}

impl qapi_qmp::QmpCommand for x_query_cpu_register {}
impl qapi::Command for x_query_cpu_register {
    const NAME: &'static str = "x-query-cpu-register";
    const ALLOW_OOB: bool = false;
    type Ok = CpuRegisterInfo;
}

/// Parser state for the expression evaluator.
struct ExprParser<'a> {
    input: &'a [u8],
    pos: usize,
    conn: Option<&'a QmpConnection>,
}

impl<'a> ExprParser<'a> {
    fn new(input: &'a str, conn: Option<&'a QmpConnection>) -> Self {
        let input = input.as_bytes();
        let mut p = ExprParser {
            input,
            pos: 0,
            conn,
        };
        p.skip_whitespace();
        p
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn next_char(&mut self) {
        if self.pos < self.input.len() {
            self.pos += 1;
            self.skip_whitespace();
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && (self.input[self.pos] as char).is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    /// Top-level: expr_sum
    async fn expr_sum(&mut self) -> Result<i64, CmdError> {
        let mut val = self.expr_logic().await?;
        loop {
            match self.peek() {
                Some(b'+') => {
                    self.next_char();
                    val = val.wrapping_add(self.expr_logic().await?);
                }
                Some(b'-') => {
                    self.next_char();
                    val = val.wrapping_sub(self.expr_logic().await?);
                }
                _ => break,
            }
        }
        Ok(val)
    }

    async fn expr_logic(&mut self) -> Result<i64, CmdError> {
        let mut val = self.expr_prod().await?;
        loop {
            match self.peek() {
                Some(b'&') => {
                    self.next_char();
                    val &= self.expr_prod().await?;
                }
                Some(b'|') => {
                    self.next_char();
                    val |= self.expr_prod().await?;
                }
                Some(b'^') => {
                    self.next_char();
                    val ^= self.expr_prod().await?;
                }
                _ => break,
            }
        }
        Ok(val)
    }

    async fn expr_prod(&mut self) -> Result<i64, CmdError> {
        let mut val = self.expr_unary().await?;
        loop {
            match self.peek() {
                Some(b'*') => {
                    self.next_char();
                    val = val.wrapping_mul(self.expr_unary().await?);
                }
                Some(b'/') => {
                    self.next_char();
                    let val2 = self.expr_unary().await?;
                    if val2 == 0 {
                        return Err(CmdError::Command("division by zero".to_string()));
                    }
                    val /= val2;
                }
                Some(b'%') => {
                    self.next_char();
                    let val2 = self.expr_unary().await?;
                    if val2 == 0 {
                        return Err(CmdError::Command("division by zero".to_string()));
                    }
                    val %= val2;
                }
                _ => break,
            }
        }
        Ok(val)
    }

    fn expr_unary(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<i64, CmdError>> + Send + '_>>
    {
        Box::pin(async move {
            match self.peek() {
                Some(b'+') => {
                    self.next_char();
                    self.expr_unary().await
                }
                Some(b'-') => {
                    self.next_char();
                    Ok(self.expr_unary().await?.wrapping_neg())
                }
                Some(b'~') => {
                    self.next_char();
                    Ok(!self.expr_unary().await?)
                }
                Some(b'(') => {
                    self.next_char();
                    let val = self.expr_sum().await?;
                    if self.peek() != Some(b')') {
                        return Err(CmdError::Command("')' expected".to_string()));
                    }
                    self.next_char();
                    Ok(val)
                }
                Some(b'\'') => {
                    // Character literal — don't skip whitespace after opening quote
                    self.pos += 1;
                    let c = self.peek().ok_or_else(|| {
                        CmdError::Command("character constant expected".to_string())
                    })?;
                    let val = c as i64;
                    self.pos += 1;
                    if self.peek() != Some(b'\'') {
                        return Err(CmdError::Command(
                            "missing terminating ' character".to_string(),
                        ));
                    }
                    self.next_char();
                    Ok(val)
                }
                Some(b'$') => {
                    // Register reference
                    self.pos += 1; // skip '$'
                    let start = self.pos;
                    while self.pos < self.input.len() {
                        let c = self.input[self.pos];
                        if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' {
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    let name = std::str::from_utf8(&self.input[start..self.pos])
                        .map_err(|_| CmdError::Command("invalid register name".to_string()))?
                        .to_string();
                    self.skip_whitespace();

                    if name.is_empty() {
                        return Err(CmdError::Command("empty register name".to_string()));
                    }

                    let conn = self.conn.ok_or_else(|| {
                        CmdError::Command("register access requires a QMP connection".to_string())
                    })?;

                    let info = conn
                        .execute(x_query_cpu_register {
                            name: name.clone(),
                            cpu: None,
                        })
                        .await
                        .map_err(|_| CmdError::Command("unknown register".to_string()))?;
                    Ok(info.value)
                }
                None => Err(CmdError::Command(
                    "unexpected end of expression".to_string(),
                )),
                _ => {
                    // Numeric literal
                    let start = self.pos;
                    let val = self.parse_number()?;
                    if self.pos == start {
                        let c = self.input[self.pos] as char;
                        return Err(CmdError::Command(format!(
                            "invalid char '{c}' in expression"
                        )));
                    }
                    self.skip_whitespace();
                    Ok(val)
                }
            }
        }) // end Box::pin(async move {
    }

    /// Parse a number with C `strtoull` semantics:
    /// `0x...` = hex, `0...` = octal, otherwise decimal.
    fn parse_number(&mut self) -> Result<i64, CmdError> {
        if self.pos >= self.input.len() {
            return Err(CmdError::Command(
                "unexpected end of expression".to_string(),
            ));
        }

        // Check for 0x/0X prefix (hex) or leading 0 (octal)
        if self.input[self.pos] == b'0' && self.pos + 1 < self.input.len() {
            let next = self.input[self.pos + 1];
            if next == b'x' || next == b'X' {
                // Hex
                self.pos += 2;
                let hex_start = self.pos;
                while self.pos < self.input.len() && self.input[self.pos].is_ascii_hexdigit() {
                    self.pos += 1;
                }
                if self.pos == hex_start {
                    return Err(CmdError::Command("invalid hex number".to_string()));
                }
                let s = std::str::from_utf8(&self.input[hex_start..self.pos]).unwrap();
                return u64::from_str_radix(s, 16)
                    .map(|v| v as i64)
                    .map_err(|e| CmdError::Command(format!("number too large: {e}")));
            }

            // Octal (leading 0 followed by octal digit)
            if (b'0'..=b'7').contains(&next) {
                let oct_start = self.pos;
                self.pos += 1; // skip leading 0
                while self.pos < self.input.len()
                    && self.input[self.pos] >= b'0'
                    && self.input[self.pos] <= b'7'
                {
                    self.pos += 1;
                }
                let s = std::str::from_utf8(&self.input[oct_start..self.pos]).unwrap();
                return u64::from_str_radix(s, 8)
                    .map(|v| v as i64)
                    .map_err(|e| CmdError::Command(format!("number too large: {e}")));
            }
        }

        // Decimal (or just "0")
        let dec_start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == dec_start {
            return Err(CmdError::Command(
                "unexpected end of expression".to_string(),
            ));
        }
        let s = std::str::from_utf8(&self.input[dec_start..self.pos]).unwrap();
        s.parse::<u64>()
            .map(|v| v as i64)
            .map_err(|e| CmdError::Command(format!("number too large: {e}")))
    }
}

/// Evaluate an expression string, resolving `$register` references via QMP.
///
/// This matches the semantics of QEMU's `get_expr()` in `monitor/hmp.c`.
pub async fn eval_expr(input: &str, conn: &QmpConnection) -> Result<i64, CmdError> {
    let mut parser = ExprParser::new(input, Some(conn));
    let val = parser.expr_sum().await?;
    if parser.peek().is_some() {
        let remaining = std::str::from_utf8(&parser.input[parser.pos..]).unwrap_or("?");
        return Err(CmdError::Command(format!(
            "unexpected trailing input: '{}'",
            remaining.trim()
        )));
    }
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Evaluate an expression without needing a QMP connection.
    /// Only works for expressions that don't reference $registers.
    fn eval_sync(input: &str) -> Result<i64, CmdError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut parser = ExprParser::new(input, None);
            let val = parser.expr_sum().await?;
            if parser.peek().is_some() {
                return Err(CmdError::Command("trailing input".to_string()));
            }
            Ok(val)
        })
    }

    #[test]
    fn test_decimal() {
        assert_eq!(eval_sync("42").unwrap(), 42);
        assert_eq!(eval_sync("0").unwrap(), 0);
        assert_eq!(eval_sync("  100  ").unwrap(), 100);
    }

    #[test]
    fn test_hex() {
        assert_eq!(eval_sync("0xff").unwrap(), 255);
        assert_eq!(eval_sync("0xFF").unwrap(), 255);
        assert_eq!(eval_sync("0x0").unwrap(), 0);
        assert_eq!(eval_sync("0x100").unwrap(), 256);
    }

    #[test]
    fn test_octal() {
        assert_eq!(eval_sync("010").unwrap(), 8);
        assert_eq!(eval_sync("0377").unwrap(), 255);
    }

    #[test]
    fn test_arithmetic() {
        assert_eq!(eval_sync("1 + 2").unwrap(), 3);
        assert_eq!(eval_sync("10 - 3").unwrap(), 7);
        assert_eq!(eval_sync("6 * 7").unwrap(), 42);
        assert_eq!(eval_sync("100 / 10").unwrap(), 10);
        assert_eq!(eval_sync("17 % 5").unwrap(), 2);
    }

    #[test]
    fn test_precedence() {
        assert_eq!(eval_sync("2 + 3 * 4").unwrap(), 14);
        assert_eq!(eval_sync("(2 + 3) * 4").unwrap(), 20);
    }

    #[test]
    fn test_bitwise() {
        assert_eq!(eval_sync("0xff & 0x0f").unwrap(), 0x0f);
        assert_eq!(eval_sync("0xf0 | 0x0f").unwrap(), 0xff);
        assert_eq!(eval_sync("0xff ^ 0x0f").unwrap(), 0xf0);
    }

    #[test]
    fn test_unary() {
        assert_eq!(eval_sync("-1").unwrap(), -1);
        assert_eq!(eval_sync("+42").unwrap(), 42);
        assert_eq!(eval_sync("~0").unwrap(), -1);
        assert_eq!(eval_sync("-(-5)").unwrap(), 5);
    }

    #[test]
    fn test_char_literal() {
        assert_eq!(eval_sync("'A'").unwrap(), 65);
        assert_eq!(eval_sync("'0'").unwrap(), 48);
    }

    #[test]
    fn test_complex_expression() {
        assert_eq!(eval_sync("(0x10 + 3) * 2").unwrap(), 38);
    }

    #[test]
    fn test_division_by_zero() {
        assert!(eval_sync("1 / 0").is_err());
        assert!(eval_sync("1 % 0").is_err());
    }

    #[test]
    fn test_register_without_connection() {
        let err = eval_sync("$eax").unwrap_err();
        match err {
            CmdError::Command(msg) => {
                assert!(msg.contains("QMP connection"), "{msg}");
            }
            _ => panic!("expected Command error"),
        }
    }
}
