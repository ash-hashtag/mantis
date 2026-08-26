fn find_std_dir() -> Option<PathBuf> {
    if let Ok(std_env) = std::env::var("MANTIS_STD") {
        let p = PathBuf::from(std_env);
        if p.exists() {
            return Some(p);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        let mut curr = exe_path.parent();
        while let Some(dir) = curr {
            let std_ms = dir.join("std.ms");
            let std_dir = dir.join("std");
            if std_ms.exists() || std_dir.exists() {
                return Some(dir.to_path_buf());
            }
            curr = dir.parent();
        }
    }

    None
}
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(
    name = "mantis",
    author,
    version = "0.1.0",
    about = "Package manager and build tool for the Mantis programming language"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Direct input file when not using subcommands (e.g. )
    #[arg(trailing_var_arg = true)]
    direct_args: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a new Mantis project in a new directory
    New {
        /// Project name
        name: String,

        /// Create a library package
        #[arg(long)]
        lib: bool,
    },

    /// Initialize a new Mantis project in the current directory
    Init {
        /// Create a binary package (default)
        #[arg(long, conflicts_with = "lib")]
        bin: bool,

        /// Create a library package
        #[arg(long, conflicts_with = "bin")]
        lib: bool,

        /// Project name (defaults to current directory name)
        name: Option<String>,
    },

    /// Add a dependency (GitHub URL or local file path)
    Add {
        /// Dependency name or URL / path
        dep: String,

        /// Explicit local path
        #[arg(long)]
        path: Option<String>,

        /// Explicit Git URL
        #[arg(long)]
        git: Option<String>,
    },

    /// Build the project and its dependencies
    Build {
        /// Release build mode
        #[arg(long)]
        release: bool,

        /// Additional libraries or objects to link
        #[arg(long = "link", short = 'L')]
        link: Vec<String>,

        /// Additional flags for C compiler/linker
        #[arg(short = 'C')]
        c_flags: Vec<String>,
    },

    /// Build and run the binary package
    Run {
        /// Release build mode
        #[arg(long)]
        release: bool,

        /// Additional libraries or objects to link
        #[arg(long = "link", short = 'L')]
        link: Vec<String>,

        /// Additional flags for C compiler/linker
        #[arg(short = 'C')]
        c_flags: Vec<String>,

        /// Arguments to pass to the binary
        #[arg(trailing_var_arg = true)]
        run_args: Vec<String>,
    },

    /// Check syntax and types without generating binary
    Check,

    /// Clean build cache and cloned dependencies
    Clean,

    /// Directly compile a single Mantis source file
    Compile {
        /// Input file (.ms)
        file: String,

        /// Compile and run output executable
        #[arg(long, short = 'r')]
        run: bool,

        /// Output executable path
        #[arg(long, short = 'e')]
        exe: Option<String>,

        /// Output object path
        #[arg(long, short = 'o')]
        obj: Option<String>,

        /// Include directories
        #[arg(long = "include", short = 'I')]
        include: Vec<String>,

        /// Libraries to link
        #[arg(long = "link", short = 'L')]
        link: Vec<String>,

        /// Arguments to pass to the binary
        #[arg(trailing_var_arg = true)]
        run_args: Vec<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    package: PackageInfo,
    #[serde(default)]
    dependencies: BTreeMap<String, DependencyValue>,
    #[serde(default)]
    links: Vec<String>,
    #[serde(default)]
    c_sources: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageInfo {
    name: String,
    version: String,
    #[serde(rename = "type")]
    pkg_type: String, // "bin" or "lib"
    #[serde(default)]
    links: Vec<String>,
    #[serde(default)]
    c_sources: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum DependencyValue {
    Simple(String),
    Detailed(DependencyDetail),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct DependencyDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // If first argument is a .ms file or direct file invocation, forward to direct compile
    if args.len() > 1 && (args[1].ends_with(".ms") || args[1] == "compile") {
        return handle_direct_invocation(&args[1..]);
    }

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::New { name, lib }) => handle_new(&name, lib)?,
        Some(Commands::Init { bin, lib, name }) => handle_init(bin, lib, name)?,
        Some(Commands::Add { dep, path, git }) => handle_add(&dep, path, git)?,
        Some(Commands::Build { release, link, c_flags }) => {
            handle_build(release, &link, &c_flags)?;
        }
        Some(Commands::Run { release, link, c_flags, run_args }) => {
            handle_run(release, &link, &c_flags, run_args)?;
        }
        Some(Commands::Check) => handle_check()?,
        Some(Commands::Clean) => handle_clean()?,
        Some(Commands::Compile { file, run, exe, obj, include, link, run_args }) => {
            handle_compile_file(&file, run, exe, obj, &include, &link, &run_args)?;
        }
        None => {
            if !cli.direct_args.is_empty() {
                return handle_direct_invocation(&cli.direct_args);
            }
            println!("Mantis Package Manager (mantis)
Run  for available commands.");
        }
    }

    Ok(())
}

fn find_manifest_path() -> Option<PathBuf> {
    if Path::new("mantis.toml").exists() {
        Some(PathBuf::from("mantis.toml"))
    } else if Path::new("mat.toml").exists() {
        Some(PathBuf::from("mat.toml"))
    } else {
        None
    }
}

fn handle_new(name: &str, lib: bool) -> Result<()> {
    let project_dir = Path::new(name);
    if project_dir.exists() {
        bail!("Directory '{}' already exists", name);
    }
    fs::create_dir_all(project_dir.join("src"))?;

    let pkg_type = if lib { "lib" } else { "bin" };
    let manifest_content = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
type = "{}"

[dependencies]
"#,
        name, pkg_type
    );

    fs::write(project_dir.join("mantis.toml"), manifest_content)?;

    if lib {
        let sample_lib = r#"use std;

pub fn add(a i32, b i32) i32 {
    return a + b;
}
"#;
        fs::write(project_dir.join("src/lib.ms"), sample_lib)?;
    } else {
        let sample_main = r#"use std;

fn main() i32 {
    print_str("Hello from Mantis!
");
    return 0;
}
"#;
        fs::write(project_dir.join("src/main.ms"), sample_main)?;
    }

    fs::write(project_dir.join(".gitignore"), "build/
*.o
")?;

    println!("Created new project '{}' (mantis.toml)", name);
    Ok(())
}

fn handle_init(_bin: bool, lib: bool, name: Option<String>) -> Result<()> {
    if find_manifest_path().is_some() {
        bail!("mantis.toml or mat.toml already exists in this directory");
    }

    let current_dir = std::env::current_dir()?;
    let fallback_name = current_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .to_string();

    let project_name = name.unwrap_or(fallback_name);
    let pkg_type = if lib { "lib" } else { "bin" };

    let manifest_content = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
type = "{}"

[dependencies]
"#,
        project_name, pkg_type
    );

    fs::write("mantis.toml", manifest_content)?;
    fs::create_dir_all("src")?;

    if lib {
        let lib_path = Path::new("src/lib.ms");
        if !lib_path.exists() {
            let sample_lib = r#"use std;

pub fn add(a i32, b i32) i32 {
    return a + b;
}
"#;
            fs::write(lib_path, sample_lib)?;
        }
        println!("Initialized library package '{}' (mantis.toml)", project_name);
    } else {
        let main_path = Path::new("src/main.ms");
        if !main_path.exists() {
            let sample_main = r#"use std;

fn main() i32 {
    print_str("Hello from Mantis!
");
    return 0;
}
"#;
            fs::write(main_path, sample_main)?;
        }
        println!("Initialized binary package '{}' (mantis.toml)", project_name);
    }

    Ok(())
}

fn handle_add(dep: &str, path_opt: Option<String>, git_opt: Option<String>) -> Result<()> {
    let manifest_path = match find_manifest_path() {
        Some(p) => p,
        None => bail!("No mantis.toml or mat.toml found. Run  first."),
    };

    let manifest_str = fs::read_to_string(&manifest_path)?;
    let mut manifest: Manifest = toml::from_str(&manifest_str)?;

    if let Some(p) = path_opt {
        manifest.dependencies.insert(
            dep.to_string(),
            DependencyValue::Detailed(DependencyDetail {
                git: None,
                path: Some(p),
                version: None,
            }),
        );
    } else if let Some(g) = git_opt {
        manifest.dependencies.insert(
            dep.to_string(),
            DependencyValue::Detailed(DependencyDetail {
                git: Some(g),
                path: None,
                version: None,
            }),
        );
    } else if dep.starts_with("http://") || dep.starts_with("https://") || dep.ends_with(".git") {
        let name = dep
            .trim_end_matches('/')
            .split('/')
            .last()
            .unwrap_or(dep)
            .trim_end_matches(".git");
        manifest.dependencies.insert(
            name.to_string(),
            DependencyValue::Detailed(DependencyDetail {
                git: Some(dep.to_string()),
                path: None,
                version: None,
            }),
        );
    } else if Path::new(dep).exists() {
        let p = Path::new(dep);
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or(dep);
        manifest.dependencies.insert(
            name.to_string(),
            DependencyValue::Detailed(DependencyDetail {
                git: None,
                path: Some(dep.to_string()),
                version: None,
            }),
        );
    } else {
        manifest
            .dependencies
            .insert(dep.to_string(), DependencyValue::Simple(dep.to_string()));
    }

    let new_manifest_str = toml::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, new_manifest_str)?;

    println!("Added dependency '{}' to {}", dep, manifest_path.display());
    Ok(())
}

fn resolve_dependencies(manifest: &Manifest) -> Result<(Vec<PathBuf>, Vec<String>)> {
    let build_deps_dir = Path::new("build/deps");
    fs::create_dir_all(build_deps_dir)?;

    let mut dep_include_paths = Vec::new();
    let mut extra_links = Vec::new();

    for (name, dep_val) in &manifest.dependencies {
        match dep_val {
            DependencyValue::Simple(s) => {
                let p = PathBuf::from(s);
                if p.exists() {
                    dep_include_paths.push(p.canonicalize()?);
                }
            }
            DependencyValue::Detailed(detail) => {
                if let Some(ref local_path) = detail.path {
                    let p = PathBuf::from(local_path);
                    if !p.exists() {
                        bail!("Local dependency '{}' at path '{}' does not exist", name, local_path);
                    }
                    dep_include_paths.push(p.canonicalize()?);
                } else if let Some(ref git_url) = detail.git {
                    let clone_target = build_deps_dir.join(name);
                    if !clone_target.exists() {
                        println!("Cloning git dependency '{}' from {} ...", name, git_url);
                        let status = Command::new("git")
                            .args(["clone", "--depth", "1", git_url, clone_target.to_str().unwrap()])
                            .status()
                            .context("failed to execute git clone")?;
                        if !status.success() {
                            bail!("Failed to clone repository {}", git_url);
                        }
                    } else {
                        println!("Using cached git dependency '{}' in build/deps/{}", name, name);
                    }
                    dep_include_paths.push(clone_target.canonicalize()?);
                }
            }
        }
    }

    Ok((dep_include_paths, extra_links))
}

fn compile_c_sources(c_sources: &[String]) -> Result<Vec<PathBuf>> {
    let mut obj_files = Vec::new();
    if c_sources.is_empty() {
        return Ok(obj_files);
    }

    let objs_dir = Path::new("build/objs");
    fs::create_dir_all(objs_dir)?;

    for src in c_sources {
        let src_path = Path::new(src);
        if !src_path.exists() {
            bail!("C source file '{}' does not exist", src);
        }
        let stem = src_path.file_stem().and_then(|s| s.to_str()).unwrap_or("c_src");
        let out_obj = objs_dir.join(format!("{}.o", stem));

        println!("Compiling C source {} -> {} ...", src, out_obj.display());
        let status = Command::new("cc")
            .args(["-c", src, "-fPIC", "-o", out_obj.to_str().unwrap()])
            .status()
            .context("failed to compile C source")?;

        if !status.success() {
            bail!("Compilation of C source '{}' failed", src);
        }
        obj_files.push(out_obj);
    }

    Ok(obj_files)
}

fn handle_build(release: bool, cli_links: &[String], cli_c_flags: &[String]) -> Result<PathBuf> {
    let manifest_path = match find_manifest_path() {
        Some(p) => p,
        None => bail!("mantis.toml not found. Run  first."),
    };

    let manifest_str = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = toml::from_str(&manifest_str)?;

    let (dep_paths, _dep_links) = resolve_dependencies(&manifest)?;

    let build_dir = Path::new("build");
    fs::create_dir_all(build_dir)?;

    // Gather and compile C sources
    let mut all_c_sources = manifest.c_sources.clone();
    all_c_sources.extend(manifest.package.c_sources.clone());
    let compiled_c_objs = compile_c_sources(&all_c_sources)?;

    // Setup includes
    let mut include_dirs = vec![".".to_string(), "src".to_string(), "std".to_string()];
    if let Some(std_path) = find_std_dir() {
        include_dirs.push(std_path.to_string_lossy().to_string());
    }
    for dp in &dep_paths {
        include_dirs.push(dp.to_string_lossy().to_string());
        let dp_src = dp.join("src");
        if dp_src.exists() {
            include_dirs.push(dp_src.to_string_lossy().to_string());
        }
    }

    // Copy dependencies into build/deps or target root for direct resolution
    for (name, dep_val) in &manifest.dependencies {
        let dep_path = match dep_val {
            DependencyValue::Simple(s) => PathBuf::from(s),
            DependencyValue::Detailed(d) => {
                if let Some(ref p) = d.path {
                    PathBuf::from(p)
                } else if let Some(ref _g) = d.git {
                    build_dir.join("deps").join(name)
                } else {
                    continue;
                }
            }
        };

        let lib_cand = dep_path.join("src/lib.ms");
        let cand = if lib_cand.exists() {
            lib_cand
        } else {
            dep_path.join(format!("{}.ms", name))
        };

        if cand.exists() {
            let target_root = PathBuf::from(format!("{}.ms", name));
            let target_in_src = Path::new("src").join(format!("{}.ms", name));
            let target_in_build = build_dir.join(format!("{}.ms", name));
            let _ = fs::copy(&cand, &target_root);
            let _ = fs::copy(&cand, &target_in_src);
            let _ = fs::copy(&cand, &target_in_build);
        }
    }

    let entry_file = if manifest.package.pkg_type == "bin" {
        PathBuf::from("src/main.ms")
    } else {
        PathBuf::from("src/lib.ms")
    };

    if !entry_file.exists() {
        bail!("Entry file '{}' does not exist", entry_file.display());
    }

    let out_exe_name = format!("build/{}", manifest.package.name);
    let out_exe_path = PathBuf::from(&out_exe_name);

    println!("Compiling {} ({}) via mantisc ...", manifest.package.name, entry_file.display());

    let mut link_args = Vec::new();
    link_args.extend(manifest.package.links.clone());
    link_args.extend(manifest.links.clone());
    link_args.extend(cli_links.to_vec());

    for obj in compiled_c_objs {
        link_args.push(obj.to_string_lossy().to_string());
    }

    let mantisc_bin = find_mantis_compiler();
    let status = run_mantisc(
        &mantisc_bin,
        &entry_file,
        Some(&out_exe_path),
        None,
        &include_dirs,
        &link_args,
        cli_c_flags,
        false,
        &[],
    )?;

    if !status.success() {
        bail!("Compilation failed.");
    }

    println!("Finished build -> {}", out_exe_path.display());
    Ok(out_exe_path)
}

fn handle_run(
    release: bool,
    link: &[String],
    c_flags: &[String],
    run_args: Vec<String>,
) -> Result<()> {
    let out_exe = handle_build(release, link, c_flags)?;

    println!("Running {} ...
", out_exe.display());
    let mut cmd = Command::new(&out_exe);
    if !run_args.is_empty() {
        cmd.args(run_args);
    }

    let status = cmd.status().context("failed to run output executable")?;
    if !status.success() {
        bail!("Executable exited with status code {:?}", status.code());
    }

    Ok(())
}

fn handle_check() -> Result<()> {
    let manifest_path = match find_manifest_path() {
        Some(p) => p,
        None => bail!("mantis.toml not found. Run  first."),
    };

    let manifest_str = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = toml::from_str(&manifest_str)?;

    let entry_file = if manifest.package.pkg_type == "bin" {
        PathBuf::from("src/main.ms")
    } else {
        PathBuf::from("src/lib.ms")
    };

    println!("Checking {} ({}) ...", manifest.package.name, entry_file.display());
    let mantisc_bin = find_mantis_compiler();
    let status = Command::new(&mantisc_bin)
        .arg(&entry_file)
        .arg("--check")
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("Check passed successfully");
            Ok(())
        }
        _ => bail!("Check failed"),
    }
}

fn handle_clean() -> Result<()> {
    let build_dir = Path::new("build");
    if build_dir.exists() {
        fs::remove_dir_all(build_dir)?;
        println!("Cleaned build/ directory");
    } else {
        println!("Nothing to clean");
    }
    Ok(())
}

fn handle_compile_file(
    file: &str,
    run: bool,
    exe_opt: Option<String>,
    obj_opt: Option<String>,
    includes: &[String],
    links: &[String],
    run_args: &[String],
) -> Result<()> {
    let input_path = Path::new(file);
    if !input_path.exists() {
        bail!("Source file '{}' does not exist", file);
    }

    let out_exe_path = exe_opt.map(PathBuf::from);
    let out_obj_path = obj_opt.map(PathBuf::from);

    let mantisc_bin = find_mantis_compiler();
    let status = run_mantisc(
        &mantisc_bin,
        input_path,
        out_exe_path.as_deref(),
        out_obj_path.as_deref(),
        includes,
        links,
        &[],
        run,
        run_args,
    )?;

    if !status.success() {
        bail!("Compilation of '{}' failed", file);
    }

    Ok(())
}

fn handle_direct_invocation(raw_args: &[String]) -> Result<()> {
    let mantisc_bin = find_mantis_compiler();
    let status = Command::new(&mantisc_bin)
        .args(raw_args)
        .status();

    match status {
        Ok(s) => {
            if !s.success() {
                std::process::exit(s.code().unwrap_or(1));
            }
            Ok(())
        }
        Err(_) => {
            let mut cargo_args = vec!["run", "--quiet", "--bin", "mantisc", "--"];
            cargo_args.extend(raw_args.iter().map(|s| s.as_str()));
            let cargo_status = Command::new("cargo")
                .args(cargo_args)
                .status()
                .context("failed to invoke mantisc via cargo")?;
            if !cargo_status.success() {
                std::process::exit(cargo_status.code().unwrap_or(1));
            }
            Ok(())
        }
    }
}

fn find_mantis_compiler() -> String {
    if let Ok(env_val) = std::env::var("MANTISC") {
        if Path::new(&env_val).exists() {
            return env_val;
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling = parent.join("mantisc");
            if sibling.exists() {
                return sibling.to_string_lossy().to_string();
            }
        }
    }

    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    let candidates = [
        repo_root.join("target/release/mantisc"),
        repo_root.join("target/debug/mantisc"),
        PathBuf::from("./target/release/mantisc"),
        PathBuf::from("./target/debug/mantisc"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            if let Ok(canon) = candidate.canonicalize() {
                return canon.to_string_lossy().to_string();
            }
        }
    }

    "mantisc".to_string()
}

fn run_mantisc(
    mantisc_cmd: &str,
    entry: &Path,
    out_exe: Option<&Path>,
    out_obj: Option<&Path>,
    includes: &[String],
    links: &[String],
    c_flags: &[String],
    run: bool,
    run_args: &[String],
) -> Result<ExitStatus> {
    let mut cmd = Command::new(mantisc_cmd);
    cmd.arg(entry);

    if let Some(exe) = out_exe {
        cmd.arg("-e").arg(exe);
    }
    if let Some(obj) = out_obj {
        cmd.arg("-o").arg(obj);
    }
    for inc in includes {
        cmd.arg("-I").arg(inc);
    }
    for l in links {
        cmd.arg("-L").arg(l);
    }
    for cf in c_flags {
        cmd.arg("-C").arg(cf);
    }
    if run {
        cmd.arg("-r");
    }
    if !run_args.is_empty() {
        cmd.args(run_args);
    }

    let status = cmd.status();
    match status {
        Ok(s) => Ok(s),
        Err(_) => {
            let mut cargo_args = vec!["run", "--quiet", "--bin", "mantisc", "--"];
            cargo_args.push(entry.to_str().unwrap());
            let exe_str;
            if let Some(exe) = out_exe {
                cargo_args.push("-e");
                exe_str = exe.to_str().unwrap();
                cargo_args.push(exe_str);
            }
            let obj_str;
            if let Some(obj) = out_obj {
                cargo_args.push("-o");
                obj_str = obj.to_str().unwrap();
                cargo_args.push(obj_str);
            }
            for inc in includes {
                cargo_args.push("-I");
                cargo_args.push(inc);
            }
            for l in links {
                cargo_args.push("-L");
                cargo_args.push(l);
            }
            for cf in c_flags {
                cargo_args.push("-C");
                cargo_args.push(cf);
            }
            if run {
                cargo_args.push("-r");
            }
            for ra in run_args {
                cargo_args.push(ra);
            }

            let cargo_status = Command::new("cargo")
                .args(cargo_args)
                .status()
                .context("failed to invoke mantisc via cargo")?;
            Ok(cargo_status)
        }
    }
}
