use super::{ParseResult, Parser};
use crate::ast::*;
use crate::span::BytePos;
use crate::token::TokenKind;

impl Parser<'_> {
    pub(super) fn parse_import_declaration(&mut self) -> ParseResult<Declaration> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Import)?;

        let mut specifiers = Vec::new();

        if matches!(self.current.kind, TokenKind::String(_)) {
            // Side-effect import: import "module";
            let source = self.parse_module_string()?;
            self.expect(&TokenKind::Semicolon)?;
            return Ok(Declaration {
                kind: DeclarationKind::ImportDeclaration {
                    specifiers,
                    source,
                },
                span: self.span_from(start),
            });
        }

        if self.eat(&TokenKind::Star) {
            // import * as ns from "module";
            self.expect(&TokenKind::As)?;
            let local = self.parse_identifier_name()?;
            specifiers.push(ImportSpecifier::Namespace {
                local,
                span: self.span_from(start),
            });
        } else if self.eat(&TokenKind::LBrace) {
            // import { a, b as c } from "module";
            loop {
                let spec_start = self.current.span.start;
                let imported = self.parse_identifier_name()?;
                let (local, spec_span) = if self.eat(&TokenKind::As) {
                    let local = self.parse_identifier_name()?;
                    (local, self.span_from(spec_start))
                } else {
                    (imported.clone(), self.span_from(spec_start))
                };
                specifiers.push(ImportSpecifier::Named {
                    imported,
                    local,
                    span: spec_span,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace)?;
        } else {
            // Default import: import foo from "module";
            let local = self.parse_identifier_name()?;
            specifiers.push(ImportSpecifier::Default {
                local,
                span: self.span_from(start),
            });
        }

        self.expect(&TokenKind::From)?;
        let source = self.parse_module_string()?;
        self.expect(&TokenKind::Semicolon)?;

        Ok(Declaration {
            kind: DeclarationKind::ImportDeclaration {
                specifiers,
                source,
            },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_export_declaration(&mut self) -> ParseResult<Declaration> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Export)?;

        if self.eat(&TokenKind::Default) {
            return self.parse_export_default(start);
        }

        if self.eat(&TokenKind::Star) {
            self.expect(&TokenKind::From)?;
            let source = self.parse_module_string()?;
            self.expect(&TokenKind::Semicolon)?;
            return Ok(Declaration {
                kind: DeclarationKind::ExportAllDeclaration {
                    source,
                    exported: None,
                },
                span: self.span_from(start),
            });
        }

        if self.eat(&TokenKind::LBrace) {
            let mut specifiers = Vec::new();
            loop {
                let spec_start = self.current.span.start;
                let local = self.parse_identifier_name()?;
                let (exported, spec_span) = if self.eat(&TokenKind::As) {
                    let exported = self.parse_identifier_name()?;
                    (exported, self.span_from(spec_start))
                } else {
                    (local.clone(), self.span_from(spec_start))
                };
                specifiers.push(ExportSpecifier {
                    local,
                    exported,
                    span: spec_span,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace)?;

            let source = if self.eat(&TokenKind::From) {
                Some(self.parse_module_string()?)
            } else {
                None
            };
            self.expect(&TokenKind::Semicolon)?;

            return Ok(Declaration {
                kind: DeclarationKind::ExportNamedDeclaration {
                    declaration: None,
                    specifiers,
                    source,
                },
                span: self.span_from(start),
            });
        }

        let declaration = if self.at(&TokenKind::Var)
            || self.at(&TokenKind::Let)
            || self.at(&TokenKind::Const)
        {
            let kind = if self.at(&TokenKind::Var) {
                VarKind::Var
            } else if self.at(&TokenKind::Let) {
                VarKind::Let
            } else {
                VarKind::Const
            };
            Some(Box::new(self.parse_variable_declaration(kind)?))
        } else if self.at(&TokenKind::Function) {
            Some(Box::new(self.parse_function_declaration()?))
        } else if self.at(&TokenKind::Class) {
            Some(Box::new(self.parse_class_declaration()?))
        } else {
            None
        };

        if let Some(declaration) = declaration {
            if matches!(
                &declaration.kind,
                DeclarationKind::VariableDeclaration { .. }
            ) {
                self.expect(&TokenKind::Semicolon)?;
            }
            return Ok(Declaration {
                kind: DeclarationKind::ExportNamedDeclaration {
                    declaration: Some(declaration),
                    specifiers: Vec::new(),
                    source: None,
                },
                span: self.span_from(start),
            });
        }

        Err(self.error("expected export declaration"))
    }

    fn parse_export_default(&mut self, start: BytePos) -> ParseResult<Declaration> {
        let default = if self.at(&TokenKind::Function) {
            let func_start = self.current.span.start;
            self.expect(&TokenKind::Function)?;
            let id = if self.at_identifier() {
                Some(self.parse_identifier_name()?)
            } else {
                None
            };
            let func = self.parse_function_after_name(id, false, false, func_start)?;
            ExportDefault::FunctionDeclaration(func)
        } else if self.at(&TokenKind::Class) {
            let class = self.parse_class_declaration()?;
            ExportDefault::ClassDeclaration(match class.kind {
                DeclarationKind::ClassDeclaration(c) => c,
                _ => return Err(self.error("expected class declaration")),
            })
        } else {
            let expr = self.parse_assignment_expression()?;
            self.expect(&TokenKind::Semicolon)?;
            ExportDefault::Expression(expr)
        };

        Ok(Declaration {
            kind: DeclarationKind::ExportDefaultDeclaration(default),
            span: self.span_from(start),
        })
    }

    fn parse_module_string(&mut self) -> ParseResult<String> {
        match &self.current.kind {
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(s)
            }
            _ => Err(self.error("expected string literal")),
        }
    }
}
