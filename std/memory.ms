// std.memory — heap allocation.
//
// `GlobalAllocator` is the process-wide allocator facade over libc
// malloc/free/memcpy. Regular code should never call libc allocation
// directly: use `Box[T]` for single owned values, and `Vec[T]` /
// `String` (which are built on GlobalAllocator) for growable data.
//
// Owning std types implement the `Drop` trait, so their memory is released
// automatically (RAII) when they go out of scope or their ownership is
// moved. Manual `.free()` is only needed for unusual lifetimes.

use libc;

// ── Standard Conversion & Dereferencing Traits ───────────────────────────────

pub trait AsRef[T] {
    pub fn as_ref(self @Self) @T;
}

pub trait AsMut[T] {
    pub fn as_mut(self @mut Self) @mut T;
}

pub trait Deref[T] {
    pub fn deref(self @Self) @T;
}

// ── Global Allocator Facade ──────────────────────────────────────────────────

pub type MemoryAllocator = struct {

};

impl MemoryAllocator {
    /// Allocate `count * item_size` bytes. Returns a raw address.
    pub fn alloc(self @MemoryAllocator, count u64, item_size u64) i64 {
        return libc.malloc((item_size * count) as i64);
    }

    /// Release an allocation obtained from `alloc`.
    pub fn dealloc(self @MemoryAllocator, ptr i64) {
        libc.free(ptr);
    }

    /// Copy `byte_count` bytes from `src` to `dest`.
    pub fn copy_bytes(self @MemoryAllocator, dest i64, src i64, byte_count u64) {
        libc.memcpy(dest, src, byte_count as i64);
    }

    /// Allocate zero-initialized storage for `count` elements of `item_size`.
    pub fn alloc_zeroed(self @MemoryAllocator, count u64, item_size u64) i64 {
        let ptr = self.alloc(count, item_size);
        libc.memset(ptr, 0, (item_size * count) as i64);
        return ptr;
    }
}

/// The global allocator instance used across std and user code.
static const GlobalAllocator : MemoryAllocator = MemoryAllocator{};

// ─────────────────────────────────────────────────────────────────────────────
// Box[T] — Rust-style owning pointer.
//
// A Box owns one heap-allocated value of type T. It takes care of the raw
// pointer so regular code never has to:
//
//     use std.memory;
//
//     let b = Box.new(42);       // allocate + store
//     let v = b.get();           // read  -> 42
//     b.set(7);                  // write
//                                 // freed automatically at end of scope (RAII)
//
// Reach for raw pointers (`@T`) only when interfacing with C or when you
// really, really need to.
// ─────────────────────────────────────────────────────────────────────────────

pub type Box[T] = struct {
    ptr @T,
};

impl[T] Box[T] {
    /// Allocate on the heap and move `value` into it.
    pub fn new(value T) Box[T] {
        let ptr = GlobalAllocator.alloc(1, #size_of(T)) as @T;
        (ptr + 0 * #size_of(T)) @= value;
        return Box[T] { ptr: ptr };
    }

    /// Adopt an existing allocation. The Box takes ownership of it.
    pub fn from_ptr(ptr @T) Box[T] {
        return Box[T] { ptr: ptr };
    }

    /// Read a copy of the contained value.
    pub fn get(self @Box[T]) T {
        return @(self.ptr);
    }

    /// Overwrite the contained value.
    pub fn set(self @Box[T], value T) {
        (self.ptr + 0 * #size_of(T)) @= value;
    }

    /// Mutable escape hatch: direct pointer to the contained value.
    /// Only for FFI or manual memory control. The Box still owns it.
    pub fn as_ptr(self @Box[T]) @T {
        return self.ptr;
    }

    /// Release early. Optional — RAII frees automatically at scope exit.
    pub fn free(self @Box[T]) {
        GlobalAllocator.dealloc(self.ptr as i64);
    }
}

impl[T] AsRef[T] for Box[T] {
    pub fn as_ref(self @Box[T]) @T {
        return self.ptr;
    }
}

impl[T] Deref[T] for Box[T] {
    pub fn deref(self @Box[T]) @T {
        return self.ptr;
    }
}

/// RAII: release the heap allocation when the Box goes out of scope.
impl[T] Drop for Box[T] {
    pub fn drop(self @mut Box[T]) {
        GlobalAllocator.dealloc(self.ptr as i64);
    }
}
