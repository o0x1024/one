use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExpressionKind {
    // Literals
    NumberLiteral(f64),
    StringLiteral(String),
    BooleanLiteral(bool),
    NullLiteral,
    BigIntLiteral(String),
    RegExpLiteral { pattern: String, flags: String },
    TemplateLiteral(TemplateLiteral),

    // Identifiers
    Identifier(String),
    This,
    Super,

    // Arrays and Objects
    ArrayExpression(Vec<Option<Expression>>),
    ObjectExpression(Vec<ObjectProperty>),

    // Unary
    UnaryExpression {
        operator: UnaryOp,
        argument: Box<Expression>,
        prefix: bool,
    },
    UpdateExpression {
        operator: UpdateOp,
        argument: Box<Expression>,
        prefix: bool,
    },

    // Binary
    BinaryExpression {
        operator: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    LogicalExpression {
        operator: LogicalOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },

    // Assignment
    AssignmentExpression {
        operator: AssignOp,
        left: Box<AssignTarget>,
        right: Box<Expression>,
    },

    // Member access
    MemberExpression {
        object: Box<Expression>,
        property: MemberProperty,
        computed: bool,
        optional: bool,
    },

    // Call / New
    CallExpression {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
        optional: bool,
    },
    NewExpression {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    TaggedTemplateExpression {
        tag: Box<Expression>,
        quasi: TemplateLiteral,
    },

    // Ternary
    ConditionalExpression {
        test: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
    },

    // Arrow / Function expression
    ArrowFunctionExpression(ArrowFunction),
    FunctionExpression(Function),
    ClassExpression(Class),

    // Sequence
    SequenceExpression(Vec<Expression>),

    // Spread
    SpreadElement(Box<Expression>),

    // Yield / Await
    YieldExpression {
        argument: Option<Box<Expression>>,
        delegate: bool,
    },
    AwaitExpression(Box<Expression>),

    // Import
    MetaProperty { meta: String, property: String },
    ImportExpression(Box<Expression>),

    // Parenthesized (for preserving source info)
    ParenthesizedExpression(Box<Expression>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Minus,
    Plus,
    Not,
    BitNot,
    Typeof,
    Void,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdateOp {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Exp,
    Eq,
    NotEq,
    StrictEq,
    StrictNotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Shl,
    Shr,
    UShr,
    BitAnd,
    BitOr,
    BitXor,
    In,
    Instanceof,
    NullishCoalescing,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    ExpAssign,
    ShlAssign,
    ShrAssign,
    UShrAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    AndAssign,
    OrAssign,
    NullishAssign,
}

#[derive(Debug, Clone)]
pub enum MemberProperty {
    Identifier(String),
    Expression(Box<Expression>),
    PrivateIdentifier(String),
}

#[derive(Debug, Clone)]
pub enum AssignTarget {
    Identifier(String),
    Member(Box<Expression>),
    Pattern(super::Pattern),
}

#[derive(Debug, Clone)]
pub struct ObjectProperty {
    pub kind: ObjectPropertyKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ObjectPropertyKind {
    Property {
        key: super::PropertyKey,
        value: Expression,
        computed: bool,
        shorthand: bool,
    },
    Method {
        key: super::PropertyKey,
        value: Function,
        kind: MethodKind,
        computed: bool,
    },
    SpreadElement(Expression),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MethodKind {
    Method,
    Get,
    Set,
}

#[derive(Debug, Clone)]
pub struct TemplateLiteral {
    pub quasis: Vec<TemplateElement>,
    pub expressions: Vec<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TemplateElement {
    pub value: String,
    pub tail: bool,
    pub span: Span,
}

/// Shared function structure (used by FunctionDeclaration, FunctionExpression, ArrowFunction, Method)
#[derive(Debug, Clone)]
pub struct Function {
    pub id: Option<String>,
    pub params: Vec<super::Pattern>,
    pub body: FunctionBody,
    pub is_async: bool,
    pub is_generator: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum FunctionBody {
    Block(Vec<super::Statement>),
    Expression(Box<Expression>),
    /// Lazy: function body was pre-scanned but not yet parsed.
    /// Contains the source range for deferred parsing.
    Lazy(LazyFunctionBody),
}

#[derive(Debug, Clone)]
pub struct LazyFunctionBody {
    /// Byte range of the function body in source (including braces)
    pub source_start: u32,
    pub source_end: u32,
    /// Pre-scan metadata
    pub has_eval: bool,
    pub has_arguments: bool,
    pub has_with: bool,
    pub is_strict: bool,
}

#[derive(Debug, Clone)]
pub struct ArrowFunction {
    pub params: Vec<super::Pattern>,
    pub body: FunctionBody,
    pub is_async: bool,
    pub span: Span,
}

/// Shared class structure
#[derive(Debug, Clone)]
pub struct Class {
    pub id: Option<String>,
    pub super_class: Option<Box<Expression>>,
    pub body: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassMember {
    pub kind: ClassMemberKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassMemberKind {
    Method {
        key: super::PropertyKey,
        value: Function,
        kind: MethodKind,
        is_static: bool,
        computed: bool,
    },
    Property {
        key: super::PropertyKey,
        value: Option<Expression>,
        is_static: bool,
        computed: bool,
    },
    StaticBlock(Vec<super::Statement>),
}
