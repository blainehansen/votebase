Lexically analyzed comments for rulesets to allow operations on objects, if technically feasible. Why? Because we're making clear this isn't a feature of the database, we're inching toward being a database ourselves, and we want these declarations to be adjacent to the things they effect

Possible good cut points:
Just fully functioning single root with no children at all, but with the full system end to end including cli etc, and good testing etc with podman
Then add events, static and dynamic
Then add children, static and dynamic, but no sharing
Then add sharing
Then add member columns etc
Then add membership propositions









































how to use deno core to allow scripts to import libraries but only from jsr

To use `deno_core` to allow scripts to import libraries **only** from JSR (and restrict all other remote sources like `npm:`, `http:`, or `https:`), you must implement a custom `ModuleLoader`. `deno_core` itself does not have built-in knowledge of `jsr:` specifiers or policy enforcement; it relies entirely on the `ModuleLoader` trait to resolve import strings to URLs and then load the source code for those URLs.

### Concept: The Module Loader
The `ModuleLoader` trait has two key methods you must implement to enforce your policy:
1.  **`resolve`**: Converts an import string (e.g., `"jsr:@std/assert"`) into a `ModuleSpecifier` (URL). Here you will detect `jsr:` specifiers and transform them into fetchable HTTPS URLs (pointing to the JSR registry). You will also **reject** unwanted schemes like `npm:` or arbitrary `https:` URLs that do not match the JSR registry.
2.  **`load`**: Takes the resolved URL and fetches the source code. Here you ensure that only local files (for the entrypoint) and valid JSR registry URLs are fetched.

### Resolution Strategy
Since `deno_core` is low-level, it does not automatically resolve `jsr:` version constraints (like `^1.0`). To fully support JSR, your loader must:
1.  **Parse** the `jsr:` specifier.
2.  **Query** the JSR registry API (e.g., `https://jsr.io/@<scope>/<package>/meta.json`) to resolve the semver constraint to a specific version.
3.  **Return** a fully qualified URL to the source file (e.g., `https://jsr.io/@<scope>/<package>/<version>/<path>`).

For the purpose of "allowing only JSR," strict filtering is applied during the `resolve` step.

### Code Example
The following Rust example demonstrates a `ModuleLoader` that permits `file:` (for the main script) and `jsr:` imports, while blocking everything else. Note that a full production-ready JSR resolver requires complex semver logic (often handled by crates like `deno_graph`), so this example mocks the resolution to a fixed URL for demonstration.

```rust
use deno_core::anyhow::{anyhow, Error};
use deno_core::{
    resolve_import, ModuleLoader, ModuleSource, ModuleSourceFuture, ModuleSpecifier,
    ResolutionKind, ModuleType,
};
use std::pin::Pin;
use std::sync::Arc;

struct JsrOnlyModuleLoader;

impl ModuleLoader for JsrOnlyModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, Error> {
        // 1. Handle "jsr:" specifiers
        if specifier.starts_with("jsr:") {
            // In a real app, you must parse "jsr:@scope/pkg@ver" and fetch metadata
            // to find the exact HTTPS URL.
            // For this example, we manually map a specific import to its JSR CDN URL.
            if specifier == "jsr:@std/assert" {
                return Ok(ModuleSpecifier::parse("https://jsr.io/@std/assert/1.0.0/mod.ts")?);
            }
            // If we can't resolve it (or it's not strictly a jsr specifier), strict reject.
            return Err(anyhow!("JSR import resolution failed for: {}", specifier));
        }

        // 2. Handle relative imports (e.g., "./utils.ts")
        // These are allowed but must resolve to either a local file or a JSR URL.
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let resolved = resolve_import(specifier, referrer)?;
            let scheme = resolved.scheme();

            // Allow if it stays within the file system (entrypoint) or JSR registry
            if scheme == "file" || (scheme == "https" && resolved.host_str() == Some("jsr.io")) {
                return Ok(resolved);
            }
            return Err(anyhow!("Relative import blocked: {} (origin: {})", specifier, scheme));
        }

        // 3. Block everything else (npm:, http:, arbitrary https:)
        Err(anyhow!("Import denied: '{}'. Only 'jsr:' imports are allowed.", specifier))
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: deno_core::RequestedModuleType,
    ) -> ModuleSourceFuture {
        let module_specifier = module_specifier.clone();

        // Return a future that fetches the code
        Box::pin(async move {
            let scheme = module_specifier.scheme();

            // Enforce Policy in Load: Only 'file' (entrypoint) or 'https' from jsr.io
            if scheme == "https" && module_specifier.host_str() != Some("jsr.io") {
                 return Err(anyhow!("Security Error: Attempted to load from unauthorized host: {}", module_specifier));
            }
            if scheme != "file" && scheme != "https" {
                return Err(anyhow!("Security Error: valid schemes are 'file' and 'jsr' (resolved to https)"));
            }

            // Implement actual fetching logic (reqwest, fs::read, etc.)
            // This is a placeholder for the fetch logic
            let code = if scheme == "file" {
                std::fs::read(module_specifier.to_file_path().unwrap())? // Local file
            } else {
                // Fetch from JSR CDN (requires an HTTP client like reqwest)
                // let resp = reqwest::get(module_specifier.clone()).await?;
                // resp.bytes().await?.to_vec()
                vec![] // Empty for demo
            };

            Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSource::FromBytes(code.into()),
                &module_specifier,
                None,
            ))
        })
    }
}
```

