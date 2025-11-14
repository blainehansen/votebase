fn main() {
	deno_core::extension!(
		// extension name
		votebase,
		// list of all JS files in the extension
		esm_entry_point = "ext:votebase/src/runtime/runtime.ts",
		// the entrypoint to our extension
		esm = ["src/runtime/runtime.ts"]
	);

	let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
	let snapshot_path = out_dir.join("VOTEBASE_SNAPSHOT.bin");

	let snapshot = deno_core::snapshot::create_snapshot(
		deno_core::snapshot::CreateSnapshotOptions {
			cargo_manifest_dir: std::env!("CARGO_MANIFEST_DIR"),
			startup_snapshot: None,
			skip_op_registration: false,
			extensions: vec![votebase::init()],
			with_runtime_cb: None,
			extension_transpiler: Some(std::rc::Rc::new(|module_name, module_code| {
				transpile_utils::transpile_typescript(module_name, module_code)
					.map_err(|e| deno_error::JsErrorBox::generic(e.to_string()))
			})),
		},
		None,
	)
	.unwrap();

	std::fs::write(snapshot_path, snapshot.output).unwrap();
}
