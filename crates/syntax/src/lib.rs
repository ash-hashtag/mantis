use mantis_parser::ast::{self, BlockItem, Declaration, Expr, Program, Statement, TypeExpr};
use mantis_parser::token::{tokenize, Span, Token};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightCategory {
    Keyword,
    Function,
    Type,
    Variable,
    Parameter,
    Field,
    String,
    Number,
    Bool,
    Comment,
    Operator,
    Punctuation,
    Macro,
    Default,
}

impl HighlightCategory {
    pub fn tree_sitter_scope(&self) -> &'static str {
        match self {
            HighlightCategory::Keyword => "keyword.control",
            HighlightCategory::Function => "entity.name.function",
            HighlightCategory::Type => "entity.name.type",
            HighlightCategory::Variable => "variable.other",
            HighlightCategory::Parameter => "variable.parameter",
            HighlightCategory::Field => "variable.other.member",
            HighlightCategory::String => "string.quoted",
            HighlightCategory::Number => "constant.numeric",
            HighlightCategory::Bool => "constant.language.boolean",
            HighlightCategory::Comment => "comment.line",
            HighlightCategory::Operator => "keyword.operator",
            HighlightCategory::Punctuation => "punctuation.bracket",
            HighlightCategory::Macro => "entity.name.function.macro",
            HighlightCategory::Default => "text",
        }
    }

    pub fn ansi_color(&self, text: &str) -> String {
        match self {
            HighlightCategory::Keyword => format!("\x1b[35;1m{}\x1b[0m", text), // Magenta bold
            HighlightCategory::Function => format!("\x1b[34;1m{}\x1b[0m", text), // Blue bold
            HighlightCategory::Type => format!("\x1b[33;1m{}\x1b[0m", text),    // Yellow bold
            HighlightCategory::Variable => format!("\x1b[37m{}\x1b[0m", text),  // White
            HighlightCategory::Parameter => format!("\x1b[36m{}\x1b[0m", text), // Cyan
            HighlightCategory::Field => format!("\x1b[36;1m{}\x1b[0m", text),   // Cyan bold
            HighlightCategory::String => format!("\x1b[32m{}\x1b[0m", text),    // Green
            HighlightCategory::Number => format!("\x1b[33m{}\x1b[0m", text),    // Yellow
            HighlightCategory::Bool => format!("\x1b[35m{}\x1b[0m", text),      // Magenta
            HighlightCategory::Comment => format!("\x1b[90;3m{}\x1b[0m", text), // Bright black italic
            HighlightCategory::Operator => format!("\x1b[31m{}\x1b[0m", text),  // Red
            HighlightCategory::Punctuation => format!("\x1b[90m{}\x1b[0m", text), // Bright black
            HighlightCategory::Macro => format!("\x1b[32;1m{}\x1b[0m", text),   // Green bold
            HighlightCategory::Default => text.to_string(),
        }
    }

    pub fn html_css_class(&self) -> &'static str {
        match self {
            HighlightCategory::Keyword => "hl-keyword",
            HighlightCategory::Function => "hl-function",
            HighlightCategory::Type => "hl-type",
            HighlightCategory::Variable => "hl-variable",
            HighlightCategory::Parameter => "hl-parameter",
            HighlightCategory::Field => "hl-field",
            HighlightCategory::String => "hl-string",
            HighlightCategory::Number => "hl-number",
            HighlightCategory::Bool => "hl-bool",
            HighlightCategory::Comment => "hl-comment",
            HighlightCategory::Operator => "hl-operator",
            HighlightCategory::Punctuation => "hl-punctuation",
            HighlightCategory::Macro => "hl-macro",
            HighlightCategory::Default => "hl-default",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub category: HighlightCategory,
    pub span: Span,
    pub text: String,
}

