use std;

// ── C Libc & Pthread Extern Functions ────────────────────────────────────────

fn pthread_create(thread @u64, attr i64, start_routine i64, arg i64) i32 extern ;
fn pthread_join(thread u64, retval @i64) i32 extern ;
fn pthread_mutex_init(mutex i64, attr i64) i32 extern ;
fn pthread_mutex_lock(mutex i64) i32 extern ;
fn pthread_mutex_unlock(mutex i64) i32 extern ;
fn pthread_mutex_destroy(mutex i64) i32 extern ;
fn pthread_cond_init(cond i64, attr i64) i32 extern ;
fn pthread_cond_wait(cond i64, mutex i64) i32 extern ;
fn pthread_cond_signal(cond i64) i32 extern ;
fn pthread_cond_broadcast(cond i64) i32 extern ;
fn pthread_cond_destroy(cond i64) i32 extern ;
fn usleep(usec u32) i32 extern ;
fn sched_yield() i32 extern ;

// ── Poll & Future Definitions ────────────────────────────────────────────────

type Poll[T] = enum {
    Ready(T),
    Pending
}

// ── Sync Primitives: Mutex & Condvar ─────────────────────────────────────────

type Mutex = struct {
    pad0 u64,
    pad1 u64,
    pad2 u64,
    pad3 u64,
    pad4 u64
}

