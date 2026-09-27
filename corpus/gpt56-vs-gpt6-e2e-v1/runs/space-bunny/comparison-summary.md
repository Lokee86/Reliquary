# Space Bunny Alpha omnibus comparison

Model: `stealth/space-bunny-alpha` via OpenRouter, low reasoning.

The complete 62-case `gpt56-vs-gpt6-e2e-v1` omnibus was replayed without rerunning the historical GPT-5.6 controls.

| Stage | GPT-5.6 control | GPT-6 | Space Bunny | Signal |
|---|---:|---:|---:|---|
| Insomnia | historical semantic audit | anchor/state/omit 100%; authority 100%; grounding 87.5%; metadata 75% | anchor/state/omit 100%; authority 87.5%; grounding 75%; metadata 62.5% | Core retention clean; provenance/metadata weaker |
| Entity extraction | 6/10 exact; F1 0.861 | 6/10 exact; F1 0.824 | **4/10 exact; F1 0.667** | Material regression |
| Entity admission | 8/8 | 7/8 | **7/8** | Tie with GPT-6; same `presenter` miss |
| Entity disambiguation | 6/6 | 6/6 | **6/6** | Tie |
| Entity resolution | 9/10 | 8/10 | **7/10** | One additional miss |
| Chronos | preserved pre-GPT-6 state | 1/6 verifier-safe; 5 invalid | **2/6 verifier-safe; 4 invalid** | Better than GPT-6, still contract-poor |
| Dream | 37-39/40 historical full-context gate | 5/10 text-only proxy | **9/10 text-only proxy** | Large gain on directly comparable proxy |

## Stage details

### Insomnia
Space Bunny retained or omitted every selected anchor correctly and preserved 100% state coverage and content guards. It scored 87.5% authority, 75% grounding, and 62.5% metadata. Relative to GPT-6 on the same selected fixture, it added one authority-provenance miss, one grounding miss, and one metadata miss.

### Entity extraction
Space Bunny produced 20 TP, 5 FP, and 15 FN: precision 0.800, recall 0.571, F1 0.667, with 4/10 exact cases. Both zero-Entity cases remained correct and there were no transport/output errors.

### Entity admission
7/8. The only miss was `presenter`, expected `create_new`, returned `unresolved / recurrence_required`. This is the same selected-case miss as GPT-6.

### Entity disambiguation
6/6 selected adversarial query cases. The broader 18-mention runner also completed with zero errors.

### Entity resolution
7/10 selected queries, 100% candidate-order invariance, and zero false merges. Misses:
- `frozen-039`: expected `create_new`, returned `reject / generic_role`.
- `frozen-040`: expected `reject`, returned `unresolved / generic_role`.
- zero-candidate `frozen-038`: expected `unresolved`, returned `create_new / persistent_artifact`.

### Chronos
Six selected cases required model inference. Two outputs were verifier-safe: one resolved `Saturday` to `2026-05-16`, and one safely returned no resolution. Four outputs supplied canonical expressions rejected by deterministic Chronos verification.

### Dream
9/10 on the same text-only selected proxy used for the GPT-6 comparison: 6/7 related cases and 3/3 unrelated cases. The sole miss was `web-023` (expected related, returned none). This is directly comparable to GPT-6's 5/10 text-only proxy; it is not directly comparable to the historical GPT-5.6 37-39/40 full-context gate.

## Result

Space Bunny is not a clean replacement for the calibrated model mix on this omnibus. It is notably strong on the Dream proxy and slightly less incompatible than GPT-6 with Chronos, but substantially weaker on Entity extraction, weaker on final Entity resolution, and weaker on Insomnia provenance/metadata contracts.
