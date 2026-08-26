use std;

fn main() i32 {
    print_str("=== Mantis Multithreaded Async Task Scheduler ===
");
    print_str("Initializing Runtime from std.async_schedular with 4 Worker Threads...
");

    let rt = Runtime.new(4);

    print_str("Spawning 6 Async State Machine Tasks across Workers...
");
    rt.spawn(101, 10, 5);  // Task 101: 10 * 5 + 101 = 151
    rt.spawn(102, 20, 3);  // Task 102: 20 * 3 + 102 = 162
    rt.spawn(103, 7, 7);   // Task 103: 7 * 7 + 103 = 150
    rt.spawn(104, 12, 4);  // Task 104: 12 * 4 + 104 = 152
    rt.spawn(105, 15, 10); // Task 105: 15 * 10 + 105 = 255
    rt.spawn(106, 8, 9);   // Task 106: 8 * 9 + 106 = 178

    print_str("Main thread blocking on completion of all spawned tasks...
");
    rt.block_on_all();

    print_str("=== All Async Tasks Completed Successfully ===
");
    return 0;
}
