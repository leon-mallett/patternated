# Security

## Reporting a vulnerability

Email **hello@captivated.online**. Please do not open a public issue for
anything that looks exploitable: an issue is visible to everyone the moment it
is filed, including before anyone has read it.

Say what you did, what happened, and what you expected. A file that triggers it
is worth more than a description of it, and this project would rather have a
crashing SVG than a careful write-up of one.

Expect an acknowledgement within a week. This is a small project and there is
no on-call rota, so that is a realistic figure rather than a flattering one.

## What the attack surface actually is

Worth stating plainly, because it is narrower than most repositories of this
size and it changes what is worth your time.

**There is no network I/O anywhere in this engine.** Not deferred, not behind a
flag: there is no HTTP client in the dependency tree. Nothing here listens on a
socket, serves a request, authenticates anybody or stores a credential. There
is no server to attack and no account to take over.

What is left is a program that reads files a person chose to open:

- **Images and SVG, through `motif-ingest`.** This is the real surface. Parsing
  attacker-supplied artwork is where a memory-safety or resource-exhaustion
  problem would live. The parsing is done by `png`, `usvg` and `resvg` rather
  than by hand, so most of what could go wrong there is theirs rather than
  ours, but a crafted file that gets through them to us is still worth
  hearing about.
- **Pattern documents, as JSON.** Validated before rendering, for tile size,
  lattice degeneracy, placement scale and palette slot range, so a malformed
  document is refused by name rather than rendered into something nobody asked
  for.
- **The desktop app's own commands**, which are reachable only from its own
  window.

A crash, a hang or unbounded memory use on a crafted input is in scope and is
worth reporting. So is anything that makes the engine write outside the paths
it was given.

## Supported versions

The tip of `main`. There are no release branches yet and nothing older is
patched. When tagged releases begin, this section will say which of them are
maintained.

## Dependencies

`cargo audit` runs in CI on every push, and weekly on a schedule, because an
advisory published against a dependency nobody has touched is exactly the case
a push-triggered run never sees.

Vulnerabilities fail the job. Unmaintained and unsound advisories are printed
without failing it: measured on 2026-09-29 the tree carried seven of those and
every one sat inside Tauri's own transitive dependencies, which this project
cannot fix. A job that is permanently red is one nobody reads on the morning it
turns red for a real reason.

---

*This file is about reporting a vulnerability in this repository. It is not a
statement of Captivated Ltd's internal engineering practice and does not
replace it.*
