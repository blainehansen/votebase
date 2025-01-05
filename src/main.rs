struct TsModuleLoader;

impl deno_core::ModuleLoader for TsModuleLoader {
	fn resolve(
		&self,
		specifier: &str,
		referrer: &str,
		_kind: deno_core::ResolutionKind,
	) -> Result<deno_core::ModuleSpecifier, DenoError> {
		deno_core::resolve_import(specifier, referrer).map_err(|e| e.into())
	}

	fn load(
		&self,
		module_specifier: &deno_core::ModuleSpecifier,
		_maybe_referrer: Option<&deno_core::ModuleSpecifier>,
		_is_dyn_import: bool,
		_requested_module_type: deno_core::RequestedModuleType,
	) -> deno_core::ModuleLoadResponse {
		let module_specifier = module_specifier.clone();

		let module_load = move || {
			let path = module_specifier.to_file_path().unwrap();

			use deno_ast::MediaType;
			let media_type = MediaType::from_path(&path);
			let (module_type, should_transpile) = match MediaType::from_path(&path) {
				MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
					(deno_core::ModuleType::JavaScript, false)
				}
				MediaType::Jsx => (deno_core::ModuleType::JavaScript, true),
				MediaType::TypeScript
				| MediaType::Mts
				| MediaType::Cts
				| MediaType::Dts
				| MediaType::Dmts
				| MediaType::Dcts
				| MediaType::Tsx => (deno_core::ModuleType::JavaScript, true),
				MediaType::Json => (deno_core::ModuleType::Json, false),
				_ => panic!("Unknown extension {:?}", path.extension()),
			};

			let code = std::fs::read_to_string(&path)?;
			use deno_ast::ParseParams;
			let code = if should_transpile {
				let parsed = deno_ast::parse_module(ParseParams {
					specifier: module_specifier.clone(),
					text: code.into(),
					media_type,
					capture_tokens: false,
					scope_analysis: false,
					maybe_syntax: None,
				})?;
				parsed
					.transpile(&Default::default(), &Default::default(), &Default::default())?
					.into_source()
					.text.into_bytes()
			} else {
				code.into_bytes()
			};
			let module = deno_core::ModuleSource::new(
				module_type,
				deno_core::ModuleSourceCode::Bytes(code.into_boxed_slice().into()),
				&module_specifier,
				None,
			);
			Ok(module)
		};

		deno_core::ModuleLoadResponse::Sync(module_load())
	}
}



// #[macro_use] extern crate rocket;

// #[derive(Debug)]
// struct PathList<'r>(Vec<&'r str>);

// impl <'r> rocket::request::FromSegments<'r> for PathList<'r> {
// 	type Error = std::convert::Infallible;

// 	fn from_segments(raw_segments: rocket::http::uri::Segments<'r, rocket::http::uri::fmt::Path>) -> Result<Self, Self::Error> {
// 		Ok(PathList(raw_segments.into_iter().collect()))
// 	}
// }

// #[catch(default)]
// fn default_catcher(status: rocket::http::Status, req: &rocket::Request<'_>) -> rocket::response::status::Custom<String> {
// 	let msg = format!("{} ({})", status, req.uri());
// 	rocket::response::status::Custom(status, msg)
// }

// #[launch]
// fn rocket() -> _ {
// 	rocket::build()
// 		.configure(rocket::Config {
// 			port: 8080,
// 			..Default::default()
// 		})
// 		.register("/", catchers![default_catcher])
// 		.mount("/", routes![execute_action, execute_view])
// }

// #[post("/action/<path..>?<_arg>")]
// fn execute_action(path: PathList<'_>, _arg: Option<&str>) -> String {
// 	format!("{:?}", path.0)
// }

// esm_entry_point = "ext:votebase/runtime.js",
// esm = [dir "src", "runtime.js"],

// #[get("/view/<_path..>?<_query>")]
// fn execute_view(_path: PathList<'_>, _query: Option<&str>) {
// 	// Initialize a runtime instance
// 	let mut runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
// 		extensions: vec![my_ext::init_ops()],
// 		// create_params: Some(v8::Isolate::create_params()
// 		// 	.heap_limits(initial, max)
// 		// ),
// 		..Default::default()
// 	});

