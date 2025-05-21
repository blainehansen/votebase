# `three-yes-proposals` ruleset

This is a ruleset for simple asynchronous proposal approvals, inspired by the metagov community. Its basic logic goes like this:

> When the proposal achieves 3 upvotes with at most one downvote after at least 3 hours, or goes 3 days with at least one upvote and no downvotes, it passes.

Some important design choices:

- The proposer is allowed to approve their own proposal.
- Upvotes and downvotes aren't allowed or counted after 3 days.
