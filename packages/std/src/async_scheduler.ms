use std.io;
use std.memory;
use libc;

// ── Poll & Future Definitions ────────────────────────────────────────────────

pub type Poll[T] = enum {
    Ready(T),
    Pending
}

impl[T] Poll[T] {
    pub fn is_ready(self) bool {
        match self {
            Poll.Ready(_) : { return true; },
            Poll.Pending : { return false; }
        }
    }

    pub fn is_pending(self) bool {
        return !self.is_ready();
    }

    pub fn unwrap(self) T {
        match self {
            Poll.Ready(val) : { return val; },
            Poll.Pending : { panic("called Poll.unwrap() on a Pending value"); }
        }
    }
}

// ── Sync Primitives: Mutex & Condvar ─────────────────────────────────────────

pub type Mutex = struct {
    pad0 u64,
    pad1 u64,
    pad2 u64,
    pad3 u64,
    pad4 u64
}

impl Mutex {
    pub fn new() @Mutex {
        let m = libc.malloc(#size_of(Mutex)) as @Mutex;
        libc.pthread_mutex_init(m as i64, 0);
        return m;
    }

    pub fn lock(self @Mutex) {
        libc.pthread_mutex_lock(self as i64);
    }

    pub fn unlock(self @Mutex) {
        libc.pthread_mutex_unlock(self as i64);
    }

    pub fn free(self @Mutex) {
        libc.pthread_mutex_destroy(self as i64);
        libc.free(self as i64);
    }
}

pub type Condvar = struct {
    pad0 u64,
    pad1 u64,
    pad2 u64,
    pad3 u64,
    pad4 u64,
    pad5 u64
}

impl Condvar {
    pub fn new() @Condvar {
        let c = libc.malloc(#size_of(Condvar)) as @Condvar;
        libc.pthread_cond_init(c as i64, 0);
        return c;
    }

    pub fn wait(self @Condvar, mutex @Mutex) {
        libc.pthread_cond_wait(self as i64, mutex as i64);
    }

    pub fn notify_one(self @Condvar) {
        libc.pthread_cond_signal(self as i64);
    }

    pub fn notify_all(self @Condvar) {
        libc.pthread_cond_broadcast(self as i64);
    }

    pub fn free(self @Condvar) {
        libc.pthread_cond_destroy(self as i64);
        libc.free(self as i64);
    }
}

// ── Thread-Safe Task Queue ───────────────────────────────────────────────────

pub type QueueNode = struct {
    task_ptr i64,
    next @QueueNode
}

pub type TaskQueue = struct {
    mutex @Mutex,
    cond @Condvar,
    head @QueueNode,
    tail @QueueNode,
    size i32,
    shutdown bool
}

impl TaskQueue {
    pub fn new() @TaskQueue {
        let q = libc.malloc(#size_of(TaskQueue)) as @TaskQueue;
        q.mutex = Mutex.new();
        q.cond = Condvar.new();
        q.head = 0 as @QueueNode;
        q.tail = 0 as @QueueNode;
        q.size = 0;
        q.shutdown = false;
        return q;
    }

    pub fn push(self @TaskQueue, task_ptr i64) {
        self.mutex.lock();
        let node = libc.malloc(#size_of(QueueNode)) as @QueueNode;
        node.task_ptr = task_ptr;
        node.next = 0 as @QueueNode;

        if (self.tail as i64) == 0 {
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

    pub fn pop(self @TaskQueue) i64 {
        self.mutex.lock();
        loop {
            if self.size > 0 {
                let node = self.head;
                let task_ptr = node.task_ptr;
                self.head = node.next;
                if (self.head as i64) == 0 {
                    self.tail = 0 as @QueueNode;
                }
                self.size = self.size - 1;
                libc.free(node as i64);
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

    pub fn stop(self @TaskQueue) {
        self.mutex.lock();
        self.shutdown = true;
        self.cond.notify_all();
        self.mutex.unlock();
    }
}

// ── Async Task & Runtime Scheduler ───────────────────────────────────────────

pub type Task = struct {
    id i32,
    task_type i32,
    state u32,
    param_a i32,
    param_b i32,
    polls_count i32,
    completed bool,
    result i32
}

pub type Runtime = struct {
    num_workers i32,
    queue @TaskQueue,
    mutex @Mutex,
    cond @Condvar,
    completed_count i32,
    total_tasks i32
}

pub type AsyncScheduler = Runtime

fn default_worker_entry(arg i64) i64 {
    let rt = arg as @Runtime;
    loop {
        let task_ptr = rt.queue.pop();
        if task_ptr == 0 {
            break;
        }

        let task = task_ptr as @Task;
        if task.polls_count < 2 {
            task.polls_count = task.polls_count + 1;
            libc.usleep(10000 as u32);
            rt.queue.push(task_ptr);
        } else {
            task.completed = true;
            task.result = task.param_a * task.param_b + task.id;

            rt.mutex.lock();
            rt.completed_count = rt.completed_count + 1;
            rt.cond.notify_all();
            rt.mutex.unlock();
        }
    }
    return 0;
}

impl Runtime {
    pub fn new(num_workers i32) @Runtime {
        let rt = libc.malloc(#size_of(Runtime)) as @Runtime;
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
            libc.pthread_create((&handle as @u64) as i64, 0, default_worker_entry as i64, rt as i64);
            i = i + 1;
        }

        return rt;
    }

    pub fn spawn(self @Runtime, task_id i32, a i32, b i32) {
        let task = libc.malloc(#size_of(Task)) as @Task;
        task.id = task_id;
        task.state = 0 as u32;
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

    pub fn block_on_all(self @Runtime) {
        self.mutex.lock();
        loop {
            if self.completed_count >= self.total_tasks {
                break;
            }
            self.cond.wait(self.mutex);
        }
        self.mutex.unlock();

        self.queue.stop();
        libc.usleep(50000 as u32);
    }
}
