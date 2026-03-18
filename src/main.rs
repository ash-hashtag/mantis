#![allow(unused)]

use std::{
    process::{ExitCode, ExitStatus},
    rc::Rc,
    time::Instant,
};

use clap::Parser;
use logos::Source;

mod backend;
mod frontend;
mod lexer;
mod libc;
mod ms;
mod native;
mod registries;
pub mod resolver;
mod scope;
mod utils;

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
    let input = std::fs::read_to_string(filepath).unwrap();

    let src = Rc::from(input.trim());

    let declarations = {
        let start = std::time::Instant::now();
        let decls = mantis_expression::pratt::parse_blocks(&src).expect("Unparsed declarations");
        let seconds = start.elapsed().as_secs_f64();
        log::info!("parsing mantis file took {:.4}s", seconds);

        decls
    };
    let include_dirs = Vec::new();
    // let (fns, sr) = collect_functions(input);

    if let Some(ast_path) = args.dbg {
        let content = format!("{:#?}", declarations);
        std::fs::write(&ast_path, &content);
        log::info!("wrote ast to {} {} bytes", ast_path, content.len());
        // std::fs::write(ast_path, format!("{:#?}", declarations)).unwrap();
    } else {
        dbg!(&declarations);
    }

    if let Some(obj_file_path) = args.obj {
        {
            let start = std::time::Instant::now();
            let bytes = backend::compile::compile_binary(
                declarations,
                include_dirs,
                &args.module_name,
                args.auto_drop,
            )
            .unwrap();
            // let bytes = compiler::ms_compile(fns, sr, fr).unwrap();
            let seconds = start.elapsed().as_secs_f64();
            std::fs::write(&obj_file_path, &bytes).unwrap();
            log::info!(
                "compilation took: {:.4}s, wrote {} bytes {}",
                seconds,
                bytes.len(),
                obj_file_path
            );
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(exe_file_path) = &args.exe {
                assert!(run_cmd(
                    "cc",
                    &[obj_file_path.as_str(), "-o", exe_file_path.as_str()]
                )
                .success());

                log::info!("executable created at {}", exe_file_path.as_str());
            } else if let Some(lib_file_path) = &args.lib {
                let lib_type = if args.static_lib {
                    "-static"
                } else if args.shared_lib {
                    "-shared"
                } else {
                    log::warn!("creating shared library");
                    "-shared"
                };
                assert!(run_cmd(
                    "cc",
                    &[
                        obj_file_path.as_str(),
                        lib_type,
                        "-o",
                        lib_file_path.as_str(),
                    ],
                )
                .success());
            }

            if args.run {
                if let Some(exe_file_path) = &args.exe {
                    {
                        let args = args.run_args.iter().map(|x| x.as_str()).collect::<Vec<_>>();
                        let _ = run_cmd(exe_file_path, &args).code().unwrap();
                    }
                } else {
                    let mut exe_file_path =
                        std::path::PathBuf::from(args.cache).join(&args.module_name);

                    let exe_file_path_str = exe_file_path.to_str().unwrap();
                    assert!(
                        run_cmd("cc", &[obj_file_path.as_str(), "-o", exe_file_path_str]).success()
                    );

                    log::info!("executable created at {} and running", exe_file_path_str);

                    {
                        let args = args.run_args.iter().map(|x| x.as_str()).collect::<Vec<_>>();
                        let code = run_cmd(exe_file_path_str, &args).code().unwrap();
                    }
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            log::error!("unsupported OS, only linux is supported right now")
        }
    }
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
