//! Compiler configuration.
//!
//! Loaded from an optional `config.toml`, overridable per-flag from the CLI.
//! Every permission is ON by default so plain `mantis file.ms -e app` keeps
//! working with zero configuration.
//!
//! ```toml
//! [project]
//! name = "my_app"        # output name (default: input file stem)
//! type = "bin"           # "bin" | "lib"
//! out-dir = "./build"    # where artifacts land (default ./build)
//!
//! [compiler]
//! allow-external-functions = true
//! allow-syscalls = true
//! allow-unsafe = true
//! allow-implicit-conversions = true
//! ```

use serde::Deserialize;

/// libc functions that map 1:1 onto kernel syscalls.
pub const SYSCALL_FNS: &[&str] = &[
    "syscall",
    "read",
    "write",
    "open",
    "close",
    "exit",
    "fork",
    "execve",
    "waitpid",
    "ioctl",
    "pipe",
    "dup",
    "dup2",
    "mmap",
    "munmap",
    "kill",
    "getpid",
    "nanosleep",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectKind {
    Bin,
    Lib,
}

#[derive(Debug, Clone)]
pub struct MantisConfig {
    /// Declare/link C functions (`fn puts(...) extern;`).
    pub allow_external_functions: bool,
    /// Declare libc functions that are thin syscall wrappers.
    pub allow_syscalls: bool,
    /// Raw pointers (`@T`, `@=`), derefs and memory intrinsics (#malloc/#init/#free).
    pub allow_unsafe: bool,
    /// Auto-deref Box[T]: `b.x`, `b.area()`, `b.x += 1` operate through the box.
    pub allow_implicit_conversions: bool,
    pub project: ProjectConfig,
}

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: Option<String>,
    pub kind: ProjectKind,
    pub out_dir: String,
}

impl Default for MantisConfig {
    fn default() -> Self {
        MantisConfig {
            allow_external_functions: true,
            allow_syscalls: true,
            allow_unsafe: true,
            allow_implicit_conversions: true,
            project: ProjectConfig {
                name: None,
                kind: ProjectKind::Bin,
                out_dir: "./build".to_string(),
            },
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ConfigFile {
    project: ProjectSection,
    compiler: CompilerSection,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ProjectSection {
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(rename = "out-dir", alias = "out_dir")]
    out_dir: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct CompilerSection {
    #[serde(
        rename = "allow-external-functions",
        alias = "allow_external_functions"
    )]
    allow_external_functions: Option<bool>,
    #[serde(rename = "allow-syscalls", alias = "allow_syscalls")]
    allow_syscalls: Option<bool>,
    #[serde(rename = "allow-unsafe", alias = "allow_unsafe")]
    allow_unsafe: Option<bool>,
    #[serde(
        rename = "allow-implicit-conversions",
        alias = "allow_implicit_conversions"
    )]
    allow_implicit_conversions: Option<bool>,
}

impl MantisConfig {
    /// All permissions on, no project metadata.
    pub fn defaults() -> Self {
        Self::default()
    }

    pub fn from_toml(src: &str) -> Result<Self, String> {
        let file: ConfigFile =
            toml::from_str(src).map_err(|e| format!("invalid config.toml: {}", e))?;
        let mut cfg = Self::defaults();
        if let Some(name) = file.project.name {
            cfg.project.name = Some(name);
        }
        if let Some(kind) = &file.project.kind {
            cfg.project.kind = match kind.as_str() {
                "bin" | "binary" | "exe" => ProjectKind::Bin,
                "lib" | "library" => ProjectKind::Lib,
                other => return Err(format!("unknown project type '{}'", other)),
            };
        }
        if let Some(dir) = file.project.out_dir {
            cfg.project.out_dir = dir;
        }
        let c = file.compiler;
        if let Some(v) = c.allow_external_functions {
            cfg.allow_external_functions = v;
        }
        if let Some(v) = c.allow_syscalls {
            cfg.allow_syscalls = v;
        }
        if let Some(v) = c.allow_unsafe {
            cfg.allow_unsafe = v;
        }
        if let Some(v) = c.allow_implicit_conversions {
            cfg.allow_implicit_conversions = v;
        }
        Ok(cfg)
    }

