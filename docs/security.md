# Security

A short document, because the attack surface is small and should stay that way.

## What the engine does not do

**No network I/O at all.** Not deferred, not configurable: there is no HTTP
client in the dependency tree. Motifs and patterns live on disk and nothing is
uploaded. This is both the right engineering choice and a selling point later,
since designers are reluctant to upload unreleased artwork to a web tool.

**No accounts, no credentials, no secrets.** Nothing to leak, rotate, or store.

**No cloud sync, no multi-user, no hosted API.** One designer, one machine.

Generation at M7 is the first thing that needs a network call, which is why it
sits behind a trait outside `pattern-core`, so it can be stubbed, swapped, and
reviewed as its own thing rather than arriving as an implicit dependency of the
compositor.

## What it does do

Reads JSON documents and PNG files given to it on the command line, and writes
PNGs. Untrusted input is therefore a malformed or malicious image file, handled
by `tiny-skia` and `png`, both pure Rust with no C decoder behind them.

Pattern documents are validated before rendering, covering tile size, lattice
degeneracy, placement scale and palette slot range, so a malformed document fails
with a named error rather than producing a pattern nobody asked for.

## Supply chain

A supply-chain baseline applies to this project from outside the repository.
The repository's own [`SECURITY.md`](../SECURITY.md) is deliberately narrower:
it tells an outside reporter where to send a vulnerability and says what the
attack surface actually is. It does not restate the practice below and does not
replace it.

The operative rules: name the exact package and version before adding one, treat
anything published within 72 hours as requiring explicit confirmation, install
with the lockfile-exact command, and note any lifecycle scripts a package
declares.

### What `cargo audit` says, and why

As of 2026-09-21 it reports **seven warnings against 487 crates**, all of them
informational and all transitive through Tauri. Recorded here rather than left
as a number somebody has to re-derive:

| Crate | Advisory | Reached via |
| --- | --- | --- |
| `proc-macro-error` | unmaintained | build-time macros |
| `unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident`, `unic-ucd-version` | unmaintained | `urlpattern`, Tauri's URL matching |
| `glib` | unsound iterator impls | Tauri's Linux backend |

Six are **unmaintained** notices rather than vulnerabilities: nobody is
publishing fixes, which is worth knowing and is not itself a defect.

The `glib` one is the only unsoundness, and **it is not compiled here**.
`cargo audit` reads `Cargo.lock`, which lists dependencies for every platform;
`glib` appears 28 times in a Linux build of this workspace and zero times in a
macOS one. It would matter if this shipped on Linux.

None is actionable without dropping Tauri, which the spec mandates. The reason
to write them down anyway is that "audit clean" was true before the app shell
existed and is not true now, and a number that silently changes meaning is worse
than one nobody checks.

**One thing learned doing this.** The only dependency M3 needed was approved as
`vtracer` 0.6.5 on accurate facts, being stable, permissive and pure Rust, and vetting
it afterwards showed it bundles an ungated CLI, bringing 36 extra crates and
four RUSTSEC advisories. `visioncortex`, the library it wraps, does the same
work with a clean audit. Reading a package's description is not vetting it;
reading its dependency tree is.

The surface grows at M4, when the Tauri shell brings npm into a project that
currently has none. That is the point at which this document needs more than a
paragraph.
