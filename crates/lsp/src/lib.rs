use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use mantis_parser::ast::{
    Block, BlockItem, Declaration, Expr, FnDecl, Ident, Program, Statement, TypeDefBody, TypeExpr,
};
use mantis_parser::borrow_checker::BorrowChecker;
use mantis_parser::token::Span;
use mantis_parser::{parse, MantisError};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  LSP Protocol Types
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Option<u32>, // 1: Error, 2: Warning, 3: Information, 4: Hint
    pub code: Option<String>,
    pub source: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishDiagnosticsParams {
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hover {
    pub contents: MarkupContent,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkupContent {
    pub kind: String, // "markdown" or "plaintext"
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<u32>, // 1: Text, 2: Method, 3: Function, 6: Variable, 14: Keyword, 22: Struct, 13: Enum
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u32, // 5: Class/Type, 6: Method, 12: Function, 23: Struct, 10: Enum
    pub range: Range,
    pub selection_range: Range,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Language Server State & Diagnostics
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct MantisLanguageServer {
    documents: HashMap<String, String>,
}

impl MantisLanguageServer {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn did_open(&mut self, uri: String, text: String) -> PublishDiagnosticsParams {
        self.documents.insert(uri.clone(), text.clone());
        let diagnostics = compute_diagnostics(&text);
        PublishDiagnosticsParams { uri, diagnostics }
    }

    pub fn did_change(&mut self, uri: String, text: String) -> PublishDiagnosticsParams {
        self.documents.insert(uri.clone(), text.clone());
        let diagnostics = compute_diagnostics(&text);
        PublishDiagnosticsParams { uri, diagnostics }
    }

    pub fn hover(&self, uri: &str, pos: Position) -> Option<Hover> {
        let text = self.documents.get(uri)?;
        let offset = position_to_offset(text, pos.clone())?;

        // Try AST-based resolution first; fall back to keyword/primitive docs
        // even while the file doesn't parse.
        let resolved = parse(text)
            .ok()
            .and_then(|prog| analyze_at_offset(&prog, text, offset));
        if let Some(hit) = resolved {
            return Some(Hover {
                contents: MarkupContent {
                    kind: "markdown".to_string(),
                    value: hit.markup,
                },
                range: Some(span_to_range(text, hit.word_span)),
            });
        }

        let (word, word_span) = word_at_offset(text, offset)?;
        let doc: String = keyword_doc(&word)
            .map(|d| d.to_string())
            .or_else(|| primitive_doc(&word))
            .or_else(|| builtin_fn_doc(&word).map(|d| d.to_string()))?;
        Some(Hover {
            contents: MarkupContent {
                kind: "markdown".to_string(),
                value: doc,
            },
            range: Some(span_to_range(text, word_span)),
        })
    }

    /// Go-to-definition within the current document. Returns the target range.
    pub fn definition(&self, uri: &str, pos: Position) -> Option<Range> {
        let text = self.documents.get(uri)?;
        let offset = position_to_offset(text, pos)?;
        let prog = parse(text).ok()?;
        let hit = analyze_at_offset(&prog, text, offset)?;
        let def_span = hit.def_span?;
        Some(span_to_range(text, def_span))
    }

    /// Go-to-definition with module-aware locations. Aliased module members
    /// must return the imported file URI, not the caller's URI.
    pub fn definition_location(&self, uri: &str, pos: Position) -> Option<Location> {
        let text = self.documents.get(uri)?;
        let offset = position_to_offset(text, pos.clone())?;
        module_path_definition(uri, text, offset)
            .or_else(|| module_alias_definition(uri, text, offset))
            .or_else(|| {
            self.definition(uri, pos).map(|range| Location {
                uri: uri.to_string(),
                range,
            })
        })
    }

    pub fn completion(&self, uri: &str, _pos: Position) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // Standard keywords with short docs
        for (kw, doc) in KEYWORD_DOCS {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(14), // Keyword
                detail: Some("Mantis keyword".to_string()),
                documentation: Some(doc.to_string()),
            });
        }

        // Built-in types
        for ty in [
            "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "char",
        ] {
            items.push(CompletionItem {
                label: ty.to_string(),
                kind: Some(22), // Struct/Type
                detail: Some("Primitive type".to_string()),
                documentation: primitive_doc(ty),
            });
        }

        // Symbols in current document
        if let Some(text) = self.documents.get(uri) {
            if let Ok(prog) = parse(text) {
                for entry in collect_top_functions(&prog) {
                    items.push(CompletionItem {
                        label: entry.name,
                        kind: Some(3), // Function
                        detail: Some(first_line(&entry.signature)),
                        documentation: entry.container.map(|c| format!("*{}*", c)),
                    });
                }
                for t in collect_top_types(&prog) {
                    items.push(CompletionItem {
                        label: t.name,
                        kind: Some(22), // Type
                        detail: Some(t.kind_label),
                        documentation: t.summary,
                    });
                }
            }
        }

        items
    }

    pub fn document_symbols(&self, uri: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();
        let text = match self.documents.get(uri) {
            Some(t) => t,
            None => return symbols,
        };

        if let Ok(prog) = parse(text) {
            for decl in &prog.declarations {
                match decl {
                    Declaration::Function(f) => {
                        let name = f
                            .name
                            .as_ref()
                            .and_then(|n| n.as_name())
                            .unwrap_or("fn")
                            .to_string();
                        let range = span_to_range(text, f.span);
                        symbols.push(DocumentSymbol {
                            name,
                            detail: Some(fn_signature(f, None)),
                            kind: 12, // Function
                            range: range.clone(),
                            selection_range: range,
                        });
                    }
                    Declaration::TypeDef(t) => {
                        let name = t.name.as_name().unwrap_or("Type").to_string();
                        let range = span_to_range(text, t.span);
                        symbols.push(DocumentSymbol {
                            name,
                            detail: Some(type_kind_label(t)),
                            kind: symbol_kind_for_typedef(t),
                            range: range.clone(),
                            selection_range: range,
                        });
                    }
                    Declaration::Trait(tr) => {
                        let name = tr.name.as_name().unwrap_or("Trait").to_string();
                        let range = span_to_range(text, tr.span);
                        symbols.push(DocumentSymbol {
                            name,
                            detail: Some("Trait".to_string()),
                            kind: 5, // Class
                            range: range.clone(),
                            selection_range: range,
                        });
                    }
                    Declaration::Static(s) => {
                        let range = span_to_range(text, s.span);
                        symbols.push(DocumentSymbol {
                            name: s.name.name.to_string(),
                            detail: Some(format!(
                                "{} {}: {}",
                                if s.is_const { "static const" } else { "static" },
                                s.name.name,
                                type_expr_to_string(&s.ty)
                            )),
                            kind: 14,
                            range: range.clone(),
                            selection_range: range,
                        });
                    }
                    _ => {}
                }
            }
        }

        symbols
    }
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .trim_matches('`')
        .to_string()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Semantic Analysis (hover + go-to-definition)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A resolved symbol under the cursor.
pub struct ResolvedSymbol {
    /// Markdown contents for hover.
    pub markup: String,
    /// Span of the identifier the user is pointing at.
    pub word_span: Span,
    /// Span of the declaration site, if known (for go-to-definition).
    pub def_span: Option<Span>,
}

/// Where an identifier appears, used to pick resolution strategy.
#[derive(Clone, Copy, PartialEq)]
enum IdentRole {
    Plain,
    Callee,
    TypePos,
    FieldName,
    FieldInit,
}

/// A local binding discovered while walking a function body.
struct LocalEntry {
    name: String,
    ty: Option<String>,
    mutable: bool,
    decl_span: Span, // span of the name itself
    value_snippet: Option<String>,
}

struct ParamEntry {
    name: String,
    ty: String,
    mutable: bool,
    decl_span: Span,
}

/// Scope info for the function containing the cursor.
struct FnScope {
    params: Vec<ParamEntry>,
    locals: Vec<LocalEntry>,
    container: Option<String>,
    self_ty: Option<String>,
}

impl FnScope {
    fn empty() -> Self {
        FnScope {
            params: Vec::new(),
            locals: Vec::new(),
            container: None,
            self_ty: None,
        }
    }

    fn container(&self) -> Option<String> {
        self.container.clone()
    }

    /// Innermost binding declared before `offset`.
    fn lookup(&self, name: &str, offset: usize) -> Option<&LocalEntry> {
        self.locals
            .iter()
            .filter(|l| l.name == name && l.decl_span.start <= offset)
            .last()
    }
}

struct Analyzer<'a> {
    src: &'a str,
    offset: usize,
    fns: Vec<FnEntry<'a>>,
    types: Vec<TypeEntry<'a>>,
    statics: Vec<&'a mantis_parser::ast::StaticDecl>,
}

struct FnEntry<'a> {
    name: String,
    signature: String,
    container: Option<String>,
    def_span: Span,
    _marker: std::marker::PhantomData<&'a ()>,
}

struct TypeEntry<'a> {
    name: String,
    def: &'a mantis_parser::ast::TypeDef,
    def_span: Span,
}

impl<'a> Analyzer<'a> {
    fn new(prog: &'a Program, src: &'a str, offset: usize) -> Self {
        let mut me = Analyzer {
            src,
            offset,
            fns: Vec::new(),
            types: Vec::new(),
            statics: Vec::new(),
        };
        me.index(prog);
        me
    }

