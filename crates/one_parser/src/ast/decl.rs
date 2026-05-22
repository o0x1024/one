use crate::span::Span;

#[derive(Debug, Clone)]
pub struct Declaration {
    pub kind: DeclarationKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DeclarationKind {
    VariableDeclaration {
        kind: VarKind,
        declarations: Vec<VariableDeclarator>,
    },
    FunctionDeclaration(super::expr::Function),
    ClassDeclaration(super::expr::Class),

    ImportDeclaration {
        specifiers: Vec<ImportSpecifier>,
        source: String,
    },
    ExportNamedDeclaration {
        declaration: Option<Box<Declaration>>,
        specifiers: Vec<ExportSpecifier>,
        source: Option<String>,
    },
    ExportDefaultDeclaration(ExportDefault),
    ExportAllDeclaration {
        source: String,
        exported: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VarKind {
    Var,
    Let,
    Const,
}

#[derive(Debug, Clone)]
pub struct VariableDeclarator {
    pub id: super::Pattern,
    pub init: Option<super::Expression>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ImportSpecifier {
    Named {
        imported: String,
        local: String,
        span: Span,
    },
    Default {
        local: String,
        span: Span,
    },
    Namespace {
        local: String,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ExportSpecifier {
    pub local: String,
    pub exported: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExportDefault {
    Expression(super::Expression),
    FunctionDeclaration(super::expr::Function),
    ClassDeclaration(super::expr::Class),
}
