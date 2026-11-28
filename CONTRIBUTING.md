# Contributing

Welcome to Cardwire ! Thanks for taking the time to contribute to this project.

## LLM Policy

All types of LLM (and AI, or AGENTS) falls into this category, this includes coding agent, AI assistant like OpenClaw or Hermes, and others.

### On Github

LLM usage is not allowed for writing Issues, PRs descriptions, comments, discussions and etc...

They must all be human-written, i wish to talk to an human and not a meat proxy.

LLM may be used to help translating to English for non-native speakers.

For Issues, we got a zone made for LLM output if you still wish to send it, please respect that.

### On the codebase

Submitters should understand the project's purpose, goals, codebase and the code they submitted.

For this reason, LLM assistant is allowed for small tasks like:

- Searching on the web or a doc
- Assisting in writing redundant code
- Debugging an issue
- Reviewing your code
- Asking questions to improve your understanding of the codebase or a programming concept

The list can be non-exhaustive, keep in mind that the only usage that is banned is:

Fully/Mostly LLM written PRs

If you don't know how to program in Rust, please just open an issue or contact a maintainer.

You won't have to waste tokens on code you do not understand, and us maintainers won't waste time on un-maintainable code

## Issue

Please follow the [template](.github/ISSUE_TEMPLATE/bug_report.yml)

Any LLM output goes into the LLM output field, this is easier for me, as LLM can spit wrong informations.

## Pull Requests

### Development Setup

[link to cardwire's doc](./docs/development/build-dev.md)

### Before beginning

A quick rule to save everyone time :

- Bug fix for a reported issue? Go ahead and open a PR.
- New feature ? whether it was requested in Github Issues or not, please discuss it with me
  first before opening a PR.

I am available on :

- Github
- Discord (OGC discord, preferred)
- Matrix
- Mail

### Commit standards

Follow Conventional Commits format for commit titles (eg: fix(cardwired): remove unwrap in gpu_unblock())

### CI

CIs must pass before merging, this includes:

- Rust Lint (Clippy)
- Rust Test
- Rust Format
- Nix VM 2 GPUs (Laptop conf)
- Nix VM 3 GPUs
- Nix VM 15 GPUs

If none of these CIs passes, the PR won't be merged

### Submitting the PR

Please follow the PR template, it was made for a reason.
Any PR that does not follow the template may be rejected

## Code of conduct

Be kind to others