    fn index(&mut self, prog: &'a Program) {
        for decl in &prog.declarations {
            match decl {
                Declaration::Function(f) => self.add_fn(f, None),
                Declaration::TypeDef(t) => {
                    if let Some(name) = t.name.as_name() {
                        self.types.push(TypeEntry {
                            name: name.to_string(),
                            def: t,
                            def_span: t.name.span(),
                        });
                    }
                }
                Declaration::Trait(tr) => {
                    let tname = tr.name.as_name().unwrap_or("Trait").to_string();
                    for m in &tr.methods {
                        self.add_fn(m, Some(format!("trait {}", tname)));
                    }
                }
                Declaration::Impl(imp) => {
                    let target = imp.for_type.as_ref().unwrap_or(&imp.trait_name);
                    let tname = type_expr_to_string(target);
                    let label = match imp.for_type.as_ref() {
                        Some(for_ty) => format!(
                            "impl {} for {}",
                            type_expr_to_string(&imp.trait_name),
                            type_expr_to_string(for_ty)
                        ),
                        None => format!("impl {}", tname),
                    };
                    for m in &imp.methods {
                        let entry = FnEntry {
                            name: m
                                .name
                                .as_ref()
                                .and_then(|n| n.as_name())
                                .unwrap_or("fn")
                                .to_string(),
                            signature: fn_signature(m, Some(&label)),
                            container: Some(label.clone()),
                            def_span: m.name.as_ref().map(|n| n.span()).unwrap_or(m.span),
                            _marker: std::marker::PhantomData,
                        };
                        self.fns.push(entry);
                    }
                }
                Declaration::Static(s) => self.statics.push(s),
                _ => {}
            }
        }
    }

    fn add_fn(&mut self, f: &'a FnDecl, container: Option<String>) {
        let entry = FnEntry {
            name: f
                .name
                .as_ref()
                .and_then(|n| n.as_name())
                .unwrap_or("fn")
                .to_string(),
            signature: fn_signature(f, container.as_deref()),
            container,
            def_span: f.name.as_ref().map(|n| n.span()).unwrap_or(f.span),
            _marker: std::marker::PhantomData,
        };
        self.fns.push(entry);
    }

