use crate::ast::*;
use crate::token::Span;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BorrowDiagnostic {
    pub message: String,
    pub span: Span,
    pub note: Option<String>,
    pub help: Option<String>,
}

impl BorrowDiagnostic {
    pub fn format(&self, source: &str) -> String {
        let (line, col) = get_line_col(source, self.span.start);
        let snippet = get_snippet(source, line);
        let pointer = " ".repeat(col.saturating_sub(1)) + "^";
        let mut out = format!(
            "borrow error: {}\n --> line {}:{}\n  |\n{} | {}\n  | {}",
            self.message, line, col, line, snippet, pointer
        );
        if let Some(note) = &self.note {
            out.push_str(&format!("\n  = note: {}", note));
        }
        if let Some(help) = &self.help {
            out.push_str(&format!("\n  = help: {}", help));
        }
        out
    }
}

fn get_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn get_snippet(source: &str, line_no: usize) -> &str {
    source.lines().nth(line_no.saturating_sub(1)).unwrap_or("")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableState {
    Active { is_mutable: bool },
    BorrowedImmutable { count: usize },
    BorrowedMutable,
    Moved,
}

#[derive(Debug, Clone)]
pub struct ScopeFrame {
    pub variables: HashMap<String, (VariableState, Span)>,
}

pub struct BorrowChecker<'a> {
    pub source: &'a str,
    pub scopes: Vec<ScopeFrame>,
    pub diagnostics: Vec<BorrowDiagnostic>,
}

