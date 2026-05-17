use std::collections::HashMap;

use deno_core::{
	ModuleLoader, ModuleSpecifier, ModuleSource, ModuleSourceCode,
	ModuleType, ResolutionKind,
};

use crate::runtime::js_err;

pub(crate) struct FileMapModuleLoader {
	files: HashMap<ModuleSpecifier, String>,
}

impl FileMapModuleLoader {
	pub(crate) fn new(files: HashMap<ModuleSpecifier, String>) -> Self {
		Self { files }
	}
}

impl ModuleLoader for FileMapModuleLoader {
	fn resolve(
		&self,
		specifier: &str,
		referrer: &str,
		kind: ResolutionKind,
	) -> Result<ModuleSpecifier, deno_error::JsErrorBox> {
		if let ResolutionKind::DynamicImport = kind {
			return Err(deno_error::JsErrorBox::generic("dynamic imports aren't allowed"))
		}

		let base = ModuleSpecifier::parse(referrer).map_err(js_err)?;
		let resolved = deno_core::resolve_import(specifier, base.as_str()).map_err(js_err)?;
		Ok(resolved)
	}

	fn load(
		&self,
		module_specifier: &ModuleSpecifier,
		_maybe_referrer: Option<&deno_core::ModuleLoadReferrer>,
		_module_load_options: deno_core::ModuleLoadOptions,
	) -> deno_core::ModuleLoadResponse {
		let spec = module_specifier.clone();
		let source = self.files.get(&spec).cloned();

		deno_core::ModuleLoadResponse::Sync(
			source
				.ok_or_else(|| {
					deno_error::JsErrorBox::generic(format!("Module not found in in‑memory map: {spec}"))
				})
				.map(|code| ModuleSource::new(
					ModuleType::JavaScript, ModuleSourceCode::String(code.into()), &spec, None,
				))
		)
	}
}