    fn find_fn(&self, name: &str) -> Option<&FnEntry<'a>> {
        self.fns.iter().find(|f| f.name == name)
    }

    fn find_type(&self, name: &str) -> Option<&TypeEntry<'a>> {
        self.types.iter().find(|t| t.name == name)
    }

    /// Entry point: locate the innermost meaningful symbol under the cursor.
    fn analyze(&self, prog: &'a Program) -> Option<ResolvedSymbol> {
        for decl in &prog.declarations {
            match decl {
                Declaration::Function(f) => {
                    if let Some(hit) = self.in_fn(f, None) {
                        return Some(hit);
                    }
                }
                Declaration::TypeDef(t) => {
                    if let Some(sp) = contained_type_span(&t.name, self.offset) {
                        return self.type_span_hit_ctx(sp, &FnScope::empty());
                    }
                    if let Some(hit) = self.in_typedef_body(t) {
                        return Some(hit);
                    }
                }
                Declaration::Trait(tr) => {
                    if contains(
                        tr.name.as_name().map(|_| tr.name.span()).unwrap_or(tr.span),
                        self.offset,
                    ) {
                        if let Some(sp) = contained_type_span(&tr.name, self.offset) {
                            return self.type_span_hit(sp);
                        }
                    }
                    for m in &tr.methods {
                        let tn = tr.name.as_name().unwrap_or("Trait");
                        if let Some(hit) = self.in_fn_in_container(m, &format!("trait {}", tn)) {
                            return Some(hit);
                        }
                    }
                }
                Declaration::Impl(imp) => {
                    for m in &imp.methods {
                        let container = imp
                            .for_type
                            .as_ref()
                            .map(type_expr_to_string)
                            .unwrap_or_else(|| type_expr_to_string(&imp.trait_name));
                        if let Some(hit) = self.in_fn_in_container(m, &container) {
                            return Some(hit);
                        }
                    }
                }
                Declaration::Static(s) => {
                    if contains(s.name.span, self.offset) {
                        let mut markup = format!(
                            "```mantis\n{} {}: {}\n```",
                            if s.is_const { "static const" } else { "static" },
                            s.name.name,
                            type_expr_to_string(&s.ty)
                        );
                        let snippet = source_slice_trimmed(self.src, s.value.span(), 60);
                        if !snippet.is_empty() {
                            markup.push_str(&format!("\n\n`= {}`", snippet));
                        }
                        markup.push_str("\n---\n*static item*");
                        return Some(ResolvedSymbol {
                            markup,
                            word_span: s.name.span,
                            def_span: Some(s.name.span),
                        });
                    }
                    if let Some(sp) = contained_type_span(&s.ty, self.offset) {
                        return self.type_span_hit(sp);
                    }
                    if contains(s.value.span(), self.offset) {
                        if let Some(hit) = self.walk_expr(&s.value, &FnScope::empty()) {
                            return Some(hit);
                        }
                    }
                }
                Declaration::Import(_) | Declaration::Use(_) => {
                    let segs: Vec<Span> = match decl {
                        Declaration::Import(i) => i.path.iter().map(|p| p.span).collect(),
                        Declaration::Use(u) => u.path.iter().map(|p| p.span).collect(),
                        _ => unreachable!(),
                    };
                    let full: Vec<&str> = match decl {
                        Declaration::Import(i) => i.path.iter().map(|p| p.name.as_str()).collect(),
                        Declaration::Use(u) => u.path.iter().map(|p| p.name.as_str()).collect(),
                        _ => unreachable!(),
                    };
                    for (i, seg) in segs.iter().enumerate() {
                        if contains(*seg, self.offset) {
                            return Some(ResolvedSymbol {
                                markup: format!(
                                    "```mantis\nimport {}\n```\n---\n*module path*{}",
                                    full.join("."),
                                    if i == 0 {
                                        "\n\nGo to definition opens this module's file scope."
                                            .to_string()
                                    } else {
                                        String::new()
                                    }
                                ),
                                word_span: *seg,
                                def_span: None,
                            });
                        }
                    }
                }
            }
        }
        None
    }

    /// Analyze inside a named function (top-level or trait method).
    fn in_fn(&self, f: &FnDecl, container: Option<&str>) -> Option<ResolvedSymbol> {
        let scope = self.build_scope(f, container);
        self.in_fn_with_scope(f, &scope)
    }

    fn in_fn_in_container(&self, f: &FnDecl, container: &str) -> Option<ResolvedSymbol> {
        self.in_fn(f, Some(container))
    }

    fn in_fn_with_scope(&self, f: &FnDecl, scope: &FnScope) -> Option<ResolvedSymbol> {
        // Cursor on the function name?
        if let Some(name_expr) = &f.name {
            if let Some(sp) = contained_type_span(name_expr, self.offset) {
                if let Some(fname) = name_expr.as_name() {
                    if let Some(entry) = self
                        .fns
                        .iter()
                        .find(|e| e.def_span == name_expr.span() || e.name == *fname)
                    {
                        return Some(ResolvedSymbol {
                            markup: entry.signature.clone(),
                            word_span: sp,
                            def_span: Some(entry.def_span),
                        });
                    }
                }
                return self.type_span_hit_ctx(sp, scope);
            }
        }

        // Params
        for p in &f.params {
            if contains(p.name.span, self.offset) {
                let ty = type_expr_to_string(&p.ty);
                let mut markup = format!(
                    "```mantis\n{}{}: {}\n```",
                    if p.mutable { "mut " } else { "" },
                    p.name.name,
                    ty
                );
                markup.push_str("\n---\n*parameter*");
                if let Some(c) = scope.container() {
                    markup.push_str(&format!(" of `{}`", c));
                }
                return Some(ResolvedSymbol {
                    markup,
                    word_span: p.name.span,
                    def_span: Some(p.name.span),
                });
            }
            if let Some(sp) = contained_type_span(&p.ty, self.offset) {
                return self.type_span_hit_ctx(sp, scope);
            }
        }
        if let Some(rt) = &f.return_type {
            if let Some(sp) = contained_type_span(rt, self.offset) {
                return self.type_span_hit_ctx(sp, scope);
            }
        }
        for wb in &f.where_clause {
            if let Some(sp) = contained_type_span(&wb.target, self.offset) {
                return self.type_span_hit_ctx(sp, scope);
            }
            for b in &wb.bounds {
                if let Some(sp) = contained_type_span(b, self.offset) {
                    return self.type_span_hit_ctx(sp, scope);
                }
            }
        }

        if let Some(body) = &f.body {
            if contains(body.span, self.offset) {
                return self.walk_block(body, scope);
            }
        }
        None
    }

    fn build_scope(&self, f: &FnDecl, container: Option<&str>) -> FnScope {
        let mut params = Vec::new();
        for p in &f.params {
            params.push(ParamEntry {
                name: p.name.name.clone(),
                ty: type_expr_to_string(&p.ty),
                mutable: p.mutable,
                decl_span: p.name.span,
            });
        }
        let mut locals = Vec::new();
        if let Some(body) = &f.body {
            collect_locals(body, self.src, &mut locals);
        }
        let self_ty = f.params.first().and_then(|p| {
            if p.name.name == "self" {
                Some(match &p.ty {
                    TypeExpr::Named(id) => strip_ref_prefix(&id.name).to_string(),
                    other => type_expr_to_string(other),
                })
            } else {
                None
            }
        });
        FnScope {
            params,
            locals,
            container: container.map(|c| c.to_string()),
            self_ty,
        }
    }

    /// Hover for a literal only when the cursor is on it.
    fn lit_hit(&self, span: Span, desc: &str) -> Option<ResolvedSymbol> {
        if contains(span, self.offset) {
            lit_hover(span, desc)
        } else {
            None
        }
    }

    fn walk_block(&self, block: &Block, scope: &FnScope) -> Option<ResolvedSymbol> {
        for item in &block.items {
            let hit = match item {
                BlockItem::Statement(st) => self.walk_statement(st, scope),
                BlockItem::IfChain(chain) => {
                    if let Some(h) = self.walk_expr(&chain.if_block.condition, scope) {
                        return Some(h);
                    }
                    if let Some(h) = self.walk_block(&chain.if_block.body, scope) {
                        return Some(h);
                    }
                    let mut found = None;
                    for elif in &chain.elif_blocks {
                        found = self
                            .walk_expr(&elif.condition, scope)
                            .or_else(|| self.walk_block(&elif.body, scope));
                        if found.is_some() {
                            break;
                        }
                    }
                    found.or_else(|| {
                        chain
                            .else_block
                            .as_ref()
                            .and_then(|b| self.walk_block(b, scope))
                    })
                }
                BlockItem::Loop(l) => self.walk_block(&l.body, scope),
                BlockItem::Match(m) => {
                    if let Some(h) = self.walk_expr(&m.scrutinee, scope) {
                        return Some(h);
                    }
                    let mut found = None;
                    for arm in &m.arms {
                        found = self
                            .walk_expr(&arm.pattern, scope)
                            .or_else(|| self.walk_block(&arm.body, scope));
                        if found.is_some() {
                            break;
                        }
                    }
                    found
                }
                BlockItem::Block(b) => self.walk_block(b, scope),
            };
            if hit.is_some() {
                return hit;
            }
        }
        None
    }

    fn walk_statement(&self, st: &Statement, scope: &FnScope) -> Option<ResolvedSymbol> {
        match st {
            Statement::Let {
                name, ty, value, ..
            } => {
                if contains(name.span, self.offset) {
                    let mut markup = String::from("```mantis\n");
                    match ty {
                        Some(t) => markup.push_str(&format!(
                            "let {}: {}\n```",
                            name.name,
                            type_expr_to_string(t)
                        )),
                        None => markup.push_str(&format!("let {}\n```", name.name)),
                    }
                    markup.push_str("\n---\n*local variable*");
                    if let Some(v) = scope_value_snippet(self.src, value) {
                        markup.push_str(&format!("\n\ninitialized to `{}`", v));
                    }
                    return Some(ResolvedSymbol {
                        markup,
                        word_span: name.span,
                        def_span: Some(name.span),
                    });
                }
                if let Some(t) = ty {
                    if let Some(sp) = contained_type_span(t, self.offset) {
                        return self.type_span_hit_ctx(sp, scope);
                    }
                }
                self.walk_expr(value, scope)
            }
            Statement::Return { value, .. } => {
                value.as_ref().and_then(|v| self.walk_expr(v, scope))
            }
            Statement::Expr { expr, .. } => self.walk_expr(expr, scope),
            Statement::Break { label, .. } | Statement::Continue { label, .. } => {
                if let Some(l) = label {
                    if contains(l.span, self.offset) {
                        return Some(ResolvedSymbol {
                            markup:
                                "```mantis\nloop label\n```\n---\nUsed with `break` / `continue`."
                                    .to_string(),
                            word_span: l.span,
                            def_span: None,
                        });
                    }
                }
                None
            }
        }
    }

    fn walk_expr(&self, expr: &Expr, scope: &FnScope) -> Option<ResolvedSymbol> {
        match expr {
            Expr::Ident(id) => {
                if contains(id.span, self.offset) {
                    return self.resolve_plain(id, scope);
                }
                None
            }
            Expr::IntLit { span, .. } => self.lit_hit(*span, "integer literal — i64"),
            Expr::FloatLit { span, .. } => self.lit_hit(*span, "float literal — f64"),
            Expr::StringLit { span, .. } => self.lit_hit(*span, "string literal — StrSlice"),
            Expr::CharLit { span, .. } => self.lit_hit(*span, "char literal"),
            Expr::BoolLit { span, .. } => self.lit_hit(*span, "bool"),
            Expr::Binary { lhs, rhs, .. } => self
                .walk_expr(lhs, scope)
                .or_else(|| self.walk_expr(rhs, scope)),
            Expr::Unary { operand, .. } => self.walk_expr(operand, scope),
            Expr::Call { callee, args, .. } => {
                if let Some(hit) = self.walk_callee(callee, scope) {
                    return Some(hit);
                }
                for a in args {
                    if let Some(h) = self.walk_expr(a, scope) {
                        return Some(h);
                    }
                }
                None
            }
            Expr::Field { object, field, .. } => {
                if contains(field.span, self.offset) {
                    return self.resolve_field(object, field, scope);
                }
                self.walk_expr(object, scope)
            }
            Expr::Cast { expr, ty, .. } => self
                .walk_expr(expr, scope)
                .or_else(|| self.type_pos_walk(ty, scope)),
            Expr::StructInit { ty, fields, .. } => {
                for fi in fields {
                    if contains(fi.name.span, self.offset) {
                        return self.resolve_field_init(ty, &fi.name);
                    }
                    if let Some(h) = self.walk_expr(&fi.value, scope) {
                        return Some(h);
                    }
                }
                self.type_pos_walk(ty, scope)
            }
            Expr::ArrayInit { elements, .. } => {
                for el in elements {
                    if let Some(h) = self.walk_expr(el, scope) {
                        return Some(h);
                    }
                }
                None
            }
            Expr::Lambda { captures, decl, .. } => {
                for cap in captures {
                    if contains(cap.name.span, self.offset) {
                        let kind = match cap.kind {
                            mantis_parser::ast::CaptureKind::Value => "by value",
                            mantis_parser::ast::CaptureKind::Ref => "by reference",
                            mantis_parser::ast::CaptureKind::MutRef => "by mutable reference",
                        };
                        return Some(ResolvedSymbol {
                            markup: format!(
                                "```mantis\n{}\n```\n---\nlambda capture ({})",
                                cap.name.name, kind
                            ),
                            word_span: cap.name.span,
                            def_span: None,
                        });
                    }
                }
                let child_scope = self.child_scope(decl);
                self.in_fn_params_and_body(decl, &child_scope)
            }
            Expr::CompilerCall { name, args, span } => {
                if contains(*span, self.offset) {
                    return Some(ResolvedSymbol {
                        markup: format!(
                            "{}\n---\n*compiler intrinsic*",
                            intrinsic_doc(name)
                                .unwrap_or_else(|| { format!("```mantis\n#{}(...)\n```", name) })
                        ),
                        word_span: *span,
                        def_span: None,
                    });
                }
                for a in args {
                    if let Some(h) = self.walk_expr(a, scope) {
                        return Some(h);
                    }
                }
                None
            }
            Expr::Await { expr, .. } | Expr::Yield { expr, .. } | Expr::Propagate { expr, .. } => {
                self.walk_expr(expr, scope)
            }
            Expr::AsyncBlock { body, .. } => self.walk_block(body, scope),
            Expr::PointerAssign { target, value, .. } => self
                .walk_expr(target, scope)
                .or_else(|| self.walk_expr(value, scope)),
            Expr::Generic { base, params, .. } => {
                if let Some(h) = self.walk_callee(base, scope) {
                    return Some(h);
                }
                for p in params {
                    if let Some(sp) = contained_type_span(p, self.offset) {
                        return self.type_span_hit_ctx(sp, scope);
                    }
                }
                None
            }
            Expr::TypeExpr(ty) => self.type_pos_walk(ty, scope),
        }
    }

    /// Hover for a call target: show the full function signature.
    fn walk_callee(&self, callee: &Expr, scope: &FnScope) -> Option<ResolvedSymbol> {
        match callee {
            Expr::Ident(id) => {
                if contains(id.span, self.offset) {
                    return self.resolve_callee(id, scope);
                }
                None
            }
            Expr::Field { object, field, .. } => {
                if contains(field.span, self.offset) {
                    // Method call: base.method(...)
                    let base_ty = self.expr_type(object, scope);
                    if let Some(bt) = &base_ty {
                        if let Some(entry) =
                            self.find_method_on_type(strip_ref_prefix(bt), &field.name)
                        {
                            return Some(ResolvedSymbol {
                                markup: entry.signature.clone(),
                                word_span: field.span,
                                def_span: Some(entry.def_span),
                            });
                        }
                    }
                    if let Some(entry) = self.find_fn(&field.name) {
                        return Some(ResolvedSymbol {
                            markup: entry.signature.clone(),
                            word_span: field.span,
                            def_span: Some(entry.def_span),
                        });
                    }
                    return self.resolve_field(object, field, scope);
                }
                self.walk_expr(object, scope)
            }
            Expr::Generic { base, .. } => self.walk_callee(base, scope),
            _ => self.walk_expr(callee, scope),
        }
    }

    fn in_fn_params_and_body(&self, decl: &FnDecl, scope: &FnScope) -> Option<ResolvedSymbol> {
        for p in &decl.params {
            if contains(p.name.span, self.offset) {
                let ty = type_expr_to_string(&p.ty);
                return Some(ResolvedSymbol {
                    markup: format!(
                        "```mantis\n{}{}: {}\n```\n---\n*closure parameter*",
                        if p.mutable { "mut " } else { "" },
                        p.name.name,
                        ty
                    ),
                    word_span: p.name.span,
                    def_span: Some(p.name.span),
                });
            }
            if let Some(sp) = contained_type_span(&p.ty, self.offset) {
                return self.type_span_hit_ctx(sp, scope);
            }
        }
        decl.body.as_ref().and_then(|b| self.walk_block(b, scope))
    }

    fn child_scope(&self, decl: &FnDecl) -> FnScope {
        let mut params = Vec::new();
        for p in &decl.params {
            params.push(ParamEntry {
                name: p.name.name.clone(),
                ty: type_expr_to_string(&p.ty),
                mutable: p.mutable,
                decl_span: p.name.span,
            });
        }
        let mut locals = Vec::new();
        if let Some(body) = &decl.body {
            collect_locals(body, self.src, &mut locals);
        }
        FnScope {
            params,
            locals,
            container: None,
            self_ty: None,
        }
    }

    // ── Resolution strategies ────────────────────────────────────────────

    fn resolve_callee(&self, id: &Ident, _scope: &FnScope) -> Option<ResolvedSymbol> {
        if let Some(entry) = self.find_fn(&id.name) {
            return Some(ResolvedSymbol {
                markup: entry.signature.clone(),
                word_span: id.span,
                def_span: Some(entry.def_span),
            });
        }
        if let Some(hit) = self.try_enum_variant(&id.name) {
            return Some(hit);
        }
        if let Some(doc) = builtin_fn_doc(&id.name) {
            return Some(ResolvedSymbol {
                markup: format!("{}\n---\n*built-in*", doc),
                word_span: id.span,
                def_span: None,
            });
        }
        None
    }

    fn resolve_plain(&self, id: &Ident, scope: &FnScope) -> Option<ResolvedSymbol> {
        // 1. Local bindings
        if let Some(local) = scope.lookup(&id.name, self.offset) {
            let kw = if local.mutable { "mut" } else { "let" };
            let mut markup = String::from("```mantis\n");
            match &local.ty {
                Some(t) => markup.push_str(&format!("{} {}: {}\n```", kw, local.name, t)),
                None => markup.push_str(&format!("{} {}\n```", kw, local.name)),
            }
            markup.push_str("\n---\n*local variable*");
            if let Some(snippet) = &local.value_snippet {
                markup.push_str(&format!("\n\ninitialized to `{}`", snippet));
            }
            return Some(ResolvedSymbol {
                markup,
                word_span: id.span,
                def_span: Some(local.decl_span),
            });
        }

        // 2. Parameters
        if let Some(p) = scope.params.iter().find(|p| p.name == id.name) {
            let mut markup = format!(
                "```mantis\n{}{}: {}\n```\n---\n*parameter*",
                if p.mutable { "mut " } else { "" },
                p.name,
                p.ty
            );
            if let Some(c) = scope.container() {
                markup.push_str(&format!(" of `{}`", c));
            }
            return Some(ResolvedSymbol {
                markup,
                word_span: id.span,
                def_span: Some(p.decl_span),
            });
        }

        // 3. Statics / constants
        for s in &self.statics {
            if s.name.name == id.name {
                let mut markup = format!(
                    "```mantis\n{} {}: {}\n```",
                    if s.is_const { "static const" } else { "static" },
                    s.name.name,
                    type_expr_to_string(&s.ty)
                );
                let snippet = source_slice_trimmed(self.src, s.value.span(), 60);
                if !snippet.is_empty() {
                    markup.push_str(&format!("\n\n`= {}`", snippet));
                }
                return Some(ResolvedSymbol {
                    markup,
                    word_span: id.span,
                    def_span: Some(s.name.span),
                });
            }
        }

        // 4. Functions
        if let Some(entry) = self.find_fn(&id.name) {
            return Some(ResolvedSymbol {
                markup: entry.signature.clone(),
                word_span: id.span,
                def_span: Some(entry.def_span),
            });
        }

        // 5. Types
        if let Some(t) = self.find_type(&id.name) {
            return Some(ResolvedSymbol {
                markup: render_type_doc(t),
                word_span: id.span,
                def_span: Some(t.def_span),
            });
        }

        // 6. Enum variants used unqualified
        if let Some(hit) = self.try_enum_variant(&id.name) {
            return Some(hit);
        }

        // 7. Builtins and primitives
        if let Some(doc) = builtin_fn_doc(&id.name) {
            return Some(ResolvedSymbol {
                markup: format!("{}\n---\n*built-in*", doc),
                word_span: id.span,
                def_span: None,
            });
        }
        if let Some(doc) = primitive_doc(&id.name) {
            return Some(ResolvedSymbol {
                markup: doc,
                word_span: id.span,
                def_span: None,
            });
        }

        None
    }

    /// Resolve `obj.field` using the base expression's inferred type.
    fn resolve_field(
        &self,
        object: &Expr,
        field: &Ident,
        scope: &FnScope,
    ) -> Option<ResolvedSymbol> {
        // self.x inside impl methods
        if let Expr::Ident(base_id) = object {
            if base_id.name == "self" {
                if let Some(ty) = &scope.self_ty {
                    if let Some(hit) = self.struct_field_hit(ty, &field.name) {
                        return Some(ResolvedSymbol {
                            markup: hit.0,
                            word_span: field.span,
                            def_span: hit.1,
                        });
                    }
                    if let Some(e) = self.find_method_on_type(ty, &field.name) {
                        return Some(ResolvedSymbol {
                            markup: e.signature.clone(),
                            word_span: field.span,
                            def_span: Some(e.def_span),
                        });
                    }
                }
            }
        }

        if let Some(base_ty) = self.expr_type(object, scope) {
            let clean = strip_ref_prefix(&base_ty);
            if let Some(hit) = self.struct_field_hit(clean, &field.name) {
                return Some(ResolvedSymbol {
                    markup: hit.0,
                    word_span: field.span,
                    def_span: hit.1,
                });
            }
            if let Some(e) = self.find_method_on_type(clean, &field.name) {
                return Some(ResolvedSymbol {
                    markup: e.signature.clone(),
                    word_span: field.span,
                    def_span: Some(e.def_span),
                });
            }
            // Enum variant constructor used with receiver syntax: Opt.None
            if let Some(hit) = self.enum_variant_on_type(clean, &field.name) {
                return Some(ResolvedSymbol {
                    markup: hit,
                    word_span: field.span,
                    def_span: None,
                });
            }
        }

        // Unqualified or unresolved: still try global enum variants.
        if let Some(hit) = self.try_enum_variant(&field.name) {
            return Some(ResolvedSymbol {
                markup: hit.markup,
                word_span: field.span,
                def_span: hit.def_span,
            });
        }

        Some(ResolvedSymbol {
            markup: format!("```mantis\n{}\n```\n---\n*member*", field.name),
            word_span: field.span,
            def_span: None,
        })
    }

    fn resolve_field_init(&self, ty: &TypeExpr, name: &Ident) -> Option<ResolvedSymbol> {
        let full = type_expr_to_string(ty);
        let tname = strip_ref_prefix(&full);
        if let Some(hit) = self.struct_field_hit(tname, &name.name) {
            return Some(ResolvedSymbol {
                markup: hit.0,
                word_span: name.span,
                def_span: hit.1,
            });
        }
        Some(ResolvedSymbol {
            markup: format!("```mantis\n{}\n```\n---\n*field initializer*", name.name),
            word_span: name.span,
            def_span: None,
        })
    }

    /// (markdown, def-span) for a struct field of the given type.
    fn struct_field_hit(&self, type_name: &str, field: &str) -> Option<(String, Option<Span>)> {
        let base = base_type_name(type_name);
        let t = self.find_type(base)?;
        let fields = match &t.def.definition {
            TypeDefBody::Struct(sd) => &sd.fields,
            _ => return None,
        };
        let f = fields.iter().find(|p| p.name.name == field)?;
        let markup = format!(
            "```mantis\n{}.{}: {}\n```\n---\n*field of `{}`*",
            base,
            field,
            type_expr_to_string(&f.ty),
            base
        );
        Some((markup, Some(f.name.span)))
    }

    /// Markdown for `Type.Variant` where Type is a known enum.
    fn enum_variant_on_type(&self, type_name: &str, variant: &str) -> Option<String> {
        let base = base_type_name(type_name);
        let t = self.find_type(base)?;
        let en = match &t.def.definition {
            TypeDefBody::Enum(en) => en,
            _ => return None,
        };
        let v = en.variants.iter().find(|v| v.name.name == variant)?;
        let fields = v
            .fields
            .iter()
            .map(type_expr_to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let payload = if fields.is_empty() {
            String::new()
        } else {
            format!("\n\nPayload: `({})`", fields)
        };
        Some(format!(
            "```mantis\n{}.{}\n```\n---\n*variant of enum `{}`*{}",
            base, variant, base, payload
        ))
    }

    fn find_method_on_type(&self, type_name: &str, method: &str) -> Option<&FnEntry<'a>> {
        let base = base_type_name(type_name);
        self.fns.iter().find(|f| {
            f.name == method
                && f.container
                    .as_deref()
                    .and_then(split_impl_target)
                    .map(|t| base_type_name(&t) == base)
                    .unwrap_or(false)
        })
    }

    fn try_enum_variant(&self, name: &str) -> Option<ResolvedSymbol> {
        for t in &self.types {
            if let TypeDefBody::Enum(en) = &t.def.definition {
                if let Some(v) = en.variants.iter().find(|v| v.name.name == name) {
                    let fields = v
                        .fields
                        .iter()
                        .map(type_expr_to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    let payload = if fields.is_empty() {
                        String::new()
                    } else {
                        format!("\n\nPayload: `({})`", fields)
                    };
                    return Some(ResolvedSymbol {
                        markup: format!(
                            "```mantis\n{}.{}\n```\n---\n*variant of enum `{}`*{}",
                            t.name, name, t.name, payload
                        ),
                        word_span: v.name.span,
                        def_span: Some(v.name.span),
                    });
                }
            }
        }
        None
    }

    /// Best-effort static type of an expression.
    fn expr_type(&self, expr: &Expr, scope: &FnScope) -> Option<String> {
        match expr {
            Expr::Ident(id) => scope
                .lookup(&id.name, self.offset)
                .and_then(|l| l.ty.clone())
                .or_else(|| {
                    scope
                        .params
                        .iter()
                        .find(|p| p.name == id.name)
                        .map(|p| p.ty.clone())
                })
                .or_else(|| {
                    self.statics
                        .iter()
                        .find(|s| s.name.name == id.name)
                        .map(|s| type_expr_to_string(&s.ty))
                }),
            Expr::IntLit { .. } => Some("i64".into()),
            Expr::FloatLit { .. } => Some("f64".into()),
            Expr::BoolLit { .. } => Some("bool".into()),
            Expr::CharLit { .. } => Some("char".into()),
            Expr::StringLit { .. } => Some("StrSlice".into()),
            Expr::Cast { ty, .. } => Some(type_expr_to_string(ty)),
            Expr::StructInit { ty, .. } => Some(type_expr_to_string(ty)),
            Expr::Unary { operand, .. } => self.expr_type(operand, scope),
            _ => None,
        }
    }

    // ── Type-position helpers ────────────────────────────────────────────

    fn type_pos_walk(&self, ty: &TypeExpr, scope: &FnScope) -> Option<ResolvedSymbol> {
        contained_type_span(ty, self.offset).and_then(|sp| self.type_span_hit_ctx(sp, scope))
    }

    fn type_span_hit(&self, sp: Span) -> Option<ResolvedSymbol> {
        self.type_span_hit_ctx(sp, &FnScope::empty())
    }

    fn type_span_hit_ctx(&self, sp: Span, scope: &FnScope) -> Option<ResolvedSymbol> {
        let word = source_slice(self.src, sp);

        if let Some(t) = self.find_type(&word) {
            return Some(ResolvedSymbol {
                markup: render_type_doc(t),
                word_span: sp,
                def_span: Some(t.def_span),
            });
        }
        if let Some(doc) = primitive_doc(&word) {
            return Some(ResolvedSymbol {
                markup: doc,
                word_span: sp,
                def_span: None,
            });
        }
        // Generic parameter like T inside impl[T]
        let is_generic_param =
            |c: &Option<String>| -> bool { c.as_deref().map(starts_impl_generic).unwrap_or(false) };
        if is_generic_param(&scope.container)
            || word.chars().next().is_some_and(|c| c.is_uppercase()) == false && false
        {
            return Some(ResolvedSymbol {
                markup: format!("```mantis\n{}\n```\n---\n*generic parameter*", word),
                word_span: sp,
                def_span: None,
            });
        }
        Some(ResolvedSymbol {
            markup: format!("```mantis\n{}\n```\n---\n*type*", word),
            word_span: sp,
            def_span: None,
        })
    }

    fn in_typedef_body(&self, t: &mantis_parser::ast::TypeDef) -> Option<ResolvedSymbol> {
        match &t.definition {
            TypeDefBody::Struct(sd) => {
                for f in &sd.fields {
                    if contains(f.name.span, self.offset) {
                        return Some(ResolvedSymbol {
                            markup: format!(
                                "```mantis\n{}.{}: {}\n```\n---\n*struct field*",
                                t.name.as_name().unwrap_or("Self"),
                                f.name.name,
                                type_expr_to_string(&f.ty)
                            ),
                            word_span: f.name.span,
                            def_span: Some(f.name.span),
                        });
                    }
                    if let Some(sp) = contained_type_span(&f.ty, self.offset) {
                        return self.type_span_hit(sp);
                    }
                }
                None
            }
            TypeDefBody::Enum(en) => {
                for v in &en.variants {
                    if contains(v.name.span, self.offset) {
                        let fields = v
                            .fields
                            .iter()
                            .map(type_expr_to_string)
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Some(ResolvedSymbol {
                            markup: format!(
                                "```mantis\n{}({})\n```\n---\n*variant of `{}`*",
                                v.name.name,
                                fields,
                                t.name.as_name().unwrap_or("?")
                            ),
                            word_span: v.name.span,
                            def_span: Some(v.name.span),
                        });
                    }
                    for fty in &v.fields {
                        if let Some(sp) = contained_type_span(fty, self.offset) {
                            return self.type_span_hit(sp);
                        }
                    }
                }
                None
            }
            TypeDefBody::Alias(a) => {
                contained_type_span(a, self.offset).and_then(|sp| self.type_span_hit(sp))
            }
        }
    }
}