impl<'a> BorrowChecker<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            scopes: vec![ScopeFrame {
                variables: HashMap::new(),
            }],
            diagnostics: Vec::new(),
        }
    }

    pub fn check_program(source: &'a str, program: &Program) -> Vec<BorrowDiagnostic> {
        let mut checker = BorrowChecker::new(source);
        for decl in &program.declarations {
            checker.check_declaration(decl);
        }
        checker.diagnostics
    }

    fn push_scope(&mut self) {
        self.scopes.push(ScopeFrame {
            variables: HashMap::new(),
        });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn register_variable(&mut self, name: &str, is_mutable: bool, span: Span) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.variables.insert(
                name.to_string(),
                (VariableState::Active { is_mutable }, span),
            );
        }
    }

    fn find_variable(&self, name: &str) -> Option<(VariableState, Span)> {
        for scope in self.scopes.iter().rev() {
            if let Some(var) = scope.variables.get(name) {
                return Some(var.clone());
            }
        }
        None
    }

    fn check_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Function(fn_decl) => self.check_fn_decl(fn_decl),
            Declaration::Impl(impl_block) => {
                for method in &impl_block.methods {
                    self.check_fn_decl(method);
                }
            }
            Declaration::Trait(trait_def) => {
                for method in &trait_def.methods {
                    self.check_fn_decl(method);
                }
            }
            _ => {}
        }
    }

    fn check_fn_decl(&mut self, fn_decl: &FnDecl) {
        self.push_scope();
        for param in &fn_decl.params {
            self.register_variable(&param.name.name, param.mutable, param.span);
        }
        if let Some(body) = &fn_decl.body {
            self.check_block(body);
        }
        self.pop_scope();
    }

    fn check_block(&mut self, block: &Block) {
        self.push_scope();
        for item in &block.items {
            match item {
                BlockItem::Statement(stmt) => self.check_statement(stmt),
                BlockItem::IfChain(if_chain) => {
                    self.check_expr(&if_chain.if_block.condition);
                    self.check_block(&if_chain.if_block.body);
                    for elif in &if_chain.elif_blocks {
                        self.check_expr(&elif.condition);
                        self.check_block(&elif.body);
                    }
                    if let Some(else_b) = &if_chain.else_block {
                        self.check_block(else_b);
                    }
                }
                BlockItem::Loop(loop_b) => self.check_block(&loop_b.body),
                BlockItem::Match(match_b) => {
                    self.check_expr(&match_b.scrutinee);
                    for arm in &match_b.arms {
                        self.check_expr(&arm.pattern);
                        self.check_block(&arm.body);
                    }
                }
                BlockItem::Block(sub_b) => self.check_block(sub_b),
            }
        }
        self.pop_scope();
    }

    fn check_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Let {
                name,
                mutable,
                value,
                span,
                ..
            } => {
                self.check_expr(value);
                self.register_variable(&name.name, *mutable, *span);
            }
            Statement::Return { value, .. } => {
                if let Some(v) = value {
                    self.check_expr(v);
                }
            }
            Statement::Expr { expr, .. } => {
                self.check_expr(expr);
            }
            _ => {}
        }
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(id) => {
                if let Some((state, _decl_span)) = self.find_variable(&id.name) {
                    if state == VariableState::Moved {
                        self.diagnostics.push(BorrowDiagnostic {
                            message: format!("use of moved value '{}'", id.name),
                            span: id.span,
                            note: Some(format!("'{}' was moved previously in this scope", id.name)),
                            help: Some(
                                "consider cloning or referencing the value instead".to_string(),
                            ),
                        });
                    }
                }
            }
            Expr::Lambda {
                captures,
                decl,
                span: _,
            } => {
                // Check each captured variable against outer scopes
                for cap in captures {
                    if let Some((state, _decl_span)) = self.find_variable(&cap.name.name) {
                        match state {
                            VariableState::Active { is_mutable } => {
                                if cap.kind == CaptureKind::MutRef && !is_mutable {
                                    self.diagnostics.push(BorrowDiagnostic {
                                        message: format!(
                                            "cannot capture immutable variable '{}' as mutable reference",
                                            cap.name.name
                                        ),
                                        span: cap.span,
                                        note: Some(format!(
                                            "'{}' is declared immutable in enclosing scope",
                                            cap.name.name
                                        )),
                                        help: Some(format!(
                                            "declare variable as `mut {}` to allow mutable captures",
                                            cap.name.name
                                        )),
                                    });
                                }
                            }
                            VariableState::Moved => {
                                self.diagnostics.push(BorrowDiagnostic {
                                    message: format!(
                                        "cannot capture moved variable '{}' in lambda",
                                        cap.name.name
                                    ),
                                    span: cap.span,
                                    note: Some(format!(
                                        "'{}' was moved before lambda definition",
                                        cap.name.name
                                    )),
                                    help: None,
                                });
                            }
                            _ => {}
                        }
                    } else if !is_builtin_symbol(&cap.name.name) {
                        self.diagnostics.push(BorrowDiagnostic {
                            message: format!(
                                "cannot capture undefined variable '{}' in lambda",
                                cap.name.name
                            ),
                            span: cap.span,
                            note: Some("variable is not defined in any outer scope".to_string()),
                            help: None,
                        });
                    }
                }

                // Verify inner function body with captures in scope
                self.push_scope();
                for cap in captures {
                    let is_mut = cap.kind == CaptureKind::MutRef;
                    self.register_variable(&cap.name.name, is_mut, cap.span);
                }
                for param in &decl.params {
                    self.register_variable(&param.name.name, param.mutable, param.span);
                }
                if let Some(body) = &decl.body {
                    self.check_block(body);
                }
                self.pop_scope();
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
            }
            Expr::Unary { operand, .. } => {
                self.check_expr(operand);
            }
            Expr::Call { callee, args, .. } => {
                self.check_expr(callee);
                for arg in args {
                    self.check_expr(arg);
                }
            }
            Expr::Field { object, .. } => {
                self.check_expr(object);
            }
            Expr::Cast { expr, .. } => {
                self.check_expr(expr);
            }
            Expr::StructInit { fields, .. } => {
                for f in fields {
                    self.check_expr(&f.value);
                }
            }
            Expr::ArrayInit { elements, .. } => {
                for el in elements {
                    self.check_expr(el);
                }
            }
            Expr::PointerAssign { target, value, .. } => {
                self.check_expr(target);
                self.check_expr(value);
            }
            Expr::Propagate { expr, .. } => {
                self.check_expr(expr);
            }
            Expr::Await { expr, .. } => {
                self.check_expr(expr);
            }
            Expr::Yield { expr, .. } => {
                self.check_expr(expr);
            }
            Expr::AsyncBlock { body, .. } => {
                self.check_block(body);
            }
            Expr::Generic { base, .. } => {
                self.check_expr(base);
            }
            _ => {}
        }
    }
}

fn is_builtin_symbol(name: &str) -> bool {
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
