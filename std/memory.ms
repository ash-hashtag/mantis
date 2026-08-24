

type MemoryAllocator = struct {

};


impl MemoryAllocator {
    fn alloc(self @MemoryAllocator, count u64, item_size u64) i64 {
        return malloc(item_size * count);
    }

    fn dealloc(self @MemoryAllocator, ptr i64) {
        free(ptr);
    }

    fn copy_bytes(self @MemoryAllocator, dest i64, src i64, byte_count u64) {
        memcpy(dest, src, byte_count);
    }
}

static const GlobalAllocator : MemoryAllocator = MemoryAllocator{};