fn starts_impl_generic(container: &str) -> bool {
    // crude: "impl[T]" style containers declare generic params
    container.starts_with("impl[")
}

fn lit_hover(span: Span, desc: &str) -> Option<ResolvedSymbol> {
    Some(ResolvedSymbol {
        markup: format!("```mantis\n{}\n```", desc),
        word_span: span,
        def_span: None,
    })
}

struct Hit {
    #[allow(dead_code)]
    markup: String,
    #[allow(dead_code)]
    word_span: Span,
    #[allow(dead_code)]
    def_span: Option<Span>,
}

// ── Locals collection ────────────────────────────────────────────────────

fn collect_locals(block: &Block, src: &str, out: &mut Vec<LocalEntry>) {
    for item in &block.items {
        match item {
            BlockItem::Statement(st) => match st {
                Statement::Let {
                    mutable,
                    name,
                    ty,
                    value,
                    ..
                } => {
                    collect_locals_in_expr(value, src, out);
                    out.push(LocalEntry {
                        name: name.name.clone(),
                        ty: ty.as_ref().map(type_expr_to_string),
                        mutable: *mutable,
                        decl_span: name.span,
                        value_snippet: scope_value_snippet(src, value),
                    });
                }
                Statement::Return { value, .. } => {
                    if let Some(v) = value {
                        collect_locals_in_expr(v, src, out);
                    }
                }
                Statement::Expr { expr, .. } => collect_locals_in_expr(expr, src, out),
                _ => {}
            },
            BlockItem::IfChain(chain) => {
                collect_locals_in_expr(&chain.if_block.condition, src, out);
                collect_locals(&chain.if_block.body, src, out);
                for elif in &chain.elif_blocks {
                    collect_locals_in_expr(&elif.condition, src, out);
                    collect_locals(&elif.body, src, out);
                }
                if let Some(b) = &chain.else_block {
                    collect_locals(b, src, out);
                }
            }
            BlockItem::Loop(l) => collect_locals(&l.body, src, out),
            BlockItem::Match(m) => {
                collect_locals_in_expr(&m.scrutinee, src, out);
                for arm in &m.arms {
                    collect_locals_in_expr(&arm.pattern, src, out);
                    collect_locals(&arm.body, src, out);
                }
            }
            BlockItem::Block(b) => collect_locals(b, src, out),
        }
    }
}

