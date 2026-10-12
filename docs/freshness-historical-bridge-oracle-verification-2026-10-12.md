# Historical recipient-only bridge oracle verification — 2026-10-12

## Purpose

Record Step 19 verification that historical propagation traverses authorized future or otherwise ineligible Memories as bridges while awarding credit only to recipients eligible at the event position.

## Overview

The import-only adapter now has explicit regression coverage for both historical propagation kinds. A semantic event with a future root still reaches an eligible downstream Memory at the configured one-hop score, while the future root receives no credit. The same oracle covers a first-cycle Dream event and preserves the +50 linkage principal.

These tests exercise the frozen recipient-only approximation: authorization controls traversal, and historical availability controls credit after traversal. Live propagation remains unchanged and does not use this adapter.

## Related docs

- [Freshness semantic + historical linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Historical Freshness manifest and timeline policy](freshness-historical-manifest-policy.md)
- [Historical propagation verification](freshness-historical-propagation-verification-2026-10-12.md)
- [Historical Dream verification](freshness-historical-dream-verification-2026-10-12.md)

## Notes

Focused verification:

- cargo test --locked --lib historical
- cargo fmt -- --check
- git diff --check
- Documentation policy checks for the repository and changed files

The semantic oracle expects the future bridge to be absent and the eligible downstream recipient to score +5; the Dream oracle expects the same bridge exclusion and a downstream score of +45.
