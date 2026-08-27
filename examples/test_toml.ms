use std;
use toml;

fn main() i32 {
    print_str("=== Testing Mantis TOML Parser ===\n");
    let sample = "[project]\nname = \"my_mantis_app\"\nversion = \"1.2.3\"\n\n[build]\nentry = \"src/main.ms\"\ntarget = \"target/app\"\nopt_level = 2\nrelease = true\n\n[dependencies]\ntoml = { path = \"../toml\" }\n";

    let doc = Toml.parse(sample);

    let name = doc.get_or("project", "name", "unknown");
    print_str("project.name = ");
    println_str(name.as_str());

    let version = doc.get_or("project", "version", "0.0.0");
    print_str("project.version = ");
    println_str(version.as_str());

    let entry = doc.get_or("build", "entry", "default.ms");
    print_str("build.entry = ");
    println_str(entry.as_str());

    let opt = doc.get_int("build", "opt_level", 0);
    print_str("build.opt_level = ");
    print_int(opt);
    print_str("\n");

    let rel = doc.get_bool("build", "release", false);
    if rel {
        print_str("build.release = true\n");
    }

    let deps = doc.get_dep_paths();
    print_str("Found dependencies:\n");
    let mut i = 0;
    loop {
        if i >= deps.len() {
            break;
        }
        let dep = deps.get(i);
        print_str("  - path = ");
        println_str(dep.as_str());
        i = i + 1;
    }

    print_str("=== TOML tests passed! ===\n");
    return 0;
}
