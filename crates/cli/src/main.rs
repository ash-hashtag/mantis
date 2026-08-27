#![allow(unused)]

use std::{
    path::{Path, PathBuf},
    process::{ExitCode, ExitStatus},
    rc::Rc,
    time::Instant,
};

use clap::Parser;

use mantis_codegen::backend;

#[derive(clap::Parser, Debug)]
#[command(
    name = "mantisc",
    version = "0.1.0",
    about = "Mantis language compiler",
    long_about = "Compiler CLI for the Mantis programming language"
)]
struct Args {
    /// Input Mantis source file (.ms)
    input: String,

    /// Write debug representation of parsed AST to path
    #[arg(long)]
    dbg: Option<String>,

    /// Write .o object file to the mentioned path
    #[arg(long, short = 'o')]
    obj: Option<String>,

    /// Write executable to the mentioned path
    #[arg(long, short = 'e')]
    exe: Option<String>,

    /// Write library to the mentioned path
    #[arg(long, short = 'l')]
    lib: Option<String>,

    /// Build as static library
    #[arg(long)]
    static_lib: bool,

    /// Build as dynamic library
    #[arg(long)]
    shared_lib: bool,

    /// Include directories for module imports
    #[arg(long = "include", short = 'I')]
    include: Vec<String>,

    /// Libraries or object files to link with cc (e.g. -L pthread -L m -L ext.o)
    #[arg(long = "link", short = 'L')]
    link: Vec<String>,

    /// Extra flags passed to C compiler/linker
    #[arg(short = 'C')]
    c_flags: Vec<String>,

    /// Module name for linking
    #[arg(long, short = 'm', default_value_t = String::from("main"))]
    module_name: String,

    /// Cache directory
    #[arg(long, default_value_t = String::from("./build/cache"))]
    cache: String,

    /// Compile and run output executable
    #[arg(long, short = 'r')]
    run: bool,

    /// Compile only to object file (.o), do not invoke linker
    #[arg(short = 'c', long = "compile-only")]
    compile_only: bool,

    /// Check syntax and types without generating binary
    #[arg(long)]
    check: bool,

    /// Disable RAII auto-drop (drops are enabled by default)
    #[arg(long)]
    no_auto_drop: bool,

    /// Disallow extern (C) function declarations
    #[arg(long)]
    no_external_functions: bool,

    /// Disallow syscall wrapper functions
    #[arg(long)]
    no_syscalls: bool,

    /// Disallow raw pointers, @= stores and memory intrinsics
    #[arg(long)]
    no_unsafe: bool,

    /// Disable Box[T] auto-deref (b.x, b.method(), b.x += 1)
    #[arg(long)]
    no_implicit_conversions: bool,

    /// Disallow using libc package (allow_libc = false)
    #[arg(long)]
    no_libc: bool,

    /// Print syntax-highlighted source code
    #[arg(long)]
    highlight: bool,

    /// Arguments to pass to the executed binary (when --run is set)
    #[arg(trailing_var_arg = true)]
    run_args: Vec<String>,
}

fn main() {
    init_logger();
    let args = Args::parse();
    handle0(args);
}

