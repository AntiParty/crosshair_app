# Contributing

Bug reports and pull requests are welcome.

## Before you open a pull request

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
```

All three must pass; CI runs the same commands.

## Rules that keep users safe

These are the project's guarantees, so changes that weaken them will not be merged:

- Writing to the game's options file must stay behind every check described in the README, and must keep making a
  backup and verifying the result.
- Only crosshair and sight settings may be written. New keys must come from the generated schema, never from user input.
- Imported crosshair codes must be validated against the game's ranges, and rejected as a whole on the first problem.
- No network access in the desktop app, and no new dependencies without a clear reason.

## Data and privacy

- Never commit a real options file, game assets, game config dumps, keys, tokens or personal paths.
  `tests/fixtures/opt.sav` is synthetic; regenerate it with `tools/make_fixture.py` if the file layout changes.
- Game data used here is limited to setting names, ranges and defaults.

## After a game update

1. Dump `DA_OptionRepository` to JSON and run `python tools/gen_schema.py <file>` to refresh the schema.
2. Check whether the options file's `Version` changed. If it did, verify the new layout on a copy before raising
   `KNOWN_SAVE_VERSION` in `src/store.rs`.
