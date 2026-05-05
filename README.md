# `votebase`

This project is *extremely* work-in-progress. If you're curious about its purpose, you can read [this blog post](https://blainehansen.me/post/votebase/).

---

## Current Design

All of votebase itself, the server and runtime and various CLIs, are written in Rust. [`deno_core`](https://docs.rs/deno_core/0.399.0/deno_core/) is used to interact with and run the Ruleset typescript, and the runtime gives access to a postgres database.

The [`runtime.ts` file](https://github.com/blainehansen/votebase/blob/main/common/src/runtime/runtime.ts) is the intended surface of the actual runtime.

### Roadmap

- [X] single root ruleset with only ability to replace self
- [ ] migration validation
- [ ] membership functions
- [ ] sql functions
- [ ] basic reset role protection
- [ ] json schema validation of inputs (and outputs?)
- [ ] static children with recursive application and validation
- [ ] static events
- [ ] dynamic events
- [ ] fetch function
- [ ] non-DAG migration validation
- [ ] dynamic children with recursive application and validation
- [ ] strong ruleset references with validation and prevention
- [ ] weak ruleset references with validation and prevention

- [ ] bundling cli bundle command and therefore variables
- [ ] bundling cli check command
- [ ] bundling cli generate migration command
- [ ] bundling cli init command

- [ ] actually usable frontend concepts
- [ ] npm library resolution
- [ ] jsr library resolution
- [ ] dynamic children keep rules
- [ ] dynamic recurring event keep rules
- [ ] dynamic standalone event keep rules

- [ ] static migration/schema validation and checking using [`postgres-static-analyzer`](https://github.com/blainehansen/postgres-static-analyzer)
