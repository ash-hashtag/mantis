// mantis — Self-hosted build tool and package manager for the Mantis language.

use libc;
use std;
use toml;

// ── System Command Execution ────────────────────────────────────────────────

fn exec_cmd(cmd Str) i32 {
    let mut buf = String.with_capacity((cmd.len + 1) as u64);
    buf.push_str(cmd);
    buf.push_byte(0);
    return libc.system(buf.as_ptr() as i64);
}

// ── Commands ────────────────────────────────────────────────────────────────

fn print_help() {
    print_str("mantis - The Mantis Programming Language Build Tool & Package Manager\n\n");
    print_str("USAGE:\n");
    print_str("    mantis <SUBCOMMAND> [OPTIONS]\n\n");
    print_str("SUBCOMMANDS:\n");
    print_str("    new <path>      Create a new Mantis project in a directory\n");
    print_str("    init            Initialize a new Mantis project in the current directory\n");
    print_str("    build           Compile the current package and link binary\n");
    print_str("    run [args...]   Build and run the binary\n");
    print_str("    check           Analyze the package and report errors without building\n");
    print_str("    clean           Remove artifacts generated in the past\n");
    print_str("    version         Show version information\n");
    print_str("    help            Print this message or the help of the given subcommand(s)\n");
}

fn print_version() {
    print_str("mantis 0.1.0 (self-hosted package manager)\n");
}

fn handle_new(name Str) i32 {
    if name.len == 0 {
        print_str("error: `mantis new` requires a project name. Example: mantis new my_app\n");
        return 1;
    }

    let mut src_dir = String.with_capacity((name.len + 5) as u64);
    src_dir.push_str(name);
    src_dir.push_str("/src");
    let _ = std.fs.create_dir_all(src_dir.as_str());

    let mut toml_path = String.with_capacity((name.len + 13) as u64);
    toml_path.push_str(name);
    toml_path.push_str("/mantis.toml");

    let mut toml_content = String.with_capacity(256);
    toml_content.push_str("[project]\nname = \"");
    toml_content.push_str(name);
    toml_content.push_str("\"\nversion = \"0.1.0\"\nauthors = []\n\n[build]\nentry = \"src/main.ms\"\ntarget = \"target/main\"\nopt_level = 0\n");

    let _ = std.fs.write(toml_path.as_str(), toml_content.as_str());

    let mut main_path = String.with_capacity((name.len + 13) as u64);
    main_path.push_str(name);
    main_path.push_str("/src/main.ms");

    let main_content = "use std;\n\nfn main() i32 {\n    println_str(\"Hello, Mantis!\");\n    return 0;\n}\n";
    let _ = std.fs.write(main_path.as_str(), main_content);

    print_str("     Created binary (application) `");
    print_str(name);
    print_str("` package\n");
    return 0;
}

fn handle_init() i32 {
    let _ = std.fs.create_dir_all("src");

    if !std.fs.exists("mantis.toml") {
        let toml_content = "[project]\nname = \"app\"\nversion = \"0.1.0\"\nauthors = []\n\n[build]\nentry = \"src/main.ms\"\ntarget = \"target/main\"\nopt_level = 0\n";
        let _ = std.fs.write("mantis.toml", toml_content);
    }

    if !std.fs.exists("src/main.ms") {
        let main_content = "use std;\n\nfn main() i32 {\n    println_str(\"Hello, Mantis!\");\n    return 0;\n}\n";
        let _ = std.fs.write("src/main.ms", main_content);
    }

    print_str("     Initialized binary (application) package in current directory\n");
    return 0;
}

fn handle_clean() i32 {
    let _ = exec_cmd("rm -rf target build");
    print_str("   Cleaned target/ and build/ directories\n");
    return 0;
}

fn handle_check() i32 {
    if !std.fs.exists("mantis.toml") {
        if !std.fs.exists("src/main.ms") {
            print_str("error: could not find `mantis.toml` or `src/main.ms` in current directory\n");
            return 1;
        }
    }

    let mut entry = String.from_str("src/main.ms");
    let mut check_cmd = String.with_capacity(512);
    check_cmd.push_str("~/.mantis/bin/mantisc ");

    if std.fs.exists("mantis.toml") {
        let toml_res = std.fs.read_to_string("mantis.toml");
        if toml_res.is_ok() {
            let content = toml_res.unwrap();
            let doc = Toml.parse(content.as_str());
            entry = doc.get_or("build", "entry", "src/main.ms");
            let deps = doc.get_dep_paths();
            let mut i = 0;
            loop {
                if i >= deps.len() {
                    break;
                }
                let dep = deps.get(i);
                check_cmd.push_str("-I ");
                check_cmd.push_str(dep.as_str());
                check_cmd.push_str(" ");
                i = i + 1;
            }
        }
    }

    check_cmd.push_str(entry.as_str());
    check_cmd.push_str(" --check");

    print_str("    Checking app (");
    print_str(entry.as_str());
    print_str(")...\n");

    let res = exec_cmd(check_cmd.as_str());
    if res == 0 {
        print_str("    Finished check successfully\n");
    } else {
        print_str("error: check failed\n");
    }
    return res;
}

