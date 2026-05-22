mod expr;
mod module;
mod prescan;
mod stmt;
mod typescript;

use crate::ast::*;
use crate::lexer::Lexer;
use crate::span::{BytePos, Span};
use crate::token::{Token, TokenKind};

#[derive(Debug, Clone)]
pub struct ParserConfig {
    pub lazy: bool,
    pub typescript: bool,
    pub jsx: bool,
    pub source_type: SourceType,
}

impl Default for ParserConfig {
    fn default() -> Self {
        ParserConfig {
            lazy: false,
            typescript: true,
            jsx: false,
            source_type: SourceType::Script,
        }
    }
}

pub struct Parser<'a> {
    pub(super) lexer: Lexer<'a>,
    pub(super) current: Token,
    pub(super) previous_end: BytePos,
    lazy: bool,
    typescript: bool,
    #[allow(dead_code)]
    source: &'a str,
    source_type: SourceType,
}

pub(super) struct ParserCheckpoint {
    pos: usize,
    current: Token,
    previous_end: BytePos,
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

pub type ParseResult<T> = Result<T, ParseError>;

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self::new_with_config(source, ParserConfig::default())
    }

    pub fn new_with_config(source: &'a str, config: ParserConfig) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token();
        let previous_end = current.span.start;
        Self {
            lexer,
            current,
            previous_end,
            lazy: config.lazy,
            typescript: config.typescript,
            source,
            source_type: config.source_type,
        }
    }

    fn new_inner(source: &'a str, lazy: bool) -> Self {
        Self::new_with_config(
            source,
            ParserConfig {
                lazy,
                ..ParserConfig::default()
            },
        )
    }

    pub fn parse(source: &str) -> ParseResult<Program> {
        Self::parse_with_config(source, ParserConfig::default())
    }

    pub fn parse_module(source: &str) -> ParseResult<Program> {
        Self::parse_with_config(
            source,
            ParserConfig {
                source_type: SourceType::Module,
                ..ParserConfig::default()
            },
        )
    }

    pub fn parse_with_config(source: &str, config: ParserConfig) -> ParseResult<Program> {
        let mut parser = Parser::new_with_config(source, config);
        parser.parse_program()
    }

    /// Parse a previously pre-scanned function body.
    /// Called when the function is first invoked at runtime.
    pub fn parse_lazy_function(
        source: &str,
        lazy: &LazyFunctionBody,
    ) -> ParseResult<Vec<Statement>> {
        let body_source = &source[lazy.source_start as usize..lazy.source_end as usize];
        let mut parser = Parser::new_inner(body_source, false);
        parser.parse_statement_list()
    }

    pub(super) fn is_lazy(&self) -> bool {
        self.lazy
    }

    pub(super) fn is_module(&self) -> bool {
        self.source_type == SourceType::Module
    }

    pub(super) fn parse_function_block_body(&mut self) -> ParseResult<FunctionBody> {
        self.expect(&TokenKind::LBrace)?;
        if self.lazy {
            Ok(FunctionBody::Lazy(prescan::PreScanner::scan_function_body(
                self,
            )?))
        } else {
            let stmts = self.parse_statement_list()?;
            self.expect(&TokenKind::RBrace)?;
            Ok(FunctionBody::Block(stmts))
        }
    }

    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let start = self.current.span.start;
        let source_type = self.source_type;
        let mut body = Vec::new();
        while !self.at_eof() {
            body.push(self.parse_statement()?);
        }
        Ok(Program {
            body,
            source_type,
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

    pub(super) fn parse_property_name(&mut self) -> ParseResult<String> {
        match &self.current.kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            TokenKind::Catch => {
                self.advance();
                Ok("catch".to_string())
            }
            TokenKind::Delete => {
                self.advance();
                Ok("delete".to_string())
            }
            TokenKind::For => {
                self.advance();
                Ok("for".to_string())
            }
            _ => Err(self.error("expected identifier")),
        }
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

    pub(super) fn checkpoint(&self) -> ParserCheckpoint {
        ParserCheckpoint {
            pos: self.lexer.position(),
            current: self.current.clone(),
            previous_end: self.previous_end,
        }
    }

    pub(super) fn restore(&mut self, cp: ParserCheckpoint) {
        self.lexer.set_position(cp.pos);
        self.current = cp.current;
        self.previous_end = cp.previous_end;
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
    fn tokenize_template_interpolation() {
        use crate::lexer::Lexer;
        let mut lexer = Lexer::new(r#"return `hello ${name}`;"#);
        let mut kinds = Vec::new();
        loop {
            let tok = lexer.next_token();
            if tok.kind == TokenKind::Eof {
                break;
            }
            kinds.push(tok.kind);
        }
        assert!(
            matches!(
                kinds.as_slice(),
                [
                    TokenKind::Return,
                    TokenKind::TemplateHead(_),
                    TokenKind::Identifier(_),
                    TokenKind::RBrace,
                    ..
                ]
            ),
            "tokens: {kinds:?}"
        );
    }

    #[test]
    fn parse_template_interpolation() {
        let expr = parse_expr("`hello ${name}`;");
        assert!(matches!(&expr.kind, ExpressionKind::TemplateLiteral(_)));
    }

    #[test]
    fn parse_template_interpolation_program() {
        let prog = parse(r#"let name = "world"; return `hello ${name}`;"#);
        assert_eq!(prog.body.len(), 2);
    }

    #[test]
    fn parse_template_expression() {
        let expr = parse_expr("`result: ${5 * 2}`;");
        assert!(matches!(
            &expr.kind,
            ExpressionKind::TemplateLiteral(_)
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

    #[test]
    fn lazy_parse_skips_function_body() {
        let config = ParserConfig {
            lazy: true,
            ..Default::default()
        };
        let prog = Parser::parse_with_config("function foo() { return 42; }", config).unwrap();
        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::FunctionDeclaration(func) => {
                    assert!(matches!(&func.body, FunctionBody::Lazy(_)));
                }
                _ => panic!("expected function declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn lazy_parse_records_metadata() {
        let config = ParserConfig {
            lazy: true,
            ..Default::default()
        };
        let prog = Parser::parse_with_config(
            "function foo() { eval('x'); arguments[0]; }",
            config,
        )
        .unwrap();
        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::FunctionDeclaration(func) => match &func.body {
                    FunctionBody::Lazy(lazy) => {
                        assert!(lazy.has_eval);
                        assert!(lazy.has_arguments);
                        assert!(!lazy.has_with);
                    }
                    _ => panic!("expected lazy body"),
                },
                _ => panic!("expected function declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn lazy_body_can_be_reparsed() {
        let source = "function foo() { return 42; }";
        let config = ParserConfig {
            lazy: true,
            ..Default::default()
        };
        let prog = Parser::parse_with_config(source, config).unwrap();

        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::FunctionDeclaration(func) => match &func.body {
                    FunctionBody::Lazy(lazy) => {
                        let stmts = Parser::parse_lazy_function(source, lazy).unwrap();
                        assert_eq!(stmts.len(), 1);
                        assert!(matches!(
                            &stmts[0].kind,
                            StatementKind::ReturnStatement(_)
                        ));
                    }
                    _ => panic!("expected lazy body"),
                },
                _ => panic!("expected function declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn non_lazy_mode_fully_parses() {
        let prog = Parser::parse("function foo() { return 42; }").unwrap();
        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::FunctionDeclaration(func) => {
                    assert!(matches!(&func.body, FunctionBody::Block(_)));
                }
                _ => panic!("expected function declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn top_level_code_always_fully_parsed() {
        let config = ParserConfig {
            lazy: true,
            ..Default::default()
        };
        let prog = Parser::parse_with_config("let x = 1 + 2;", config).unwrap();
        assert_eq!(prog.body.len(), 1);
        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::VariableDeclaration { declarations, .. } => {
                    assert!(declarations[0].init.is_some());
                }
                _ => panic!("expected var decl"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn nested_braces_handled() {
        let config = ParserConfig {
            lazy: true,
            ..Default::default()
        };
        let prog = Parser::parse_with_config(
            "function foo() { if (true) { return { a: 1 }; } }",
            config,
        )
        .unwrap();
        match &prog.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::FunctionDeclaration(func) => match &func.body {
                    FunctionBody::Lazy(lazy) => {
                        assert!(lazy.source_end > lazy.source_start);
                    }
                    _ => panic!("expected lazy body"),
                },
                _ => panic!("expected function declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn parse_import_default() {
        let program = Parser::parse_module(r#"import foo from "module";"#).unwrap();
        assert!(!program.body.is_empty());
        match &program.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::ImportDeclaration { specifiers, source } => {
                    assert_eq!(source, "module");
                    assert_eq!(specifiers.len(), 1);
                    assert!(matches!(
                        &specifiers[0],
                        ImportSpecifier::Default { local, .. } if local == "foo"
                    ));
                }
                _ => panic!("expected import declaration"),
            },
            _ => panic!("expected declaration statement"),
        }
    }

    #[test]
    fn parse_import_named() {
        let program = Parser::parse_module(r#"import { a, b } from "module";"#).unwrap();
        assert!(!program.body.is_empty());
        match &program.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::ImportDeclaration { specifiers, .. } => {
                    assert_eq!(specifiers.len(), 2);
                }
                _ => panic!("expected import declaration"),
            },
            _ => panic!("expected declaration statement"),
        }
    }

    #[test]
    fn parse_export_let() {
        let program = Parser::parse_module("export let x = 42;").unwrap();
        assert!(!program.body.is_empty());
        match &program.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::ExportNamedDeclaration { declaration, .. } => {
                    assert!(declaration.is_some());
                }
                _ => panic!("expected export declaration"),
            },
            _ => panic!("expected declaration statement"),
        }
    }

    #[test]
    fn parse_export_default() {
        let program = Parser::parse_module("export default 42;").unwrap();
        assert!(!program.body.is_empty());
        match &program.body[0].kind {
            StatementKind::Declaration(decl) => {
                assert!(matches!(
                    &decl.kind,
                    DeclarationKind::ExportDefaultDeclaration(_)
                ));
            }
            _ => panic!("expected declaration statement"),
        }
    }

    #[test]
    fn parse_export_function() {
        let program =
            Parser::parse_module("export function hello() { return 1; }").unwrap();
        assert!(!program.body.is_empty());
        match &program.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::ExportNamedDeclaration { declaration, .. } => {
                    assert!(matches!(
                        declaration.as_deref(),
                        Some(Declaration {
                            kind: DeclarationKind::FunctionDeclaration(_),
                            ..
                        })
                    ));
                }
                _ => panic!("expected export declaration"),
            },
            _ => panic!("expected declaration statement"),
        }
    }

    #[test]
    fn ts_type_annotation() {
        let program = Parser::parse("let x: number = 42;").unwrap();
        assert!(!program.body.is_empty());
        match &program.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::VariableDeclaration { declarations, .. } => {
                    assert!(declarations[0].init.is_some());
                }
                _ => panic!("expected variable declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn ts_function_types() {
        let program = Parser::parse(
            "function add(a: number, b: number): number { return a + b; }",
        )
        .unwrap();
        assert!(!program.body.is_empty());
    }

    #[test]
    fn ts_interface_skip() {
        let program = Parser::parse("interface Foo { x: number; y: string; }").unwrap();
        assert_eq!(program.body.len(), 1);
        assert!(matches!(
            program.body[0].kind,
            StatementKind::EmptyStatement
        ));
    }

    #[test]
    fn ts_type_alias_skip() {
        let program = Parser::parse("type MyType = string | number;").unwrap();
        assert_eq!(program.body.len(), 1);
        assert!(matches!(
            program.body[0].kind,
            StatementKind::EmptyStatement
        ));
    }

    #[test]
    fn ts_generic_function() {
        let program =
            Parser::parse("function identity<T>(x: T): T { return x; }").unwrap();
        assert!(!program.body.is_empty());
    }

    #[test]
    fn ts_optional_param() {
        let program = Parser::parse("function f(x?: number) { return x; }").unwrap();
        assert!(!program.body.is_empty());
    }

    #[test]
    fn ts_as_expression() {
        let program = Parser::parse("let x = 42 as number;").unwrap();
        assert!(!program.body.is_empty());
    }

    #[test]
    fn ts_enum_declaration() {
        let program = Parser::parse("enum Color { Red, Green, Blue }").unwrap();
        assert_eq!(program.body.len(), 1);
        match &program.body[0].kind {
            StatementKind::Declaration(decl) => match &decl.kind {
                DeclarationKind::VariableDeclaration {
                    kind: VarKind::Const,
                    declarations,
                } => {
                    assert_eq!(declarations.len(), 1);
                    assert!(declarations[0].init.is_some());
                }
                _ => panic!("expected const declaration"),
            },
            _ => panic!("expected declaration"),
        }
    }

    #[test]
    fn ts_module_mode() {
        let program =
            Parser::parse_module("interface Foo { x: number; } export let x: number = 1;")
                .unwrap();
        assert_eq!(program.body.len(), 2);
    }
}