fn collect_locals_in_expr(expr: &Expr, src: &str, out: &mut Vec<LocalEntry>) {
    match expr {
        Expr::Binary { lhs, rhs, .. } => {
            collect_locals_in_expr(lhs, src, out);
            collect_locals_in_expr(rhs, src, out);
        }
        Expr::Unary { operand, .. } => collect_locals_in_expr(operand, src, out),
        Expr::Call { callee, args, .. } => {
            collect_locals_in_expr(callee, src, out);
            for a in args {
                collect_locals_in_expr(a, src, out);
            }
        }
        Expr::Field { object, .. } => collect_locals_in_expr(object, src, out),
        Expr::Cast { expr, .. } => collect_locals_in_expr(expr, src, out),
        Expr::StructInit { fields, .. } => {
            for f in fields {
                collect_locals_in_expr(&f.value, src, out);
            }
        }
        Expr::ArrayInit { elements, .. } => {
            for e in elements {
                collect_locals_in_expr(e, src, out);
            }
        }
        Expr::Await { expr, .. } | Expr::Yield { expr, .. } | Expr::Propagate { expr, .. } => {
            collect_locals_in_expr(expr, src, out)
        }
        Expr::PointerAssign { target, value, .. } => {
            collect_locals_in_expr(target, src, out);
            collect_locals_in_expr(value, src, out);
        }
        Expr::Generic { base, .. } => collect_locals_in_expr(base, src, out),
        Expr::Lambda { decl, .. } => {
            if let Some(b) = &decl.body {
                collect_locals(b, src, out);
            }
        }
        Expr::AsyncBlock { body, .. } => collect_locals(body, src, out),
        _ => {}
    }
}

// ── Rendering helpers ────────────────────────────────────────────────────

