use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(
    name = "mat",
    author,
    version,
    about = "Package manager for the Mantis programming language"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Mantis project
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
        /// GitHub URL (e.g., https://github.com/user/repo) or local path (e.g., ../my_lib)
        dep: String,
    },

    /// Build the project and its dependencies
    Build {
        /// Release build mode
        #[arg(long)]
        release: bool,
    },

    /// Build and run the binary package
    Run {
        /// Arguments to pass to the binary
        #[arg(trailing_var_arg = true)]
        run_args: Vec<String>,
    },

    /// Clean build cache and cloned dependencies
    Clean,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    package: PackageInfo,
    #[serde(default)]
    dependencies: BTreeMap<String, DependencyValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageInfo {
    name: String,
    version: String,
    #[serde(rename = "type")]
    pkg_type: String, // "bin" or "lib"
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
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { bin, lib, name } => handle_init(bin, lib, name)?,
        Commands::Add { dep } => handle_add(&dep)?,
        Commands::Build { release } => {
            handle_build(release)?;
        }
        Commands::Run { run_args } => handle_run(run_args)?,
        Commands::Clean => handle_clean()?,
    }

    Ok(())
}

fn handle_init(_bin: bool, lib: bool, name: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let pkg_name = name.unwrap_or_else(|| {
        cwd.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("mantis_app")
            .to_string()
    });

    let pkg_type = if lib { "lib" } else { "bin" };
    let manifest_path = Path::new("mat.toml");

    if manifest_path.exists() {
        bail!("mat.toml already exists in this directory");
    }

    let manifest = Manifest {
        package: PackageInfo {
            name: pkg_name.clone(),
            version: "0.0.1".to_string(),
            pkg_type: pkg_type.to_string(),
        },
        dependencies: BTreeMap::new(),
    };

    let toml_str = toml::to_string_pretty(&manifest)?;
    fs::write(manifest_path, toml_str)?;

    fs::create_dir_all("src")?;

    if pkg_type == "bin" {
        let main_ms = Path::new("src/main.ms");
        if !main_ms.exists() {
            fs::write(
                main_ms,
                "use std;\n\nfn main() i32 {\n    print_str(\"Hello from Mantis package!\\n\");\n    return 0;\n}\n",
            )?;
        }
        println!(
            "Created binary package '{}' (mat.toml, src/main.ms)",
            pkg_name
        );
    } else {
        let lib_ms = Path::new("src/lib.ms");
        if !lib_ms.exists() {
            fs::write(
                lib_ms,
                "use std;\n\nfn add(a i32, b i32) i32 {\n    return a + b;\n}\n",
            )?;
        }
        println!(
            "Created library package '{}' (mat.toml, src/lib.ms)",
            pkg_name
        );
    }

    Ok(())
}

fn handle_add(dep: &str) -> Result<()> {
    let manifest_path = Path::new("mat.toml");
    if !manifest_path.exists() {
        bail!("mat.toml not found. Run `mat init` first.");
    }

    let manifest_str = fs::read_to_string(manifest_path)?;
    let mut manifest: Manifest = toml::from_str(&manifest_str)?;

    let path_buf = PathBuf::from(dep);
    if path_buf.exists() {
        let name = path_buf
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("dependency")
            .to_string();

        manifest.dependencies.insert(
            name.clone(),
            DependencyValue::Detailed(DependencyDetail {
                path: Some(dep.to_string()),
                git: None,
                version: None,
            }),
        );
        println!("Added local dependency '{}' -> {}", name, dep);
    } else if dep.starts_with("http://")
        || dep.starts_with("https://")
        || dep.starts_with("git@")
        || dep.contains('/')
    {
        let full_url = if !dep.contains("://") && !dep.starts_with("git@") {
            format!("https://github.com/{}", dep)
        } else {
            dep.to_string()
        };

        let repo_name = full_url
            .trim_end_matches(".git")
            .split('/')
            .last()
            .unwrap_or("git_dep")
            .to_string();

        manifest.dependencies.insert(
            repo_name.clone(),
            DependencyValue::Detailed(DependencyDetail {
                git: Some(full_url.clone()),
                path: None,
                version: None,
            }),
        );
        println!("Added git dependency '{}' -> {}", repo_name, full_url);
    } else {
        bail!("Dependency path or URL '{}' not found or invalid", dep);
    }

    let toml_str = toml::to_string_pretty(&manifest)?;
    fs::write(manifest_path, toml_str)?;

    Ok(())
}

