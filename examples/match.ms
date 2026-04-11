type OptionStrSlice = enum { Some(StrSlice), None }

type SplitIterator = struct {
	src StrSlice,
	delim char,
	cursor i64
}

impl OptionStrSlice {
	fn is_some(self) bool {
		match self {
			OptionStrSlice.Some(_) : { return true; },
			OptionStrSlice.None : { return false; }
		}
	}

	fn unwrap(self) StrSlice {
		match self {
			OptionStrSlice.Some(x) : { return x; },
			OptionStrSlice.None : { return ""; }
		}
	}
}

impl StrSlice {
	fn split(self, delim char) @SplitIterator {
		let iter = #malloc(#size_of(SplitIterator)) as @SplitIterator;
		iter.src = self;
		iter.delim = delim;
		iter.cursor = 0;
		return iter;
	}
}

impl SplitIterator {
	fn next(mut self) OptionStrSlice {
		if self.cursor >= self.src.len {
			return OptionStrSlice.None;
		}
		
		let start = self.cursor;
		mut end = start;
		loop {
			if end >= self.src.len {
				break;
			}
			let ptr = (self.src.pointer + end) as @char;
			if @ptr == self.delim {
				break;
			}
			end = end + 1;
		}
		
		self.cursor = end + 1;
		
		let slice_ptr = #malloc(#size_of(StrSlice)) as @StrSlice;
		(@slice_ptr).pointer = self.src.pointer + start;
		(@slice_ptr).len = end - start;
		return OptionStrSlice.Some(@slice_ptr);
	}
}

fn main() {

	let src = "hello world";
	mut iter = src.split(' '); 

	loop {
		let next = iter.next();
		if next.is_some() {
			let next_val = next.unwrap();
			match next_val {
				"hello" : {
					print("Hello");
				},
				"world": {
					print("World");
				},
				_ : {
					print("Other");
				}
			}
		} else {
			break;
		}
	}
}
