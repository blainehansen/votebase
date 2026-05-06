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









when the server is bootstrapped, the "accept-any" ruleset is loaded in with the name "root"
this ruleset simply accepts the first replacement ruleset that it's given

these replacement rulesets can specify static children, meaning there are some known children rulesets that will be created immediately after this one that themselves can be replaced or changed without changing the main ruleset. this would be useful for things like the persistent democracy kernel, which can't be replaced at all, so the static child is the practical root ruleset that people will actually interact with, the base constitution.

here's one of the key questions: do child rulesets replace themselves? or are they replaced by their parent?

in the case of the persistent democracy kernel, there's a single static child for the actual "real" root constitution, and *how* that root is replaced is determined by the kernel, so the replacement is done by the parent, not the child.

I have a suspicion self replacement is only relevant for the true top root, or for "away teams" so to speak, where some ruleset is given full ability to govern itself, with only possible dissolution by higher rulesets








Rulesets declare foreign sql functions they can call, and can parameterize both calls of foreign functions and references using ghetto declarations in some toml file or something put together with simple string templating. During preparation time the cli dummies together tables and empty functions that satisfies those declarations to make the type checker happy
The cli also gathers the normal functions and prepares everything for the server ahead of time, including type checking everything. Maybe the server does that as well

Someday the real thing will be a unified language, and rulesets can be parameterized by generics and full functions, even ones that return the special database reference type
The real full idea relies on a powerful database language with a powerful type system and static checker that can check incoming migrations for total consistency without actually changing anything, including difficult things like whether all references will remain valid
This will mean that when anything changes you have to go through candidates and either discard them or check them to see if they still make sense



https://crates.io/crates/strfmt



What is the technical relationship between a ruleset and it's child?
I'm guessing that the parent ruleset has the ability to do anything its children can do:
- the parent roles (view, action, migrator) has all abilities of the corresponding child role (and recursive)
- the parent roles can *grant* things that *it has* to those child roles, so the child can access or use things from above


at what point does this become simpler to just build a simple database language????
all this stuff is so entangled. working around the limitations of postgres is driving me insane. and no other database is going to be better, no database is built to be this meta
the biggest conceptual idea that's complex is that the ruleset *data structure* owns a *schema*, basically owns a *dynamic* sub-tree of data structures which are its children
this is why building a language is what you wanted to do!
so what now? figuring out actually fully useful parent/child (or even sibling??? way harder)
does this get way simpler if all rulesets don't exist in parent/child relationships, but instead they're all independent and can *message* each other? but really that means they can just call actions on one another
there are a couple things that make that idea hard:
- robustness, how do we know what actions exist? we need a safe way to ensure that a "sender" in one ruleset will always have a "receiver" of the right message type
- so this means the ruleset state is always a typed web. it doesn't have to be hierarchical, but any modifications of the web have to leave it in a consistent state. this means all relationships have to be clearly registered and we need an algorithm to ensure all links are correct

this begs the question, if it isn't hierarchical, how do we know how to change the web?
this is why a hierarchy is helpful. the higher rulesets determine the rules for modifying all the parts of the web they "own". it might specify *no* way for the web to be changed!

man I just want a real language

this implies that one of the characteristics that must be declared about a ruleset are the "senders" it has that must have their destination declared. it's also imaginable for a ruleset to have *optional* senders (maybe use the word "capability"? what's a good term for this? "foreign action"?)
further
this idea of a typed link between rulesets seems rich and important. a receiving end is simple, a ruleset declares it and it has a type. it's totally imaginable that a ruleset

Further, what about situations where a ruleset wants to have some *static* parent/child structure? Rather than allow dynamic creation?
This is what I




Scheduled events table could have a log table tracking all occurrences
Use a compare and swap (update that's conditional on the row not having a value that indicates it's been grabbed by the server) to mark the row as handled

Post route to advance time, only turned on in debug mode, used for testing

Figure out some little utility script to package up a ruleset as a json or yaml or something document

Zod schema and type for ruleset to allow ruleset functions to talk about them and accept them as input




https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#examples

the simplest way to allow bootstrapping a server is for the initial schema creation to *itself* bootstrap a root ruleset with a single action that merely accepts a proposal replacement ruleset, and if that proposal checks as well-founded it is immediately instituted
this way no special server permissions or sql templating are needed, everything happens in the logic of the server
or perhaps it makes more sense to do this at server startup? it checks to see if there is a root ruleset, and if not it inserts

