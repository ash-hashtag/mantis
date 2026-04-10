

fn main() {
	let v = Vec[f32].new();

	let f = (x f32) f64 {
		return x.into[f64]();
	};

	let b = v.iter().map(f).collect[Vec[f64]]();
}
