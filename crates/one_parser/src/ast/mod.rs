use crate::span::Span;

pub mod decl;
pub mod expr;
pub mod pat;
pub mod stmt;

pub use decl::*;
pub use expr::*;
pub use pat::*;
pub use stmt::*;

/// Root AST node
#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Statement>,
    pub source_type: SourceType,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SourceType {
    Script,
    Module,
}

/// Property key (for object literals, class members)
#[derive(Debug, Clone)]
pub enum PropertyKey {
    Identifier(String),
    String(String),
    Number(f64),
    Computed(Box<Expression>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    #[test]
    fn can_construct_simple_program() {
        let program = Program {
            body: vec![Statement {
                kind: StatementKind::ExpressionStatement(Expression {
                    kind: ExpressionKind::NumberLiteral(42.0),
                    span: Span::new(0, 2),
                }),
                span: Span::new(0, 3),
            }],
            source_type: SourceType::Script,
            span: Span::new(0, 3),
        };
        assert_eq!(program.body.len(), 1);
    }

    #[test]
    fn can_construct_variable_declaration() {
        let decl = Declaration {
            kind: DeclarationKind::VariableDeclaration {
                kind: VarKind::Let,
                declarations: vec![VariableDeclarator {
                    id: Pattern {
                        kind: PatternKind::Identifier {
                            name: "x".into(),
                            type_annotation: None,
                        },
                        span: Span::new(4, 5),
                    },
                    init: Some(Expression {
                        kind: ExpressionKind::NumberLiteral(42.0),
                        span: Span::new(8, 10),
                    }),
                    span: Span::new(4, 10),
                }],
            },
            span: Span::new(0, 11),
        };
        match &decl.kind {
            DeclarationKind::VariableDeclaration { kind, declarations } => {
                assert_eq!(*kind, VarKind::Let);
                assert_eq!(declarations.len(), 1);
            }
            _ => panic!("wrong kind"),
        }
    }

    #[test]
    fn can_construct_function() {
        let func = Function {
            id: Some("add".into()),
            params: vec![
                Pattern {
                    kind: PatternKind::Identifier {
                        name: "a".into(),
                        type_annotation: None,
                    },
                    span: Span::empty(),
                },
                Pattern {
                    kind: PatternKind::Identifier {
                        name: "b".into(),
                        type_annotation: None,
                    },
                    span: Span::empty(),
                },
            ],
            body: FunctionBody::Block(vec![]),
            is_async: false,
            is_generator: false,
            span: Span::empty(),
        };
        assert_eq!(func.id.as_deref(), Some("add"));
        assert_eq!(func.params.len(), 2);
    }

    #[test]
    fn can_construct_binary_expression() {
        let expr = Expression {
            kind: ExpressionKind::BinaryExpression {
                operator: BinaryOp::Add,
                left: Box::new(Expression {
                    kind: ExpressionKind::NumberLiteral(1.0),
                    span: Span::empty(),
                }),
                right: Box::new(Expression {
                    kind: ExpressionKind::NumberLiteral(2.0),
                    span: Span::empty(),
                }),
            },
            span: Span::empty(),
        };
        assert!(matches!(
            expr.kind,
            ExpressionKind::BinaryExpression {
                operator: BinaryOp::Add,
                ..
            }
        ));
    }
}
