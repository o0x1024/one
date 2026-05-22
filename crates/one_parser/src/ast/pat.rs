use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    Identifier {
        name: String,
        type_annotation: Option<Box<TypeAnnotation>>,
    },
    ObjectPattern {
        properties: Vec<ObjectPatternProperty>,
        rest: Option<Box<Pattern>>,
    },
    ArrayPattern {
        elements: Vec<Option<Pattern>>,
        rest: Option<Box<Pattern>>,
    },
    AssignmentPattern {
        left: Box<Pattern>,
        right: Box<super::Expression>,
    },
    RestElement(Box<Pattern>),
}

#[derive(Debug, Clone)]
pub struct ObjectPatternProperty {
    pub key: super::PropertyKey,
    pub value: Pattern,
    pub computed: bool,
    pub shorthand: bool,
    pub span: Span,
}

/// Placeholder for TypeScript type annotations (parsed but ignored in Phase 1)
#[derive(Debug, Clone)]
pub struct TypeAnnotation {
    pub span: Span,
}
