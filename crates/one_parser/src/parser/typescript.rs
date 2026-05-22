use super::{ParseResult, Parser};
use crate::ast::*;
use crate::span::BytePos;
use crate::token::TokenKind;

impl Parser<'_> {
    pub(super) fn is_typescript(&self) -> bool {
        self.typescript
    }

    pub(super) fn skip_type_annotation(&mut self) {
        if !self.is_typescript() {
            return;
        }
        if !self.eat(&TokenKind::Colon) {
            return;
        }
        self.skip_type();
    }

    pub(super) fn skip_type(&mut self) {
        if !self.is_typescript() {
            return;
        }
        let mut depth = 0;
        while !self.at_eof() {
            match &self.current.kind {
                TokenKind::Eq
                | TokenKind::Assign
                | TokenKind::Comma
                | TokenKind::Semicolon
                | TokenKind::Arrow => {
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                TokenKind::Gt => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                TokenKind::LParen | TokenKind::LBracket | TokenKind::Lt => {
                    depth += 1;
                }
                TokenKind::LBrace => {
                    if depth == 0 {
                        break;
                    }
                    depth += 1;
                }
                TokenKind::Eof => break,
                _ => {}
            }
            self.advance();
        }
    }

    pub(super) fn skip_generic_params(&mut self) {
        if !self.is_typescript() || !self.at(&TokenKind::Lt) {
            return;
        }
        let mut depth = 1;
        self.advance();
        while depth > 0 && !self.at_eof() {
            match &self.current.kind {
                TokenKind::Lt => depth += 1,
                TokenKind::Gt => depth -= 1,
                TokenKind::Eof => break,
                _ => {}
            }
            self.advance();
        }
    }

    pub(super) fn skip_access_modifiers(&mut self) {
        if !self.is_typescript() {
            return;
        }
        while matches!(
            &self.current.kind,
            TokenKind::Public
                | TokenKind::Private
                | TokenKind::Protected
                | TokenKind::Readonly
        ) {
            self.advance();
        }
    }

    pub(super) fn skip_ts_postfix_modifiers(&mut self) {
        if !self.is_typescript() {
            return;
        }
        self.eat(&TokenKind::QuestionMark);
    }

    pub(super) fn parse_ts_as_or_satisfies(&mut self, expr: Expression) -> Expression {
        if !self.is_typescript() {
            return expr;
        }
        while self.at(&TokenKind::As) || self.at(&TokenKind::Satisfies) {
            self.advance();
            self.skip_type();
        }
        expr
    }

    pub(super) fn is_type_assertion_start(&self) -> bool {
        if !self.is_typescript() || !self.at(&TokenKind::Lt) {
            return false;
        }
        matches!(
            self.lexer.peek_kind(),
            Some(
                TokenKind::Identifier(_)
                    | TokenKind::Type
                    | TokenKind::Void
                    | TokenKind::This
                    | TokenKind::LBrace
                    | TokenKind::LBracket
                    | TokenKind::LParen
                    | TokenKind::Readonly
                    | TokenKind::Keyof
                    | TokenKind::Infer
            )
        )
    }

    pub(super) fn parse_type_assertion(&mut self, start: BytePos) -> ParseResult<Expression> {
        self.expect(&TokenKind::Lt)?;
        self.skip_type();
        self.expect(&TokenKind::Gt)?;
        let argument = self.parse_unary_expression()?;
        Ok(Expression {
            kind: argument.kind,
            span: self.span_from(start),
        })
    }

    pub(super) fn skip_braced_block(&mut self) {
        if !self.eat(&TokenKind::LBrace) {
            return;
        }
        let mut depth = 1;
        while depth > 0 && !self.at_eof() {
            match &self.current.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth -= 1,
                TokenKind::Eof => break,
                _ => {}
            }
            self.advance();
        }
    }

    pub(super) fn skip_interface_declaration(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Interface)?;
        let _ = self.parse_identifier_name()?;
        self.skip_generic_params();
        if self.eat(&TokenKind::Extends) {
            self.skip_type();
        }
        self.skip_braced_block();
        self.eat(&TokenKind::Semicolon);
        Ok(Statement {
            kind: StatementKind::EmptyStatement,
            span: self.span_from(start),
        })
    }

    pub(super) fn skip_type_alias_declaration(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Type)?;
        let _ = self.parse_identifier_name()?;
        self.skip_generic_params();
        self.expect(&TokenKind::Assign)?;
        self.skip_type();
        self.expect(&TokenKind::Semicolon)?;
        Ok(Statement {
            kind: StatementKind::EmptyStatement,
            span: self.span_from(start),
        })
    }

    pub(super) fn skip_namespace_declaration(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.advance();
        if matches!(self.current.kind, TokenKind::String(_)) {
            self.advance();
        } else {
            let _ = self.parse_identifier_name()?;
        }
        self.skip_braced_block();
        Ok(Statement {
            kind: StatementKind::EmptyStatement,
            span: self.span_from(start),
        })
    }

    pub(super) fn skip_declare_declaration(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Declare)?;
        match &self.current.kind {
            TokenKind::Interface => self.skip_interface_declaration(),
            TokenKind::Type => self.skip_type_alias_declaration(),
            TokenKind::Enum => self.parse_enum_declaration_statement(),
            TokenKind::Namespace | TokenKind::Module => self.skip_namespace_declaration(),
            TokenKind::Function | TokenKind::Class | TokenKind::Var | TokenKind::Let
            | TokenKind::Const => {
                let stmt = self.parse_statement()?;
                Ok(Statement {
                    kind: stmt.kind,
                    span: self.span_from(start),
                })
            }
            _ => Err(self.error("expected declaration after 'declare'")),
        }
    }

    pub(super) fn parse_enum_declaration(&mut self) -> ParseResult<Declaration> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Enum)?;
        let name = self.parse_identifier_name()?;
        self.expect(&TokenKind::LBrace)?;

        let mut properties = Vec::new();
        let mut auto_value = 0i64;
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            let member_start = self.current.span.start;
            let member_name = self.parse_identifier_name()?;
            let value = if self.eat(&TokenKind::Assign) {
                let expr = self.parse_assignment_expression()?;
                if let ExpressionKind::NumberLiteral(n) = expr.kind {
                    auto_value = n as i64 + 1;
                }
                expr
            } else {
                let expr = Expression {
                    kind: ExpressionKind::NumberLiteral(auto_value as f64),
                    span: self.span_from(member_start),
                };
                auto_value += 1;
                expr
            };
            properties.push(ObjectProperty {
                kind: ObjectPropertyKind::Property {
                    key: PropertyKey::Identifier(member_name),
                    value,
                    computed: false,
                    shorthand: false,
                },
                span: self.span_from(member_start),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace)?;

        Ok(Declaration {
            kind: DeclarationKind::VariableDeclaration {
                kind: VarKind::Const,
                declarations: vec![VariableDeclarator {
                    id: Pattern {
                        kind: PatternKind::Identifier {
                            name,
                            type_annotation: None,
                        },
                        span: self.span_from(start),
                    },
                    init: Some(Expression {
                        kind: ExpressionKind::ObjectExpression(properties),
                        span: self.span_from(start),
                    }),
                    span: self.span_from(start),
                }],
            },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_enum_declaration_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        let decl = self.parse_enum_declaration()?;
        Ok(Statement {
            kind: StatementKind::Declaration(decl),
            span: self.span_from(start),
        })
    }
}
