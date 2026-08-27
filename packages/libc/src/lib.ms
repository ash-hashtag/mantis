// C Standard Library (libc) Bindings for Mantis

// ── Memory Management ────────────────────────────────────────────────────────
fn malloc(size i64) i64 extern ;
fn free(ptr i64) extern ;
fn realloc(ptr i64, size i64) i64 extern ;
fn memcpy(dest i64, src i64, n i64) i64 extern ;
fn memset(s i64, c i32, n i64) i64 extern ;
fn memcmp(s1 i64, s2 i64, n i64) i32 extern ;

// ── String Operations ────────────────────────────────────────────────────────
fn strlen(str i64) u64 extern ;
fn strcmp(s1 i64, s2 i64) i32 extern ;
fn strncmp(s1 i64, s2 i64, n i64) i32 extern ;
fn strcpy(dest i64, src i64) i64 extern ;
fn strncpy(dest i64, src i64, n i64) i64 extern ;
fn atoi(str i64) i32 extern ;
fn atof(str i64) f64 extern ;

// ── File I/O & System Calls ──────────────────────────────────────────────────
fn open(pathname i64, flags i32, mode i32) i32 extern ;
fn close(fd i32) i32 extern ;
fn read(fd i64, buf i64, count i64) i64 extern ;
fn write(fd i64, buf i64, count i64) i64 extern ;
fn lseek(fd i32, offset i64, whence i32) i64 extern ;
fn access(pathname i64, mode i32) i32 extern ;
fn unlink(pathname i64) i32 extern ;
fn mkdir(pathname i64, mode u32) i32 extern ;

// ── Environment & Process ───────────────────────────────────────────────────
fn getenv(name i64) i64 extern ;
fn setenv(name i64, value i64, overwrite i32) i32 extern ;
fn unsetenv(name i64) i32 extern ;
fn getcwd(buf i64, size u64) i64 extern ;
fn system(command i64) i32 extern ;
fn exit(code i32) extern ;
fn usleep(usec u32) i32 extern ;
fn sched_yield() i32 extern ;
fn time(tloc i64) i64 extern ;

// ── Sockets & Networking ─────────────────────────────────────────────────────
fn socket(domain i32, type_i32 i32, protocol i32) i32 extern ;
fn bind(sockfd i32, addr i64, addrlen u32) i32 extern ;
fn listen(sockfd i32, backlog i32) i32 extern ;
fn accept(sockfd i32, addr i64, addrlen i64) i32 extern ;
fn connect(sockfd i32, addr i64, addrlen u32) i32 extern ;
fn setsockopt(sockfd i32, level i32, optname i32, optval i64, optlen u32) i32 extern ;
fn shutdown(sockfd i32, how i32) i32 extern ;
fn htons(hostshort u16) u16 extern ;
fn ntohs(netshort u16) u16 extern ;

// ── Threads & Mutexes (POSIX Threads) ───────────────────────────────────────
fn pthread_create(thread i64, attr i64, start_routine i64, arg i64) i32 extern ;
fn pthread_join(thread u64, retval i64) i32 extern ;
fn pthread_detach(thread u64) i32 extern ;
fn pthread_mutex_init(mutex i64, attr i64) i32 extern ;
fn pthread_mutex_lock(mutex i64) i32 extern ;
fn pthread_mutex_unlock(mutex i64) i32 extern ;
fn pthread_mutex_destroy(mutex i64) i32 extern ;
fn pthread_cond_init(cond i64, attr i64) i32 extern ;
fn pthread_cond_wait(cond i64, mutex i64) i32 extern ;
fn pthread_cond_signal(cond i64) i32 extern ;
fn pthread_cond_broadcast(cond i64) i32 extern ;
fn pthread_cond_destroy(cond i64) i32 extern ;

// ── Mantis Runtime Entrypoints ──────────────────────────────────────────────
fn mantis_get_argc() i32 extern ;
fn mantis_get_argv() i64 extern ;
fn mantis_get_envp() i64 extern ;
