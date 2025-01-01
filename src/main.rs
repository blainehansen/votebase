#[macro_use] extern crate rocket;

#[derive(Debug)]
struct PathList<'r>(Vec<&'r str>);

impl <'r> rocket::request::FromSegments<'r> for PathList<'r> {
	type Error = std::convert::Infallible;

	fn from_segments(raw_segments: rocket::http::uri::Segments<'r, rocket::http::uri::fmt::Path>) -> Result<Self, Self::Error> {
		Ok(PathList(raw_segments.into_iter().collect()))
	}
}

#[catch(default)]
fn default_catcher(status: rocket::http::Status, req: &rocket::Request<'_>) -> rocket::response::status::Custom<String> {
	let msg = format!("{} ({})", status, req.uri());
	rocket::response::status::Custom(status, msg)
}

#[launch]
fn rocket() -> _ {
	rocket::build()
		.configure(rocket::Config {
			port: 8080,
			..Default::default()
		})
		.register("/", catchers![default_catcher])
		.mount("/", routes![execute_action, execute_view])
}

#[post("/action/<path..>?<_arg>")]
fn execute_action(path: PathList<'_>, _arg: Option<&str>) -> String {
	format!("{:?}", path.0)
}


/// An op for summing an array of numbers. The op-layer automatically
/// deserializes inputs and serializes the returned Result & value.
#[deno_core::op2]
fn op_sum(#[serde] nums: Vec<f64>) -> Result<f64, deno_core::error::AnyError> {
	// Sum inputs
	let sum = nums.iter().fold(0.0, |a, v| a + v);
	// return as a Result<f64, AnyError>
	Ok(sum)
}


deno_core::extension!(
	my_ext,
	ops = [
		op_sum,
	],
);
// esm_entry_point = "ext:runjs/runtime.js",
// esm = [dir "src", "runtime.js"],

#[get("/view/<_path..>?<_query>")]
fn execute_view(_path: PathList<'_>, _query: Option<&str>) {
	// Initialize a runtime instance
	let mut runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
		extensions: vec![my_ext::init_ops()],
		// create_params: Some(v8::Isolate::create_params()
		// 	.heap_limits(initial, max)
		// ),
		..Default::default()
	});

	// Now we see how to invoke the op we just defined. The runtime automatically
	// contains a Deno.core object with several functions for interacting with it.
	// You can find its definition in core.js.
	runtime
		.execute_script(
			"<usage>",
			r#"
// Print helper function, calling Deno.core.print()
function print(value) {
	Deno.core.print(value.toString()+"\n");
}

const arr = [1, 2, 3];
print("The sum of");
print(arr);
print("is");
print(Deno.core.ops.op_sum(arr));

// And incorrect usage
try {
	print(Deno.core.ops.op_sum(0));
} catch(e) {
	print('Exception:');
	print(e);
}
"#,
		)
		.unwrap();
}
