# Estate fork of apache/iggy

This repository is the unsigned-gg fork of [apache/iggy](https://github.com/apache/iggy). It exists to try estate engineering practice on Iggy: supply-chain pinning, container and chart hardening, release attestations and workload identity. Changes that are useful to everyone can be offered upstream, one at a time, with the rule below.

## Branches

- **`master`** mirrors `apache/iggy` `master` exactly. Nobody commits to it. Sync it by fast-forward only: `gh repo sync unsigned-gg/iggy -b master`.
- **`estate/main`** carries the estate changes, as a stack of small signed commits on top of `master`. After each sync of `master`, rebase `estate/main` onto it.

## Upstream boundary

- Never push to, open a pull request on, or comment on `apache/iggy` without the operator's go for that specific pull request.
- Work aimed upstream follows upstream's `AGENTS.md`, including its list of actions that need a person's approval first, and its `CONTRIBUTING.md`.
- Security reports about Iggy go only to security@apache.org, as upstream's `SECURITY.md` says. Never to a public issue, here or upstream.

## CI

GitHub Actions is disabled on this fork. Do not enable it until an estate CI for the fork is designed and the operator has approved it.

## License

Iggy is Apache-2.0, and its `LICENSE` and `NOTICE` apply unchanged. Estate changes in this fork are contributed under the same license.