fn handle_build() i32 {
    if !std.fs.exists("mantis.toml") {
        if !std.fs.exists("src/main.ms") {
            print_str("error: could not find `mantis.toml` or `src/main.ms` in current directory\n");
            return 1;
        }
    }

    let _ = std.fs.create_dir_all("target");

    let mut proj_name = String.from_str("app");
    let mut proj_ver = String.from_str("0.1.0");
    let mut entry = String.from_str("src/main.ms");
    let mut target_out = String.from_str("target/main");

    let mut compile_cmd = String.with_capacity(512);
    compile_cmd.push_str("~/.mantis/bin/mantisc -c ");

    if std.fs.exists("mantis.toml") {
        let toml_res = std.fs.read_to_string("mantis.toml");
        if toml_res.is_ok() {
            let content = toml_res.unwrap();
            let doc = Toml.parse(content.as_str());
            proj_name = doc.get_or("project", "name", "app");
            proj_ver = doc.get_or("project", "version", "0.1.0");
            entry = doc.get_or("build", "entry", "src/main.ms");
            target_out = doc.get_or("build", "target", "target/main");

            let deps = doc.get_dep_paths();
            let mut i = 0;
            loop {
                if i >= deps.len() {
                    break;
                }
                let dep = deps.get(i);
                compile_cmd.push_str("-I ");
                compile_cmd.push_str(dep.as_str());
                compile_cmd.push_str(" ");
                i = i + 1;
            }
        }
    }

    compile_cmd.push_str(entry.as_str());
    compile_cmd.push_str(" -o target/main.o");

    print_str("   Compiling ");
    print_str(proj_name.as_str());
    print_str(" v");
    print_str(proj_ver.as_str());
    print_str(" (");
    print_str(entry.as_str());
    print_str(") ...\n");

    let comp_status = exec_cmd(compile_cmd.as_str());
    if comp_status != 0 {
        print_str("error: compilation failed\n");
        return comp_status;
    }

    print_str("     Linking target/main.o -> ");
    print_str(target_out.as_str());
    print_str(" ...\n");

    let mut link_cmd = String.with_capacity(256);
    link_cmd.push_str("cc target/main.o -o ");
    link_cmd.push_str(target_out.as_str());
    link_cmd.push_str(" -lpthread -lm -ldl");

    let link_status = exec_cmd(link_cmd.as_str());
    if link_status != 0 {
        print_str("error: linking failed\n");
        return link_status;
    }

    print_str("    Finished dev [unoptimized] target(s) in ");
    print_str(target_out.as_str());
    print_str("\n");
    return 0;
}

fn handle_run(args Vec[String]) i32 {
    let build_res = handle_build();
    if build_res != 0 {
        return build_res;
    }

    let mut target_out = String.from_str("target/main");
    if std.fs.exists("mantis.toml") {
        let toml_res = std.fs.read_to_string("mantis.toml");
        if toml_res.is_ok() {
            let content = toml_res.unwrap();
            let doc = Toml.parse(content.as_str());
            target_out = doc.get_or("build", "target", "target/main");
        }
    }

    print_str("     Running `");
    print_str(target_out.as_str());
    print_str("`\n\n");
    
    let mut run_cmd = String.with_capacity(256);
    let tgt_str = target_out.as_str();
    if tgt_str.len > 0 {
        let first_byte = @(tgt_str.pointer as @u8);
        if first_byte != 47 { // '/'
            run_cmd.push_str("./");
        }
    }
    run_cmd.push_str(target_out.as_str());

    let mut i = 2;
    loop {
        if i >= args.len() {
            break;
        }
        run_cmd.push_str(" ");
        let arg = args.get(i);
        run_cmd.push_str(arg.as_str());
        i = i + 1;
    }

    return exec_cmd(run_cmd.as_str());
}

// ── Main Entry Point ────────────────────────────────────────────────────────

fn main() i32 {
    let args = std.env.args();
    if args.size <= 1 {
        print_help();
        return 0;
    }

    let subcmd_ptr = (args.ptr as i64 + 24) as @String;
    let subcmd = subcmd_ptr.as_str();

    if subcmd == "help" {
        print_help();
        return 0;
    }
    if subcmd == "--help" {
        print_help();
        return 0;
    }
    if subcmd == "-h" {
        print_help();
        return 0;
    }

    if subcmd == "version" {
        print_version();
        return 0;
    }
    if subcmd == "--version" {
        print_version();
        return 0;
    }
    if subcmd == "-V" {
        print_version();
        return 0;
    }

    if subcmd == "new" {
        if args.len() < 3 {
            print_str("error: `mantis new` requires a project name. Example: mantis new my_app\n");
            return 1;
        }
        let name_str = args.get(2);
        return handle_new(name_str.as_str());
    }

    if subcmd == "init" {
        return handle_init();
    }

    if subcmd == "build" {
        return handle_build();
    }

    if subcmd == "run" {
        return handle_run(args);
    }

    if subcmd == "check" {
        return handle_check();
    }

    if subcmd == "clean" {
        return handle_clean();
    }

    print_str("error: no such command: `");
    print_str(subcmd);
    print_str("`\n\nView available commands with `mantis help`\n");
    return 1;
}
