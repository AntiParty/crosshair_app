# Finals Crosshair

Design, preview, save and share crosshairs for **THE FINALS**, and apply them to the game in one click.

> **Unofficial.** This project is not made by, affiliated with, or endorsed by Embark Studios. THE FINALS and
> its related marks belong to Embark Studios AB. This repository contains no game assets.

## What it does

- **Live preview** of your crosshair and scope sight, drawn from the game's own setting values, on several
  backdrops (including a deliberately busy one to test visibility), with a "shoot" animation for the moving reticle.
- **Presets**: save named crosshairs and copy or paste a text code to share one. Pasted codes are validated against
  the game's own ranges before anything is accepted.
- **Apply to game**: writes only the crosshair and sight settings into the game's options file. It is disabled
  while the game is running.

It is a small native Windows app (Rust, [egui](https://github.com/emilk/egui)), about 6 MB, and uses almost no CPU
when idle.

## Read this before using "Apply to game"

Apply edits a file the game owns: `%LOCALAPPDATA%\Discovery\Saved\SaveGames\EmbarkOptionSaveGame.sav`.
It does so only while the game is **closed**, which is like editing a config file offline. It does not touch
the game process or its memory. Even so, **use it at your own risk**: the game's anti-cheat is not something
this project can speak for, and a game update can change the file. If you want zero risk, use the app as a
designer only and type the values into the in-game menu yourself.

Safety checks made before any write:

1. THE FINALS must be closed (checked again immediately before writing) and the file must not be open elsewhere.
2. The file must be the options save, at the version this app was verified against. If a game update changes it,
   the app refuses to write.
3. An untouched re-save of the file must reproduce it byte for byte, or nothing is written.
4. A backup is made first (`%APPDATA%\FinalsCrosshair\backups`, plus a permanent `original.sav`).
5. The edited file is re-read and checked: only the intended settings may differ.
6. Any backup can be restored from the **Restore backup** menu.

Only crosshair and sight settings are ever written. Sensitivity, graphics and everything else are left as they are.

## Install

Download `FinalsCrosshair.exe` from the Releases page, then check it against `SHA256SUMS.txt`:

```powershell
Get-FileHash .\FinalsCrosshair.exe -Algorithm SHA256
```

The executable is not code-signed yet, so Windows SmartScreen may warn the first time. Building it yourself is
straightforward (below) if you prefer.

## Build and test

Needs Rust 1.92 or newer on Windows.

```powershell
cargo test --workspace --release    # parser, validation, guards, apply-on-a-copy
cargo build --release               # -> target\release\finals_crosshair.exe
```

Layout:

| Path | What |
|---|---|
| `core/` | The crosshair model and validation. Shared with anything that must apply the same rules. |
| `src/` | The desktop app: file editor (`gvas.rs`), safety checks (`guard.rs`), backups and apply (`store.rs`), preview and UI. |
| `tools/gen_schema.py` | Regenerates the setting ranges and defaults from the game's option definitions after a game update. |
| `tools/make_fixture.py` | Builds the synthetic test fixture from a real options file, removing every personal value. |
| `tests/fixtures/opt.sav` | Synthetic options file used by the tests. Contains no real preferences. |

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).

## Licence

[MIT](LICENSE).
