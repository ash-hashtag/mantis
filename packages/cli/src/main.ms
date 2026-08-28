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

// The CLI only needs borrowed command-line slices. Avoid materializing the
// entire argv array here; this also keeps the command dispatcher independent
// of Vec[String]'s owning representation.
fn arg_slice(index i64) Str {
    let argv = libc.mantis_get_argv();
    let ptr = @((argv + (index * 8)) as @i64);
    if ptr == 0 {
        return StrSlice { pointer: 0, len: 0 };
    }
    return StrSlice { pointer: ptr, len: libc.strlen(ptr) as i64 };
}

fn shell_write(path Str, contents Str) i32 {
    let mut cmd = String.with_capacity(path.len + contents.len + 32);
    cmd.push_str("printf '%s' '");
    cmd.push_str(contents);
    cmd.push_str("' > ");
    cmd.push_str(path);
    return exec_cmd(cmd.as_str());
}

fn path_exists(path Str) bool {
    let mut cmd = String.with_capacity((path.len + 12) as u64);
    cmd.push_str("test -e ");
    cmd.push_str(path);
    return exec_cmd(cmd.as_str()) == 0;
}

fn build_root() String {
    return String.from_str("${MANTIS_TARGET_DIR:-build}");
}

fn debug_root() String {
    let mut root = build_root();
    root.push_str("/debug");
    return root;
}

fn prepare_git_dependencies(debug_dir Str) i32 {
    // Read simple `[dependencies] name = { git = "..." }` entries, cache
    // shallow clones, expose name aliases, then compile package libraries.
    let mut cmd = String.with_capacity(1024);
    cmd.push_str("if test -f mantis.toml; then ");
    cmd.push_str("cache=\"$HOME/.mantis/pkgcache\"; ");
    cmd.push_str("sed -n '/^\\[dependencies\\]/,/^\\[/p' mantis.toml | ");
    cmd.push_str("sed -n 's/^[[:space:]]*\\([A-Za-z0-9_-]*\\)[[:space:]]*=.*git[[:space:]]*=[[:space:]]*\"\\([^\"]*\\)\".*/\\1 \\2/p' | ");
    cmd.push_str("while read name url; do ");
    cmd.push_str("commit=$(git ls-remote \"$url\" HEAD 2>/dev/null | cut -c1-12); ");
    cmd.push_str("test -n \"$commit\" || exit 1; dir=\"$cache/$name-$commit\"; ");
    cmd.push_str("test -d \"$dir\" || git clone --recursive --depth 1 \"$url\" \"$dir\" || exit 1; ");
    cmd.push_str("ln -sfn \"$dir\" \"$cache/$name\"; ");
    cmd.push_str("src=\"$dir/src/lib.ms\"; test -f \"$src\" || src=\"$dir/src/main.ms\"; ");
    cmd.push_str("test -f \"$src\" || continue; ");
    cmd.push_str("base=$(basename \"$dir\"); \"$HOME/.mantis/bin/mantisc\" -c \"$src\" -I \"$cache\" -o \"$PWD/");
    cmd.push_str(debug_dir);
    cmd.push_str("/$base.o\" || exit 1; done; fi");
    return exec_cmd(cmd.as_str());
}

fn handle_new(name Str) i32 {
    if name.len == 0 {
        print_str("error: `mantis new` requires a project name. Example: mantis new my_app\n");
        return 1;
    }

    let mut src_dir = String.with_capacity((name.len + 5) as u64);
    src_dir.push_str(name);
    src_dir.push_str("/src");
    let mut mkdir_cmd = String.with_capacity((src_dir.len() + 10) as u64);
    mkdir_cmd.push_str("mkdir -p ");
    mkdir_cmd.push_str(src_dir.as_str());
    let mkdir_status = exec_cmd(mkdir_cmd.as_str());
    if mkdir_status != 0 {
        print_str("error: could not create project directory\n");
        return mkdir_status;
    }

    let mut toml_path = String.with_capacity((name.len + 13) as u64);
    toml_path.push_str(name);
    toml_path.push_str("/mantis.toml");

    let mut toml_content = String.with_capacity(256);
    toml_content.push_str("[project]\nname = \"");
    toml_content.push_str(name);
    toml_content.push_str("\"\nversion = \"0.1.0\"\nauthors = []\n\n[build]\nentry = \"src/main.ms\"\ntarget = \"build/debug/main\"\nopt_level = 0\n");

    let write_status = shell_write(toml_path.as_str(), toml_content.as_str());
    if write_status != 0 {
        print_str("error: could not write mantis.toml\n");
        return write_status;
    }

    let mut main_path = String.with_capacity((name.len + 13) as u64);
    main_path.push_str(name);
    main_path.push_str("/src/main.ms");

    let main_content = "use std;\n\nfn main() i32 {\n    println_str(\"Hello, Mantis!\");\n    return 0;\n}\n";
    let write_status = shell_write(main_path.as_str(), main_content);
    if write_status != 0 {
        print_str("error: could not write src/main.ms\n");
        return write_status;
    }

    print_str("     Created binary (application) `");
    print_str(name);
    print_str("` package\n");
    return 0;
}

