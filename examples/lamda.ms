
fn main() {
	// A simple lambda that converts f32 to f64
	let f = (x f32) f64 {
		return x as f64;
	};

	// Call the lambda
	let val = 42.0 as f32;
	let result = f(val);

	// A lambda as a parameter (simulated)
	let apply = (f (f32) f64, x f32) f64 {
		return f(x);
	};

	let final_result = apply(f, 10.0 as f32);
}
