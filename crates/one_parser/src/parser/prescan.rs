use super::{ParseError, ParseResult, Parser};
use crate::ast::LazyFunctionBody;
use crate::token::TokenKind;

/// Pre-scan a function body: skip to matching `}`, record metadata.
/// Called when the parser encounters a function body in lazy mode.
pub struct PreScanner;

impl PreScanner {
    /// Skip a function body, returning metadata.
    /// Assumes the opening `{` has already been consumed.
    pub fn scan_function_body(parser: &mut Parser) -> ParseResult<LazyFunctionBody> {
        let start = parser.previous_end;
        let mut depth = 1u32;
        let mut has_eval = false;
        let mut has_arguments = false;
        let mut has_with = false;
        let mut is_strict = false;
        let mut first_statement = true;

        loop {
            let token = parser.advance();
            match &token.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(LazyFunctionBody {
                            source_start: start,
                            source_end: token.span.end,
                            has_eval,
                            has_arguments,
                            has_with,
                            is_strict,
                        });
                    }
                }
                TokenKind::Identifier(name) if name == "eval" => has_eval = true,
                TokenKind::Identifier(name) if name == "arguments" => has_arguments = true,
                TokenKind::With => has_with = true,
                TokenKind::String(s) if first_statement && s == "use strict" => {
                    is_strict = true;
                }
                TokenKind::Eof => {
                    return Err(ParseError {
                        message: "Unexpected end of input in function body".into(),
                        span: token.span,
                    });
                }
                _ => {}
            }
            if first_statement
                && !matches!(token.kind, TokenKind::String(_) | TokenKind::Semicolon)
            {
                first_statement = false;
            }
        }
    }
}
