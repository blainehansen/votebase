use deno_core::v8;

pub type DenoError = deno_core::error::AnyError;

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

pub async fn run_function(
	constitution_code: String,
	function_name: &str,
	function_arg: serde_json::Value,
) -> Result<serde_json::Value, DenoError> {
	let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
		module_loader: None,
		startup_snapshot: Some(RUNTIME_SNAPSHOT),
		extensions: vec![votebase::init_ops()],
		..Default::default()
	});

	const MAIN_SPECIFIER: &'static str = "votebase:<main>";
	let (constitution_code, _) = transpile_helpers::transpile_typescript(
		deno_core::ascii_str!(MAIN_SPECIFIER).into(),
		constitution_code.into(),
	)?;
	let specifier = deno_core::resolve_url(MAIN_SPECIFIER)?;
	let mod_id = js_runtime.load_main_es_module_from_code(&specifier, constitution_code).await?;
	let result = js_runtime.mod_evaluate(mod_id);
	js_runtime.run_event_loop(Default::default()).await?;
	result.await?;

	let state: FunctionMap = js_runtime.op_state().borrow_mut().take();
	let function = state.get(function_name).unwrap();

	let function_arg = {
		let mut scope = js_runtime.handle_scope();
		let function_arg = deno_core::serde_v8::to_v8(&mut scope, function_arg)?;
		v8::Global::new(&mut scope, function_arg)
	};

	let call = js_runtime.call_with_args(&function, &[function_arg]);
	let call_return_value = js_runtime
		.with_event_loop_promise(call, deno_core::PollEventLoopOptions::default())
		.await?;

	let mut scope = js_runtime.handle_scope();
	let call_return_value = v8::Local::new(&mut scope, call_return_value);
	Ok(deno_core::serde_v8::from_v8(&mut scope, call_return_value)?)
}