### Implementation Details

#### Resolving JSR Specifiers
`jsr:` specifiers are not native URLs; they are aliases that point to a registry [1]. In the `resolve` method, you normally cannot just "allow" the string `jsr:...` because the V8 engine requires a valid resource URL.
*   **Real-world implementation**: You would use the `deno_graph` crate to handle the complex resolution of `jsr:` imports [2][3]. This crate can traverse the dependency graph and resolve semver constraints using the JSR API.
*   **Manual implementation**: If you avoid `deno_graph`, you must manually request the `meta.json` from JSR (e.g., `https://jsr.io/@std/assert/meta.json`), parse the available versions to satisfy the requested range (e.g., `^1.0`), and construct the final URL [4].

#### Security and Filtering
The "only jsr" policy is enforced by returning an error for any scheme that isn't `jsr:` in the `resolve` function [5][6].
*   **Blocking npm**: Explicitly check `specifier.starts_with("npm:")` and return an error.
*   **Blocking http/https**: Check `specifier.starts_with("http")` and return an error unless the referrer is already a trusted JSR URL (indicating an internal dependency of a JSR package) [7].

#### Transitive Dependencies
Be aware that JSR packages themselves can import `npm:` packages or other JSR packages [5]. If you strictly block `npm:`, some JSR libraries may fail to load if they depend on npm compatibility. You may need to inspect the dependency graph of the specific JSR libraries you intend to support.

