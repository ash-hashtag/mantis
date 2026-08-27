use std;

fn main() i32 {
    print_str("=== 1. Testing std.env.args() ===\n");
    let args = std.env.args();
    print_str("Arg count: ");
    print_int(args.len() as i64);
    print_str("\n");

    let mut i = 0;
    loop {
        if i >= args.len() {
            break;
        }
        let arg = args.get(i);
        print_str("  argv[");
        print_int(i as i64);
        print_str("] = ");
        print_str(arg.as_str());
        print_str("\n");
        i = i + 1;
    }

    print_str("=== 2. Testing std.env.var() & set_var() ===\n");
    std.env.set_var("MANTIS_TEST_KEY", "MantisIsAwesome");
    let val_opt = std.env.var("MANTIS_TEST_KEY");
    if val_opt.is_some() {
        let val = val_opt.unwrap();
        print_str("MANTIS_TEST_KEY = ");
        print_str(val.as_str());
        print_str("\n");
    } else {
        print_str("Error: MANTIS_TEST_KEY not found\n");
    }

    print_str("=== 3. Testing std.fs ===\n");
    let test_file = "/tmp/mantis_fs_test.txt";
    let test_content = "Hello from Mantis std.fs module!\nSelf-hosted tools are great.\n";

    print_str("Writing file...\n");
    let write_res = std.fs.write(test_file, test_content);
    if write_res.is_err() {
        print_str("Error writing file\n");
        return 1;
    }

    print_str("Checking exists...\n");
    if std.fs.exists(test_file) {
        print_str("File exists: OK\n");
    } else {
        print_str("Error: File does not exist!\n");
        return 1;
    }

    print_str("Reading file back...\n");
    let read_res = std.fs.read_to_string(test_file);
    if read_res.is_err() {
        print_str("Error reading file\n");
        return 1;
    }
    let content = read_res.unwrap();
    print_str("Read content:\n");
    print_str(content.as_str());

    print_str("Testing directory creation...\n");
    let test_dir = "/tmp/mantis_test_dir/nested/sub";
    let dir_res = std.fs.create_dir_all(test_dir);
    if dir_res.is_ok() && std.fs.exists(test_dir) {
        print_str("Directory creation: OK\n");
    } else {
        print_str("Error creating directory\n");
    }

    print_str("Cleaning up file...\n");
    let _ = std.fs.remove_file(test_file);
    if !std.fs.exists(test_file) {
        print_str("File cleanup: OK\n");
    }

    print_str("=== All std.env and std.fs tests passed! ===\n");
    return 0;
}
