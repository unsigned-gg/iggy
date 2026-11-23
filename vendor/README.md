# vendor

Estate-maintained replacement crates. They are not workspace members. The
`[patch.crates-io]` table in the root `Cargo.toml` routes every use of the
named crate, direct and transitive, to the copy here.

| Crate | Replaces | Why | Origin |
|---|---|---|---|
| `chrono` | `chrono` 0.4.45 | Upstream chrono 0.4 is unmaintained (RUSTSEC-2026-0543). | chrono v0.4.45 source with all `unsafe` and the OS time zone code removed; local time zone lookup goes to `jiff` 0.2. See `chrono/PROVENANCE.toml`. |
| `humantime` | `humantime` 2.4.0 | Upstream humantime 2 is unmaintained (RUSTSEC-2026-0542). | Written for this fork against the public API of humantime 2.4.0. RFC 3339 calendar arithmetic goes to `jiff` 0.2. The duration grammar is humantime's own and is implemented over `std::time::Duration`. |

Both crates keep the upstream licence, `MIT OR Apache-2.0`, and carry the
upstream licence text.

## Checks

- `cargo test --manifest-path vendor/humantime/Cargo.toml` runs the fixed case
  tables.
- The `humantime` replacement was compared with humantime 2.4.0 from crates.io
  on 9,000,000 generated inputs (duration parse and format, RFC 3339 parse and
  format) with no difference in result, error value or error text.
- Known difference: formatting a time before the Unix epoch returns
  `fmt::Error`. humantime 2.4.0 panics.

## Updating

Do not edit these crates to change behavior that upstream Iggy relies on. When
upstream Iggy stops depending on `chrono` or `humantime`, delete the crate here
and its `[patch.crates-io]` line.