Citations:
[1] Native JSR imports https://jsr.io/docs/native-imports
[2] @deno/graph - JSR https://jsr.io/@deno/graph
[3] denoland/deno_graph: The module graph logic for Deno CLI - GitHub https://github.com/denoland/deno_graph
[4] Standard Library (@std) - Deno Docs https://docs.deno.com/runtime/reference/std/
[5] Troubleshooting - Docs - JSR https://jsr.io/docs/troubleshooting
[6] 2.4 Deno components - The Internals of Deno - GitBook https://choubey.gitbook.io/internals-of-deno/architecture/core
[7] Prevent modules imported through https to internally import http ... https://github.com/denoland/deno/issues/1064
[8] Proposal: Module Loader API · Issue #8327 · denoland/deno - GitHub https://github.com/denoland/deno/issues/8327
[9] Loading a module with `.load_main_es_module_from_code ... - Deno https://questions.deno.com/m/1234905144517459968
[10] FsModuleLoader in deno_core - Rust - Docs.rs https://docs.rs/deno_core/latest/deno_core/struct.FsModuleLoader.html
[11] andreubotella/deno-simple-module-loader - GitHub https://github.com/andreubotella/deno-simple-module-loader
[12] How Deno works - Roman Zaynetdinov (zaynetro) https://www.zaynetro.com/post/2023-how-deno-works
[13] Using JSR with Deno https://jsr.io/docs/with/deno
[14] Security and permissions - Deno Docs https://docs.deno.com/runtime/fundamentals/security/
[15] Roll your own JavaScript runtime, pt. 2 - Deno https://deno.com/blog/roll-your-own-javascript-runtime-pt2
[16] deno_core - Rust - Docs.rs https://docs.rs/deno_core/latest/deno_core/
[17] Deno panic when importing jsr package with @next label #22420 https://github.com/denoland/deno/issues/22420
[18] Modules and dependencies - Deno Docs https://docs.deno.com/runtime/fundamentals/modules/
[19] jsr/frontend/docs/with/deno.md at main · jsr-io/jsr https://github.com/jsr-io/jsr/blob/main/frontend/docs/with/deno.md
[20] how can i get this import to work in deno? - Stack Overflow https://stackoverflow.com/questions/79215355/how-can-i-get-this-import-to-work-in-deno
[21] jsr: scheme not supported in package.json #30568 - GitHub https://github.com/denoland/deno/issues/30568
[22] Confuse about the way import module on Deno project https://stackoverflow.com/questions/78505466/confuse-about-the-way-import-module-on-deno-project
[23] JsRuntime initialization fails when integrating deno_url extension https://github.com/denoland/deno/issues/27413
[24] jsr specifier resolves to local package when referencing itself #22667 https://github.com/denoland/deno/issues/22667
[25] deno_graph - crates.io: Rust Package Registry https://crates.io/crates/deno_graph
[26] Deno panics on specifying latest JSR dependencies #31298 - GitHub https://github.com/denoland/deno/issues/31298
[27] How we built JSR | Deno https://deno.com/blog/how-we-built-jsr
[28] js-resolve - crates.io: Rust Package Registry https://crates.io/crates/js-resolve
[29] deno publish --resolve-import-map-specifiers flag #24496 - GitHub https://github.com/denoland/deno/discussions/24496
[30] Specifying Dependencies - The Cargo Book - Rust Documentation https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html
[31] Publishing packages - Docs - JSR https://jsr.io/docs/publishing-packages
[32] rustbolt_resolver - Rust - Docs.rs https://docs.rs/rustbolt_resolver
[33] Publishing Modules with JSR - Deno Docs https://docs.deno.com/examples/publishing_modules_with_jsr/
[34] Deno: What we got wrong about HTTP imports - Hacker News https://news.ycombinator.com/item?id=41101429
[35] deno install https://docs.deno.com/runtime/reference/cli/install/
[36] How to make rust crate with C lib dependency "wasm32-unknown ... https://users.rust-lang.org/t/how-to-make-rust-crate-with-c-lib-dependency-wasm32-unknown-unknown-compatible/132125
[37] deno_graph - Rust - Docs.rs https://docs.rs/deno_graph
[38] deno compile, standalone executables https://docs.deno.com/runtime/reference/cli/compile/
[39] deno - crates.io: Rust Package Registry https://crates.io/crates/deno
[40] deno - crates.io: Rust Package Registry https://crates.io/crates/deno/dependencies
[41] deno_core - crates.io: Rust Package Registry https://crates.io/crates/deno_core
[42] deno_io - crates.io: Rust Package Registry https://crates.io/crates/deno_io
[43] jsr-io/jsr: The open-source package registry for modern ... - GitHub https://github.com/jsr-io/jsr
[44] node_resolver - crates.io: Rust Package Registry https://crates.io/crates/node_resolver
[45] node_resolver - crates.io: Rust Package Registry https://crates.io/crates/node_resolver/0.54.0
[46] The Deno Toolchain https://deno.niklasmtj.de/guide/toolchain/
[47] node_resolver - crates.io: Rust Package Registry https://crates.io/crates/node_resolver/0.58.0/dependencies
[48] deno_web - crates.io: Rust Package Registry https://crates.io/crates/deno_web
[49] deno_resolver - crates.io: Rust Package Registry https://crates.io/crates/deno_resolver/0.37.0
[50] Roll your own JavaScript runtime | Deno https://deno.com/blog/roll-your-own-javascript-runtime
[51] deno_error - crates.io: Rust Package Registry https://crates.io/crates/deno_error
[52] deno_resolver - crates.io: Rust Package Registry https://crates.io/crates/deno_resolver/dependencies
[53] std@0.182.0 | Deno https://deno.land/std@0.182.0
[54] deno_runtime - crates.io: Rust Package Registry https://crates.io/crates/deno_runtime
