use cranelift::prelude::*;
use std::collections::HashMap;
use std::fmt;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Variable metadata
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Information stored alongside every variable binding in a scope.
#[derive(Debug, Clone)]
pub struct VarInfo {
    /// The Cranelift SSA variable handle.
    pub variable: cranelift::frontend::Variable,
    /// The Cranelift IR type of this variable (e.g. `types::I64`).
    pub ty: types::Type,
    /// Whether the binding was declared as mutable (`mut`).
    pub is_mutable: bool,
}

impl VarInfo {
    pub fn new(variable: cranelift::frontend::Variable, ty: types::Type, is_mutable: bool) -> Self {
        Self {
            variable,
            ty,
            is_mutable,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Resolution errors
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Errors that can arise when resolving or inserting variables.
#[derive(Debug)]
pub enum ScopeError {
    /// Variable was not found in any enclosing scope.
    Undeclared {
        name: String,
    },
    /// An assignment targeted an immutable binding.
    AssignToImmutable {
        name: String,
    },
    /// A variable with the same name already exists in the *current* scope.
    AlreadyDeclared {
        name: String,
        depth: usize,
    },
    /// A break/continue was used outside of a loop.
    NotInLoop {
        kind: &'static str,
    },
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeError::Undeclared { name } => {
                write!(f, "undeclared variable `{}`", name)
            }
            ScopeError::AssignToImmutable { name } => {
                write!(f, "cannot assign to immutable variable `{}`", name)
            }
            ScopeError::AlreadyDeclared { name, depth } => {
                write!(
                    f,
                    "variable `{}` already declared in the current scope (depth {})",
                    name, depth
                )
            }
            ScopeError::NotInLoop { kind } => {
                write!(f, "`{}` used outside of a loop", kind)
            }
        }
    }
}

impl std::error::Error for ScopeError {}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  ScopeEnv
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Lexical scope environment for the simple Cranelift backend.
///
/// Manages a stack of scopes (each a `HashMap<String, VarInfo>`), as well as
/// the loop break/continue target stacks.
pub struct ScopeEnv {
    scopes: Vec<HashMap<String, VarInfo>>,
    pub break_targets: Vec<cranelift::codegen::ir::Block>,
    pub continue_targets: Vec<cranelift::codegen::ir::Block>,
}

impl ScopeEnv {
    /// Creates a new environment with a single (global) scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
        }
    }

    // ── Scope management ────────────────────────────────────────────────

    /// Push a new empty scope onto the stack.
    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the innermost scope.  Returns the bindings that were in it.
    pub fn exit_scope(&mut self) -> Option<HashMap<String, VarInfo>> {
        // Never pop the root scope.
        if self.scopes.len() > 1 {
            self.scopes.pop()
        } else {
            None
        }
    }

    /// Current nesting depth (0 = root scope).
    pub fn depth(&self) -> usize {
        self.scopes.len() - 1
    }

    // ── Variable resolution ─────────────────────────────────────────────

    /// Look up a variable by name, walking from the innermost scope outward.
    /// Returns `None` if not found in any scope.
    pub fn get_var(&self, name: &str) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info);
            }
        }
        None
    }

    /// Look up a variable, returning a `ScopeError::Undeclared` on failure.
    pub fn resolve(&self, name: &str) -> Result<&VarInfo, ScopeError> {
        self.get_var(name).ok_or_else(|| ScopeError::Undeclared {
            name: name.to_string(),
        })
    }

    /// Resolve and assert that the variable is mutable (for assignments).
    pub fn resolve_mut(&self, name: &str) -> Result<&VarInfo, ScopeError> {
        let info = self.resolve(name)?;
        if !info.is_mutable {
            return Err(ScopeError::AssignToImmutable {
                name: name.to_string(),
            });
        }
        Ok(info)
    }

    // ── Variable insertion ──────────────────────────────────────────────

    /// Insert a variable into the *current* (innermost) scope.
    ///
    /// Returns `Ok(())` on success, or `Err(ScopeError::AlreadyDeclared)` if
    /// the name is already bound in the current scope.
    pub fn insert_var(&mut self, name: String, info: VarInfo) -> Result<(), ScopeError> {
        let depth = self.depth();
        let scope = self.scopes.last_mut().expect("scope stack is never empty");
        if scope.contains_key(&name) {
            return Err(ScopeError::AlreadyDeclared { name, depth });
        }
        scope.insert(name, info);
        Ok(())
    }

    /// Insert (or shadow) a variable in the current scope regardless of
    /// whether one already exists.  Returns the old binding if any.
    pub fn insert_var_shadow(&mut self, name: String, info: VarInfo) -> Option<VarInfo> {
        let scope = self.scopes.last_mut().expect("scope stack is never empty");
        scope.insert(name, info)
    }

    // ── Loop target helpers ─────────────────────────────────────────────

    pub fn current_break_target(&self) -> Result<cranelift::codegen::ir::Block, ScopeError> {
        self.break_targets
            .last()
            .copied()
            .ok_or(ScopeError::NotInLoop { kind: "break" })
    }

    pub fn current_continue_target(&self) -> Result<cranelift::codegen::ir::Block, ScopeError> {
        self.continue_targets
            .last()
            .copied()
            .ok_or(ScopeError::NotInLoop { kind: "continue" })
    }
}
