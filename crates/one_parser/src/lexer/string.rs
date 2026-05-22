use crate::token::TokenKind;

use super::Lexer;

impl Lexer<'_> {
    pub(super) fn scan_string(&mut self, quote: u8) -> TokenKind {
        self.advance(); // opening quote
        let mut value = String::new();
        loop {
            match self.peek_byte() {
                None => break,
                Some(b) if b == quote => {
                    self.advance();
                    return TokenKind::String(value);
                }
                Some(b'\\') => {
                    self.advance();
                    if let Some(ch) = self.scan_escape_sequence() {
                        value.push(ch);
                    }
                }
                Some(b'\n' | b'\r') => break,
                Some(b) => {
                    if let Some(ch) = self.decode_char(b) {
                        value.push(ch);
                    }
                    self.advance();
                }
            }
        }
        TokenKind::String(value)
    }

    pub(super) fn scan_template(&mut self, start: usize) -> TokenKind {
        self.advance(); // `
        let mut value = String::new();
        loop {
            match self.peek_byte() {
                None => break,
                Some(b'`') => {
                    self.advance();
                    let _ = start;
                    return TokenKind::NoSubstitutionTemplate(value);
                }
                Some(b'$') if self.peek_byte_at(1) == Some(b'{') => {
                    self.advance();
                    self.advance();
                    let _ = start;
                    return TokenKind::TemplateHead(value);
                }
                Some(b'\\') => {
                    self.advance();
                    if let Some(ch) = self.scan_escape_sequence() {
                        value.push(ch);
                    }
                }
                Some(b) => {
                    if let Some(ch) = self.decode_char(b) {
                        value.push(ch);
                    }
                    self.advance();
                }
            }
        }
        let _ = start;
        TokenKind::NoSubstitutionTemplate(value)
    }

    pub(super) fn scan_template_part_inner(&mut self) -> TokenKind {
        let mut value = String::new();
        loop {
            match self.peek_byte() {
                None => break,
                Some(b'`') => {
                    self.advance();
                    return TokenKind::TemplateTail(value);
                }
                Some(b'$') if self.peek_byte_at(1) == Some(b'{') => {
                    self.advance();
                    self.advance();
                    return TokenKind::TemplateMiddle(value);
                }
                Some(b'\\') => {
                    self.advance();
                    if let Some(ch) = self.scan_escape_sequence() {
                        value.push(ch);
                    }
                }
                Some(b) => {
                    if let Some(ch) = self.decode_char(b) {
                        value.push(ch);
                    }
                    self.advance();
                }
            }
        }
        TokenKind::TemplateTail(value)
    }

    pub(super) fn scan_escape_sequence(&mut self) -> Option<char> {
        match self.advance()? {
            b'n' => Some('\n'),
            b't' => Some('\t'),
            b'r' => Some('\r'),
            b'\\' => Some('\\'),
            b'\'' => Some('\''),
            b'"' => Some('"'),
            b'0' => Some('\0'),
            b'x' => self.scan_hex_escape(2),
            b'u' => self.scan_unicode_escape(),
            ch => Some(ch as char),
        }
    }

    fn scan_hex_escape(&mut self, len: usize) -> Option<char> {
        let mut hex = String::with_capacity(len);
        for _ in 0..len {
            let b = self.advance()?;
            if !b.is_ascii_hexdigit() {
                return None;
            }
            hex.push(b as char);
        }
        u32::from_str_radix(&hex, 16)
            .ok()
            .and_then(char::from_u32)
    }

    fn scan_unicode_escape(&mut self) -> Option<char> {
        if self.peek_byte() == Some(b'{') {
            self.advance();
            let mut hex = String::new();
            while let Some(b) = self.peek_byte() {
                if b == b'}' {
                    self.advance();
                    break;
                }
                if !b.is_ascii_hexdigit() {
                    return None;
                }
                hex.push(b as char);
                self.advance();
            }
            u32::from_str_radix(&hex, 16)
                .ok()
                .and_then(char::from_u32)
        } else {
            self.scan_hex_escape(4)
        }
    }

    pub(super) fn decode_char(&self, first: u8) -> Option<char> {
        if first.is_ascii() {
            return Some(first as char);
        }
        let s = std::str::from_utf8(&self.source[self.pos..]).ok()?;
        s.chars().next()
    }
}
