mod expr;
mod stmt;

use crate::ast::*;
use crate::lexer::Lexer;
use crate::span::{BytePos, Span};
use crate::token::{Token, TokenKind};

pub struct Parser<'a> {
    pub(super) lexer: Lexer<'a>,
    pub(super) current: Token,
    pub(super) previous_end: BytePos,
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

pub type ParseResult<T> = Result<T, ParseError>;

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token();
        let previous_end = current.span.start;
        Self {
            lexer,
            current,
            previous_end,
        }
    }

    pub fn parse(source: &'a str) -> ParseResult<Program> {
        let mut parser = Self::new(source);
        parser.parse_program()
    }

    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let start = self.current.span.start;
        let mut body = Vec::new();
        while !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        Ok(Program {
            body,
            source_type: SourceType::Script,
            span: self.span_from(start),
        })
    }

    pub(super) fn at(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(kind)
    }

    pub(super) fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub(super) fn expect(&mut self, kind: &TokenKind) -> ParseResult<Token> {
        if self.at(kind) {
            Ok(self.advance())
        } else {
            Err(self.error(&format!("expected {kind:?}")))
        }
    }

    pub(super) fn advance(&mut self) -> Token {
        let prev = std::mem::replace(&mut self.current, self.lexer.next_token());
        self.previous_end = prev.span.end;
        prev
    }

    pub(super) fn peek(&self) -> &TokenKind {
        &self.current.kind
    }

    pub(super) fn error(&self, msg: &str) -> ParseError {
        ParseError {
            message: msg.to_string(),
            span: self.current.span,
        }
    }

    pub(super) fn span_from(&self, start: BytePos) -> Span {
        Span::new(start, self.previous_end)
    }

    pub(super) fn at_eof(&self) -> bool {
        matches!(self.current.kind, TokenKind::Eof)
    }

    pub(super) fn at_identifier(&self) -> bool {
        matches!(self.current.kind, TokenKind::Identifier(_))
    }

    pub(super) fn parse_identifier_name(&mut self) -> ParseResult<String> {
        match &self.current.kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => Err(self.error("expected identifier")),
        }
    }

    pub(super) fn parse_statement_list(&mut self) -> ParseResult<Vec<Statement>> {
        let mut stmts = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at_eof() {
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        Parser::parse(src).expect("parse failed")
    }

    fn parse_expr(src: &str) -> Expression {
        let prog = parse(src);
        match &prog.body[0].kind {
            StatementKind::ExpressionStatement(expr) => expr.clone(),
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn parse_number() {
        let expr = parse_expr("42;");
        assert!(matches!(expr.kind, ExpressionKind::NumberLiteral(n) if n == 42.0));
    }

    #[test]
    fn parse_string() {
        let expr = parse_expr(r#""hello";"#);
        assert!(matches!(&expr.kind, ExpressionKind::StringLiteral(s) if s == "hello"));
    }

    #[test]
    fn parse_binary_add() {
        let expr = parse_expr("1 + 2;");
        match &expr.kind {
            ExpressionKind::BinaryExpression {
                operator: BinaryOp::Add,
                left,
                right,
            } => {
                assert!(matches!(left.kind, ExpressionKind::NumberLiteral(n) if n == 1.0));
                assert!(matches!(right.kind, ExpressionKind::NumberLiteral(n) if n == 2.0));
            }
            _ => panic!("expected binary add"),
        }
    }

    #[test]
    fn parse_precedence() {
        let expr = parse_expr("1 + 2 * 3;");
        match &expr.kind {
            ExpressionKind::BinaryExpression {
                operator: BinaryOp::Add,
                left,
                right,
            } => {
                assert!(matches!(left.kind, ExpressionKind::NumberLiteral(n) if n == 1.0));
                assert!(matches!(
                    &right.kind,
                    ExpressionKind::BinaryExpression {
                        operator: BinaryOp::Mul,
                        ..
                    }
                ));
            }
            _ => panic!("expected add(1, mul(2, 3))"),
        }
    }

    #[test]
    fn parse_unary() {
        let expr = parse_expr("-x;");
        assert!(matches!(
            &expr.kind,
            ExpressionKind::UnaryExpression {
                operator: UnaryOp::Minus,
                prefix: true,
                ..
            }
        ));
    }

    #[test]
    fn parse_member_expression() {
        let expr = parse_expr("a.b.c;");
        match &expr.kind {
            ExpressionKind::MemberExpression {
                object,
                property: MemberProperty::Identifier(name),
                ..
            } => {
                assert_eq!(name, "c");
                assert!(matches!(&object.kind, ExpressionKind::MemberExpression { .. }));
            }
            _ => panic!("expected member expression"),
        }
    }

    #[test]
    fn parse_call_expression() {
        let expr = parse_expr("foo(1, 2);");
        match &expr.kind {
            ExpressionKind::CallExpression {
                callee,
                arguments,
                ..
            } => {
                assert!(matches!(
                    &callee.kind,
                    ExpressionKind::Identifier(name) if name == "foo"
                ));
                assert_eq!(arguments.len(), 2);
            }
            _ => panic!("expected call expression"),
        }
    }

    #[test]
    fn parse_console_log() {
        let expr = parse_expr(r#"console.log("Hello World");"#);
        match &expr.kind {
            ExpressionKind::CallExpression {
                callee,
                arguments,
                ..
            } => {
                assert!(matches!(
                    &callee.kind,
                    ExpressionKind::MemberExpression { .. }
                ));
                assert_eq!(arguments.len(), 1);
                assert!(matches!(
                    &arguments[0].kind,
                    ExpressionKind::StringLiteral(s) if s == "Hello World"
                ));
            }
            _ => panic!("expected call expression"),
        }
    }

    #[test]
    fn parse_let_declaration() {
        let prog = parse("let x = 42;");
        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::VariableDeclaration {
                    kind: VarKind::Let,
                    declarations,
                } => {
                    assert_eq!(declarations.len(), 1);
                }
                _ => panic!("expected variable declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn parse_if_statement() {
        let prog = parse("if (x) { y; }");
        assert!(matches!(&prog.body[0].kind, StatementKind::IfStatement { .. }));
    }

    #[test]
    fn parse_if_else() {
        let prog = parse("if (x) { y; } else { z; }");
        match &prog.body[0].kind {
            StatementKind::IfStatement { alternate, .. } => {
                assert!(alternate.is_some());
            }
            _ => panic!("expected if statement"),
        }
    }

    #[test]
    fn parse_while_loop() {
        let prog = parse("while (true) { x; }");
        assert!(matches!(
            &prog.body[0].kind,
            StatementKind::WhileStatement { .. }
        ));
    }

    #[test]
    fn parse_for_loop() {
        let prog = parse("for (let i = 0; i < 10; i++) { x; }");
        assert!(matches!(
            &prog.body[0].kind,
            StatementKind::ForStatement { .. }
        ));
    }

    #[test]
    fn parse_function_declaration() {
        let prog = parse("function add(a, b) { return a + b; }");
        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::FunctionDeclaration(func) => {
                    assert_eq!(func.id.as_deref(), Some("add"));
                    assert_eq!(func.params.len(), 2);
                }
                _ => panic!("expected function declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn parse_return_statement() {
        let prog = parse("return 42;");
        match &prog.body[0].kind {
            StatementKind::ReturnStatement(Some(expr)) => {
                assert!(matches!(expr.kind, ExpressionKind::NumberLiteral(n) if n == 42.0));
            }
            _ => panic!("expected return statement"),
        }
    }

    #[test]
    fn parse_array_literal() {
        let expr = parse_expr("[1, 2, 3];");
        match &expr.kind {
            ExpressionKind::ArrayExpression(elements) => {
                assert_eq!(elements.len(), 3);
            }
            _ => panic!("expected array expression"),
        }
    }

    #[test]
    fn parse_object_literal() {
        let expr = parse_expr("({ a: 1, b: 2 });");
        match &expr.kind {
            ExpressionKind::ObjectExpression(props) => {
                assert_eq!(props.len(), 2);
            }
            _ => panic!("expected object expression"),
        }
    }

    #[test]
    fn parse_ternary() {
        let expr = parse_expr("x ? 1 : 2;");
        assert!(matches!(
            &expr.kind,
            ExpressionKind::ConditionalExpression { .. }
        ));
    }

    #[test]
    fn parse_arrow_function() {
        let expr = parse_expr("(x) => x + 1;");
        assert!(matches!(
            &expr.kind,
            ExpressionKind::ArrowFunctionExpression(_)
        ));
    }

    #[test]
    fn parse_try_catch() {
        let prog = parse("try { x; } catch (e) { y; }");
        assert!(matches!(
            &prog.body[0].kind,
            StatementKind::TryStatement { .. }
        ));
    }

    #[test]
    fn parse_class_declaration() {
        let prog = parse("class Foo { constructor() {} method() {} }");
        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::ClassDeclaration(class) => {
                    assert_eq!(class.id.as_deref(), Some("Foo"));
                    assert_eq!(class.body.len(), 2);
                }
                _ => panic!("expected class declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn parse_multiple_statements() {
        let prog = parse("let x = 1; let y = 2; x + y;");
        assert_eq!(prog.body.len(), 3);
    }

    #[test]
    fn parse_complex_expression() {
        let _prog = parse("foo.bar(1 + 2, [3, 4], { a: 5 });");
    }
}
