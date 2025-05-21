# `accept-any` ruleset

This is the simplest ruleset I can think of, and is really only useful to efficiently ["bootstrap"](https://en.wikipedia.org/wiki/Bootstrapping#Applications) a votebase server.

This ruleset has one action, `__insert_initial`, that simply accepts the first candidate ruleset and immediately replaces itself with that ruleset.
