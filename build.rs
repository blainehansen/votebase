use deno_core::extension;
use std::env;
use std::path::PathBuf;

fn main() {
	extension!(
		// extension name
		votebase,
		// list of all JS files in the extension
		esm_entry_point = "ext:votebase/src/runtime.ts",
		// the entrypoint to our extension
		esm = ["src/runtime.ts"]
	);

	let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
	let snapshot_path = out_dir.join("VOTEBASE_SNAPSHOT.bin");

	let snapshot = deno_core::snapshot::create_snapshot(
		deno_core::snapshot::CreateSnapshotOptions {
			cargo_manifest_dir: env!("CARGO_MANIFEST_DIR"),
			startup_snapshot: None,
			skip_op_registration: false,
			extensions: vec![votebase::init_ops_and_esm()],
			with_runtime_cb: None,
			extension_transpiler: Some(std::rc::Rc::new(transpile_helpers::transpile_typescript)),
		},
		None,
	)
	.unwrap();

	std::fs::write(snapshot_path, snapshot.output).unwrap();
}