# phases
- standalone rulesets that can only replace themselves
  - X view route
  - X action route (without migration)
  - X ruleset replacement proposal (in runtime)
    - X ruleset migration checking
  - X ruleset replacement enaction
    - X ruleset migrations
    - X action role
    - X view role
  - events
    - recurring declared in ruleset itself
    - one off
  - safe fns with schemars and zod jsonschema
  - sql runtime functions https://docs.rs/sqlx/latest/sqlx/trait.Executor.html
    - fetch many (queryRows)
    - fetch optional (queryMaybeOne)
    - fetch one (queryOne)
    - execute
    - raw sql https://docs.rs/sqlx/latest/sqlx/fn.raw_sql.html
    - query maybe scalar https://docs.rs/sqlx/latest/sqlx/fn.query_scalar.html
    - query scalar https://docs.rs/sqlx/latest/sqlx/fn.query_scalar.html
    - query rows https://docs.rs/sqlx/latest/sqlx/fn.query_with.html
- client site
  - nuxt static generated?
  - login/auth nonsense
  - X async list views
  - X async call and display view html (views are responsible for rendering controls to call actions)
- could release?

- parent and child rulesets, but without any communication or data sharing
  - new child ruleset proposal and enaction, child ruleset replacement proposal and enaction
- could release?

- full featured `fetch`?
  - more or less just copy how deno does it? or just implement the spec using something simple like reqwest?
- parent and child rulesets, with function calling for all communication (simpler?)
  - client site display rulesets one is a member of
