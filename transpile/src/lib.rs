fn js_err<E: deno_error::JsErrorClass>(e: E) -> deno_error::JsErrorBox {
	deno_error::JsErrorBox::from_err(e)
}

pub fn transpile_typescript(
	module_name: deno_core::ModuleName,
	module_code: deno_core::ModuleCodeString,
) -> Result<(deno_core::ModuleCodeString, Option<deno_core::SourceMapData>), deno_error::JsErrorBox> {
	let parsed = deno_ast::parse_module(deno_ast::ParseParams {
		specifier: deno_core::resolve_url(module_name.as_str()).map_err(js_err)?,
		text: module_code.into(),
		media_type: deno_ast::MediaType::TypeScript,
		capture_tokens: false,
		scope_analysis: false,
		maybe_syntax: None,
	}).map_err(js_err)?;
	let transpiled_code = parsed
		.transpile(&Default::default(), &Default::default(), &Default::default()).map_err(js_err)?
		.into_source().text.into();

	Ok((transpiled_code, None))
}
