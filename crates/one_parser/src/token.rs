use crate::span::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // === Literals ===
    Number(f64),
    BigInt(String),
    String(String),
    Template(TemplatePart),
    RegExp { pattern: String, flags: String },

    // === Identifiers ===
    Identifier(String),
    PrivateIdentifier(String), // #name

    // === Keywords ===
    // Declarations
    Var,
    Let,
    Const,
    Function,
    Class,
    // Control flow
    If,
    Else,
    Switch,
    Case,
    Default,
    For,
    While,
    Do,
    Break,
    Continue,
    Return,
    // Exception
    Try,
    Catch,
    Finally,
    Throw,
    // Operators as keywords
    New,
    Delete,
    Typeof,
    Void,
    In,
    Instanceof,
    Of,
    // Values
    This,
    Super,
    Null,
    True,
    False,
    // Modules
    Import,
    Export,
    From,
    As,
    // Async
    Async,
    Await,
    // Generator
    Yield,
    // Class
    Extends,
    Static,
    // Other
    Debugger,
    With,
    // TypeScript keywords (parsed in TS mode, treated as identifiers in JS mode)
    Type,
    Interface,
    Enum,
    Implements,
    Declare,
    Abstract,
    Readonly,
    Public,
    Private,
    Protected,
    Namespace,
    Module,
    Keyof,
    Infer,
    Is,
    Asserts,
    Override,
    Satisfies,

    // === Punctuation ===
    // Grouping
    LParen,   // (
    RParen,   // )
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]

    // Delimiters
    Semicolon,    // ;
    Comma,        // ,
    Dot,          // .
    DotDotDot,    // ...
    Colon,        // :
    QuestionMark, // ?
    QuestionDot,  // ?.
    Arrow,        // =>
    At,           // @ (decorators)

    // Arithmetic
    Plus,     // +
    Minus,    // -
    Star,     // *
    StarStar, // **
    Slash,    // /
    Percent,  // %

    // Comparison
    Eq,          // ==
    NotEq,       // !=
    StrictEq,    // ===
    StrictNotEq, // !==
    Lt,          // <
    Gt,          // >
    LtEq,        // <=
    GtEq,        // >=

    // Logical
    And,               // &&
    Or,                // ||
    Not,               // !
    NullishCoalescing, // ??

    // Bitwise
    BitAnd, // &
    BitOr,  // |
    BitXor, // ^
    BitNot, // ~
    Shl,    // <<
    Shr,    // >>
    UShr,   // >>>

    // Assignment
    Assign, // =
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    StarStarAssign,
    ShlAssign,
    ShrAssign,
    UShrAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    AndAssign,
    OrAssign,
    NullishAssign,

    // Increment/Decrement
    PlusPlus,   // ++
    MinusMinus, // --

    // Template
    TemplateHead(String),             // `...${
    TemplateMiddle(String),           // }...${
    TemplateTail(String),             // }...`
    NoSubstitutionTemplate(String), // `...` (no interpolation)

    // Special
    Eof,
    // Used internally for error recovery
    Invalid(char),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TemplatePart {
    Head(String),
    Middle(String),
    Tail(String),
    NoSub(String),
}

impl TokenKind {
    /// Check if this token is a keyword that can also be used as an identifier
    /// in non-strict mode (contextual keywords)
    pub fn is_contextual_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Let
                | TokenKind::Of
                | TokenKind::As
                | TokenKind::From
                | TokenKind::Async
                | TokenKind::Yield
                | TokenKind::Static
                | TokenKind::Type
                | TokenKind::Interface
                | TokenKind::Enum
                | TokenKind::Implements
                | TokenKind::Declare
                | TokenKind::Abstract
                | TokenKind::Readonly
                | TokenKind::Public
                | TokenKind::Private
                | TokenKind::Protected
                | TokenKind::Namespace
                | TokenKind::Module
                | TokenKind::Keyof
                | TokenKind::Infer
                | TokenKind::Is
                | TokenKind::Asserts
                | TokenKind::Override
                | TokenKind::Satisfies
        )
    }

    pub fn is_assignment_operator(&self) -> bool {
        matches!(
            self,
            TokenKind::Assign
                | TokenKind::PlusAssign
                | TokenKind::MinusAssign
                | TokenKind::StarAssign
                | TokenKind::SlashAssign
                | TokenKind::PercentAssign
                | TokenKind::StarStarAssign
                | TokenKind::ShlAssign
                | TokenKind::ShrAssign
                | TokenKind::UShrAssign
                | TokenKind::BitAndAssign
                | TokenKind::BitOrAssign
                | TokenKind::BitXorAssign
                | TokenKind::AndAssign
                | TokenKind::OrAssign
                | TokenKind::NullishAssign
        )
    }

    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            TokenKind::Number(_)
                | TokenKind::BigInt(_)
                | TokenKind::String(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::RegExp { .. }
        )
    }
}