fn handle_init() i32 {
    let mkdir_status = exec_cmd("mkdir -p src");
    if mkdir_status != 0 {
        print_str("error: could not create src directory\n");
        return mkdir_status;
    }

    if !path_exists("mantis.toml") {
        let toml_content = "[project]\nname = \"app\"\nversion = \"0.1.0\"\nauthors = []\n\n[build]\nentry = \"src/main.ms\"\ntarget = \"build/debug/main\"\nopt_level = 0\n";
        let _ = shell_write("mantis.toml", toml_content);
    }

    if !path_exists("src/main.ms") {
        let main_content = "use std;\n\nfn main() i32 {\n    println_str(\"Hello, Mantis!\");\n    return 0;\n}\n";
        let _ = shell_write("src/main.ms", main_content);
    }

    print_str("     Initialized binary (application) package in current directory\n");
    return 0;
}

fn handle_clean() i32 {
    let _ = exec_cmd("rm -rf build target");
    print_str("   Cleaned build/ and target/ directories\n");
    return 0;
}

fn handle_check() i32 {
    let mut entry = String.from_str("src/main.ms");
    let mut check_cmd = String.with_capacity(512);
    check_cmd.push_str("~/.mantis/bin/mantisc ");

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
    let mut debug_dir = debug_root();
    let mut mkdir_cmd = String.with_capacity((debug_dir.len() + 10) as u64);
    mkdir_cmd.push_str("mkdir -p ");
    mkdir_cmd.push_str(debug_dir.as_str());
    let _ = exec_cmd(mkdir_cmd.as_str());
    let dep_status = prepare_git_dependencies(debug_dir.as_str());
    if dep_status != 0 {
        print_str("error: dependency preparation failed\n");
        return dep_status;
    }

    let mut proj_name = String.from_str("app");
    let mut proj_ver = String.from_str("0.1.0");
    let mut entry = String.from_str("src/main.ms");
    let mut target_out = String.new();
    target_out.push_str(debug_dir.as_str());
    target_out.push_str("/main");

    let mut compile_cmd = String.with_capacity(512);
    compile_cmd.push_str("~/.mantis/bin/mantisc -c ");
    compile_cmd.push_str("-I ~/.mantis/pkgcache/std-0.1.0 -I ~/.mantis/pkgcache/libc-0.1.0 ");

    compile_cmd.push_str(entry.as_str());
    compile_cmd.push_str(" -o ");
    compile_cmd.push_str(debug_dir.as_str());
    compile_cmd.push_str("/main.o");

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

    print_str("     Linking dependencies + main.o -> ");
    print_str(target_out.as_str());
    print_str(" ...\n");

    let mut link_cmd = String.with_capacity(512);
    // main.o contains the dependency implementations selected for this
    // compilation unit. Put it first so duplicate generated block symbols do
    // not get replaced by stale cached package objects.
    link_cmd.push_str("cc ");
    link_cmd.push_str(debug_dir.as_str());
    link_cmd.push_str("/main.o ~/.mantis/pkgcache/*/*.o -Wl,--allow-multiple-definition -o ");
    link_cmd.push_str(target_out.as_str());
    link_cmd.push_str(" -lpthread -lm -ldl");

    let link_status = exec_cmd(link_cmd.as_str());
    if link_status != 0 {
        print_str("error: linking failed\n");
        return link_status;
    }

    print_str("    Finished dev [unoptimized] build in ");
    print_str(target_out.as_str());
    print_str("\n");
    return 0;
}

fn handle_run() i32 {
    let build_res = handle_build();
    if build_res != 0 {
        return build_res;
    }

    let mut target_out = String.new();
    let debug_dir = debug_root();
    target_out.push_str(debug_dir.as_str());
    target_out.push_str("/main");
    print_str("     Running `");
    print_str(target_out.as_str());
    print_str("`\n\n");
    
    let mut run_cmd = String.with_capacity(256);
    run_cmd.push_str(target_out.as_str());

    return exec_cmd(run_cmd.as_str());
}

// ── Main Entry Point ────────────────────────────────────────────────────────

fn main() i32 {
    let argc = libc.mantis_get_argc();
    if argc <= 1 {
        print_help();
        return 0;
    }

    let subcmd = arg_slice(1);

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
        if argc < 3 {
            print_str("error: `mantis new` requires a project name. Example: mantis new my_app\n");
            return 1;
        }
        return handle_new(arg_slice(2));
    }

    if subcmd == "init" {
        return handle_init();
    }

    if subcmd == "build" {
        return handle_build();
    }

    if subcmd == "run" {
        return handle_run();
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
