# M03 — owner-authenticated initial Dream replay for completed imports

**Status: focused correction verified, GM remains under independent review.** This note extends `m03-import-reconciliation-corrections-2026-11-06.md` and supersedes that note's previously narrower import examples.

## Defect exposed

The frozen reference supports completed grouped-Memory native imports carrying a completed first-cycle Dream settlement. The rebuild's `require_supported_tail` initially rejected the native `CVDREAM2`/ `RELDRF02` record families, despite the enclosing completed import being supported. A second reference regression allows a right-import Dream's historical preexisting population to remain fixed even if unrelated Memory births exist on the destination's left branch.

## Correction

- Source Dream admissions are selected against each source's exact physical Begin, rejecting missing, conflicting or ambiguous admissions. Duplicate identical authority supplied on a repeated merge is folded to one.
- A completed, source-authenticated, zero-root/no-promotion initial Dream pass is recreated through destination-native import owner methods. It rebinds destination turn, graph and Memory cut without reusing source physical clocks.
- A narrow replay-only owner method retains the authenticated source preexisting population when it is a subset of the destination's independently proven birth population. Existing compact prefix proofs still require exact equality; explicit Begin membership is checked as a physical subset before the settlement can reopen.
- The output report includes destination Dream admissions needed to open the rewritten owner; source and output settlements and preexisting population memberships are independently compared. Missing passes and semantic changes fail rather than disappearing.
- Source files remain unchanged; publication is staged and atomic no-replace.

## Evidence

- `completed_import_initial_dream_pass_is_preserved_in_divergent_merge`: native grouped Memory import + Dream settlement survives reconciliation, reopens with destination admissions, and repeat reconciliation preserves output without duplicate Memory or Freshness credit.
- `replayed_import_dream_preserves_source_population_with_extra_left_birth`: two branches share a grouped Memory birth; the destination left introduces an extra Memory before right-import Dream. The merged Dream retains the right source's preexisting membership rather than silently adopting the left-only Memory; source remains immutable.
- `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: passed after this implementation as part of the current independent verification session.

## Deliberate boundary / pending independent coverage

M03 does **not** treat arbitrary Dream relations, positive-root propagation, lifecycle promotion, unresolved or unsupported Dream journals as successfully merged. They require independently proven destination semantics or a typed error. The reference also retains richer Insomnia retry/terminal work histories: one-completion import replay is not proof of that fuller journal contract, so GM acceptance still requires direct coverage rather than assuming it is resolved.