    /// Find `config.toml`: current directory first, then upward.
    pub fn discover() -> Option<std::path::PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        let mut dir: Option<&std::path::Path> = Some(cwd.as_path());
        for _ in 0..16 {
            let d = dir?;
            let candidate = d.join("config.toml");
            if candidate.is_file() {
                return Some(candidate);
            }
            dir = d.parent();
        }
        None
    }

    pub fn load_file(path: &std::path::Path) -> Result<Self, String> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
        Self::from_toml(&src)
    }

    /// CLI overrides applied last — each `Some` wins over the file.
    pub fn apply_cli_overrides(
        &mut self,
        no_external_functions: bool,
        no_syscalls: bool,
        no_unsafe: bool,
        no_implicit_conversions: bool,
    ) {
        if no_external_functions {
            self.allow_external_functions = false;
        }
        if no_syscalls {
            self.allow_syscalls = false;
        }
        if no_unsafe {
            self.allow_unsafe = false;
        }
        if no_implicit_conversions {
            self.allow_implicit_conversions = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_permissive() {
        let c = MantisConfig::defaults();
        assert!(c.allow_external_functions);
        assert!(c.allow_syscalls);
        assert!(c.allow_unsafe);
        assert!(c.allow_implicit_conversions);
        assert_eq!(c.project.out_dir, "./build");
        assert_eq!(c.project.kind, ProjectKind::Bin);
    }

    #[test]
    fn parses_full_file() {
        let c = MantisConfig::from_toml(
            r#"
[project]
name = "demo"
type = "lib"
out-dir = "./artifacts"

[compiler]
allow-unsafe = false
allow-syscalls = false
"#,
        )
        .unwrap();
        assert_eq!(c.project.name.as_deref(), Some("demo"));
        assert_eq!(c.project.kind, ProjectKind::Lib);
        assert_eq!(c.project.out_dir, "./artifacts");
        assert!(!c.allow_unsafe);
        assert!(!c.allow_syscalls);
        // untouched flags stay on
        assert!(c.allow_external_functions);
        assert!(c.allow_implicit_conversions);
    }

    #[test]
    fn cli_overrides_win() {
        let mut c = MantisConfig::from_toml("[compiler]\nallow-unsafe = true\n").unwrap();
        c.apply_cli_overrides(false, false, true, false);
        assert!(!c.allow_unsafe);
        assert!(c.allow_implicit_conversions);
    }

    #[test]
    fn rejects_unknown_kind() {
        assert!(MantisConfig::from_toml("[project]\ntype = \"widget\"\n").is_err());
    }
}

// ── Policy enforcement ───────────────────────────────────────────────────

use mantis_parser::ast::{
    Declaration, Expr, Program, Statement, TypeDefBody, TypeExpr, UnaryOp,
};

impl MantisConfig {
    /// Validate a program against the enabled permissions.
    /// Returns human-readable violations (empty = OK).
    pub fn check_program(&self, prog: &Program) -> Vec<String> {
        let mut v = Vec::new();
        for decl in &prog.declarations {
            self.check_decl(decl, &mut v);
        }
        v
    }

    fn check_decl(&self, decl: &Declaration, out: &mut Vec<String>) {
        match decl {
            Declaration::Function(f) => {
                let name = f.name.as_ref().and_then(|n| n.as_name()).unwrap_or("?");
                if f.is_extern {
                    if !self.allow_external_functions {
                        out.push(format!(
                            "external function '{}' is not allowed (allow_external_functions = false) at span {:?}",
                            name, f.span
                        ));
                    }
                    if !self.allow_syscalls && SYSCALL_FNS.contains(&name) {
                        out.push(format!(
                            "syscall wrapper '{}' is not allowed (allow_syscalls = false) at span {:?}",
                            name, f.span
                        ));
                    }
                }
                for p in &f.params {
                    self.check_type(&p.ty, f.span, out);
                }
                if let Some(rt) = &f.return_type {
                    self.check_type(rt, f.span, out);
                }
                if let Some(body) = &f.body {
                    self.check_block(body, out);
                }
            }
            Declaration::TypeDef(t) => {
                let tname = t.name.as_name().unwrap_or("?");
                if let TypeDefBody::Struct(sd) = &t.definition {
                    for field in &sd.fields {
                        if matches!(field.ty, TypeExpr::Ref(_, _)) && !self.allow_unsafe {
                            out.push(format!(
                                "raw pointer field '{}.{}' is not allowed (allow_unsafe = false) at span {:?}",
                                tname, field.name.name, field.name.span
                            ));
                        }
                    }
                }
            }
            Declaration::Static(s) => self.check_type(&s.ty, s.span, out),
            _ => {}
        }
    }

    fn check_type(&self, ty: &TypeExpr, span: mantis_parser::token::Span, out: &mut Vec<String>) {
        if let TypeExpr::Ref(_, _) = ty {
            if !self.allow_unsafe {
                out.push(format!(
                    "raw pointer type is not allowed (allow_unsafe = false) at span {:?}",
                    span
                ));
            }
        }
    }

    fn check_block(
        &self,
        block: &mantis_parser::ast::Block,
        out: &mut Vec<String>,
    ) {
        for item in &block.items {
            match item {
                mantis_parser::ast::BlockItem::Statement(st) => match st {
                    Statement::Let { ty, value, .. } => {
                        if let Some(t) = ty {
                            self.check_type(t, t.span(), out);
                        }
                        self.check_expr(value, out);
                    }
                    Statement::Return { value, .. } => {
                        if let Some(e) = value {
                            self.check_expr(e, out);
                        }
                    }
                    Statement::Expr { expr, .. } => self.check_expr(expr, out),
                    _ => {}
                },
                mantis_parser::ast::BlockItem::IfChain(chain) => {
                    self.check_expr(&chain.if_block.condition, out);
                    self.check_block(&chain.if_block.body, out);
                    for elif in &chain.elif_blocks {
                        self.check_expr(&elif.condition, out);
                        self.check_block(&elif.body, out);
                    }
                    if let Some(b) = &chain.else_block {
                        self.check_block(b, out);
                    }
                }
                mantis_parser::ast::BlockItem::Loop(l) => self.check_block(&l.body, out),
                mantis_parser::ast::BlockItem::Match(m) => {
                    self.check_expr(&m.scrutinee, out);
                    for arm in &m.arms {
                        self.check_expr(&arm.pattern, out);
                        self.check_block(&arm.body, out);
                    }
                }
                mantis_parser::ast::BlockItem::Block(b) => self.check_block(b, out),
            }
        }
    }

    fn check_expr(&self, expr: &Expr, out: &mut Vec<String>) {
        match expr {
            Expr::PointerAssign { span, .. } => {
                if !self.allow_unsafe {
                    out.push(format!(
                        "pointer store '@=' is not allowed (allow_unsafe = false) at span {:?}",
                        span
                    ));
                }
            }
            Expr::Unary { op: UnaryOp::Deref, span, .. } => {
                if !self.allow_unsafe {
                    out.push(format!(
                        "pointer dereference is not allowed (allow_unsafe = false) at span {:?}",
                        span
                    ));
                }
            }
            Expr::CompilerCall { name, span, .. } => {
                if !self.allow_unsafe
                    && matches!(name.as_str(), "malloc" | "free" | "init" | "ref" | "as_ref" | "ptr")
                {
                    out.push(format!(
                        "memory intrinsic '#{}' is not allowed (allow_unsafe = false) at span {:?}",
                        name, span
                    ));
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.check_expr(lhs, out);
                self.check_expr(rhs, out);
            }
            Expr::Unary { operand, .. } => self.check_expr(operand, out),
            Expr::Call { callee, args, .. } => {
                self.check_expr(callee, out);
                for a in args {
                    self.check_expr(a, out);
                }
            }
            Expr::Field { object, .. } => self.check_expr(object, out),
            Expr::Cast { expr, .. } => self.check_expr(expr, out),
            Expr::StructInit { fields, .. } => {
                for f in fields {
                    self.check_expr(&f.value, out);
                }
            }
            Expr::ArrayInit { elements, .. } => {
                for e in elements {
                    self.check_expr(e, out);
                }
            }
            Expr::Await { expr, .. }
            | Expr::Yield { expr, .. }
            | Expr::Propagate { expr, .. } => self.check_expr(expr, out),
            Expr::PointerAssign { target, value, .. } => {
                self.check_expr(target, out);
                self.check_expr(value, out);
            }
            Expr::Generic { base, .. } => self.check_expr(base, out),
            Expr::Lambda { decl, .. } => {
                for p in &decl.params {
                    self.check_type(&p.ty, p.name.span, out);
                }
                if let Some(b) = &decl.body {
                    self.check_block(b, out);
                }
            }
            Expr::AsyncBlock { body, .. } => self.check_block(body, out),
            _ => {}
        }
    }
}
