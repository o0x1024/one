use crate::token::TokenKind;

use super::Lexer;

impl Lexer<'_> {
    pub(super) fn scan_number(&mut self, start: usize) -> TokenKind {
        let first = self.source[start];
        if first == b'0' {
            match self.peek_byte_at(1) {
                Some(b'x' | b'X') => return self.scan_radix_number(start, 16, true),
                Some(b'o' | b'O') => return self.scan_radix_number(start, 8, true),
                Some(b'b' | b'B') => return self.scan_radix_number(start, 2, true),
                _ => {}
            }
        }
        self.scan_decimal_number(start)
    }

    fn scan_radix_number(&mut self, start: usize, radix: u32, has_prefix: bool) -> TokenKind {
        if has_prefix {
            self.advance(); // 0
            self.advance(); // x/o/b
        }
        let digits = self.scan_digits(radix);
        if self.peek_byte() == Some(b'n') {
            self.advance();
            return TokenKind::BigInt(digits);
        }
        let value = u64::from_str_radix(&digits, radix).unwrap_or(0);
        let _ = start;
        TokenKind::Number(value as f64)
    }

    fn scan_decimal_number(&mut self, start: usize) -> TokenKind {
        let mut literal = String::new();

        if self.source[start] == b'.' {
            literal.push('.');
            self.advance();
            literal.push_str(&self.scan_digits(10));
        } else {
            literal.push_str(&self.scan_digits(10));
            if self.peek_byte() == Some(b'.') && self.peek_byte_at(1) != Some(b'.') {
                literal.push('.');
                self.advance();
                literal.push_str(&self.scan_digits(10));
            }
        }

        if matches!(self.peek_byte(), Some(b'e' | b'E')) {
            literal.push(self.advance().unwrap() as char);
            if matches!(self.peek_byte(), Some(b'+' | b'-')) {
                literal.push(self.advance().unwrap() as char);
            }
            literal.push_str(&self.scan_digits(10));
        }

        if self.peek_byte() == Some(b'n') {
            self.advance();
            let int_str = literal.replace('_', "");
            return TokenKind::BigInt(int_str.trim_end_matches('.').into());
        }

        let float_str = literal.replace('_', "");
        let value: f64 = float_str.parse().unwrap_or(f64::NAN);
        let _ = start;
        TokenKind::Number(value)
    }

    pub(super) fn scan_digits(&mut self, radix: u32) -> String {
        let mut digits = String::new();
        let mut saw_digit = false;

        while let Some(b) = self.peek_byte() {
            if b == b'_' {
                if !saw_digit {
                    break;
                }
                self.advance();
                continue;
            }
            if !Self::is_digit_for_radix(b, radix) {
                break;
            }
            digits.push(b as char);
            saw_digit = true;
            self.advance();
        }
        digits
    }

    pub(super) fn is_digit_for_radix(b: u8, radix: u32) -> bool {
        match radix {
            2 => matches!(b, b'0' | b'1'),
            8 => matches!(b, b'0'..=b'7'),
            10 => b.is_ascii_digit(),
            16 => b.is_ascii_digit() || matches!(b, b'a'..=b'f' | b'A'..=b'F'),
            _ => false,
        }
    }
}
