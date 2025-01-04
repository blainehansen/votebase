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

// esm_entry_point = "ext:runjs/runtime.js",
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

#[deno_core::op2(async)]
#[string]
async fn op_read_file(#[string] path: String) -> Result<String, deno_core::error::AnyError> {
	let contents = tokio::fs::read_to_string(path).await?;
	Ok(contents)
}

#[deno_core::op2(async)]
async fn op_write_file(
	#[string] path: String,
	#[string] contents: String,
) -> Result<(), deno_core::error::AnyError> {
	tokio::fs::write(path, contents).await?;
	Ok(())
}

#[deno_core::op2(async)]
#[string]
async fn op_fetch(#[string] url: String) -> Result<String, deno_core::error::AnyError> {
	let body = reqwest::get(url).await?.text().await?;
	Ok(body)
}

#[deno_core::op2(async)]
async fn op_set_timeout(delay: f64) -> Result<(), deno_core::error::AnyError> {
	tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
	Ok(())
}

#[deno_core::op2(fast)]
fn op_remove_file(#[string] path: String) -> Result<(), deno_core::error::AnyError> {
	std::fs::remove_file(path)?;
	Ok(())
}

struct TsModuleLoader;

impl deno_core::ModuleLoader for TsModuleLoader {
	fn resolve(
		&self,
		specifier: &str,
		referrer: &str,
		_kind: deno_core::ResolutionKind,
	) -> Result<deno_core::ModuleSpecifier, deno_core::error::AnyError> {
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

static RUNTIME_SNAPSHOT: &[u8] =
	include_bytes!(concat!(env!("OUT_DIR"), "/RUNJS_SNAPSHOT.bin"));

deno_core::extension!(
	runjs,
	ops = [
		op_read_file,
		op_write_file,
		op_remove_file,
		op_fetch,
		op_set_timeout,
	]
);

async fn run_js(file_path: &str) -> Result<(), deno_core::error::AnyError> {
	let current_dir = std::env::current_dir()?;

	let side_module = deno_core::resolve_path(file_path, &current_dir)?;
	let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
		module_loader: Some(std::rc::Rc::new(TsModuleLoader)),
		startup_snapshot: Some(RUNTIME_SNAPSHOT),
		extensions: vec![runjs::init_ops()],
		..Default::default()
	});

	let mod_id = js_runtime.load_side_es_module(&side_module).await?;
	let result = js_runtime.mod_evaluate(mod_id);
	js_runtime.run_event_loop(Default::default()).await?;
	result.await?;

	let function_name = "yo";
	let module_name = "example";
	let raw_input = "4";
	let main_code = format!("import {{{function_name}}} from './{module_name}.ts'; {function_name}({raw_input})");
	let main_module = deno_core::resolve_path("<main>", &current_dir)?;
	let mod_id = js_runtime.load_main_es_module_from_code(&main_module, main_code).await?;
	let result = js_runtime.mod_evaluate(mod_id);
	js_runtime.run_event_loop(Default::default()).await?;
	result.await?;

	// https://github.com/denoland/deno_core/issues/515
	// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1201661871959310346
	// https://gist.github.com/alshdavid/c9e5bc0d794e3ec9dba6afaa689b704e#file-main-rs-L51

	// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1074150763460313128

	Ok(())
}

fn main() {
	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap();

	if let Err(error) = runtime.block_on(run_js("example.ts")) {
		eprintln!("error: {error}");
	}
}
