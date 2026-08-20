# Contributing
Welcome to Cardwire ! Thanks for taking the time to contribute to this project.

## LLM Policy
LLM are always a source of debate, so, here's my take:

- LLM must be used as tools, not as an employe executing your orders.
- Fully vibe-coded/slop PR are not allowed. The Contributor must be able to understand and to explain his changes.
- If the PR is fully LLM generated and i didn't notice it, Well Played !
- If a AI/LLM was used, please mention it with either the LLM co-signing the commit, or with a <assisted-by: MODEL> message in the PR

## Pull Requests

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
Any PR that does not follow the template will be rejected

## Code of conduct
Be kind to others
