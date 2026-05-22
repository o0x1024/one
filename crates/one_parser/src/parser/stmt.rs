use super::{ParseResult, Parser};
use crate::ast::*;
use crate::span::BytePos;
use crate::token::TokenKind;

impl Parser<'_> {
    pub(super) fn parse_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        match &self.current.kind {
            TokenKind::LBrace => self.parse_block_statement(),
            TokenKind::Var => {
                let decl = self.parse_variable_declaration(VarKind::Var)?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Statement {
                    kind: StatementKind::Declaration(decl),
                    span: self.span_from(start),
                })
            }
            TokenKind::Let => {
                let decl = self.parse_variable_declaration(VarKind::Let)?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Statement {
                    kind: StatementKind::Declaration(decl),
                    span: self.span_from(start),
                })
            }
            TokenKind::Const => {
                let decl = self.parse_variable_declaration(VarKind::Const)?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Statement {
                    kind: StatementKind::Declaration(decl),
                    span: self.span_from(start),
                })
            }
            TokenKind::Function => {
                let decl = self.parse_function_declaration()?;
                Ok(Statement {
                    kind: StatementKind::Declaration(decl),
                    span: self.span_from(start),
                })
            }
            TokenKind::Class => {
                let decl = self.parse_class_declaration()?;
                Ok(Statement {
                    kind: StatementKind::Declaration(decl),
                    span: self.span_from(start),
                })
            }
            TokenKind::Import if self.is_module() => {
                let decl = self.parse_import_declaration()?;
                Ok(Statement {
                    kind: StatementKind::Declaration(decl),
                    span: self.span_from(start),
                })
            }
            TokenKind::Export if self.is_module() => {
                let decl = self.parse_export_declaration()?;
                Ok(Statement {
                    kind: StatementKind::Declaration(decl),
                    span: self.span_from(start),
                })
            }
            TokenKind::If => self.parse_if_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::Do => self.parse_do_while_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::Throw => self.parse_throw_statement(),
            TokenKind::Try => self.parse_try_statement(),
            TokenKind::Switch => self.parse_switch_statement(),
            TokenKind::Break => self.parse_break_continue_statement(true),
            TokenKind::Continue => self.parse_break_continue_statement(false),
            TokenKind::Semicolon => {
                self.advance();
                Ok(Statement {
                    kind: StatementKind::EmptyStatement,
                    span: self.span_from(start),
                })
            }
            _ => {
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::Semicolon)?;
                Ok(Statement {
                    kind: StatementKind::ExpressionStatement(expr),
                    span: self.span_from(start),
                })
            }
        }
    }

    pub(super) fn parse_block_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::LBrace)?;
        let body = self.parse_statement_list()?;
        self.expect(&TokenKind::RBrace)?;
        Ok(Statement {
            kind: StatementKind::BlockStatement(body),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_variable_declaration(&mut self, kind: VarKind) -> ParseResult<Declaration> {
        let start = self.current.span.start;
        match kind {
            VarKind::Var => {
                self.expect(&TokenKind::Var)?;
            }
            VarKind::Let => {
                self.expect(&TokenKind::Let)?;
            }
            VarKind::Const => {
                self.expect(&TokenKind::Const)?;
            }
        }
        let mut declarations = Vec::new();
        loop {
            let decl_start = self.current.span.start;
            let id = self.parse_pattern()?;
            let init = if self.eat(&TokenKind::Assign) {
                Some(self.parse_assignment_expression()?)
            } else {
                None
            };
            declarations.push(VariableDeclarator {
                id,
                init,
                span: self.span_from(decl_start),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(Declaration {
            kind: DeclarationKind::VariableDeclaration { kind, declarations },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_if_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::If)?;
        self.expect(&TokenKind::LParen)?;
        let test = self.parse_expression()?;
        self.expect(&TokenKind::RParen)?;
        let consequent = Box::new(self.parse_statement()?);
        let alternate = if self.eat(&TokenKind::Else) {
            Some(Box::new(self.parse_statement()?))
        } else {
            None
        };
        Ok(Statement {
            kind: StatementKind::IfStatement {
                test,
                consequent,
                alternate,
            },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_while_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::While)?;
        self.expect(&TokenKind::LParen)?;
        let test = self.parse_expression()?;
        self.expect(&TokenKind::RParen)?;
        let body = Box::new(self.parse_statement()?);
        Ok(Statement {
            kind: StatementKind::WhileStatement { test, body },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_do_while_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Do)?;
        let body = Box::new(self.parse_statement()?);
        self.expect(&TokenKind::While)?;
        self.expect(&TokenKind::LParen)?;
        let test = self.parse_expression()?;
        self.expect(&TokenKind::RParen)?;
        self.expect(&TokenKind::Semicolon)?;
        Ok(Statement {
            kind: StatementKind::DoWhileStatement { test, body },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_for_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::For)?;
        self.expect(&TokenKind::LParen)?;

        if self.at(&TokenKind::Semicolon) {
            self.advance();
            let test = if !self.at(&TokenKind::Semicolon) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            self.expect(&TokenKind::Semicolon)?;
            let update = if !self.at(&TokenKind::RParen) {
                Some(self.parse_expression()?)
            } else {
                None
            };
            self.expect(&TokenKind::RParen)?;
            let body = Box::new(self.parse_statement()?);
            return Ok(Statement {
                kind: StatementKind::ForStatement {
                    init: None,
                    test,
                    update,
                    body,
                },
                span: self.span_from(start),
            });
        }

        if self.at(&TokenKind::Var) || self.at(&TokenKind::Let) || self.at(&TokenKind::Const) {
            let kind = if self.at(&TokenKind::Var) {
                VarKind::Var
            } else if self.at(&TokenKind::Let) {
                VarKind::Let
            } else {
                VarKind::Const
            };
            let decl = self.parse_variable_declaration(kind)?;
            if self.at(&TokenKind::In) || self.at(&TokenKind::Of) {
                return self.finish_for_in_of(ForInOfLeft::Declaration(decl), start);
            }
            let init = Some(ForInit::Declaration(decl));
            self.expect(&TokenKind::Semicolon)?;
            return self.finish_classic_for(init, start);
        }

        let expr = self.parse_expression()?;
        if self.at(&TokenKind::In) || self.at(&TokenKind::Of) {
            return self.finish_for_in_of(ForInOfLeft::Expression(expr), start);
        }
        self.finish_classic_for(Some(ForInit::Expression(expr)), start)
    }

    fn finish_classic_for(
        &mut self,
        init: Option<ForInit>,
        start: BytePos,
    ) -> ParseResult<Statement> {
        let test = if !self.at(&TokenKind::Semicolon) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        let update = if !self.at(&TokenKind::RParen) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.expect(&TokenKind::RParen)?;
        let body = Box::new(self.parse_statement()?);
        Ok(Statement {
            kind: StatementKind::ForStatement {
                init,
                test,
                update,
                body,
            },
            span: self.span_from(start),
        })
    }

    fn finish_for_in_of(&mut self, left: ForInOfLeft, start: BytePos) -> ParseResult<Statement> {
        let is_of = self.eat(&TokenKind::Of);
        if !is_of {
            self.expect(&TokenKind::In)?;
        }
        let right = self.parse_expression()?;
        self.expect(&TokenKind::RParen)?;
        let body = Box::new(self.parse_statement()?);
        if is_of {
            Ok(Statement {
                kind: StatementKind::ForOfStatement {
                    left,
                    right,
                    body,
                    is_await: false,
                },
                span: self.span_from(start),
            })
        } else {
            Ok(Statement {
                kind: StatementKind::ForInStatement { left, right, body },
                span: self.span_from(start),
            })
        }
    }

    pub(super) fn parse_return_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Return)?;
        let argument = if self.at(&TokenKind::Semicolon)
            || self.at(&TokenKind::RBrace)
            || self.at_eof()
        {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(&TokenKind::Semicolon)?;
        Ok(Statement {
            kind: StatementKind::ReturnStatement(argument),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_throw_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Throw)?;
        let argument = self.parse_expression()?;
        self.expect(&TokenKind::Semicolon)?;
        Ok(Statement {
            kind: StatementKind::ThrowStatement(argument),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_try_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Try)?;
        self.expect(&TokenKind::LBrace)?;
        let block = self.parse_statement_list()?;
        self.expect(&TokenKind::RBrace)?;

        let handler = if self.eat(&TokenKind::Catch) {
            self.expect(&TokenKind::LParen)?;
            let param = if self.at(&TokenKind::RParen) {
                None
            } else {
                Some(self.parse_pattern()?)
            };
            self.expect(&TokenKind::RParen)?;
            self.expect(&TokenKind::LBrace)?;
            let catch_start = self.current.span.start;
            let body = self.parse_statement_list()?;
            self.expect(&TokenKind::RBrace)?;
            Some(CatchClause {
                param,
                body,
                span: self.span_from(catch_start),
            })
        } else {
            None
        };

        let finalizer = if self.eat(&TokenKind::Finally) {
            self.expect(&TokenKind::LBrace)?;
            let body = self.parse_statement_list()?;
            self.expect(&TokenKind::RBrace)?;
            Some(body)
        } else {
            None
        };

        Ok(Statement {
            kind: StatementKind::TryStatement {
                block,
                handler,
                finalizer,
            },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_switch_statement(&mut self) -> ParseResult<Statement> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Switch)?;
        self.expect(&TokenKind::LParen)?;
        let discriminant = self.parse_expression()?;
        self.expect(&TokenKind::RParen)?;
        self.expect(&TokenKind::LBrace)?;

        let mut cases = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            cases.push(self.parse_switch_case()?);
        }
        self.expect(&TokenKind::RBrace)?;

        Ok(Statement {
            kind: StatementKind::SwitchStatement {
                discriminant,
                cases,
            },
            span: self.span_from(start),
        })
    }

    fn parse_switch_case(&mut self) -> ParseResult<SwitchCase> {
        let start = self.current.span.start;
        let test = if self.eat(&TokenKind::Case) {
            Some(self.parse_expression()?)
        } else {
            self.expect(&TokenKind::Default)?;
            None
        };
        self.expect(&TokenKind::Colon)?;
        let mut consequent = Vec::new();
        while !self.at(&TokenKind::RBrace)
            && !self.at(&TokenKind::Case)
            && !self.at(&TokenKind::Default)
            && !self.at_eof()
        {
            consequent.push(self.parse_statement()?);
        }
        Ok(SwitchCase {
            test,
            consequent,
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_break_continue_statement(
        &mut self,
        is_break: bool,
    ) -> ParseResult<Statement> {
        let start = self.current.span.start;
        if is_break {
            self.expect(&TokenKind::Break)?;
        } else {
            self.expect(&TokenKind::Continue)?;
        }
        let label = if self.at_identifier() {
            Some(self.parse_identifier_name()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        Ok(Statement {
            kind: if is_break {
                StatementKind::BreakStatement(label)
            } else {
                StatementKind::ContinueStatement(label)
            },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_function_declaration(&mut self) -> ParseResult<Declaration> {
        let start = self.current.span.start;
        let is_async = self.eat(&TokenKind::Async);
        self.expect(&TokenKind::Function)?;
        let id = Some(self.parse_identifier_name()?);
        let func = self.parse_function_after_name(id, is_async, false, start)?;
        Ok(Declaration {
            kind: DeclarationKind::FunctionDeclaration(func),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_class_declaration(&mut self) -> ParseResult<Declaration> {
        let start = self.current.span.start;
        self.expect(&TokenKind::Class)?;
        let id = Some(self.parse_identifier_name()?);
        let class = self.parse_class_tail(id, start)?;
        Ok(Declaration {
            kind: DeclarationKind::ClassDeclaration(class),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_class_tail(
        &mut self,
        id: Option<String>,
        start: BytePos,
    ) -> ParseResult<Class> {
        let super_class = if self.eat(&TokenKind::Extends) {
            Some(Box::new(self.parse_assignment_expression()?))
        } else {
            None
        };
        self.expect(&TokenKind::LBrace)?;
        let mut body = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            body.push(self.parse_class_member()?);
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(Class {
            id,
            super_class,
            body,
            span: self.span_from(start),
        })
    }

    fn parse_class_member(&mut self) -> ParseResult<ClassMember> {
        let start = self.current.span.start;
        let is_static = self.eat(&TokenKind::Static);
        let key = self.parse_property_key()?;
        let computed = matches!(key, PropertyKey::Computed(_));

        if self.at(&TokenKind::LParen) {
            let func = self.parse_method_function(None)?;
            return Ok(ClassMember {
                kind: ClassMemberKind::Method {
                    key,
                    value: func,
                    kind: MethodKind::Method,
                    is_static,
                    computed,
                },
                span: self.span_from(start),
            });
        }

        let value = if self.eat(&TokenKind::Assign) {
            Some(self.parse_assignment_expression()?)
        } else {
            None
        };
        self.expect(&TokenKind::Semicolon)?;
        Ok(ClassMember {
            kind: ClassMemberKind::Property {
                key,
                value,
                is_static,
                computed,
            },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_function_params(&mut self) -> ParseResult<Vec<Pattern>> {
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            params.push(self.parse_pattern()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    pub(super) fn parse_pattern(&mut self) -> ParseResult<Pattern> {
        let start = self.current.span.start;
        match &self.current.kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(Pattern {
                    kind: PatternKind::Identifier {
                        name,
                        type_annotation: None,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                let mut rest = None;
                while !self.at(&TokenKind::RBracket) && !self.at_eof() {
                    if self.eat(&TokenKind::Comma) {
                        elements.push(None);
                        if self.at(&TokenKind::RBracket) {
                            break;
                        }
                        continue;
                    }
                    if self.at(&TokenKind::DotDotDot) {
                        self.advance();
                        rest = Some(Box::new(self.parse_pattern()?));
                        break;
                    }
                    elements.push(Some(self.parse_pattern()?));
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBracket)?;
                Ok(Pattern {
                    kind: PatternKind::ArrayPattern { elements, rest },
                    span: self.span_from(start),
                })
            }
            TokenKind::LBrace => {
                self.advance();
                let mut properties = Vec::new();
                let mut rest = None;
                while !self.at(&TokenKind::RBrace) && !self.at_eof() {
                    if self.at(&TokenKind::DotDotDot) {
                        self.advance();
                        rest = Some(Box::new(self.parse_pattern()?));
                        break;
                    }
                    let prop_start = self.current.span.start;
                    let key = self.parse_property_key()?;
                    if self.eat(&TokenKind::Colon) {
                        let computed = matches!(&key, PropertyKey::Computed(_));
                        let value = self.parse_pattern()?;
                        properties.push(ObjectPatternProperty {
                            key,
                            value,
                            computed,
                            shorthand: false,
                            span: self.span_from(prop_start),
                        });
                    } else {
                        let PropertyKey::Identifier(name) = key else {
                            return Err(self.error("expected ':' after property key"));
                        };
                        properties.push(ObjectPatternProperty {
                            key: PropertyKey::Identifier(name.clone()),
                            value: Pattern {
                                kind: PatternKind::Identifier {
                                    name,
                                    type_annotation: None,
                                },
                                span: self.span_from(prop_start),
                            },
                            computed: false,
                            shorthand: true,
                            span: self.span_from(prop_start),
                        });
                    }
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RBrace)?;
                Ok(Pattern {
                    kind: PatternKind::ObjectPattern { properties, rest },
                    span: self.span_from(start),
                })
            }
            _ => Err(self.error("expected pattern")),
        }
    }
}
