use crate::token::Span;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Top-Level
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Root of a parsed Mantis source file.
#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Declarations
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone)]
pub enum Declaration {
    Function(FnDecl),
    TypeDef(TypeDef),
    Import(ImportDecl),
    Trait(TraitDef),
    Impl(ImplBlock),
    Use(UseDecl),
}

// ── Import ───────────────────────────────────────────────────────────────────

/// `import std.net.IpAddr;`
#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub path: Vec<Ident>,
    pub alias: Option<Ident>,
    pub span: Span,
}

// ── Use ──────────────────────────────────────────────────────────────────────

/// `use std.libc as c;`
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Vec<Ident>,
    pub alias: Option<Ident>,
    pub span: Span,
}

// ── Function ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: Option<TypeExpr>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Option<Block>,
    pub is_extern: bool,
    pub trailing_params: Option<Vec<Param>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: Ident,
    pub mutable: bool,
    pub ty: TypeExpr,
    pub span: Span,
}

// ── Type Definition ──────────────────────────────────────────────────────────

/// `type Option[T] = enum { Some(T), None }`
/// `type Vec[T] = struct { capacity u64, slice ArraySlice[T] }`
/// `type ptr[T] = i64;`
#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: TypeExpr,
    pub definition: TypeDefBody,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeDefBody {
    Alias(TypeExpr),
    Struct(StructDef),
    Enum(EnumDef),
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub fields: Vec<Param>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    pub fields: Vec<TypeExpr>,
    pub span: Span,
}

// ── Trait ─────────────────────────────────────────────────────────────────────

/// `trait Drop { fn drop(self @mut Self); }`
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub name: TypeExpr,
    pub methods: Vec<FnDecl>,
    pub span: Span,
}

// ── Impl ─────────────────────────────────────────────────────────────────────

/// `impl[T] Drop for Vec[T] { ... }`
/// `impl Self for String { ... }`
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub generics: Vec<Ident>,
    pub trait_name: TypeExpr,
    pub for_type: Option<TypeExpr>,
    pub methods: Vec<FnDecl>,
    pub span: Span,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Types
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Type expression: `Vec[T]`, `@mut Foo`, `std.net.IpAddr`, etc.
#[derive(Debug, Clone)]
pub enum TypeExpr {
    /// Simple named type: `i64`, `Self`, `Foo`
    Named(Ident),
    /// Dot-path: `std.net.IpAddr`
    Nested(Box<TypeExpr>, Box<TypeExpr>),
    /// Generic: `Vec[T]`, `Option[T]`
    Generic(Box<TypeExpr>, Vec<TypeExpr>),
    /// Reference: `@T` or `@mut T` or `&T` or `&mut T`
    Ref(Box<TypeExpr>, bool), // (inner, is_mutable)
    /// Unknown / not specified
    Unknown,
}

impl TypeExpr {
    /// Get the word if this is a simple Named type.
    pub fn as_name(&self) -> Option<&str> {
        match self {
            TypeExpr::Named(id) => Some(&id.name),
            _ => None,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Named(id) => id.span,
            TypeExpr::Nested(a, b) => a.span().merge(b.span()),
            TypeExpr::Generic(base, params) => {
                let mut s = base.span();
                if let Some(last) = params.last() {
                    s = s.merge(last.span());
                }
                s
            }
            TypeExpr::Ref(inner, _) => inner.span(),
            TypeExpr::Unknown => Span::new(0, 0),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Blocks & Statements
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone)]
pub struct Block {
    pub items: Vec<BlockItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum BlockItem {
    Statement(Statement),
    IfChain(IfChain),
    Loop(LoopBlock),
    Match(MatchBlock),
    Block(Block),
}

#[derive(Debug, Clone)]
pub enum Statement {
    /// `let x = expr;` or `mut x = expr;` or `let x Type = expr;`
    Let {
        mutable: bool,
        name: Ident,
        ty: Option<TypeExpr>,
        value: Expr,
        span: Span,
    },
    /// `return expr;`
    Return {
        value: Option<Expr>,
        span: Span,
    },
    /// `break label?;`
    Break {
        label: Option<Ident>,
        span: Span,
    },
    /// `continue label?;`
    Continue {
        label: Option<Ident>,
        span: Span,
    },
    /// Expression used as a statement: `foo();`
    Expr {
        expr: Expr,
        span: Span,
    },
}

// ── If / Elif / Else ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct IfChain {
    pub if_block: ConditionalBlock,
    pub elif_blocks: Vec<ConditionalBlock>,
    pub else_block: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ConditionalBlock {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

// ── Loop ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LoopBlock {
    pub label: Option<Ident>,
    pub body: Block,
    pub span: Span,
}

// ── Match ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MatchBlock {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Expr,
    pub body: Block,
    pub span: Span,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Expressions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone)]
pub enum Expr {
    /// Integer literal
    IntLit { value: i64, span: Span },
    /// Float literal
    FloatLit { value: f64, span: Span },
    /// String literal
    StringLit { value: String, span: Span },
    /// Char literal
    CharLit { value: char, span: Span },
    /// Bool literal
    BoolLit { value: bool, span: Span },
    /// Identifier
    Ident(Ident),
    /// Binary operation: `a + b`, `a == b`, `a = b`, etc.
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// Unary prefix: `-x`, `@x`, `*x`
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    /// Function / method call: `foo(a, b)`
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    /// Field access: `a.b`
    Field {
        object: Box<Expr>,
        field: Ident,
        span: Span,
    },
    /// Type cast: `x as i64`
    Cast {
        expr: Box<Expr>,
        ty: TypeExpr,
        span: Span,
    },
    /// Struct initialization: `Foo { a: 1, b: 2 }` or `Foo { a = 1, b = 2 }`
    StructInit {
        ty: TypeExpr,
        fields: Vec<FieldInit>,
        span: Span,
    },
    /// Array initialization: `[1, 2, 3]`
    ArrayInit {
        elements: Vec<Expr>,
        span: Span,
    },
    /// Lambda: `fn (x i32) i64 { return x as i64; }`
    Lambda {
        decl: Box<FnDecl>,
        span: Span,
    },
    /// Compiler intrinsic call: `#import("libc")`
    CompilerCall {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Type used as expression (e.g. `Self`, `Option.Some`)
    TypeExpr(TypeExpr),
    /// Pointer-assign: `a @= expr`
    PointerAssign {
        target: Box<Expr>,
        value: Box<Expr>,
        span: Span,
    },
    /// Propagate operator: `expr?`
    Propagate {
        expr: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::CharLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Call { span, .. }
            | Expr::Field { span, .. }
            | Expr::Cast { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::ArrayInit { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::CompilerCall { span, .. }
            | Expr::PointerAssign { span, .. }
            | Expr::Propagate { span, .. } => *span,
            Expr::Ident(id) => id.span,
            Expr::TypeExpr(ty) => ty.span(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

// ── Operators ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Gt,
    Lt,
    GtEq,
    LtEq,
    Assign,
    Shr,
    Shl,
    BitAnd,
    BitOr,
    BitXor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,      // -x
    Deref,    // *x
    AddrOf,   // @x
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Common
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// An identifier with its source span.
#[derive(Debug, Clone)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}