fn fn_signature(f: &FnDecl, container: Option<&str>) -> String {
    let mut sig = String::new();
    if f.is_extern {
        sig.push_str("extern ");
    }
    if f.is_async {
        sig.push_str("async ");
    }
    sig.push_str("fn ");
    sig.push_str(f.name.as_ref().and_then(|n| n.as_name()).unwrap_or("fn"));
    sig.push('(');
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            format!(
                "{}{} {}",
                if p.mutable { "mut " } else { "" },
                p.name.name,
                type_expr_to_string(&p.ty)
            )
        })
        .collect();
    sig.push_str(&params.join(", "));
    sig.push(')');
    if let Some(rt) = &f.return_type {
        sig.push_str(&format!(" {}", type_expr_to_string(rt)));
    }
    let mut md = format!("```mantis\n{}\n```", sig);

    let non_self: Vec<_> = f.params.iter().filter(|p| p.name.name != "self").collect();
    if !non_self.is_empty() {
        md.push_str("\n---\n**Parameters**\n");
        for p in non_self {
            md.push_str(&format!(
                "- `{}` — `{}`{}\n",
                p.name.name,
                type_expr_to_string(&p.ty),
                if p.mutable { " *(mutable)*" } else { "" }
            ));
        }
    }
    if let Some(rt) = &f.return_type {
        let rt_str = type_expr_to_string(rt);
        if !rt_str.is_empty() && rt_str != "void" {
            md.push_str(&format!("\n**Returns** `{}`\n", rt_str));
        }
    }
    if !f.where_clause.is_empty() {
        md.push_str("\n**Bounds**\n");
        for wb in &f.where_clause {
            md.push_str(&format!(
                "- `{}` : {}\n",
                type_expr_to_string(&wb.target),
                wb.bounds
                    .iter()
                    .map(type_expr_to_string)
                    .collect::<Vec<_>>()
                    .join(" + ")
            ));
        }
    }
    if let Some(c) = container {
        md.push_str(&format!("\n*{}*", c));
    }
    if f.is_extern {
        md.push_str("\n*extern — provided by libc at link time*");
    }
    md.trim_end().to_string()
}