/// Lookup table: string → keyword TokenKind
pub fn lookup_keyword(word: &str) -> Option<TokenKind> {
    match word {
        "var" => Some(TokenKind::Var),
        "let" => Some(TokenKind::Let),
        "const" => Some(TokenKind::Const),
        "function" => Some(TokenKind::Function),
        "class" => Some(TokenKind::Class),
        "if" => Some(TokenKind::If),
        "else" => Some(TokenKind::Else),
        "switch" => Some(TokenKind::Switch),
        "case" => Some(TokenKind::Case),
        "default" => Some(TokenKind::Default),
        "for" => Some(TokenKind::For),
        "while" => Some(TokenKind::While),
        "do" => Some(TokenKind::Do),
        "break" => Some(TokenKind::Break),
        "continue" => Some(TokenKind::Continue),
        "return" => Some(TokenKind::Return),
        "try" => Some(TokenKind::Try),
        "catch" => Some(TokenKind::Catch),
        "finally" => Some(TokenKind::Finally),
        "throw" => Some(TokenKind::Throw),
        "new" => Some(TokenKind::New),
        "delete" => Some(TokenKind::Delete),
        "typeof" => Some(TokenKind::Typeof),
        "void" => Some(TokenKind::Void),
        "in" => Some(TokenKind::In),
        "instanceof" => Some(TokenKind::Instanceof),
        "of" => Some(TokenKind::Of),
        "this" => Some(TokenKind::This),
        "super" => Some(TokenKind::Super),
        "null" => Some(TokenKind::Null),
        "true" => Some(TokenKind::True),
        "false" => Some(TokenKind::False),
        "import" => Some(TokenKind::Import),
        "export" => Some(TokenKind::Export),
        "from" => Some(TokenKind::From),
        "as" => Some(TokenKind::As),
        "async" => Some(TokenKind::Async),
        "await" => Some(TokenKind::Await),
        "yield" => Some(TokenKind::Yield),
        "extends" => Some(TokenKind::Extends),
        "static" => Some(TokenKind::Static),
        "debugger" => Some(TokenKind::Debugger),
        "with" => Some(TokenKind::With),
        // TS keywords
        "type" => Some(TokenKind::Type),
        "interface" => Some(TokenKind::Interface),
        "enum" => Some(TokenKind::Enum),
        "implements" => Some(TokenKind::Implements),
        "declare" => Some(TokenKind::Declare),
        "abstract" => Some(TokenKind::Abstract),
        "readonly" => Some(TokenKind::Readonly),
        "public" => Some(TokenKind::Public),
        "private" => Some(TokenKind::Private),
        "protected" => Some(TokenKind::Protected),
        "namespace" => Some(TokenKind::Namespace),
        "module" => Some(TokenKind::Module),
        "keyof" => Some(TokenKind::Keyof),
        "infer" => Some(TokenKind::Infer),
        "is" => Some(TokenKind::Is),
        "asserts" => Some(TokenKind::Asserts),
        "override" => Some(TokenKind::Override),
        "satisfies" => Some(TokenKind::Satisfies),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_lookup_finds_all_js_keywords() {
        let js_keywords = [
            "var", "let", "const", "function", "class", "if", "else", "for", "while", "do",
            "break", "continue", "return", "try", "catch", "finally", "throw", "new", "delete",
            "typeof", "void", "in", "instanceof", "this", "super", "null", "true", "false",
            "import", "export", "async", "await", "yield", "switch", "case", "default", "extends",
            "static", "debugger", "with", "from", "as", "of",
        ];
        for kw in js_keywords {
            assert!(lookup_keyword(kw).is_some(), "missing keyword: {kw}");
        }
    }

    #[test]
    fn keyword_lookup_returns_none_for_identifiers() {
        assert_eq!(lookup_keyword("foo"), None);
        assert_eq!(lookup_keyword("bar"), None);
        assert_eq!(lookup_keyword("console"), None);
        assert_eq!(lookup_keyword("Math"), None);
    }

    #[test]
    fn contextual_keywords() {
        assert!(TokenKind::Let.is_contextual_keyword());
        assert!(TokenKind::Async.is_contextual_keyword());
        assert!(TokenKind::Yield.is_contextual_keyword());
        assert!(!TokenKind::If.is_contextual_keyword());
        assert!(!TokenKind::Function.is_contextual_keyword());
    }

    #[test]
    fn assignment_operators() {
        assert!(TokenKind::Assign.is_assignment_operator());
        assert!(TokenKind::PlusAssign.is_assignment_operator());
        assert!(TokenKind::NullishAssign.is_assignment_operator());
        assert!(!TokenKind::Plus.is_assignment_operator());
        assert!(!TokenKind::Eq.is_assignment_operator());
    }

    #[test]
    fn span_operations() {
        let a = Span::new(5, 10);
        let b = Span::new(8, 15);
        assert_eq!(a.len(), 5);
        assert!(!a.is_empty());
        assert!(a.contains(7));
        assert!(!a.contains(10));
        let merged = a.merge(b);
        assert_eq!(merged, Span::new(5, 15));
    }
}
