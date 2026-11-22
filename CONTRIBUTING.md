# Contributing

Welcome to Cardwire ! Thanks for taking the time to contribute to this project.

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
