# Pattern Engine — documentation

Last confirmed against: 54bf6fe (2026-10-01)

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
- [From brief to pattern](./from-brief-to-pattern.md) — how a child's
  interests become a printable repeat, and the one problem that stops an
  order shipping.

## Checking it

```bash
python3 scripts/check-docs.py
```

Every relative link and anchor must resolve; it exits non-zero when one does
not. The set is held together by its links, so a broken one is a real defect
rather than a typo — and it is the thing most likely to break silently when a
heading is renamed.

## Reference

- [Technical spec](./pattern-engine-spec.md) — the full design, M1 to M7. The
  source of the invariants everything else here defers to. Dated 20 September
  2026.

There is no runbook. Nothing is deployed and nothing runs unattended: the engine
is a local CLI and a library. One becomes necessary at M7, where Dreamwoven
generates a pattern per order with no human approving it, and the quality gates
are the thing that would need operating.