fn init_logger() {
    use std::io::Write;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format(|buf, record| {
            let ts = buf.timestamp();
            writeln!(
                buf,
                "{} {} [{}:{}] - {}",
                ts,
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();
}

fn handle0(args: Args) {
    // ── Configuration: config.toml, then CLI overrides ──────────────────
    let mut config = match mantis_codegen::config::MantisConfig::discover() {
        Some(path) => {
            let cfg = match mantis_codegen::config::MantisConfig::load_file(&path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[31;1merror:[0m {}", e);
                    std::process::exit(1);
                }
            };
            log::info!("loaded config from {}", path.display());
            cfg
        }
        None => mantis_codegen::config::MantisConfig::defaults(),
    };
    config.apply_cli_overrides(
        args.no_external_functions,
        args.no_syscalls,
        args.no_unsafe,
        args.no_implicit_conversions,
        args.no_libc,
    );

    let input_path = std::path::PathBuf::from(&args.input);
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main")
        .to_string();
    let project_name = config.project.name.clone().unwrap_or(stem);

    let out_dir = std::path::PathBuf::from(&config.project.out_dir);
    let _ = std::fs::create_dir_all(&out_dir);

    let filepath = args.input;
    let input = match std::fs::read_to_string(&filepath) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[31;1merror:[0m failed to read input file '{}': {}",
                filepath, e
            );
            std::process::exit(1);
        }
    };

    if args.highlight {
        println!("{}", mantis_syntax::highlight_to_ansi(&input));
        return;
    }

    let src = Rc::from(input.as_str());

    let declarations = {
        let start = std::time::Instant::now();
        let ast = match mantis_parser::parse(&src) {
            Ok(prog) => prog,
            Err(e) => {
                emit_source_error(&filepath, &input, &e.to_string(), e.span());
                std::process::exit(1);
            }
        };
        let seconds = start.elapsed().as_secs_f64();
        log::info!("parsing mantis file took {:.4}s", seconds);

        ast
    };

    // Run Borrow Checker
    let borrow_errors =
        mantis_parser::borrow_checker::BorrowChecker::check_program(&src, &declarations);
    if !borrow_errors.is_empty() {
        for err in borrow_errors {
            eprintln!("{}", err.format(&src));
        }
        std::process::exit(1);
    }

    if args.check {
        log::info!("syntax and type checking passed");
        return;
    }

    if let Some(ast_path) = &args.dbg {
        let content = format!("{:#?}", declarations);
        let _ = std::fs::write(ast_path, &content);
        log::info!("wrote ast to {} {} bytes", ast_path, content.len());
    }

    let default_obj = out_dir.join(format!("{}.o", project_name));
    let obj_file_path = args
        .obj
        .unwrap_or_else(|| default_obj.to_str().unwrap().to_string());

    {
        let start = std::time::Instant::now();
        let bytes = backend::compile::compile_binary(
            declarations,
            args.include.clone(),
            &args.module_name,
            !args.no_auto_drop,
            config.clone(),
        )
        .unwrap();
        let seconds = start.elapsed().as_secs_f64();
        std::fs::write(&obj_file_path, &bytes).unwrap();
        log::info!(
            "compilation took: {:.4}s, wrote {} bytes {}",
            seconds,
            bytes.len(),
            obj_file_path
        );
    }

    if args.compile_only {
        return;
    }

    #[cfg(unix)]
    {
        let _ = std::fs::create_dir_all(&args.cache);
        let default_exe = if matches!(config.project.kind, mantis_codegen::config::ProjectKind::Lib) {
            out_dir.join(format!("lib{}.a", project_name))
        } else {
            out_dir.join(&project_name)
        };
        let exe_file_path = args.exe.unwrap_or_else(|| default_exe.to_str().unwrap().to_string());

        let mut cc_args: Vec<String> = vec![
            obj_file_path.clone(),
            "-pthread".to_string(),
            "-o".to_string(),
            exe_file_path.clone(),
        ];

        for l in &args.link {
            if l.ends_with(".o") || l.ends_with(".a") || l.ends_with(".so") || l.contains('/') {
                cc_args.push(l.clone());
            } else if l.starts_with("-l") {
                cc_args.push(l.clone());
            } else {
                cc_args.push(format!("-l{}", l));
            }
        }

        for c_flag in &args.c_flags {
            cc_args.push(c_flag.clone());
        }

        let cc_arg_refs: Vec<&str> = cc_args.iter().map(|s| s.as_str()).collect();
        assert!(
            run_cmd("cc", &cc_arg_refs).success(),
            "linking executable failed"
        );

        log::info!("executable created at {}", exe_file_path);

        if args.run {
            let cmd_args = args.run_args.iter().map(|x| x.as_str()).collect::<Vec<_>>();
            let exit_code = run_cmd(&exe_file_path, &cmd_args).code().unwrap_or(1);
            if exit_code != 0 {
                std::process::exit(exit_code);
            }
        }
    }
}

fn emit_source_error(path: &str, source: &str, message: &str, span: mantis_parser::token::Span) {
    let start = span.start.min(source.len());
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |index| start + index);
    let line_number = source[..line_start]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let column = source[line_start..start].chars().count() + 1;
    let width = source[start..span.end.min(line_end)].chars().count().max(1);

    eprintln!("[31;1merror:[0m {}", message);
    eprintln!(" [34m-->[0m {}:{}:{}", path, line_number, column);
    eprintln!("  [34m|[0m");
    eprintln!(
        "[34m{:>2} |[0m {}",
        line_number,
        &source[line_start..line_end]
    );
    eprintln!(
        "  [34m|[0m {}[31m{}[0m",
        " ".repeat(column - 1),
        "^".repeat(width)
    );
}

pub fn run_cmd(exe: &str, args: &[&str]) -> ExitStatus {
    log::info!("running {} with args {}", exe, args.join(" "));
    let instant = Instant::now();
    let mut child = std::process::Command::new(exe).args(args).spawn().unwrap();
    let exit_status = child.wait().unwrap();
    let seconds = instant.elapsed().as_secs_f64();
    log::info!("Exited with {:?} spent {:.4}s", exit_status, seconds);

    exit_status
}
