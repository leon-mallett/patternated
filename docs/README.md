# Pattern Engine — documentation

Last confirmed against: 2381b14 (2026-09-20)

Deterministic repeat-pattern engine for textile printing. Motifs in,
print-ready seamless patterns out.

## The set

- [About](./about.md) — what this is and who it is for, in plain English.
- [Architecture](./architecture.md) — the crates, what each carries, and the
  boundaries that are load-bearing.
- [Features](./features.md) — what exists today.
- [Roadmap](./roadmap.md) — what does not exist yet, and in what order.
- [Decisions](./decisions.md) — the non-obvious calls, and what would reopen them.
- [Test strategy](./test-strategy.md) — what is tested, how, and what is
  deliberately not.
- [Licensing](./licensing.md) — ownership, and the one question that needs
  answering before Dreamwoven.
- [Security](./security.md) — a short section, because the engine has no network
  and no accounts.
- [Commercialisation](./commercialisation.md) — two consumer brands over one
  engine.

## Reference

- [Technical spec](./pattern-engine-spec.md) — the full design, M1 to M7. The
  source of the invariants everything else here defers to. Dated 20 September
  2026.

There is no runbook. Nothing is deployed and nothing runs unattended: the engine
is a local CLI and a library. One becomes necessary at M7, where Dreamwoven
generates a pattern per order with no human approving it, and the quality gates
are the thing that would need operating.
