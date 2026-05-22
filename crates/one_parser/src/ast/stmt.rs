use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    ExpressionStatement(super::Expression),

    BlockStatement(Vec<Statement>),

    EmptyStatement,

    IfStatement {
        test: super::Expression,
        consequent: Box<Statement>,
        alternate: Option<Box<Statement>>,
    },
    SwitchStatement {
        discriminant: super::Expression,
        cases: Vec<SwitchCase>,
    },

    WhileStatement {
        test: super::Expression,
        body: Box<Statement>,
    },
    DoWhileStatement {
        test: super::Expression,
        body: Box<Statement>,
    },
    ForStatement {
        init: Option<ForInit>,
        test: Option<super::Expression>,
        update: Option<super::Expression>,
        body: Box<Statement>,
    },
    ForInStatement {
        left: ForInOfLeft,
        right: super::Expression,
        body: Box<Statement>,
    },
    ForOfStatement {
        left: ForInOfLeft,
        right: super::Expression,
        body: Box<Statement>,
        is_await: bool,
    },

    ReturnStatement(Option<super::Expression>),
    BreakStatement(Option<String>),
    ContinueStatement(Option<String>),
    ThrowStatement(super::Expression),

    TryStatement {
        block: Vec<Statement>,
        handler: Option<CatchClause>,
        finalizer: Option<Vec<Statement>>,
    },

    LabeledStatement {
        label: String,
        body: Box<Statement>,
    },

    WithStatement {
        object: super::Expression,
        body: Box<Statement>,
    },

    DebuggerStatement,

    Declaration(super::Declaration),
}

#[derive(Debug, Clone)]
pub struct SwitchCase {
    pub test: Option<super::Expression>,
    pub consequent: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ForInit {
    Expression(super::Expression),
    Declaration(super::Declaration),
}

#[derive(Debug, Clone)]
pub enum ForInOfLeft {
    Declaration(super::Declaration),
    Pattern(super::Pattern),
    Expression(super::Expression),
}

#[derive(Debug, Clone)]
pub struct CatchClause {
    pub param: Option<super::Pattern>,
    pub body: Vec<Statement>,
    pub span: Span,
}
