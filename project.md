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
