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
    Static(StaticDecl),
}

/// A module-level object with a stable address.
/// `static const NAME: Type = value;`
#[derive(Debug, Clone)]
pub struct StaticDecl {
    pub name: Ident,
    pub ty: TypeExpr,
    pub value: Expr,
    pub is_const: bool,
    pub span: Span,
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
pub struct WhereBound {
    pub target: TypeExpr,
    pub bounds: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: Option<TypeExpr>,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub where_clause: Vec<WhereBound>,
    pub body: Option<Block>,
    pub is_extern: bool,
    pub is_async: bool,
    pub is_pub: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureKind {
    Value,
    Ref,
    MutRef,
}

#[derive(Debug, Clone)]
pub struct CaptureItem {
    pub name: Ident,
    pub kind: CaptureKind,
    pub span: Span,
}

// ── Type Definition ──────────────────────────────────────────────────────────

/// `type Option[T] = enum { Some(T), None }`
/// `type Vec[T] = struct { capacity u64, slice ArraySlice[T] }`
/// `type ptr[T] = @T;`
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
    /// Function type: `(i32, f32) i64`
    Function(Vec<TypeExpr>, Box<TypeExpr>), // (params, return_type)
    /// Unknown / not specified
    Unknown,
}

impl TypeExpr {
    /// Get the word if this is a simple Named type.
    pub fn as_name(&self) -> Option<&str> {
        match self {
            TypeExpr::Named(id) => Some(&id.name),
            TypeExpr::Generic(base, _) => base.as_name(),
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
            TypeExpr::Function(params, ret) => {
                let s = params.first().map(|x| x.span()).unwrap_or(ret.span());
                s.merge(ret.span())
            }
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
    Return { value: Option<Expr>, span: Span },
    /// `break label?;`
    Break { label: Option<Ident>, span: Span },
    /// `continue label?;`
    Continue { label: Option<Ident>, span: Span },
    /// Expression used as a statement: `foo();`
    Expr { expr: Expr, span: Span },
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
    /// Await expression: `await expr` or `expr.await`
    Await { expr: Box<Expr>, span: Span },
    /// Yield expression: `yield expr`
    Yield { expr: Box<Expr>, span: Span },
    /// Async block: `async { ... }`
    AsyncBlock { body: Block, span: Span },
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
    ArrayInit { elements: Vec<Expr>, span: Span },
    /// Lambda: `[a, @mut b] (x i32) i64 { return x as i64; }`
    Lambda {
        captures: Vec<CaptureItem>,
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
    Propagate { expr: Box<Expr>, span: Span },
    /// Generic arguments on an expression: `foo[T]`, `obj.method[T]`
    Generic {
        base: Box<Expr>,
        params: Vec<TypeExpr>,
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
            | Expr::Propagate { span, .. }
            | Expr::Await { span, .. }
            | Expr::Yield { span, .. }
            | Expr::AsyncBlock { span, .. }
            | Expr::Generic { span, .. } => *span,
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
    Neg,    // -x
    Not,    // !x
    Deref,  // *x
    AddrOf, // @x
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

pub fn infer_captures(decl: &FnDecl) -> Vec<CaptureItem> {
    let mut params = std::collections::HashSet::new();
    for p in &decl.params {
        params.insert(p.name.name.clone());
    }

    let mut captured_map = std::collections::HashMap::<String, (Span, bool)>::new();
    let mut locals = std::collections::HashSet::new();

    if let Some(body) = &decl.body {
        collect_vars_in_block(body, &params, &mut locals, &mut captured_map);
    }

    let mut captures = Vec::new();
    for (name, (span, is_mutated)) in captured_map {
        let kind = if is_mutated {
            CaptureKind::MutRef
        } else {
            CaptureKind::Ref
        };
        captures.push(CaptureItem {
            name: Ident::new(name, span),
            kind,
            span,
        });
    }

    captures
}

fn collect_vars_in_block(
    block: &Block,
    params: &std::collections::HashSet<String>,
    locals: &mut std::collections::HashSet<String>,
    captured: &mut std::collections::HashMap<String, (Span, bool)>,
) {
    let outer_locals = locals.clone();
    for item in &block.items {
        match item {
            BlockItem::Statement(stmt) => match stmt {
                Statement::Let { name, value, .. } => {
                    collect_vars_in_expr(value, params, locals, captured, false);
                    locals.insert(name.name.clone());
                }
                Statement::Return { value, .. } => {
                    if let Some(v) = value {
                        collect_vars_in_expr(v, params, locals, captured, false);
                    }
                }
                Statement::Expr { expr, .. } => {
                    collect_vars_in_expr(expr, params, locals, captured, false);
                }
                _ => {}
            },
            BlockItem::IfChain(if_chain) => {
                collect_vars_in_expr(
                    &if_chain.if_block.condition,
                    params,
                    locals,
                    captured,
                    false,
                );
                collect_vars_in_block(&if_chain.if_block.body, params, locals, captured);
                for elif in &if_chain.elif_blocks {
                    collect_vars_in_expr(&elif.condition, params, locals, captured, false);
                    collect_vars_in_block(&elif.body, params, locals, captured);
                }
                if let Some(else_b) = &if_chain.else_block {
                    collect_vars_in_block(else_b, params, locals, captured);
                }
            }
            BlockItem::Loop(loop_b) => {
                collect_vars_in_block(&loop_b.body, params, locals, captured);
            }
            BlockItem::Match(match_b) => {
                collect_vars_in_expr(&match_b.scrutinee, params, locals, captured, false);
                for arm in &match_b.arms {
                    collect_vars_in_expr(&arm.pattern, params, locals, captured, false);
                    collect_vars_in_block(&arm.body, params, locals, captured);
                }
            }
            BlockItem::Block(sub_b) => {
                collect_vars_in_block(sub_b, params, locals, captured);
            }
        }
    }
    *locals = outer_locals;
}

fn collect_vars_in_expr(
    expr: &Expr,
    params: &std::collections::HashSet<String>,
    locals: &std::collections::HashSet<String>,
    captured: &mut std::collections::HashMap<String, (Span, bool)>,
    is_assignment_lhs: bool,
) {
    match expr {
        Expr::Ident(id) => {
            let n = id.name.as_str();
            if !params.contains(n) && !locals.contains(n) && !is_builtin_or_primitive(n) {
                let entry = captured.entry(id.name.clone()).or_insert((id.span, false));
                if is_assignment_lhs {
                    entry.1 = true;
                }
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let is_assign = *op == BinOp::Assign;
            collect_vars_in_expr(lhs, params, locals, captured, is_assign);
            collect_vars_in_expr(rhs, params, locals, captured, false);
        }
        Expr::Unary { operand, .. } => {
            collect_vars_in_expr(operand, params, locals, captured, false);
        }
        Expr::Call { callee, args, .. } => {
            collect_vars_in_expr(callee, params, locals, captured, false);
            for arg in args {
                collect_vars_in_expr(arg, params, locals, captured, false);
            }
        }
        Expr::Field { object, .. } => {
            collect_vars_in_expr(object, params, locals, captured, false);
        }
        Expr::Cast { expr, .. } => {
            collect_vars_in_expr(expr, params, locals, captured, false);
        }
        Expr::StructInit { fields, .. } => {
            for f in fields {
                collect_vars_in_expr(&f.value, params, locals, captured, false);
            }
        }
        Expr::ArrayInit { elements, .. } => {
            for el in elements {
                collect_vars_in_expr(el, params, locals, captured, false);
            }
        }
        Expr::Lambda { decl, .. } => {
            let mut sub_params = params.clone();
            for p in &decl.params {
                sub_params.insert(p.name.name.clone());
            }
            if let Some(body) = &decl.body {
                let mut sub_locals = locals.clone();
                collect_vars_in_block(body, &sub_params, &mut sub_locals, captured);
            }
        }
        Expr::PointerAssign { target, value, .. } => {
            collect_vars_in_expr(target, params, locals, captured, true);
            collect_vars_in_expr(value, params, locals, captured, false);
        }
        Expr::Propagate { expr, .. } => {
            collect_vars_in_expr(expr, params, locals, captured, false);
        }
        Expr::Await { expr, .. } => {
            collect_vars_in_expr(expr, params, locals, captured, false);
        }
        Expr::Yield { expr, .. } => {
            collect_vars_in_expr(expr, params, locals, captured, false);
        }
        Expr::AsyncBlock { body, .. } => {
            let mut sub_locals = locals.clone();
            collect_vars_in_block(body, params, &mut sub_locals, captured);
        }
        Expr::Generic { base, .. } => {
            collect_vars_in_expr(base, params, locals, captured, false);
        }
        Expr::CompilerCall { args, .. } => {
            for arg in args {
                collect_vars_in_expr(arg, params, locals, captured, false);
            }
        }
        _ => {}
    }
}

fn is_builtin_or_primitive(name: &str) -> bool {
    matches!(
        name,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "f32"
            | "f64"
            | "bool"
            | "char"
            | "String"
            | "str"
            | "void"
            | "self"
            | "Self"
            | "true"
            | "false"
            | "print"
            | "println"
    )
}
