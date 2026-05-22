mod number;
mod string;

use crate::span::Span;
use crate::token::{Token, TokenKind, lookup_keyword};

pub struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
}

impl Lexer<'_> {
    pub fn tokenize(source: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(source);
        let mut tokens = Vec::new();
        loop {
            let tok = lexer.next_token();
            if tok.kind == TokenKind::Eof {
                break;
            }
            tokens.push(tok);
        }
        tokens
    }
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Lexer {
            source: source.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn scan_template_part(&mut self) -> TokenKind {
        self.scan_template_part_inner()
    }

    pub fn consume_closing_template_brace(&mut self) {
        if self.peek_byte() == Some(b'}') {
            self.advance();
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();
        let start = self.pos;
        let Some(b) = self.peek_byte() else {
            return self.make_token(TokenKind::Eof, start);
        };

        let kind = match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' => self.scan_identifier(start),
            b'0'..=b'9' => self.scan_number(start),
            b'.' if self
                .peek_byte_at(1)
                .is_some_and(|next| next.is_ascii_digit()) =>
            {
                self.scan_number(start)
            }
            b'"' | b'\'' => self.scan_string(b),
            b'`' => self.scan_template(start),
            b'#' => self.scan_private_identifier(start),
            _ => self.scan_punctuation(start),
        };

        self.make_token(kind, start)
    }

    pub fn peek_byte(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    pub fn advance(&mut self) -> Option<u8> {
        let b = *self.source.get(self.pos)?;
        if b == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.pos += 1;
        Some(b)
    }

    pub fn skip_whitespace(&mut self) {
        loop {
            match self.peek_byte() {
                Some(b' ' | b'\t' | b'\r') => {
                    self.advance();
                }
                Some(b'\n') => {
                    self.advance();
                }
                Some(b'/') => match self.peek_byte_at(1) {
                    Some(b'/') => {
                        self.advance();
                        self.advance();
                        while matches!(self.peek_byte(), Some(b) if b != b'\n') {
                            self.advance();
                        }
                    }
                    Some(b'*') => {
                        self.advance();
                        self.advance();
                        loop {
                            match self.advance() {
                                None => return,
                                Some(b'*') if self.peek_byte() == Some(b'/') => {
                                    self.advance();
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => return,
                },
                _ => return,
            }
        }
    }

    pub fn scan_identifier(&mut self, start: usize) -> TokenKind {
        while self.is_id_continue(self.peek_char()) {
            self.advance();
        }
        let word = std::str::from_utf8(&self.source[start..self.pos]).unwrap();
        lookup_keyword(word).unwrap_or_else(|| TokenKind::Identifier(word.to_string()))
    }

    pub fn scan_punctuation(&mut self, start: usize) -> TokenKind {
        let _ = start;
        let b = self.advance().unwrap();
        match b {
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b';' => TokenKind::Semicolon,
            b',' => TokenKind::Comma,
            b':' => TokenKind::Colon,
            b'@' => TokenKind::At,
            b'~' => TokenKind::BitNot,
            b'=' => match self.peek_byte() {
                Some(b'=') => {
                    self.advance();
                    if self.peek_byte() == Some(b'=') {
                        self.advance();
                        TokenKind::StrictEq
                    } else {
                        TokenKind::Eq
                    }
                }
                Some(b'>') => {
                    self.advance();
                    TokenKind::Arrow
                }
                _ => TokenKind::Assign,
            },
            b'?' => {
                if self.peek_byte() == Some(b'.')
                    && !self
                        .peek_byte_at(1)
                        .is_some_and(|next| next.is_ascii_digit())
                {
                    self.advance();
                    TokenKind::QuestionDot
                } else if self.peek_byte() == Some(b'?') {
                    self.advance();
                    if self.peek_byte() == Some(b'=') {
                        self.advance();
                        TokenKind::NullishAssign
                    } else {
                        TokenKind::NullishCoalescing
                    }
                } else {
                    TokenKind::QuestionMark
                }
            }
            b'.' => {
                if self.peek_byte() == Some(b'.') && self.peek_byte_at(1) == Some(b'.') {
                    self.advance();
                    self.advance();
                    TokenKind::DotDotDot
                } else {
                    TokenKind::Dot
                }
            }
            b'+' => match self.peek_byte() {
                Some(b'+') => {
                    self.advance();
                    TokenKind::PlusPlus
                }
                Some(b'=') => {
                    self.advance();
                    TokenKind::PlusAssign
                }
                _ => TokenKind::Plus,
            },
            b'-' => match self.peek_byte() {
                Some(b'-') => {
                    self.advance();
                    TokenKind::MinusMinus
                }
                Some(b'=') => {
                    self.advance();
                    TokenKind::MinusAssign
                }
                _ => TokenKind::Minus,
            },
            b'*' => match self.peek_byte() {
                Some(b'*') => {
                    self.advance();
                    if self.peek_byte() == Some(b'=') {
                        self.advance();
                        TokenKind::StarStarAssign
                    } else {
                        TokenKind::StarStar
                    }
                }
                Some(b'=') => {
                    self.advance();
                    TokenKind::StarAssign
                }
                _ => TokenKind::Star,
            },
            b'/' => {
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    TokenKind::SlashAssign
                } else {
                    TokenKind::Slash
                }
            }
            b'%' => {
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    TokenKind::PercentAssign
                } else {
                    TokenKind::Percent
                }
            }
            b'&' => match self.peek_byte() {
                Some(b'&') => {
                    self.advance();
                    if self.peek_byte() == Some(b'=') {
                        self.advance();
                        TokenKind::AndAssign
                    } else {
                        TokenKind::And
                    }
                }
                Some(b'=') => {
                    self.advance();
                    TokenKind::BitAndAssign
                }
                _ => TokenKind::BitAnd,
            },
            b'|' => match self.peek_byte() {
                Some(b'|') => {
                    self.advance();
                    if self.peek_byte() == Some(b'=') {
                        self.advance();
                        TokenKind::OrAssign
                    } else {
                        TokenKind::Or
                    }
                }
                Some(b'=') => {
                    self.advance();
                    TokenKind::BitOrAssign
                }
                _ => TokenKind::BitOr,
            },
            b'^' => {
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    TokenKind::BitXorAssign
                } else {
                    TokenKind::BitXor
                }
            }
            b'<' => match self.peek_byte() {
                Some(b'<') => {
                    self.advance();
                    if self.peek_byte() == Some(b'=') {
                        self.advance();
                        TokenKind::ShlAssign
                    } else {
                        TokenKind::Shl
                    }
                }
                Some(b'=') => {
                    self.advance();
                    TokenKind::LtEq
                }
                _ => TokenKind::Lt,
            },
            b'>' => match self.peek_byte() {
                Some(b'>') => {
                    self.advance();
                    if self.peek_byte() == Some(b'>') {
                        self.advance();
                        if self.peek_byte() == Some(b'=') {
                            self.advance();
                            TokenKind::UShrAssign
                        } else {
                            TokenKind::UShr
                        }
                    } else if self.peek_byte() == Some(b'=') {
                        self.advance();
                        TokenKind::ShrAssign
                    } else {
                        TokenKind::Shr
                    }
                }
                Some(b'=') => {
                    self.advance();
                    TokenKind::GtEq
                }
                _ => TokenKind::Gt,
            },
            b'!' => match self.peek_byte() {
                Some(b'=') => {
                    self.advance();
                    if self.peek_byte() == Some(b'=') {
                        self.advance();
                        TokenKind::StrictNotEq
                    } else {
                        TokenKind::NotEq
                    }
                }
                _ => TokenKind::Not,
            },
            ch => TokenKind::Invalid(ch as char),
        }
    }

    fn scan_private_identifier(&mut self, start: usize) -> TokenKind {
        self.advance(); // #
        if !self.is_id_start(self.peek_char()) {
            let _ = start;
            return TokenKind::Invalid('#');
        }
        let id_start = self.pos;
        while self.is_id_continue(self.peek_char()) {
            self.advance();
        }
        let name = std::str::from_utf8(&self.source[id_start..self.pos])
            .unwrap()
            .to_string();
        let _ = start;
        TokenKind::PrivateIdentifier(name)
    }

    fn make_token(&self, kind: TokenKind, start: usize) -> Token {
        Token::new(kind, Span::new(start as u32, self.pos as u32))
    }

    pub(super) fn peek_byte_at(&self, offset: usize) -> Option<u8> {
        self.source.get(self.pos + offset).copied()
    }

    fn peek_char(&self) -> Option<char> {
        let s = std::str::from_utf8(&self.source[self.pos..]).ok()?;
        s.chars().next()
    }

    fn is_id_start(&self, ch: Option<char>) -> bool {
        ch.is_some_and(|c| c == '$' || c == '_' || c.is_alphabetic() || (!c.is_ascii() && c.is_alphanumeric()))
    }

    fn is_id_continue(&self, ch: Option<char>) -> bool {
        ch.is_some_and(|c| {
            c == '$'
                || c == '_'
                || c.is_alphanumeric()
                || (!c.is_ascii() && c.is_alphabetic())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(src: &str) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(src);
        let mut tokens = vec![];
        loop {
            let tok = lexer.next_token();
            if tok.kind == TokenKind::Eof {
                break;
            }
            tokens.push(tok.kind);
        }
        tokens
    }

    #[test]
    fn empty_source() {
        assert_eq!(tokenize(""), vec![]);
    }

    #[test]
    fn simple_number() {
        assert_eq!(tokenize("42"), vec![TokenKind::Number(42.0)]);
    }

    #[test]
    fn float_numbers() {
        assert_eq!(tokenize("3.14"), vec![TokenKind::Number(3.14)]);
        assert_eq!(tokenize(".5"), vec![TokenKind::Number(0.5)]);
        assert_eq!(tokenize("1e10"), vec![TokenKind::Number(1e10)]);
        assert_eq!(tokenize("1.5e-3"), vec![TokenKind::Number(1.5e-3)]);
    }

    #[test]
    fn hex_octal_binary() {
        assert_eq!(tokenize("0xFF"), vec![TokenKind::Number(255.0)]);
        assert_eq!(tokenize("0o77"), vec![TokenKind::Number(63.0)]);
        assert_eq!(tokenize("0b1010"), vec![TokenKind::Number(10.0)]);
    }

    #[test]
    fn numeric_separators() {
        assert_eq!(tokenize("1_000_000"), vec![TokenKind::Number(1_000_000.0)]);
    }

    #[test]
    fn bigint_literal() {
        assert_eq!(tokenize("42n"), vec![TokenKind::BigInt("42".into())]);
    }

    #[test]
    fn string_literals() {
        assert_eq!(tokenize(r#""hello""#), vec![TokenKind::String("hello".into())]);
        assert_eq!(tokenize("'world'"), vec![TokenKind::String("world".into())]);
    }

    #[test]
    fn string_escape_sequences() {
        assert_eq!(tokenize(r#""\n\t\\""#), vec![TokenKind::String("\n\t\\".into())]);
        assert_eq!(tokenize(r#""\x41""#), vec![TokenKind::String("A".into())]);
        assert_eq!(tokenize(r#""\u0041""#), vec![TokenKind::String("A".into())]);
        assert_eq!(tokenize(r#""\u{1F600}""#), vec![TokenKind::String("😀".into())]);
    }

    #[test]
    fn identifiers_and_keywords() {
        assert_eq!(tokenize("foo"), vec![TokenKind::Identifier("foo".into())]);
        assert_eq!(tokenize("if"), vec![TokenKind::If]);
        assert_eq!(tokenize("function"), vec![TokenKind::Function]);
        assert_eq!(tokenize("async"), vec![TokenKind::Async]);
    }

    #[test]
    fn operators() {
        assert_eq!(tokenize("==="), vec![TokenKind::StrictEq]);
        assert_eq!(tokenize("!=="), vec![TokenKind::StrictNotEq]);
        assert_eq!(tokenize(">>>"), vec![TokenKind::UShr]);
        assert_eq!(tokenize("??"), vec![TokenKind::NullishCoalescing]);
        assert_eq!(tokenize("?."), vec![TokenKind::QuestionDot]);
        assert_eq!(tokenize("=>"), vec![TokenKind::Arrow]);
        assert_eq!(tokenize("**"), vec![TokenKind::StarStar]);
    }

    #[test]
    fn assignment_operators() {
        assert_eq!(tokenize("+="), vec![TokenKind::PlusAssign]);
        assert_eq!(tokenize(">>>="), vec![TokenKind::UShrAssign]);
        assert_eq!(tokenize("??="), vec![TokenKind::NullishAssign]);
        assert_eq!(tokenize("&&="), vec![TokenKind::AndAssign]);
        assert_eq!(tokenize("||="), vec![TokenKind::OrAssign]);
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(
            tokenize("1 // comment\n2"),
            vec![TokenKind::Number(1.0), TokenKind::Number(2.0)]
        );
        assert_eq!(
            tokenize("1 /* block */ 2"),
            vec![TokenKind::Number(1.0), TokenKind::Number(2.0)]
        );
    }

    #[test]
    fn hello_world() {
        let tokens = tokenize(r#"console.log("Hello World")"#);
        assert_eq!(
            tokens,
            vec![
                TokenKind::Identifier("console".into()),
                TokenKind::Dot,
                TokenKind::Identifier("log".into()),
                TokenKind::LParen,
                TokenKind::String("Hello World".into()),
                TokenKind::RParen,
            ]
        );
    }

    #[test]
    fn variable_declaration() {
        let tokens = tokenize("let x = 42;");
        assert_eq!(
            tokens,
            vec![
                TokenKind::Let,
                TokenKind::Identifier("x".into()),
                TokenKind::Assign,
                TokenKind::Number(42.0),
                TokenKind::Semicolon,
            ]
        );
    }

    #[test]
    fn arrow_function() {
        let tokens = tokenize("(x) => x + 1");
        assert_eq!(
            tokens,
            vec![
                TokenKind::LParen,
                TokenKind::Identifier("x".into()),
                TokenKind::RParen,
                TokenKind::Arrow,
                TokenKind::Identifier("x".into()),
                TokenKind::Plus,
                TokenKind::Number(1.0),
            ]
        );
    }

    #[test]
    fn template_literal_no_sub() {
        let tokens = tokenize("`hello world`");
        assert_eq!(
            tokens,
            vec![TokenKind::NoSubstitutionTemplate("hello world".into())]
        );
    }

    #[test]
    fn spread_operator() {
        assert_eq!(tokenize("..."), vec![TokenKind::DotDotDot]);
    }

    #[test]
    fn private_identifier() {
        assert_eq!(
            tokenize("#name"),
            vec![TokenKind::PrivateIdentifier("name".into())]
        );
    }

    #[test]
    fn span_tracking() {
        let mut lexer = Lexer::new("let x");
        let t1 = lexer.next_token();
        let t2 = lexer.next_token();
        assert_eq!(t1.span, Span::new(0, 3));
        assert_eq!(t2.span, Span::new(4, 5));
    }
}