- parent and child rulesets, with function calling and permissions data sharing (this might actually be simpler, since it can just be a templated sql string to run over the child ruleset's roles. especially given that a parent ruleset can specify safe functions and then just allow the child to call them, and have read access to things like weight tables, this is actually probably much simpler)


how much can even be achieved with parent and child systems without communication? full persistent



// https://github.com/denoland/deno_core/issues/515
// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1201661871959310346
// https://gist.github.com/alshdavid/c9e5bc0d794e3ec9dba6afaa689b704e#file-main-rs-L51

// https://discord.com/channels/684898665143206084/1022163295895027722/threads/1074150763460313128




- postgres table to schedule or cancel one-off events
  - runtime function for actions to schedule them
- postgres table to hold static recurring events
  - runtime function fo server to call at startup for existing and when rulset instituted
- runtime function to propose self replacement ruleset (actions returning a string means replace that ruleset)
- runtime function to propose new child ruleset
- runtime function to institute new child ruleset

- runtime function to send well-typed message (using jsonschema) to parent or child ruleset?
  - when parent institutes child ruleset, all messages that can be passed up and down and their corresponding actions are declared?


Rust library that builds validators from json scheme rules
https://docs.rs/jsonschema/latest/jsonschema/
https://github.com/StefanTerdell/zod-to-json-schema
https://github.com/anatine/zod-plugins/tree/main/packages/zod-openapi

Scheduled events, both in creation of ruleset definition and using a runtime function
A scheduled event is a table in the catalog that has a start date and an interval (and maybe an occurrence count) and an action name to run
Maybe also a blob of json to act as a memory? Or the creation function returns an id people can use to refer to the event and therefore remember things about it?
To figure out the next occurrence, we measure the distance since the start date and ceiling divide it by the interval, and multiply that round number by the interval and add it to the start date
The server manages the timers and futures etc to know when to run these. At these times it reads the table and sets up timers to run the next occurrence: at startup, when a new event or ruleset is created
These events need to be smart about checking whether they should even happen. They could be disabled. When we replace a ruleset, we might replace it's recurring events, but only maybe! We can read the segment of the table for the ruleset in question and also iterate over the current timer tasks that fall under that ruleset, and replace them? Or it's probably simpler to just abort all of them beforehand and create new ones after. Since they'll basically certainly be at await points they'll all cancel and then if the exact same thing is created in its place afterward that's fine
For now I'll just run in the actix runtime, and maybe at some point when it seems to matter I'll do something more complicated. Maybe a global async messaging thing would be useful, it receives messages
https://github.com/tokio-rs/tokio/issues/3710
https://docs.rs/tokio-test/0.4.4/tokio_test/





# all the things I could do next:
- figure out how the current rulesets are catalogued
- figure out permissioning and roles (both for the server, and for each ruleset and its different definitions)
- get the action route working (look in the ruleset catalog, invoke the ruleset with its ability to execute queries revoked to gather the function, then invoke that function with its ability to execute queries reinstated with a full writeable role)
- get the view route working (look in the ruleset catalog, invoke the ruleset with its ability to execute queries revoked to gather the function, then invoke that function with its ability to execute queries reinstated with a read-only role)
- get the route working that simply shows views (and actions?) that are available to the user
- figure out the mechanism whereby new constitutions are proposed (from a separate top-level route? probably not, but through a function in a ruleset, which means it's possible to write an unchangable ruleset)
- figure out how the function that proposes new constitutions works, and what it's purpose is (I feel like the purpose is basically to run *whatever* checks for basic well-formedness, such as migration checking, possibly type checking!). it seems a ruleset would have a table that *points to* the global constitution table, and when it wants to switch constitutions it *must*

- figure out how rulesets have children rulesets!

# tests
- sql and http aren't allowed when gathering functions
-


# runtime functions I need to add:
- function to propose new constitution (demands parameter of the constitution, then checks it for well-formedness, then returns an id that can be used to refer to it)
- function to switch a ruleset out for a new constitution (demands parameter of id originally given when it was proposed, returns success or failure)
- get http fetching working, hopefully just import and use implementation from deno
- get sql execution working, mirror the api of sqlx faithfully?


# ruleset examples I could make:
- simple, three yeses with no nos to propose a new constitution and instate it immediately
  - an action that proposes a new constitution (this calls the function provided by the runtime to submit this to the main list)
  - another action that accepts yeses or nos, and either instates a new constitution if it succeeds (and wipes out all other proposals), or removes/archives a proposal if it receives a no, or does nothing if nothing crossed any thresholds
  - a view that looks at all currently proposed constitutions and their vote counts, and has hooks to propose one and yes or no existing ones
  - a view that looks at the current constitution
- need an example with scheduled events, such as a constitutional approval election every whatever period (for testing purposes every 5 minutes or something)


# later stuff
- figure out user management and auth stuff in general
- figure out "membership", partitioning members into rulesets, and the inherently nested nature of that

ruleset passwords are only internal, so they might as well be rotated constantly












What if the role names were all cryptographically secure?
Reset role makes the more annoying, but here me out
We have four main roles and pools:
One for server operations which is special
Three for views, actions, and migrations, each of each is a "shell" role that has no powers to do anything but switch to those roles underneath. Then security is gained by making the role names underneath cryptographically secure, meaning




https://secutils.dev/docs/blog/rust-application-with-js-extensions
https://deno.com/blog/roll-your-own-javascript-runtime



The purpose of this tool is to completely solve the purely technological sub-problems (incidental complexity) of governance, and leave only policy questions (which are sometimes technical!). Those sub-questions are:

- data storage
- server routes/liveness
- event architecture
- role checking

every `governance-instance` is akin to a database. a server running `votebase` can hold multiple `governance-instance`s

every `governance-instance` is essentially a root `policy`, the core primitive. a `policy` inherently allows for itself to have it's internal contents changed by any `process`. a `policy` can be:

- a leaf, meaning it's an `official` (a person who essentially delegated powers to make particular decisions by the policy) (the ability to have *groups* of `official`s is an important one, and these groups need to have the ability to be dynamically sized according to some other calculation or process); or a `document`, meaning it's just a rich text whatever that humans will have to interpret and act according to.
- a branch, meaning it's another `policy`

every policy must specify how it can be changed. should it even be possible to state that a policy *can't* be changed? certainly if that is possible, specifying that is true must be explicit and required

the system must allow for calling arbitrary computations to perform possibly technical work. every external and therefore opaque computation must be declared in the terms of the `votebase` type system, and must have a description of it's purpose (possibly with a minimal length?)

role declarations are also essential

```
// root is special
policy root
  selected by SomeProcess
  // or
  selected by
    // inner things
    // the important thing is that there isn't only *one* way to select/change things
    // the selection criteria are merely a set of data collections, inputs, events, and roles
    // there can be multiple inputs that produce events dynamically that supersede normally declared ones

  // allows for either named references to policies defined elsewhere, or named declarations in place
  official governor
    // is it possible for all official power to be designated in the form of roles?
    role SomeRole
    metadata
      // this is a function that returns the powers and other metadata, such as salary, of this official
      // this should also have a separate call schedule
    selected by
      // in selected by we can declare data tables and inputs
      data
      input
  document mandate
    contents
      Four-score and seven years ago...
    selected by
      ...

  // this policy can be changed individually through it's own selection mechanism, without using the selection mechanism of root
  policy OversightReferendum
  // this policy can't be changed individually, only through root
  ruleset SomethingProcess
```

it's clear there's a difference between a policy's *selection* ruleset and the mere embedded rulesets underneath it

since this needs to be a highly readable language, policy declarations vs "invocations" (statements that actually say the policy is in force rather than merely existing or being imported) need to be possible, perhaps with `using` keyword (`official using governor` vs `official governor`)
`define policy` vs `policy` could be the syntax for this distinction

blaine, focus on semantics, not syntax


the question of policies is quite simple and intuitive, and seems to fit any and all needs of any polity
it seems both `selected by` and `described by` n

it seems we need another concept, that of a `ruleset`, basically a reusable bundle of collections, inputs, events, and roles
this is important *because* of reusability, and not really for any other reason?
the interesting part is how the interaction between a containing policy and a ruleset should work

this reusability has to be composable, so multiple rulesets can be combined in a single policy
it also has to allow for selective overriding (`override input something`)
inputs can be called as normal functions from any other input

the semantics of all these things are clear, they're just data as we underestand them in databases, events as we understand them in things like cron, inputs as we understand them from servers, and roles as we understand them in servers and databases
events are mere primitives, probably better understand as just a sugared type, so I guess the *real* primitive here is "triggers", or `on` events

the interesting thing are the commands to *change* policies
let's say there's a policy that specifies its selection method as some reusable
the only time we're *changing* something that isn't just a data collection is when we're changing a policy

rulesets can take in parameters in the event they need to interact with other unknown rulesets in some way


can all governance be fully implemented by data collections, inputs, events, and roles?
those seem like the minimal primitives
those afterall are the minimal primitives of a database/api combination!

we've said external computations should be allowed

events are interesting, because they ultimately have to be editable and arbitrary
but we want the ability to draw from generators of events

how do we get both?
perhaps this is one of those things we punt?



TODO Elinor Ostrom quote about iteration and local conditions and flexibility

there are two separate things groups need to do to successfully coordinate
- enough members of the group need to realize it's a good idea to coordinate, and have sufficient motivation to do so
- then they need to have a way to communicate, share coordination options, and make binding decisions about those options

- will to coordinate
- ability to communicate
- ability to decide

coordination isn't merely communication, it requires the ability to create precise rules, making binding decisions to commit to those rules, and build other rules to enforce the rules


Examples always win

# Persistent Election

```
ruleset PersistentWeights
  data WeightAllowance
    voter: &VoterType
    weight: numeric & > 0
  data Allocation
    voter: &VoterType
    weight: numeric
  constraints
    sum(abs(Allocation.weight)) <= sum(WeightAllowance.weight)
    group by voter

ruleset PersistentApprovalElection(VoterType, AllocationType, CandidateType)
  // every voter can only have a single allocation in this election at a time, since it's a weighted approval ballot
  // here the consistency of weight sums is left up to higher levels, this ruleset doesn't worry about it
  data Ballot
    voter: &VoterType
    weight_allocation: &AllocationType
    approvals: &CandidateType#{}
    disapprovals: &CandidateType#{}
    // #{} indicates a set, so the candidates have to be unique and non-duplicated
  constraints
    approvals :union disapprovals == #{}

  data Bucket
    candidate: &CandidateType
    total_vote
    accrued
    // TODO look at this next

  event Update = every week on Monday midnight

  // should think about nomination process
  input enter_candidate()
  input withdraw_candidate()

  input update_ballot(weight_allocation: &AllocationType, approvals: &CandidateType[])
    voter = ~caller
    Ballot |find .voter == voter |set { weight_allocation = weight_allocation, approvals = approvals }

  on Update

```

# Persistent Commitment

# Normal Regularly occuring event-based election

```
ruleset EventApprovalElection(VoterType, CandidateType)
  data Candidate; CandidateType
  data Approval
    voter: &VoterType
    candidate: &CandidateType
  constraints; unique (voter, candidate)

  event ElectionOpen = ElectionEnds - 3 months
  event ElectionStarts = ElectionEnds - 1 month
  event ElectionEnds every four years starting 2020, second Thursday of November

  @allowed(from ElectionOpen, upto ElectionStarts)
  input enter_candidate(candidate: CandidateType)
    Candidate |insert { candidate }

  @allowed(owns candidate)
  input withdraw_candidate(candidate: &CandidateType)
    Candidate |find .candidate == candidate |delete

  @allowed(from ElectionStarts, upto ElectionEnds)
  input vote_for_candidate(candidate: &CandidateType)
    voter = ~caller
    Approval |insert { voter, candidate }

  on ElectionEnds
    winner =
      from Approval
      aggregate [group by candidate] votes = sum
      select-by max(votes) candidate

    // handle possible ties

    ~emit-change winner
```

From this code we can infer that CandidateType has to be a policy
and that VoterType has to be a caller? or at least that the caller has to *own* a VoterType

```
ruleset Something
  input yo()
    // a guard-role construct is basically a "priority ordered inclusionary match"
    // the caller
    vote_strength = ~guard-role
      Governor; 5
      Voter; 1
      _; ~reject
```


---

future things to think about

- all data saved needs to be saved in an "event-based" way, meaning the entire history of the entire polity could be reconstructed in precisely auditable detail
- the above needs to be compatible with privacy and secret ballots! zero-knowledge proofs and cryptography etc
