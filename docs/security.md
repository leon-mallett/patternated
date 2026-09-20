# Security

A short document, because the attack surface is small and should stay that way.

## What the engine does not do

**No network I/O at all.** Not deferred, not configurable — there is no HTTP
client in the dependency tree. Motifs and patterns live on disk and nothing is
uploaded. This is both the right engineering choice and a selling point later,
since designers are reluctant to upload unreleased artwork to a web tool.

**No accounts, no credentials, no secrets.** Nothing to leak, rotate, or store.

**No cloud sync, no multi-user, no hosted API.** One designer, one machine.

Generation at M7 is the first thing that needs a network call, which is why it
sits behind a trait outside `pattern-core` — so it can be stubbed, swapped, and
reviewed as its own thing rather than arriving as an implicit dependency of the
compositor.

## What it does do

Reads JSON documents and PNG files given to it on the command line, and writes
PNGs. Untrusted input is therefore a malformed or malicious image file, handled
by `tiny-skia` and `png`, both pure Rust with no C decoder behind them.

Pattern documents are validated before rendering — tile size, lattice
degeneracy, placement scale, palette slot range — so a malformed document fails
with a named error rather than producing a pattern nobody asked for.

## Supply chain

The baseline in `~/Projects/SECURITY.md` applies; this project has no
`SECURITY.md` of its own and needs none yet.

The operative rules: name the exact package and version before adding one, treat
anything published within 72 hours as requiring explicit confirmation, install
with the lockfile-exact command, and note any lifecycle scripts a package
declares.

`cargo audit` reports no advisories against the tree as of 2026-09-20.

**One thing learned doing this.** The only dependency M3 needed was approved as
`vtracer` 0.6.5 on accurate facts — stable, permissive, pure Rust — and vetting
it afterwards showed it bundles an ungated CLI, bringing 36 extra crates and
four RUSTSEC advisories. `visioncortex`, the library it wraps, does the same
work with a clean audit. Reading a package's description is not vetting it;
reading its dependency tree is.

The surface grows at M4, when the Tauri shell brings npm into a project that
currently has none. That is the point at which this document needs more than a
paragraph.