fn resolve_dependencies(manifest: &Manifest) -> Result<Vec<PathBuf>> {
    let build_deps_dir = Path::new("build/deps");
    fs::create_dir_all(build_deps_dir)?;

    let mut dep_include_paths = Vec::new();

    for (name, dep_val) in &manifest.dependencies {
        match dep_val {
            DependencyValue::Simple(s) => {
                let p = PathBuf::from(s);
                if p.exists() {
                    dep_include_paths.push(p);
                }
            }
            DependencyValue::Detailed(detail) => {
                if let Some(ref local_path) = detail.path {
                    let p = PathBuf::from(local_path);
                    if !p.exists() {
                        bail!(
                            "Local dependency '{}' at path '{}' does not exist",
                            name,
                            local_path
                        );
                    }
                    dep_include_paths.push(p.canonicalize()?);
                } else if let Some(ref git_url) = detail.git {
                    let clone_target = build_deps_dir.join(name);
                    if !clone_target.exists() {
                        println!("Cloning git dependency '{}' from {} ...", name, git_url);
                        let status = Command::new("git")
                            .args([
                                "clone",
                                "--depth",
                                "1",
                                git_url,
                                clone_target.to_str().unwrap(),
                            ])
                            .status()
                            .context("failed to execute git clone")?;

                        if !status.success() {
                            bail!("Failed to clone repository {}", git_url);
                        }
                    } else {
                        println!(
                            "Using cached git dependency '{}' in build/deps/{}",
                            name, name
                        );
                    }
                    dep_include_paths.push(clone_target.canonicalize()?);
                }
            }
        }
    }

    Ok(dep_include_paths)
}

fn handle_build(_release: bool) -> Result<PathBuf> {
    let manifest_path = Path::new("mat.toml");
    if !manifest_path.exists() {
        bail!("mat.toml not found. Run `mat init` first.");
    }

    let manifest_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = toml::from_str(&manifest_str)?;

    let _dep_paths = resolve_dependencies(&manifest)?;

    let build_dir = Path::new("build");
    fs::create_dir_all(build_dir)?;

    // Copy std.ms, libc.ms, and std/ if present in workspace root so `use std;` and `use libc;` resolve
    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../.."));

    let libc_ms = repo_root.join("libc.ms");
    if libc_ms.exists() {
        let _ = fs::copy(&libc_ms, "libc.ms");
        let _ = fs::copy(&libc_ms, build_dir.join("libc.ms"));
    }

    let std_ms = repo_root.join("std.ms");
    if std_ms.exists() {
        let _ = fs::copy(&std_ms, "std.ms");
        let _ = fs::copy(&std_ms, build_dir.join("std.ms"));
    }
    let std_dir = repo_root.join("std");
    if std_dir.exists() {
        let target_std_dir = Path::new("std");
        let _ = fs::create_dir_all(target_std_dir);
        let target_build_std_dir = build_dir.join("std");
        let _ = fs::create_dir_all(&target_build_std_dir);
        if let Ok(entries) = fs::read_dir(&std_dir) {
            for entry in entries.flatten() {
                let _ = fs::copy(entry.path(), target_std_dir.join(entry.file_name()));
                let _ = fs::copy(entry.path(), target_build_std_dir.join(entry.file_name()));
            }
        }
    }

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

    println!(
        "Compiling {} ({}) ...",
        manifest.package.name,
        entry_file.display()
    );

    let mantis_bin = find_mantis_compiler();
    let status = run_mantis_compiler(&mantis_bin, &entry_file, &out_exe_path)?;
    if !status.success() {
        bail!("Compilation failed.");
    }

    println!("Finished build -> {}", out_exe_path.display());
    Ok(out_exe_path)
}

fn find_mantis_compiler() -> String {
    let repo_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    let candidates = [
        repo_root.join("target/release/mantis"),
        repo_root.join("target/debug/mantis"),
        PathBuf::from("./target/release/mantis"),
        PathBuf::from("./target/debug/mantis"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return candidate
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
        }
    }

    "mantis".to_string()
}

fn run_mantis_compiler(mantis_cmd: &str, entry: &Path, out_exe: &Path) -> Result<ExitStatus> {
    let status = Command::new(mantis_cmd)
        .arg(entry)
        .arg("-e")
        .arg(out_exe)
        .status();

    match status {
        Ok(s) => Ok(s),
        Err(_) => {
            println!("Fallback: running compiler via cargo ...");
            let cargo_status = Command::new("cargo")
                .arg("run")
                .arg("--quiet")
                .arg("--bin")
                .arg("mantis")
                .arg("--")
                .arg(entry)
                .arg("-e")
                .arg(out_exe)
                .status()
                .context("failed to invoke compiler via cargo")?;
            Ok(cargo_status)
        }
    }
}

fn handle_run(run_args: Vec<String>) -> Result<()> {
    let out_exe = handle_build(false)?;

    println!("Running {} ...\n", out_exe.display());
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