pub fn highlight(source: &str) -> Vec<HighlightSpan> {
    let tokens = match tokenize(source) {
        Ok(toks) => toks,
        Err(_) => return Vec::new(),
    };

    let ast_scopes = build_ast_symbol_table(source);
    let mut spans = Vec::new();

    for spanned in tokens {
        let text = source[spanned.span.start..spanned.span.end].to_string();
        let cat = match spanned.token {
            Token::Pub
            | Token::Fn
            | Token::Let
            | Token::Mut
            | Token::Return
            | Token::If
            | Token::Elif
            | Token::Else
            | Token::Loop
            | Token::Break
            | Token::Continue
            | Token::Match
            | Token::As
            | Token::Type
            | Token::Struct
            | Token::Enum
            | Token::Trait
            | Token::Impl
            | Token::For
            | Token::Extern
            | Token::Import
            | Token::Use
            | Token::Where
            | Token::Async
            | Token::Await
            | Token::Yield
            | Token::Static
            | Token::Const => HighlightCategory::Keyword,
            Token::Bool(_) => HighlightCategory::Bool,
            Token::Int(_) | Token::Float(_) => HighlightCategory::Number,
            Token::String(_) | Token::Char(_) => HighlightCategory::String,
            Token::CompilerFn(_) | Token::CompileTimeType(_) => HighlightCategory::Macro,
            Token::Ident(ref name) => {
                if let Some(cat) = ast_scopes.get(&spanned.span.start) {
                    *cat
                } else if is_builtin_type(name) {
                    HighlightCategory::Type
                } else if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    HighlightCategory::Type
                } else {
                    HighlightCategory::Variable
                }
            }
            Token::EqEq
            | Token::NotEq
            | Token::Bang
            | Token::GtEq
            | Token::LtEq
            | Token::Shl
            | Token::Shr
            | Token::Arrow
            | Token::Eq
            | Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::Percent
            | Token::Gt
            | Token::Lt
            | Token::At
            | Token::AtAssign
            | Token::Question
            | Token::Amp
            | Token::Pipe
            | Token::Caret => HighlightCategory::Operator,
            Token::LParen
            | Token::RParen
            | Token::LBrace
            | Token::RBrace
            | Token::LBracket
            | Token::RBracket
            | Token::Dot
            | Token::Comma
            | Token::Colon
            | Token::Semi => HighlightCategory::Punctuation,
            Token::Comment => HighlightCategory::Comment,
        };

        spans.push(HighlightSpan {
            category: cat,
            span: spanned.span,
            text,
        });
    }

    spans
}

pub fn highlight_to_ansi(source: &str) -> String {
    let spans = highlight(source);
    let mut out = String::new();
    let mut last_end = 0;

    for span in spans {
        if span.span.start > last_end {
            out.push_str(&source[last_end..span.span.start]);
        }
        out.push_str(&span.category.ansi_color(&span.text));
        last_end = span.span.end;
    }
    if last_end < source.len() {
        out.push_str(&source[last_end..]);
    }
    out
}

pub fn highlight_to_html(source: &str) -> String {
    let spans = highlight(source);
    let mut out = String::new();
    let mut last_end = 0;

    out.push_str("<pre class=\"mantis-code\">");
    for span in spans {
        if span.span.start > last_end {
            out.push_str(&escape_html(&source[last_end..span.span.start]));
        }
        out.push_str(&format!(
            "<span class=\"{}\">{}</span>",
            span.category.html_css_class(),
            escape_html(&span.text)
        ));
        last_end = span.span.end;
    }
    if last_end < source.len() {
        out.push_str(&escape_html(&source[last_end..]));
    }
    out.push_str("</pre>");
    out
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn is_builtin_type(name: &str) -> bool {
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
            | "Self"
    )
}

fn build_ast_symbol_table(source: &str) -> HashMap<usize, HighlightCategory> {
    let mut table = HashMap::new();
    if let Ok(prog) = mantis_parser::parse(source) {
        for decl in &prog.declarations {
            visit_decl(decl, &mut table);
        }
    }
    table
}

fn visit_decl(decl: &Declaration, table: &mut HashMap<usize, HighlightCategory>) {
    match decl {
        Declaration::Function(fn_decl) => {
            if let Some(name_expr) = &fn_decl.name {
                mark_type_expr(name_expr, HighlightCategory::Function, table);
            }
            for p in &fn_decl.params {
                table.insert(p.name.span.start, HighlightCategory::Parameter);
                mark_type_expr(&p.ty, HighlightCategory::Type, table);
            }
            if let Some(ret) = &fn_decl.return_type {
                mark_type_expr(ret, HighlightCategory::Type, table);
            }
            if let Some(body) = &fn_decl.body {
                visit_block(body, table);
            }
        }
        Declaration::TypeDef(type_def) => {
            mark_type_expr(&type_def.name, HighlightCategory::Type, table);
        }
        Declaration::Trait(trait_def) => {
            mark_type_expr(&trait_def.name, HighlightCategory::Type, table);
            for m in &trait_def.methods {
                visit_decl(&Declaration::Function(m.clone()), table);
            }
        }
        Declaration::Impl(impl_block) => {
            mark_type_expr(&impl_block.trait_name, HighlightCategory::Type, table);
            if let Some(for_ty) = &impl_block.for_type {
                mark_type_expr(for_ty, HighlightCategory::Type, table);
            }
            for m in &impl_block.methods {
                visit_decl(&Declaration::Function(m.clone()), table);
            }
        }
        _ => {}
    }
}