impl Mutex {
    fn new() @Mutex {
        let m = malloc(#size_of(Mutex)) as @Mutex;
        pthread_mutex_init(m as i64, 0);
        return m;
    }

    fn lock(self @Mutex) {
        pthread_mutex_lock(self as i64);
    }

    fn unlock(self @Mutex) {
        pthread_mutex_unlock(self as i64);
    }

    fn free(self @Mutex) {
        pthread_mutex_destroy(self as i64);
        free(self as i64);
    }
}

type Condvar = struct {
    pad0 u64,
    pad1 u64,
    pad2 u64,
    pad3 u64,
    pad4 u64,
    pad5 u64
}

impl Condvar {
    fn new() @Condvar {
        let c = malloc(#size_of(Condvar)) as @Condvar;
        pthread_cond_init(c as i64, 0);
        return c;
    }

    fn wait(self @Condvar, mutex @Mutex) {
        pthread_cond_wait(self as i64, mutex as i64);
    }

    fn notify_one(self @Condvar) {
        pthread_cond_signal(self as i64);
    }

    fn notify_all(self @Condvar) {
        pthread_cond_broadcast(self as i64);
    }

    fn free(self @Condvar) {
        pthread_cond_destroy(self as i64);
        free(self as i64);
    }
}

// ── Thread-Safe Task Queue ───────────────────────────────────────────────────

type QueueNode = struct {
    task_ptr i64,
    next @QueueNode
}

type TaskQueue = struct {
    mutex @Mutex,
    cond @Condvar,
    head @QueueNode,
    tail @QueueNode,
    size i32,
    shutdown bool
}

impl TaskQueue {
    fn new() @TaskQueue {
        let q = malloc(#size_of(TaskQueue)) as @TaskQueue;
        q.mutex = Mutex.new();
        q.cond = Condvar.new();
        q.head = 0 as @QueueNode;
        q.tail = 0 as @QueueNode;
        q.size = 0;
        q.shutdown = false;
        return q;
    }

    fn push(self @TaskQueue, task_ptr i64) {
        self.mutex.lock();
        let node = malloc(#size_of(QueueNode)) as @QueueNode;
        node.task_ptr = task_ptr;
        node.next = 0 as @QueueNode;

        if self.tail as i64 == 0 {
            self.head = node;
            self.tail = node;
        } else {
            self.tail.next = node;
            self.tail = node;
        }
        self.size = self.size + 1;
        self.cond.notify_one();
        self.mutex.unlock();
    }

    fn pop(self @TaskQueue) i64 {
        self.mutex.lock();
        loop {
            if self.size > 0 {
                let node = self.head;
                let task_ptr = node.task_ptr;
                self.head = node.next;
                if self.head as i64 == 0 {
                    self.tail = 0 as @QueueNode;
                }
                self.size = self.size - 1;
                free(node as i64);
                self.mutex.unlock();
                return task_ptr;
            }
            if self.shutdown {
                self.mutex.unlock();
                return 0;
            }
            self.cond.wait(self.mutex);
        }
        self.mutex.unlock();
        return 0;
    }

    fn stop(self @TaskQueue) {
        self.mutex.lock();
        self.shutdown = true;
        self.cond.notify_all();
        self.mutex.unlock();
    }
}

// ── Tokio-Style Async Task & Multithreaded Runtime ───────────────────────────

type Task = struct {
    id i32,
    task_type i32,
    state u32,
    param_a i32,
    param_b i32,
    polls_count i32,
    completed bool,
    result i32
}

type Runtime = struct {
    num_workers i32,
    queue @TaskQueue,
    mutex @Mutex,
    cond @Condvar,
    completed_count i32,
    total_tasks i32
}

fn worker_entry(arg i64) i64 {
    let rt = arg as @Runtime;
    let worker_id = 1;
    print_str("  [Worker Thread] Started and ready for tasks.\n");

    loop {
        let task_ptr = rt.queue.pop();
        if task_ptr == 0 {
            break;
        }

        let task = task_ptr as @Task;
        print_str("  [Worker Thread] Stepped task #");
        print_int(task.id as i64);
        print_str("\n");

        if task.polls_count < 2 {
            task.polls_count = task.polls_count + 1;
            usleep(10000);
            rt.queue.push(task_ptr);
        } else {
            task.completed = true;
            task.result = task.param_a * task.param_b + task.id;

            rt.mutex.lock();
            rt.completed_count = rt.completed_count + 1;
            print_str("  [Worker Thread] Task #");
            print_int(task.id as i64);
            print_str(" FINISHED with result: ");
            print_int(task.result as i64);
            print_str("\n");

            rt.cond.notify_all();
            rt.mutex.unlock();
        }
    }

    print_str("  [Worker Thread] Shutting down.\n");
    return 0;
}

impl Runtime {
    fn new(num_workers i32) @Runtime {
        let rt = malloc(#size_of(Runtime)) as @Runtime;
        rt.num_workers = num_workers;
        rt.queue = TaskQueue.new();
        rt.mutex = Mutex.new();
        rt.cond = Condvar.new();
        rt.completed_count = 0;
        rt.total_tasks = 0;

        let mut i = 0;
        loop {
            if i >= num_workers {
                break;
            }
            let mut handle: u64 = 0;
            pthread_create(&handle, 0, worker_entry as i64, rt as i64);
            i = i + 1;
        }

        return rt;
    }

    fn spawn(self @Runtime, task_id i32, a i32, b i32) {
        let task = malloc(#size_of(Task)) as @Task;
        task.id = task_id;
        task.state = 0;
        task.param_a = a;
        task.param_b = b;
        task.polls_count = 0;
        task.completed = false;
        task.result = 0;

        self.mutex.lock();
        self.total_tasks = self.total_tasks + 1;
        self.mutex.unlock();

        self.queue.push(task as i64);
    }

    fn block_on_all(self @Runtime) {
        self.mutex.lock();
        loop {
            if self.completed_count >= self.total_tasks {
                break;
            }
            self.cond.wait(self.mutex);
        }
        self.mutex.unlock();

        self.queue.stop();
        usleep(50000);
    }
}

fn main() i32 {
    print_str("=== Mantis Multithreaded Tokio-style Task Scheduler ===\n");
    print_str("Initializing Runtime with 4 Worker Threads...\n");

    let rt = Runtime.new(4);

    print_str("Spawning 6 Async State Machine Tasks across Workers...\n");
    rt.spawn(101, 10, 5);  // Task 101: 10 * 5 + 101 = 151
    rt.spawn(102, 20, 3);  // Task 102: 20 * 3 + 102 = 162
    rt.spawn(103, 7, 7);   // Task 103: 7 * 7 + 103 = 150
    rt.spawn(104, 12, 4);  // Task 104: 12 * 4 + 104 = 152
    rt.spawn(105, 15, 10); // Task 105: 15 * 10 + 105 = 255
    rt.spawn(106, 8, 9);   // Task 106: 8 * 9 + 106 = 178

    print_str("Main thread blocking on completion of all spawned tasks...\n");
    rt.block_on_all();

    print_str("=== All Tokio-style Async Tasks Completed Successfully ===\n");
    return 0;
}
