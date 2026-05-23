use super::{ParseError, ParseResult, Parser};
use crate::ast::*;
use crate::span::BytePos;
use crate::token::TokenKind;

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        let first = self.parse_assignment_expression()?;
        if !self.at(&TokenKind::Comma) {
            return Ok(first);
        }
        let mut exprs = vec![first];
        while self.eat(&TokenKind::Comma) {
            exprs.push(self.parse_assignment_expression()?);
        }
        Ok(Expression {
            kind: ExpressionKind::SequenceExpression(exprs),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_assignment_expression(&mut self) -> ParseResult<Expression> {
        let expr = self.parse_conditional_expression()?;
        if self.current.kind.is_assignment_operator() {
            let start = expr.span.start;
            let op = token_to_assign_op(self.advance().kind);
            let right = self.parse_assignment_expression()?;
            let left = expression_to_assign_target(expr)?;
            return Ok(Expression {
                kind: ExpressionKind::AssignmentExpression {
                    operator: op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: self.span_from(start),
            });
        }
        Ok(expr)
    }

    fn parse_conditional_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        let test = self.parse_binary_expression(0)?;
        let test = self.parse_ts_as_or_satisfies(test);
        if self.eat(&TokenKind::QuestionMark) {
            let consequent = self.parse_assignment_expression()?;
            self.expect(&TokenKind::Colon)?;
            let alternate = self.parse_assignment_expression()?;
            return Ok(Expression {
                kind: ExpressionKind::ConditionalExpression {
                    test: Box::new(test),
                    consequent: Box::new(consequent),
                    alternate: Box::new(alternate),
                },
                span: self.span_from(start),
            });
        }
        Ok(test)
    }

    fn parse_binary_expression(&mut self, min_prec: u8) -> ParseResult<Expression> {
        let mut left = self.parse_unary_expression()?;
        loop {
            let op_kind = self.current.kind.clone();
            let Some(prec) = binary_precedence(&op_kind) else {
                break;
            };
            if prec < min_prec {
                break;
            }
            let next_min = if matches!(op_kind, TokenKind::StarStar) {
                prec
            } else {
                prec + 1
            };
            self.advance();
            let right = self.parse_binary_expression(next_min)?;
            let span = left.span.merge(right.span);
            left = Expression {
                kind: make_binary_or_logical(op_kind, left, right),
                span,
            };
        }
        Ok(left)
    }

    pub(super) fn parse_unary_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        if self.is_type_assertion_start() {
            return self.parse_type_assertion(start);
        }
        match &self.current.kind {
            TokenKind::Minus => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UnaryExpression {
                        operator: UnaryOp::Minus,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::Plus => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UnaryExpression {
                        operator: UnaryOp::Plus,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::Not => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UnaryExpression {
                        operator: UnaryOp::Not,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::BitNot => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UnaryExpression {
                        operator: UnaryOp::BitNot,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::Typeof => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UnaryExpression {
                        operator: UnaryOp::Typeof,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::Void => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UnaryExpression {
                        operator: UnaryOp::Void,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::Delete => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UnaryExpression {
                        operator: UnaryOp::Delete,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::Await => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::AwaitExpression(Box::new(argument)),
                    span: self.span_from(start),
                })
            }
            TokenKind::PlusPlus => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UpdateExpression {
                        operator: UpdateOp::Increment,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            TokenKind::MinusMinus => {
                self.advance();
                let argument = self.parse_unary_expression()?;
                Ok(Expression {
                    kind: ExpressionKind::UpdateExpression {
                        operator: UpdateOp::Decrement,
                        argument: Box::new(argument),
                        prefix: true,
                    },
                    span: self.span_from(start),
                })
            }
            _ => self.parse_postfix_expression(),
        }
    }

    fn parse_postfix_expression(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_call_expression()?;
        while matches!(
            self.current.kind,
            TokenKind::PlusPlus | TokenKind::MinusMinus
        ) {
            let start = expr.span.start;
            let operator = if self.eat(&TokenKind::PlusPlus) {
                UpdateOp::Increment
            } else {
                self.eat(&TokenKind::MinusMinus);
                UpdateOp::Decrement
            };
            expr = Expression {
                kind: ExpressionKind::UpdateExpression {
                    operator,
                    argument: Box::new(expr),
                    prefix: false,
                },
                span: self.span_from(start),
            };
        }
        Ok(expr)
    }

    fn parse_call_expression(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_primary_expression()?;
        loop {
            match &self.current.kind {
                TokenKind::Dot => {
                    let start = expr.span.start;
                    self.advance();
                    let property = self.parse_property_name()?;
                    expr = Expression {
                        kind: ExpressionKind::MemberExpression {
                            object: Box::new(expr),
                            property: MemberProperty::Identifier(property),
                            computed: false,
                            optional: false,
                        },
                        span: self.span_from(start),
                    };
                }
                TokenKind::QuestionDot => {
                    let start = expr.span.start;
                    self.advance();
                    if self.at(&TokenKind::LParen) {
                        self.advance();
                        let arguments = self.parse_arguments()?;
                        self.expect(&TokenKind::RParen)?;
                        expr = Expression {
                            kind: ExpressionKind::CallExpression {
                                callee: Box::new(expr),
                                arguments,
                                optional: true,
                            },
                            span: self.span_from(start),
                        };
                    } else if self.at(&TokenKind::LBracket) {
                        self.advance();
                        let property = self.parse_expression()?;
                        self.expect(&TokenKind::RBracket)?;
                        expr = Expression {
                            kind: ExpressionKind::MemberExpression {
                                object: Box::new(expr),
                                property: MemberProperty::Expression(Box::new(property)),
                                computed: true,
                                optional: true,
                            },
                            span: self.span_from(start),
                        };
                    } else {
                        let property = self.parse_property_name()?;
                        expr = Expression {
                            kind: ExpressionKind::MemberExpression {
                                object: Box::new(expr),
                                property: MemberProperty::Identifier(property),
                                computed: false,
                                optional: true,
                            },
                            span: self.span_from(start),
                        };
                    }
                }
                TokenKind::LBracket => {
                    let start = expr.span.start;
                    self.advance();
                    let property = self.parse_expression()?;
                    self.expect(&TokenKind::RBracket)?;
                    expr = Expression {
                        kind: ExpressionKind::MemberExpression {
                            object: Box::new(expr),
                            property: MemberProperty::Expression(Box::new(property)),
                            computed: true,
                            optional: false,
                        },
                        span: self.span_from(start),
                    };
                }
                TokenKind::LParen => {
                    let start = expr.span.start;
                    self.advance();
                    let arguments = self.parse_arguments()?;
                    self.expect(&TokenKind::RParen)?;
                    expr = Expression {
                        kind: ExpressionKind::CallExpression {
                            callee: Box::new(expr),
                            arguments,
                            optional: false,
                        },
                        span: self.span_from(start),
                    };
                }
                TokenKind::NoSubstitutionTemplate(_)
                | TokenKind::TemplateHead(_)
                | TokenKind::TemplateMiddle(_)
                | TokenKind::TemplateTail(_) => {
                    let start = expr.span.start;
                    let quasi = self.parse_template_literal()?;
                    expr = Expression {
                        kind: ExpressionKind::TaggedTemplateExpression {
                            tag: Box::new(expr),
                            quasi,
                        },
                        span: self.span_from(start),
                    };
                }
                TokenKind::Not
                    if self.is_typescript()
                        && !matches!(self.lexer.peek_kind(), Some(TokenKind::Eq)) =>
                {
                    self.advance();
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        match &self.current.kind {
            TokenKind::Number(n) => {
                let n = *n;
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::NumberLiteral(n),
                    span: self.span_from(start),
                })
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::StringLiteral(s),
                    span: self.span_from(start),
                })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::BooleanLiteral(true),
                    span: self.span_from(start),
                })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::BooleanLiteral(false),
                    span: self.span_from(start),
                })
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::NullLiteral,
                    span: self.span_from(start),
                })
            }
            TokenKind::BigInt(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::BigIntLiteral(s),
                    span: self.span_from(start),
                })
            }
            TokenKind::RegExp { pattern, flags } => {
                let pattern = pattern.clone();
                let flags = flags.clone();
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::RegExpLiteral { pattern, flags },
                    span: self.span_from(start),
                })
            }
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                if self.eat(&TokenKind::Arrow) {
                    let params = vec![Pattern {
                        kind: PatternKind::Identifier {
                            name,
                            type_annotation: None,
                        },
                        span: self.span_from(start),
                    }];
                    return self.finish_arrow_function(params, false, start);
                }
                Ok(Expression {
                    kind: ExpressionKind::Identifier(name),
                    span: self.span_from(start),
                })
            }
            TokenKind::This => {
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::This,
                    span: self.span_from(start),
                })
            }
            TokenKind::Super => {
                self.advance();
                Ok(Expression {
                    kind: ExpressionKind::Super,
                    span: self.span_from(start),
                })
            }
            TokenKind::LParen => self.parse_arrow_function_or_group(),
            TokenKind::LBracket => self.parse_array_expression(),
            TokenKind::LBrace => self.parse_object_expression(),
            TokenKind::Function => self.parse_function_expression(),
            TokenKind::Async => {
                let cp = self.checkpoint();
                self.advance();
                if self.at(&TokenKind::Function) {
                    self.restore(cp);
                    self.parse_function_expression()
                } else if self.at(&TokenKind::LParen) {
                    self.advance();
                    if self.at(&TokenKind::RParen) {
                        self.advance();
                        self.skip_type_annotation();
                        if self.eat(&TokenKind::Arrow) {
                            return self.finish_arrow_function(vec![], true, start);
                        }
                        self.restore(cp);
                        Err(self.error("expected expression"))
                    } else {
                        let params = self.try_parse_arrow_params()?;
                        self.expect(&TokenKind::RParen)?;
                        self.skip_type_annotation();
                        self.expect(&TokenKind::Arrow)?;
                        self.finish_arrow_function(params, true, start)
                    }
                } else if self.at_identifier() {
                    let name = self.parse_identifier_name()?;
                    let param = Pattern {
                        kind: PatternKind::Identifier {
                            name,
                            type_annotation: None,
                        },
                        span: self.span_from(start),
                    };
                    self.expect(&TokenKind::Arrow)?;
                    self.finish_arrow_function(vec![param], true, start)
                } else {
                    self.restore(cp);
                    Err(self.error("expected expression"))
                }
            }
            TokenKind::Class => self.parse_class_expression(),
            TokenKind::New => self.parse_new_expression(),
            TokenKind::NoSubstitutionTemplate(_)
            | TokenKind::TemplateHead(_)
            | TokenKind::TemplateMiddle(_)
            | TokenKind::TemplateTail(_) => {
                let template = self.parse_template_literal()?;
                Ok(Expression {
                    kind: ExpressionKind::TemplateLiteral(template),
                    span: self.span_from(start),
                })
            }
            _ => Err(self.error("expected expression")),
        }
    }

    fn parse_new_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        self.expect(&TokenKind::New)?;
        let callee_expr = self.parse_call_expression()?;
        let span = self.span_from(start);
        let (callee, arguments) = match callee_expr.kind {
            ExpressionKind::CallExpression {
                callee,
                arguments,
                optional: false,
            } => (*callee, arguments),
            other => (
                Expression {
                    kind: other,
                    span: callee_expr.span,
                },
                Vec::new(),
            ),
        };
        Ok(Expression {
            kind: ExpressionKind::NewExpression {
                callee: Box::new(callee),
                arguments,
            },
            span,
        })
    }

    fn parse_array_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        self.expect(&TokenKind::LBracket)?;
        let mut elements = Vec::new();
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
                let arg = self.parse_assignment_expression()?;
                elements.push(Some(Expression {
                    kind: ExpressionKind::SpreadElement(Box::new(arg)),
                    span: self.span_from(start),
                }));
            } else {
                elements.push(Some(self.parse_assignment_expression()?));
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBracket)?;
        Ok(Expression {
            kind: ExpressionKind::ArrayExpression(elements),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_object_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        self.expect(&TokenKind::LBrace)?;
        let mut props = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            props.push(self.parse_object_property()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(Expression {
            kind: ExpressionKind::ObjectExpression(props),
            span: self.span_from(start),
        })
    }

    fn parse_object_property(&mut self) -> ParseResult<ObjectProperty> {
        let start = self.current.span.start;
        if self.at(&TokenKind::DotDotDot) {
            self.advance();
            let arg = self.parse_assignment_expression()?;
            return Ok(ObjectProperty {
                kind: ObjectPropertyKind::SpreadElement(arg),
                span: self.span_from(start),
            });
        }

        let key = self.parse_property_key()?;
        if self.eat(&TokenKind::Colon) {
            let computed = matches!(&key, PropertyKey::Computed(_));
            let value = self.parse_assignment_expression()?;
            return Ok(ObjectProperty {
                kind: ObjectPropertyKind::Property {
                    key,
                    value,
                    computed,
                    shorthand: false,
                },
                span: self.span_from(start),
            });
        }

        if self.at(&TokenKind::LParen) {
            let computed = matches!(&key, PropertyKey::Computed(_));
            let func = self.parse_method_function(None)?;
            return Ok(ObjectProperty {
                kind: ObjectPropertyKind::Method {
                    key,
                    value: func,
                    kind: MethodKind::Method,
                    computed,
                },
                span: self.span_from(start),
            });
        }

        let PropertyKey::Identifier(name) = key else {
            return Err(self.error("expected ':' after property key"));
        };
        Ok(ObjectProperty {
            kind: ObjectPropertyKind::Property {
                key: PropertyKey::Identifier(name.clone()),
                value: Expression {
                    kind: ExpressionKind::Identifier(name),
                    span: self.span_from(start),
                },
                computed: false,
                shorthand: true,
            },
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_property_key(&mut self) -> ParseResult<PropertyKey> {
        match &self.current.kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(PropertyKey::Identifier(name))
            }
            TokenKind::String(s) => {
                let s = s.clone();
                self.advance();
                Ok(PropertyKey::String(s))
            }
            TokenKind::Number(n) => {
                let n = *n;
                self.advance();
                Ok(PropertyKey::Number(n))
            }
            TokenKind::LBracket => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&TokenKind::RBracket)?;
                Ok(PropertyKey::Computed(Box::new(expr)))
            }
            _ => Err(self.error("expected property key")),
        }
    }

    pub(super) fn parse_arguments(&mut self) -> ParseResult<Vec<Expression>> {
        let mut args = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at_eof() {
            if self.at(&TokenKind::DotDotDot) {
                self.advance();
                let arg = self.parse_assignment_expression()?;
                let span = arg.span;
                args.push(Expression {
                    kind: ExpressionKind::SpreadElement(Box::new(arg)),
                    span,
                });
            } else {
                args.push(self.parse_assignment_expression()?);
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    fn parse_function_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        let is_async = self.eat(&TokenKind::Async);
        self.expect(&TokenKind::Function)?;
        let id = if self.at_identifier() {
            Some(self.parse_identifier_name()?)
        } else {
            None
        };
        self.skip_generic_params();
        let func = self.parse_function_after_name(id, is_async, false, start)?;
        Ok(Expression {
            kind: ExpressionKind::FunctionExpression(func),
            span: self.span_from(start),
        })
    }

    fn parse_class_expression(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        let class = self.parse_class_tail(None, start)?;
        Ok(Expression {
            kind: ExpressionKind::ClassExpression(class),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_arrow_function_or_group(&mut self) -> ParseResult<Expression> {
        let start = self.current.span.start;
        self.expect(&TokenKind::LParen)?;

        if self.at(&TokenKind::RParen) {
            self.advance();
            self.skip_type_annotation();
            if self.eat(&TokenKind::Arrow) {
                return self.finish_arrow_function(vec![], false, start);
            }
            return Err(self.error("expected expression"));
        }

        if self.is_typescript() {
            let checkpoint = self.checkpoint();
            if let Ok(params) = self.try_parse_arrow_params()
                && self.eat(&TokenKind::RParen)
            {
                self.skip_type_annotation();
                if self.eat(&TokenKind::Arrow) {
                    return self.finish_arrow_function(params, false, start);
                }
            }
            self.restore(checkpoint);
        }

        let expr = self.parse_expression()?;
        self.expect(&TokenKind::RParen)?;
        if self.eat(&TokenKind::Arrow) {
            let params = expression_to_arrow_params(expr)?;
            return self.finish_arrow_function(params, false, start);
        }
        Ok(expr)
    }

    fn try_parse_arrow_params(&mut self) -> ParseResult<Vec<Pattern>> {
        let mut params = vec![self.parse_pattern()?];
        loop {
            if self.at(&TokenKind::RParen) {
                return Ok(params);
            }
            if !self.eat(&TokenKind::Comma) {
                return Err(self.error("expected ',' or ')' in arrow parameter list"));
            }
            if self.at(&TokenKind::RParen) {
                return Ok(params);
            }
            params.push(self.parse_pattern()?);
        }
    }

    fn finish_arrow_function(
        &mut self,
        params: Vec<Pattern>,
        is_async: bool,
        start: BytePos,
    ) -> ParseResult<Expression> {
        let body = if self.at(&TokenKind::LBrace) {
            if self.is_lazy() {
                self.parse_function_block_body()?
            } else {
                self.advance();
                let stmts = self.parse_statement_list()?;
                self.expect(&TokenKind::RBrace)?;
                FunctionBody::Block(stmts)
            }
        } else {
            let expr = self.parse_assignment_expression()?;
            FunctionBody::Expression(Box::new(expr))
        };
        Ok(Expression {
            kind: ExpressionKind::ArrowFunctionExpression(ArrowFunction {
                params,
                body,
                is_async,
                span: self.span_from(start),
            }),
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_template_literal(&mut self) -> ParseResult<TemplateLiteral> {
        let start = self.current.span.start;
        let mut quasis = Vec::new();
        let mut expressions = Vec::new();

        match &self.current.kind {
            TokenKind::NoSubstitutionTemplate(s) => {
                let s = s.clone();
                self.advance();
                quasis.push(TemplateElement {
                    value: s,
                    tail: true,
                    span: self.span_from(start),
                });
            }
            TokenKind::TemplateHead(s) => {
                let s = s.clone();
                self.advance();
                quasis.push(TemplateElement {
                    value: s,
                    tail: false,
                    span: self.span_from(start),
                });
                loop {
                    expressions.push(self.parse_expression()?);
                    if !self.at(&TokenKind::RBrace) {
                        return Err(self.error("expected '}' after template expression"));
                    }
                    self.lexer.consume_closing_template_brace();
                    let quasi_start = self.lexer.position() as u32;
                    let part = self.lexer.scan_template_part();
                    match part {
                        TokenKind::TemplateMiddle(s) => {
                            quasis.push(TemplateElement {
                                value: s,
                                tail: false,
                                span: self.span_from(quasi_start),
                            });
                            let next = self.lexer.next_token();
                            self.previous_end = next.span.end;
                            self.current = next;
                        }
                        TokenKind::TemplateTail(s) => {
                            quasis.push(TemplateElement {
                                value: s,
                                tail: true,
                                span: self.span_from(quasi_start),
                            });
                            let next = self.lexer.next_token();
                            self.previous_end = next.span.end;
                            self.current = next;
                            break;
                        }
                        _ => return Err(self.error("expected template continuation")),
                    }
                }
            }
            _ => return Err(self.error("expected template literal")),
        }

        Ok(TemplateLiteral {
            quasis,
            expressions,
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_function_after_name(
        &mut self,
        id: Option<String>,
        is_async: bool,
        is_generator: bool,
        start: BytePos,
    ) -> ParseResult<Function> {
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_function_params()?;
        self.expect(&TokenKind::RParen)?;
        self.skip_type_annotation();
        let body = self.parse_function_block_body()?;
        Ok(Function {
            id,
            params,
            body,
            is_async,
            is_generator,
            span: self.span_from(start),
        })
    }

    pub(super) fn parse_method_function(
        &mut self,
        id: Option<String>,
    ) -> ParseResult<Function> {
        let start = self.current.span.start;
        let is_async = self.eat(&TokenKind::Async);
        let is_generator = self.eat_generator_star();
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_function_params()?;
        self.expect(&TokenKind::RParen)?;
        let body = self.parse_function_block_body()?;
        Ok(Function {
            id,
            params,
            body,
            is_async,
            is_generator,
            span: self.span_from(start),
        })
    }

    fn eat_generator_star(&mut self) -> bool {
        if self.at(&TokenKind::Star) {
            self.advance();
            true
        } else {
            false
        }
    }
}

fn binary_precedence(op: &TokenKind) -> Option<u8> {
    match op {
        TokenKind::NullishCoalescing => Some(4),
        TokenKind::Or => Some(5),
        TokenKind::And => Some(6),
        TokenKind::BitOr => Some(7),
        TokenKind::BitXor => Some(8),
        TokenKind::BitAnd => Some(9),
        TokenKind::Eq | TokenKind::NotEq | TokenKind::StrictEq | TokenKind::StrictNotEq => Some(10),
        TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::LtEq
        | TokenKind::GtEq
        | TokenKind::Instanceof
        | TokenKind::In => Some(11),
        TokenKind::Shl | TokenKind::Shr | TokenKind::UShr => Some(12),
        TokenKind::Plus | TokenKind::Minus => Some(13),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some(14),
        TokenKind::StarStar => Some(15),
        _ => None,
    }
}

fn make_binary_or_logical(op: TokenKind, left: Expression, right: Expression) -> ExpressionKind {
    match op {
        TokenKind::Or => ExpressionKind::LogicalExpression {
            operator: LogicalOp::Or,
            left: Box::new(left),
            right: Box::new(right),
        },
        TokenKind::And => ExpressionKind::LogicalExpression {
            operator: LogicalOp::And,
            left: Box::new(left),
            right: Box::new(right),
        },
        _ => ExpressionKind::BinaryExpression {
            operator: token_to_binary_op(op),
            left: Box::new(left),
            right: Box::new(right),
        },
    }
}

fn token_to_binary_op(op: TokenKind) -> BinaryOp {
    match op {
        TokenKind::Plus => BinaryOp::Add,
        TokenKind::Minus => BinaryOp::Sub,
        TokenKind::Star => BinaryOp::Mul,
        TokenKind::Slash => BinaryOp::Div,
        TokenKind::Percent => BinaryOp::Mod,
        TokenKind::StarStar => BinaryOp::Exp,
        TokenKind::Eq => BinaryOp::Eq,
        TokenKind::NotEq => BinaryOp::NotEq,
        TokenKind::StrictEq => BinaryOp::StrictEq,
        TokenKind::StrictNotEq => BinaryOp::StrictNotEq,
        TokenKind::Lt => BinaryOp::Lt,
        TokenKind::Gt => BinaryOp::Gt,
        TokenKind::LtEq => BinaryOp::LtEq,
        TokenKind::GtEq => BinaryOp::GtEq,
        TokenKind::Shl => BinaryOp::Shl,
        TokenKind::Shr => BinaryOp::Shr,
        TokenKind::UShr => BinaryOp::UShr,
        TokenKind::BitAnd => BinaryOp::BitAnd,
        TokenKind::BitOr => BinaryOp::BitOr,
        TokenKind::BitXor => BinaryOp::BitXor,
        TokenKind::In => BinaryOp::In,
        TokenKind::Instanceof => BinaryOp::Instanceof,
        TokenKind::NullishCoalescing => BinaryOp::NullishCoalescing,
        _ => BinaryOp::Add,
    }
}

fn token_to_assign_op(op: TokenKind) -> AssignOp {
    match op {
        TokenKind::Assign => AssignOp::Assign,
        TokenKind::PlusAssign => AssignOp::AddAssign,
        TokenKind::MinusAssign => AssignOp::SubAssign,
        TokenKind::StarAssign => AssignOp::MulAssign,
        TokenKind::SlashAssign => AssignOp::DivAssign,
        TokenKind::PercentAssign => AssignOp::ModAssign,
        TokenKind::StarStarAssign => AssignOp::ExpAssign,
        TokenKind::ShlAssign => AssignOp::ShlAssign,
        TokenKind::ShrAssign => AssignOp::ShrAssign,
        TokenKind::UShrAssign => AssignOp::UShrAssign,
        TokenKind::BitAndAssign => AssignOp::BitAndAssign,
        TokenKind::BitOrAssign => AssignOp::BitOrAssign,
        TokenKind::BitXorAssign => AssignOp::BitXorAssign,
        TokenKind::AndAssign => AssignOp::AndAssign,
        TokenKind::OrAssign => AssignOp::OrAssign,
        TokenKind::NullishAssign => AssignOp::NullishAssign,
        _ => AssignOp::Assign,
    }
}

fn expression_to_assign_target(expr: Expression) -> ParseResult<AssignTarget> {
    match expr.kind {
        ExpressionKind::Identifier(name) => Ok(AssignTarget::Identifier(name)),
        ExpressionKind::MemberExpression {
            object,
            property,
            computed,
            optional,
        } => {
            if optional {
                return Err(ParseError {
                    message: "invalid assignment target".into(),
                    span: expr.span,
                });
            }
            Ok(AssignTarget::Member(Box::new(Expression {
                kind: ExpressionKind::MemberExpression {
                    object,
                    property,
                    computed,
                    optional: false,
                },
                span: expr.span,
            })))
        }
        _ => Err(ParseError {
            message: "invalid assignment target".into(),
            span: expr.span,
        }),
    }
}

fn expression_to_arrow_params(expr: Expression) -> ParseResult<Vec<Pattern>> {
    match expr.kind {
        ExpressionKind::Identifier(name) => Ok(vec![Pattern {
            kind: PatternKind::Identifier {
                name,
                type_annotation: None,
            },
            span: expr.span,
        }]),
        ExpressionKind::SequenceExpression(exprs) => {
            let mut params = Vec::with_capacity(exprs.len());
            for e in exprs {
                match e.kind {
                    ExpressionKind::Identifier(name) => {
                        params.push(Pattern {
                            kind: PatternKind::Identifier {
                                name,
                                type_annotation: None,
                            },
                            span: e.span,
                        });
                    }
                    _ => {
                        return Err(ParseError {
                            message: "invalid arrow function parameter".into(),
                            span: e.span,
                        });
                    }
                }
            }
            Ok(params)
        }
        _ => Err(ParseError {
            message: "invalid arrow function parameter".into(),
            span: expr.span,
        }),
    }
}
