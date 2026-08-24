#![allow(unused)]

use std::{
    process::{ExitCode, ExitStatus},
    rc::Rc,
    time::Instant,
};

use clap::Parser;

use mantis_codegen::backend;

#[derive(clap::Parser, Debug)]
#[command(
    version = "0.0.1",
    about = "mantis compiler",
    long_about = "mantis language compiler"
)]
struct Args {
    // Input Mantis File
    input: String,
    // Print AST to a file or console
    #[arg(long, help = "write dbg! of parsed file to the mentioned path")]
    dbg: Option<String>,
    // Output .o file
    #[arg(long, short, help = "write .o file to the mentioned path")]
    obj: Option<String>,

    #[arg(long, short, help = "write executable to the mentioned path")]
    exe: Option<String>,

    #[arg(long, short, help = "write library to the mentioned path")]
    lib: Option<String>,

    #[arg(long, help = "if its static library")]
    static_lib: bool,

    #[arg(long, help = "if its dynamic library")]
    shared_lib: bool,

    #[arg(long, short, help = "module name for linking", default_value_t = String::from("main"))]
    module_name: String,

    #[arg(long, short, help = "cache directory", default_value_t = String::from("./build/cache"))]
    cache: String,

    #[arg(long, short, help = "compile and run")]
    run: bool,

    #[arg(
        long,
        help = "RAII, Or Auto Drop very unstable, by default is disabled, enable it with this flag"
    )]
    auto_drop: bool,

    #[arg(long, help = "print syntax-highlighted source code")]
    highlight: bool,

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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
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
    let filepath = args.input;
    let input = match std::fs::read_to_string(&filepath) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "\x1b[31;1merror:\x1b[0m failed to read input file '{}': {}",
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

    if let Some(ast_path) = &args.dbg {
        let content = format!("{:#?}", declarations);
        let _ = std::fs::write(ast_path, &content);
        log::info!("wrote ast to {} {} bytes", ast_path, content.len());
    }

    let default_obj = format!("{}.o", args.module_name);
    let obj_file_path = args.obj.unwrap_or(default_obj);

    {
        let start = std::time::Instant::now();
        let bytes = backend::compile::compile_binary(
            declarations,
            vec![], // include_dirs
            &args.module_name,
            args.auto_drop,
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

    #[cfg(unix)]
    {
        println!("Compiling C executable with cc...");
        std::fs::create_dir_all(&args.cache).unwrap_or(());
        let default_exe = std::path::PathBuf::from(&args.cache)
            .join(&args.module_name)
            .to_str()
            .unwrap()
            .to_string();
        let exe_file_path = args.exe.unwrap_or(default_exe);

        assert!(run_cmd("cc", &[&obj_file_path, "-pthread", "-o", &exe_file_path]).success());

        log::info!("executable created at {}", exe_file_path);

        if args.run {
            let cmd_args = args.run_args.iter().map(|x| x.as_str()).collect::<Vec<_>>();
            let _ = run_cmd(&exe_file_path, &cmd_args).code().unwrap();
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

    eprintln!("\x1b[31;1merror:\x1b[0m {}", message);
    eprintln!(" \x1b[34m-->\x1b[0m {}:{}:{}", path, line_number, column);
    eprintln!("  \x1b[34m|\x1b[0m");
    eprintln!(
        "\x1b[34m{:>2} |\x1b[0m {}",
        line_number,
        &source[line_start..line_end]
    );
    eprintln!(
        "  \x1b[34m|\x1b[0m {}\x1b[31m{}\x1b[0m",
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
