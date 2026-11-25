# Security policy

## Reporting a vulnerability

Please **do not open a public issue** for a security problem. Use GitHub's private reporting instead:
**Security tab -> Report a vulnerability** on this repository. Include what you found, how to reproduce it, and
the version of the app.

You can expect an acknowledgement within a few days. This is a volunteer project, so there is no bounty.

## What counts

- The app writing outside the crosshair and sight settings, or corrupting the game's options file.
- A crafted crosshair code that gets past validation (out-of-range values, unknown keys) or crashes the app.
- The app writing while the game is running, or skipping a backup.
- Anything that lets a downloaded release differ from what the repository builds.

## Out of scope

- Whether the game's anti-cheat accepts a modified settings file. That is Embark's system; see the README.
- Problems that need an attacker who already controls your PC.

## Verifying a release

Each release lists a SHA-256 for `FinalsCrosshair.exe` in `SHA256SUMS.txt`. Compare it with
`Get-FileHash .\FinalsCrosshair.exe -Algorithm SHA256` before running.
