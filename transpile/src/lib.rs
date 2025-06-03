#[derive(thiserror::Error, Debug)]
pub enum TranspileError {
	#[error(transparent)]
	DenoModuleResolutionError(#[from] deno_core::ModuleResolutionError),
	#[error(transparent)]
	DenoTranspileError(#[from] deno_ast::TranspileError),
	#[error(transparent)]
	DenoParseDiagnostic(#[from] deno_ast::ParseDiagnostic),
}

pub fn transpile_typescript(
	module_name: deno_core::ModuleName,
	module_code: deno_core::ModuleCodeString,
) -> Result<(deno_core::ModuleCodeString, Option<deno_core::SourceMapData>), TranspileError> {
	let parsed = deno_ast::parse_module(deno_ast::ParseParams {
		specifier: deno_core::resolve_url(module_name.as_str())?,
		text: module_code.into(),
		media_type: deno_ast::MediaType::TypeScript,
		capture_tokens: false,
		scope_analysis: false,
		maybe_syntax: None,
	})?;
	let transpiled_code = parsed
		.transpile(&Default::default(), &Default::default(), &Default::default())?
		.into_source().text.into();

	Ok((transpiled_code, None))
}