// 	// Now we see how to invoke the op we just defined. The runtime automatically
// 	// contains a Deno.core object with several functions for interacting with it.
// 	// You can find its definition in core.js.
// 	runtime
// 		.execute_script(
// 			"<usage>",
// 			r#"
// // Print helper function, calling Deno.core.print()
// function print(value) {
// 	Deno.core.print(value.toString()+"\n");
// }

// const arr = [1, 2, 3];
// print("The sum of");
// print(arr);
// print("is");
// print(Deno.core.ops.op_sum(arr));

// // And incorrect usage
// try {
// 	print(Deno.core.ops.op_sum(0));
// } catch(e) {
// 	print('Exception:');
// 	print(e);
// }
// "#,
// 		)
// 		.unwrap();
// }

use deno_core::v8;

type DenoError = deno_core::error::AnyError;

#[deno_core::op2(async)]
#[string]
async fn op_fetch(#[string] url: String) -> Result<String, DenoError> {
	let body = reqwest::get(url).await?.text().await?;
	Ok(body)
}

#[deno_core::op2(async)]
async fn op_set_timeout(delay: f64) -> Result<(), DenoError> {
	tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
	Ok(())
}


static RUNTIME_SNAPSHOT: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/VOTEBASE_SNAPSHOT.bin"));

type FunctionMap = std::collections::HashMap<String, v8::Global<v8::Function>>;


// https://github.com/denoland/deno_core/issues/515
// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1201661871959310346
// https://gist.github.com/alshdavid/c9e5bc0d794e3ec9dba6afaa689b704e#file-main-rs-L51

// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1074150763460313128

#[deno_core::op2]
fn op_register_func(
	#[state] function_state: &mut FunctionMap,
	#[string] key: String,
	#[global] func: v8::Global<v8::Function>,
) {
	function_state.insert(key, func);
}

deno_core::extension!(
	votebase,
	ops = [
		op_fetch,
		op_set_timeout,
		op_register_func,
	],
	state = |state: &mut deno_core::OpState| {
		state.put(std::collections::HashMap::<String, v8::Global<v8::Function>>::new());
	},
);

async fn run_function(constitution_code: String, function_name: &str) -> Result<(), DenoError> {
	let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
		// extension_transpiler: Some(),
		module_loader: None,
		startup_snapshot: Some(RUNTIME_SNAPSHOT),
		extensions: vec![votebase::init_ops()],
		..Default::default()
	});

	let specifier = deno_core::resolve_url("votebase:<main>")?;
	let mod_id = js_runtime.load_main_es_module_from_code(&specifier, constitution_code).await?;
	let result = js_runtime.mod_evaluate(mod_id);
	js_runtime.run_event_loop(Default::default()).await?;
	result.await?;

	let state: FunctionMap = js_runtime.op_state().borrow_mut().take();
	let function = state.get(function_name).unwrap();

	let arg = {
		let mut scope = js_runtime.handle_scope();
		let arg = deno_core::serde_v8::to_v8(&mut scope, "hello arg")?;
		v8::Global::new(&mut scope, arg)
	};

	let call = js_runtime.call_with_args(&function, &[arg]);

	let function_ret = js_runtime
		.with_event_loop_promise(call, deno_core::PollEventLoopOptions::default())
		.await?;

	let mut scope = js_runtime.handle_scope();
	let function_ret = v8::Local::new(&mut scope, function_ret);
	let function_ret: bool = deno_core::serde_v8::from_v8(&mut scope, function_ret)?;
	dbg!(function_ret);

	Ok(())
}

fn main() {
	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();

	if let Err(error) = runtime.block_on(run_function(CONSTITUTION_CODE.to_string(), FUNCTION_NAME)) {
		eprintln!("error: {error}");
	}
}

const CONSTITUTION_CODE: &'static str = r#"
console.log("wassup")

votebase.reg("hello", (arg) => {
	console.log(arg)
	return arg.startsWith("hello")
})
"#;
const FUNCTION_NAME: &'static str = "hello";
