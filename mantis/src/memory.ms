


fn split_strings(s String, c Char) Vec[String] {
	mut result = Vec[String].new();
	mut prev = 0;
	
	loop {
		if (s[i] == c) {
			result.push(s.substring(prev, i));
			prev = i + 1;
		}
	}

	return result;

} (mut ref allocator = GlobalAllocator)


type ArenaAllocator = struct {
	pages Vec[Array[u8]],
	cursor u64
}

#impl(ArenaAllocator, Default);
#impl(ArenaAllocator, Clone);

impl ArenaAllocator {
	fn new() Self {
		Self.default()
	}

	fn has_space_for(self, size u64) {
		mut last_page = self.pages.last();
		if last_page.is_none() {
			self.pages.push(Array[u8].fill(0, 16 * 1024));
			last_page = self.pages.last();
		}
		last_page.unwrap();
		#todo();
	}
}



impl Allocator for ArenaAllocator {


	pub fn alloc(v T) ptr[T] {
		let size = #sizeof(T);

	}

	pub fn dealloc(v ptr[T]) unsafe {
		
	}
}



fn foo() {
	mut arena = ArenaAllocator.new();

	with (allocator = arena) {
		let s = String.from("Hello World Hello World");
		let result = without split_strings(s, ' '); // Type of result = Vec[String]ref GlobalAllocator

		return result;
	}

	
	let s = with (allocator = arena) String.from("Hello World Hello World");
	let result = with (alloactor = arena) split_strings(s, ' '); // Type of result = Vec[String]@[ref arena]

	arena.free();

	result // shouldn't be used;
	
}