fn type_expr_to_string(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Named(id) => id.name.clone(),
        TypeExpr::Nested(a, b) => format!("{}.{}", type_expr_to_string(a), type_expr_to_string(b)),
        TypeExpr::Generic(base, gens) => format!(
            "{}[{}]",
            type_expr_to_string(base),
            gens.iter()
                .map(type_expr_to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExpr::Ref(inner, is_mut) => format!(
            "@{}{}",
            if *is_mut { "mut " } else { "" },
            type_expr_to_string(inner)
        ),
        TypeExpr::Function(params, ret) => format!(
            "({}) {}",
            params
                .iter()
                .map(type_expr_to_string)
                .collect::<Vec<_>>()
                .join(", "),
            type_expr_to_string(ret)
        ),
        TypeExpr::Unknown => String::new(),
    }
}

fn type_kind_label(t: &mantis_parser::ast::TypeDef) -> String {
    match &t.definition {
        TypeDefBody::Struct(_) => "struct".to_string(),
        TypeDefBody::Enum(_) => "enum".to_string(),
        TypeDefBody::Alias(_) => "type alias".to_string(),
    }
}

fn symbol_kind_for_typedef(t: &mantis_parser::ast::TypeDef) -> u32 {
    match &t.definition {
        TypeDefBody::Enum(_) => 10,  // Enum
        TypeDefBody::Alias(_) => 25, // TypeAlias
        TypeDefBody::Struct(_) => 23,
    }
}

fn render_type_doc(t: &TypeEntry) -> String {
    let name = t.name.as_str();
    match &t.def.definition {
        TypeDefBody::Alias(a) => {
            format!("```mantis\ntype {} = {}\n```", name, type_expr_to_string(a))
        }
        TypeDefBody::Struct(sd) => {
            let mut md = format!("```mantis\ntype {} = struct {{\n", name);
            for f in &sd.fields {
                md.push_str(&format!(
                    "    {} {},\n",
                    f.name.name,
                    type_expr_to_string(&f.ty)
                ));
            }
            md.push_str("}\n```\n---\n**Fields**\n");
            for f in &sd.fields {
                md.push_str(&format!(
                    "- `{}` — `{}`\n",
                    f.name.name,
                    type_expr_to_string(&f.ty)
                ));
            }
            md
        }
        TypeDefBody::Enum(en) => {
            let mut md = format!("```mantis\ntype {} = enum {{\n", name);
            for v in &en.variants {
                let fields = v
                    .fields
                    .iter()
                    .map(type_expr_to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                md.push_str(&format!("    {}({}),\n", v.name.name, fields));
            }
            md.push_str("}\n```\n---\n**Variants**\n");
            for v in &en.variants {
                let fields = v
                    .fields
                    .iter()
                    .map(type_expr_to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                md.push_str(&format!("- `{}` — `({})`\n", v.name.name, fields));
            }
            md
        }
    }
}

// ── Source / span utilities ──────────────────────────────────────────────

fn contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end.max(span.start + 1)
}

fn source_slice(src: &str, span: Span) -> String {
    src.get(span.start..span.end.max(span.start))
        .unwrap_or("")
        .to_string()
}

fn source_slice_trimmed(src: &str, span: Span, max: usize) -> String {
    let raw = source_slice(src, span);
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > max {
        let cut: String = collapsed.chars().take(max).collect();
        format!("{} …", cut)
    } else {
        collapsed
    }
}

fn scope_value_snippet(src: &str, value: &Expr) -> Option<String> {
    let s = source_slice_trimmed(src, value.span(), 40);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn strip_ref_prefix(ty: &str) -> &str {
    let mut t = ty.trim();
    loop {
        if let Some(rest) = t
            .strip_prefix("@mut ")
            .or_else(|| t.strip_prefix("&mut "))
            .or_else(|| t.strip_prefix('@'))
            .or_else(|| t.strip_prefix('&'))
        {
            t = rest.trim();
        } else {
            break;
        }
    }
    t
}

fn base_type_name(ty: &str) -> &str {
    let t = strip_ref_prefix(ty);
    match t.find('[') {
        Some(i) => &t[..i],
        None => t,
    }
}

fn split_impl_target(container: &str) -> Option<&str> {
    // "impl Drop for Vec[T]" → "Vec[T]"; "impl String" → "String"
    if let Some(idx) = container.find(" for ") {
        Some(container[idx + 5..].trim())
    } else {
        container.strip_prefix("impl ").map(str::trim)
    }
}

fn contained_type_span(ty: &TypeExpr, offset: usize) -> Option<Span> {
    match ty {
        TypeExpr::Named(id) => {
            if contains(id.span, offset) {
                Some(id.span)
            } else {
                None
            }
        }
        TypeExpr::Nested(a, b) => {
            contained_type_span(a, offset).or_else(|| contained_type_span(b, offset))
        }
        TypeExpr::Generic(base, gens) => contained_type_span(base, offset).or_else(|| {
            for g in gens {
                if let Some(sp) = contained_type_span(g, offset) {
                    return Some(sp);
                }
            }
            None
        }),
        TypeExpr::Ref(inner, _) => contained_type_span(inner, offset),
        TypeExpr::Function(params, ret) => {
            for p in params {
                if let Some(sp) = contained_type_span(p, offset) {
                    return Some(sp);
                }
            }
            contained_type_span(ret, offset)
        }
        TypeExpr::Unknown => None,
    }
}

fn module_alias_definition(uri: &str, src: &str, offset: usize) -> Option<Location> {
    let (member, member_span) = word_at_offset(src, offset)?;
    let start = member_span.start;
    if start == 0 || src.as_bytes().get(start - 1) != Some(&b'.') {
        return None;
    }
    let (alias, _) = word_at_offset(src, start - 2)?;
    let program = parse(src).ok()?;
    let path = program.declarations.iter().find_map(|decl| match decl {
        Declaration::Use(u) if u.alias.as_ref().map(|a| a.name.as_str()) == Some(alias.as_str()) => {
            Some(u.path.iter().map(|p| p.name.clone()).collect::<Vec<_>>())
        }
        Declaration::Import(i) if i.alias.as_ref().map(|a| a.name.as_str()) == Some(alias.as_str()) => {
            Some(i.path.iter().map(|p| p.name.clone()).collect::<Vec<_>>())
        }
        _ => None,
    })?;
    let (module_uri, module_src) = resolve_module_source(uri, &path)?;
    let module = parse(&module_src).ok()?;
    let span = module.declarations.iter().find_map(|decl| match decl {
        Declaration::Function(f) => f.name.as_ref().and_then(|n| {
            (n.as_name() == Some(member.as_str())).then_some(n.span())
        }),
        Declaration::TypeDef(t) => (t.name.as_name() == Some(member.as_str())).then_some(t.name.span()),
        Declaration::Trait(t) => (t.name.as_name() == Some(member.as_str())).then_some(t.name.span()),
        Declaration::Static(s) => (s.name.name == member).then_some(s.name.span),
        _ => None,
    })?;
    Some(Location {
        uri: module_uri,
        range: span_to_range(&module_src, span),
    })
}

fn module_path_definition(uri: &str, src: &str, offset: usize) -> Option<Location> {
    let program = parse(src).ok()?;
    let path = program.declarations.iter().find_map(|decl| {
        let (parts, spans) = match decl {
            Declaration::Use(u) => (&u.path, u.path.iter().map(|p| p.span).collect::<Vec<_>>()),
            Declaration::Import(i) => (&i.path, i.path.iter().map(|p| p.span).collect::<Vec<_>>()),
            _ => return None,
        };
        let index = spans
            .iter()
            .position(|span| span.start <= offset && offset <= span.end)?;
        Some(parts[..=index].iter().map(|part| part.name.clone()).collect::<Vec<_>>())
    })?;
    let (module_uri, module_src) = resolve_module_source(uri, &path)?;
    let target_span = module_src
        .lines()
        .next()
        .map(|line| Span::new(0, line.len()))
        .unwrap_or(Span::new(0, 0));
    Some(Location {
        uri: module_uri,
        range: span_to_range(&module_src, target_span),
    })
}

fn resolve_module_source(uri: &str, path: &[String]) -> Option<(String, String)> {
    let mut roots = Vec::new();
    if let Some(file) = uri.strip_prefix("file://") {
        if let Some(parent) = std::path::Path::new(file).parent() {
            roots.push(parent.to_path_buf());
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        roots.push(std::path::PathBuf::from(format!("{home}/.mantis/pkgcache")));
        roots.push(std::path::PathBuf::from(format!("{home}/.mantis/packages")));
    }
    roots.push(std::env::current_dir().ok()?);
    let relative = path.iter().collect::<std::path::PathBuf>();
    for root in roots {
        let base = root.join(&relative);
        let candidates = [
            base.with_extension("ms"),
            base.join("src/lib.ms"),
            base.join("src/main.ms"),
            base.join("lib.ms"),
            base.join("mod.ms"),
        ];
        for candidate in candidates {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                return Some((format!("file://{}", candidate.display()), content));
            }
        }
    }
    None
}

fn word_at_offset(src: &str, offset: usize) -> Option<(String, Span)> {
    let bytes = src.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut start = offset.min(bytes.len() - 1);
    let mut end = start;
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    while end < bytes.len() && is_word(bytes[end]) {
        end += 1;
    }
    if start >= end {
        return None;
    }
    Some((src[start..end].to_string(), Span::new(start, end)))
}

/// Top-level analysis entry point.
pub fn analyze_at_offset(prog: &Program, src: &str, offset: usize) -> Option<ResolvedSymbol> {
    let analyzer = Analyzer::new(prog, src, offset);
    analyzer.analyze(prog)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Built-in Documentation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const KEYWORD_DOCS: &[(&str, &str)] = &[
    ("pub", "Makes a function visible outside its module."),
    ("fn", "Declares a function.\n\n```mantis\nfn add(a i64, b i64) i64 { return a + b; }\n```"),
    ("let", "Declares an immutable local binding.\n\n```mantis\nlet x = 42;\nlet y i64 = 7;\n```"),
    ("mut", "Declares a mutable binding or parameter."),
    ("return", "Returns from the current function."),
    ("if", "Conditional branch."),
    ("elif", "Else-if branch."),
    ("else", "Fallback branch."),
    ("loop", "Infinite loop; exit with `break`.\n\n```mantis\nloop {\n    break;\n}\n```"),
    ("break", "Exits the enclosing loop."),
    ("continue", "Skips to the next loop iteration."),
    ("match", "Pattern matching on enums and values."),
    ("type", "Declares a struct, enum, or alias:\n\n```mantis\ntype Foo = struct { x i64 }\ntype Opt[T] = enum { Some(T), None }\n```"),
    ("struct", "Product type definition."),
    ("enum", "Sum type definition."),
    ("trait", "Declares an interface with required methods."),
    ("impl", "Implements a trait or inherent methods for a type."),
    ("extern", "Declares a function provided by libc at link time:\n\n```mantis\nfn write(fd i64, ptr i64, n i64) i64 extern;\n```"),
    ("import", "Imports another module:\n\n```mantis\nimport std.net;\n```"),
    ("use", "Brings a module's declarations into scope:\n\n```mantis\nuse std;\n```"),
    ("async", "Marks an async function or block (Mantis uses a scheduler-less poll model)."),
    ("await", "Suspends until an async value resolves."),
    ("yield", "Yields a value from a generator-style async fn."),
    ("static", "Module-level variable with stable address:\n\n```mantis\nstatic X i64 = 10;\nstatic const K i32 = 3;\n```"),
    ("const", "Compile-time constant modifier for `static`."),
];

fn primitive_doc(word: &str) -> Option<String> {
    let doc: &str = match word {
        "i8" => "`i8` — 8-bit signed integer",
        "i16" => "`i16` — 16-bit signed integer",
        "i32" => "`i32` — 32-bit signed integer",
        "i64" => "`i64` — 64-bit signed integer (default integer type)",
        "u8" => "`u8` — 8-bit unsigned integer (byte)",
        "u16" => "`u16` — 16-bit unsigned integer",
        "u32" => "`u32` — 32-bit unsigned integer",
        "u64" => "`u64` — 64-bit unsigned integer (sizes/lengths)",
        "f32" => "`f32` — 32-bit float",
        "f64" => "`f64` — 64-bit float (default float type)",
        "bool" => "`bool` — `true` / `false`",
        "char" => "`char` — single character",
        "void" => "`void` — no value",
        "StrSlice" => "`StrSlice` — borrowed view of UTF-8 bytes (`pointer`, `len`)",
        "String" => "`String` — growable owned UTF-8 string backed by `Vec[u8]`",
        "Vec" => "`Vec[T]` — growable array\n- `with_capacity(cap)` / `new()`\n- `push`, `pop`, `get`, `set`\n- `len`, `capacity`, `free`",
        "Option" => "`Option[T]` — `Some(T)` or `None`",
        "Result" => "`Result[T, E]` — `Ok(T)` or `Err(E)`",
        "Box" => "`Box[T]` — owning heap allocation\n- `Box.new(value)` allocates and stores\n- `get()` / `set(v)` read/write\n- `free()` releases\n- `as_ptr()` escape hatch",
        "GlobalAllocator" => "`GlobalAllocator` — default heap allocator (`alloc`, `dealloc`, `copy_bytes`)",
        "self" => "The method receiver.",
        "Self" => "The type being implemented.",
        _ => return None,
    };
    Some(format!("```mantis\n{}\n```", doc))
}

const BUILTIN_FN_DOCS: &[(&str, &str)] = &[
    (
        "println",
        "```mantis\nprintln(...)\n```\nPrints a line to stdout.",
    ),
    ("print", "```mantis\nprint(...)\n```\nPrints to stdout without newline."),
    (
        "malloc",
        "```mantis\nmalloc(size i64) -> raw ptr\n```\nlibc malloc — prefer `GlobalAllocator.alloc` or `Box.new`.",
    ),
    ("free", "```mantis\nfree(ptr)\n```\nlibc free — prefer `Box.free` / `Vec.free`."),
    (
        "memcpy",
        "```mantis\nmemcpy(dest, src, n)\n```\nlibc memcpy — prefer `GlobalAllocator.copy_bytes`.",
    ),
    ("size_of", "```mantis\n#size_of(T)\n```\nCompile-time size of a type in bytes."),
];

fn keyword_doc(word: &str) -> Option<&'static str> {
    KEYWORD_DOCS
        .iter()
        .find(|(k, _)| *k == word)
        .map(|(_, d)| *d)
}

fn builtin_fn_doc(word: &str) -> Option<&'static str> {
    BUILTIN_FN_DOCS
        .iter()
        .find(|(k, _)| *k == word)
        .map(|(_, d)| *d)
}

fn intrinsic_doc(name: &str) -> Option<String> {
    let doc: &str = match name {
        "size_of" => "#size_of(T) — compile-time size of T in bytes",
        "init" => "#init(T) — zero-initialized heap instance as @T",
        "ref" | "as_ref" => "#ref(x) — convert pointer to shared reference",
        "ptr" => "#ptr(x) — convert to mutable pointer",
        "free" => "#free(ptr) — free a #init/#malloc allocation",
        "malloc" => "#malloc(bytes) — raw allocation returning @u8",
        "type" => "#type(x) — compile-time type introspection (macros)",
        _ => return None,
    };
    Some(format!("```mantis\n{}\n```", doc))
}

// Convenience wrappers used by completion.
struct FnCompletion {
    name: String,
    signature: String,
    container: Option<String>,
}

fn collect_top_functions(prog: &Program) -> Vec<FnCompletion> {
    let mut out = Vec::new();
    for decl in &prog.declarations {
        match decl {
            Declaration::Function(f) => out.push(FnCompletion {
                name: f
                    .name
                    .as_ref()
                    .and_then(|n| n.as_name())
                    .unwrap_or("fn")
                    .to_string(),
                signature: fn_signature(f, None),
                container: None,
            }),
            Declaration::Trait(tr) => {
                let tn = tr.name.as_name().unwrap_or("Trait").to_string();
                for m in &tr.methods {
                    out.push(FnCompletion {
                        name: m
                            .name
                            .as_ref()
                            .and_then(|n| n.as_name())
                            .unwrap_or("fn")
                            .to_string(),
                        signature: fn_signature(m, None),
                        container: Some(format!("trait {}", tn)),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

struct TypeCompletion {
    name: String,
    kind_label: String,
    summary: Option<String>,
}

fn collect_top_types(prog: &Program) -> Vec<TypeCompletion> {
    let mut out = Vec::new();
    for decl in &prog.declarations {
        if let Declaration::TypeDef(t) = decl {
            let name = t.name.as_name().unwrap_or("Type").to_string();
            let summary = render_type_doc(&TypeEntry {
                name: name.clone(),
                def: t,
                def_span: t.name.span(),
            });
            out.push(TypeCompletion {
                name,
                kind_label: type_kind_label(t),
                summary: Some(summary),
            });
        }
    }
    out
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Diagnostic Computation & Position Conversion
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn compute_diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    match parse(source) {
        Ok(prog) => {
            // Run borrow checker
            let borrow_errors = BorrowChecker::check_program(source, &prog);
            for b in borrow_errors {
                diags.push(Diagnostic {
                    range: span_to_range(source, b.span),
                    severity: Some(1), // Error
                    code: Some("E0001".to_string()),
                    source: Some("mantis-borrow-checker".to_string()),
                    message: b.format(source),
                });
            }
        }
        Err(err) => match err {
            MantisError::Lex(e) => {
                diags.push(Diagnostic {
                    range: span_to_range(source, e.span),
                    severity: Some(1),
                    code: Some("E0002".to_string()),
                    source: Some("mantis-lexer".to_string()),
                    message: format!("unexpected token '{}'", e.slice),
                });
            }
            MantisError::Parse(e) => {
                diags.push(Diagnostic {
                    range: span_to_range(source, e.span),
                    severity: Some(1),
                    code: Some("E0003".to_string()),
                    source: Some("mantis-parser".to_string()),
                    message: format!("parse error: {}", e.message),
                });
            }
        },
    }

    diags
}

pub fn span_to_range(source: &str, span: Span) -> Range {
    let start_pos = offset_to_position(source, span.start);
    let end_pos = offset_to_position(source, span.end);
    Range {
        start: start_pos,
        end: end_pos,
    }
}

pub fn offset_to_position(source: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position {
        line,
        character: col,
    }
}

pub fn position_to_offset(source: &str, pos: Position) -> Option<usize> {
    let mut current_line = 0;
    let mut current_col = 0;
    for (i, ch) in source.char_indices() {
        if current_line == pos.line && current_col == pos.character {
            return Some(i);
        }
        if ch == '\n' {
            if current_line == pos.line {
                return Some(i);
            }
            current_line += 1;
            current_col = 0;
        } else {
            current_col += 1;
        }
    }
    if current_line == pos.line {
        return Some(source.len());
    }
    None
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_diagnostics() {
        let source = "fn main() { let x = 10; let f = [@mut x] () i64 { return x; }; }";
        let diags = compute_diagnostics(source);
        assert!(!diags.is_empty());
        assert!(diags[0]
            .message
            .contains("cannot capture immutable variable"));
    }

    fn server_with(src: &str) -> MantisLanguageServer {
        let mut s = MantisLanguageServer::new();
        s.did_open("test://ms".into(), src.to_string());
        s
    }

    fn hover_at(server: &MantisLanguageServer, src: &str, needle: &str) -> Option<String> {
        let off = src.find(needle)? + needle.len() / 2;
        server
            .hover("test://ms", offset_to_position(src, off))
            .map(|h| h.contents.value)
    }

    #[test]
    fn hover_shows_function_signature_with_params() {
        let src = "fn add(a i64, b i64) i64 {\n    return a + b;\n}\n\nfn main() i32 {\n    let sum = add(1, 2);\n    return sum as i32;\n}\n";
        let s = server_with(src);
        let h = hover_at(&s, src, "add(1").expect("hover callee");
        assert!(h.contains("fn add"), "got: {}", h);
        assert!(h.contains("**Parameters**"), "got: {}", h);
        assert!(h.contains("`a` — `i64`"), "got: {}", h);
        assert!(h.contains("**Returns** `i64`"), "got: {}", h);
    }

    #[test]
    fn hover_shows_let_variable_declared_type() {
        let src = "fn main() i32 {\n    let count i64 = 3;\n    return count as i32;\n}\n";
        let s = server_with(src);
        let h = hover_at(&s, src, "count as").expect("hover local");
        assert!(h.contains("let count: i64"), "got: {}", h);
    }

    #[test]
    fn hover_on_binding_name_shows_decl() {
        let src = "fn main() i32 {\n    let total i64 = 9;\n    return total as i32;\n}\n";
        let s = server_with(src);
        let h = hover_at(&s, src, "total i64").expect("hover binding");
        assert!(h.contains("let total: i64"), "got: {}", h);
    }

    #[test]
    fn hover_shows_struct_def_and_fields() {
        let src = "type Point = struct {\n    x i64,\n    y i64\n}\n\nfn area(p Point) i64 {\n    return p.x * p.y;\n}\n";
        let s = server_with(src);
        let h = hover_at(&s, src, "Point)").expect("hover type param");
        assert!(h.contains("type Point"), "got: {}", h);
        assert!(h.contains("Fields"), "got: {}", h);
    }

    #[test]
    fn hover_resolves_self_field_to_struct_field() {
        let src = "type Acc = struct {\n    total i64,\n}\n\nimpl Acc {\n    fn bump(self @Acc) i64 {\n        return self.total;\n    }\n}\n";
        let s = server_with(src);
        let h = hover_at(&s, src, ".total;").expect("hover self field");
        assert!(
            h.contains("total: i64") || h.contains("Acc.total"),
            "got: {}",
            h
        );
    }

    #[test]
    fn hover_method_call_shows_signature() {
        let src = "type Counter = struct {\n    n i64,\n}\n\nimpl Counter {\n    fn inc(self @Counter, by i64) i64 {\n        return self.n + by;\n    }\n}\n\nfn main() i32 {\n    let c = Counter { n: 0 };\n    let r = c.inc(2);\n    return r as i32;\n}\n";
        let s = server_with(src);
        let h = hover_at(&s, src, ".inc(2)").expect("hover method");
        assert!(h.contains("inc"), "got: {}", h);
        assert!(h.contains("`by` — `i64`"), "got: {}", h);
    }

    #[test]
    fn hover_works_mid_typing_for_keywords() {
        let src = "fn main() i32 {\n    let x = tru;\n    return 0;\n}\n";
        let s = server_with(src);
        // 'tru' won't parse as valid expr necessarily; ensure no panic
        let _ = hover_at(&s, src, "tru");
        let src2 = "fn main() i32 {\n    let x = true;\n    return 0;\n}\n";
        let s2 = server_with(src2);
        let h = hover_at(&s2, src2, "true").expect("primitive");
        assert!(h.contains("bool"), "got: {}", h);
    }

    #[test]
    fn pub_is_documented_and_completed_as_a_keyword() {
        let src = "pub fn hello() i32 { return 0; }\n";
        let s = server_with(src);
        let h = hover_at(&s, src, "pub fn").expect("pub hover");
        assert!(h.contains("visible outside its module"), "got: {}", h);
        assert!(s
            .completion("test://ms", Position { line: 0, character: 0 })
            .iter()
            .any(|item| item.label == "pub"));
    }

    #[test]
    fn definition_jumps_to_function_decl() {
        let src = "fn helper() i64 {\n    return 1;\n}\n\nfn main() i32 {\n    let v = helper();\n    return v as i32;\n}\n";
        let s = server_with(src);
        let off = src.find("helper();").unwrap();
        let r = s
            .definition("test://ms", offset_to_position(src, off))
            .expect("definition");
        assert_eq!(r.start.line, 0, "should jump to declaration on line 0");
    }

    #[test]
    fn definition_follows_module_alias_into_file() {
        let root = std::env::temp_dir().join(format!("mantis-lsp-{}", std::process::id()));
        let module_dir = root.join("demo").join("src");
        std::fs::create_dir_all(&module_dir).unwrap();
        let module_file = module_dir.join("lib.ms");
        std::fs::write(&module_file, "fn hello() i64 { return 1; }\n").unwrap();

        let uri = format!("file://{}", root.join("main.ms").display());
        let src = "use demo as d;\nfn main() i32 { return d.hello() as i32; }\n";
        let mut server = MantisLanguageServer::new();
        server.did_open(uri.clone(), src.to_string());
        let offset = src.find("hello").unwrap();
        let location = server
            .definition_location(&uri, offset_to_position(src, offset))
            .expect("aliased module definition");

        assert_eq!(location.uri, format!("file://{}", module_file.display()));
        assert_eq!(location.range.start.line, 0);
        assert_eq!(location.range.start.character, 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn definition_follows_plain_use_module_path() {
        let root = std::env::temp_dir().join(format!("mantis-lsp-use-{}", std::process::id()));
        let module_dir = root.join("std").join("src");
        std::fs::create_dir_all(&module_dir).unwrap();
        let module_file = module_dir.join("lib.ms");
        std::fs::write(&module_file, "pub fn hello() i64 { return 1; }\n").unwrap();

        let uri = format!("file://{}", root.join("main.ms").display());
        let src = "use std;\nfn main() i32 { return 0; }\n";
        let mut server = MantisLanguageServer::new();
        server.did_open(uri.clone(), src.to_string());
        let offset = src.find("std").unwrap();
        let location = server
            .definition_location(&uri, offset_to_position(src, offset))
            .expect("plain use module definition");

        assert_eq!(location.uri, format!("file://{}", module_file.display()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hover_enum_variant() {
        let src = "type Opt = enum {\n    Some(i64),\n    None\n}\n\nfn go(o Opt) i64 {\n    match o {\n        Opt.Some(v) : { return v; },\n        Opt.None : { return 0; }\n    }\n}\n";
        let s = server_with(src);
        let h = hover_at(&s, src, ".None :").expect("hover variant");
        assert!(h.contains("None"), "got: {}", h);
        assert!(h.contains("Opt"), "got: {}", h);
    }
}