fn visit_block(block: &ast::Block, table: &mut HashMap<usize, HighlightCategory>) {
    for item in &block.items {
        match item {
            BlockItem::Statement(stmt) => match stmt {
                Statement::Let {
                    name, ty, value, ..
                } => {
                    table.insert(name.span.start, HighlightCategory::Variable);
                    if let Some(t) = ty {
                        mark_type_expr(t, HighlightCategory::Type, table);
                    }
                    visit_expr(value, table);
                }
                Statement::Return { value, .. } => {
                    if let Some(v) = value {
                        visit_expr(v, table);
                    }
                }
                Statement::Expr { expr, .. } => visit_expr(expr, table),
                _ => {}
            },
            BlockItem::IfChain(if_chain) => {
                visit_expr(&if_chain.if_block.condition, table);
                visit_block(&if_chain.if_block.body, table);
                for elif in &if_chain.elif_blocks {
                    visit_expr(&elif.condition, table);
                    visit_block(&elif.body, table);
                }
                if let Some(else_b) = &if_chain.else_block {
                    visit_block(else_b, table);
                }
            }
            BlockItem::Loop(loop_b) => visit_block(&loop_b.body, table),
            BlockItem::Match(match_b) => {
                visit_expr(&match_b.scrutinee, table);
                for arm in &match_b.arms {
                    visit_expr(&arm.pattern, table);
                    visit_block(&arm.body, table);
                }
            }
            BlockItem::Block(b) => visit_block(b, table),
        }
    }
}

fn visit_expr(expr: &Expr, table: &mut HashMap<usize, HighlightCategory>) {
    match expr {
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident(id) = &**callee {
                table.insert(id.span.start, HighlightCategory::Function);
            } else {
                visit_expr(callee, table);
            }
            for arg in args {
                visit_expr(arg, table);
            }
        }
        Expr::Field { object, field, .. } => {
            visit_expr(object, table);
            table.insert(field.span.start, HighlightCategory::Field);
        }
        Expr::Lambda { captures, decl, .. } => {
            for cap in captures {
                table.insert(cap.name.span.start, HighlightCategory::Variable);
            }
            for p in &decl.params {
                table.insert(p.name.span.start, HighlightCategory::Parameter);
                mark_type_expr(&p.ty, HighlightCategory::Type, table);
            }
            if let Some(body) = &decl.body {
                visit_block(body, table);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            visit_expr(lhs, table);
            visit_expr(rhs, table);
        }
        Expr::Unary { operand, .. } => visit_expr(operand, table),
        Expr::Cast { expr, ty, .. } => {
            visit_expr(expr, table);
            mark_type_expr(ty, HighlightCategory::Type, table);
        }
        _ => {}
    }
}

fn mark_type_expr(
    ty: &TypeExpr,
    category: HighlightCategory,
    table: &mut HashMap<usize, HighlightCategory>,
) {
    match ty {
        TypeExpr::Named(id) => {
            table.insert(id.span.start, category);
        }
        TypeExpr::Generic(base, params) => {
            mark_type_expr(base, category, table);
            for p in params {
                mark_type_expr(p, HighlightCategory::Type, table);
            }
        }
        TypeExpr::Nested(a, b) => {
            mark_type_expr(a, category, table);
            mark_type_expr(b, category, table);
        }
        TypeExpr::Ref(inner, _) => mark_type_expr(inner, category, table),
        TypeExpr::Function(params, ret) => {
            for p in params {
                mark_type_expr(p, HighlightCategory::Type, table);
            }
            mark_type_expr(ret, HighlightCategory::Type, table);
        }
        TypeExpr::Unknown => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_highlighting() {
        let code = r#"
            fn add(a i64, b i64) i64 {
                let sum = a + b;
                return sum;
            }
        "#;
        let spans = highlight(code);
        assert!(!spans.is_empty());
        let ansi = highlight_to_ansi(code);
        assert!(ansi.contains("\x1b["));
        let html = highlight_to_html(code);
        assert!(html.contains("<span class=\"hl-keyword\">fn</span>"));
    }
}
