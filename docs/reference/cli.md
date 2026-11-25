# Command line reference

Parent index: [Reference index](./INDEX.md)

## Purpose

This page records the current `reliquary` standalone CLI grammar, shared output contract, defaults and command-family behavior.

## Overview

Build with `cargo build --manifest-path cli/Cargo.toml --locked`. The program is a local host using the `reliquary-memory` Root API. It emits one JSON value on stdout for successful one-shot operations. Errors are JSON on stderr in the form `{"error":{"kind":"...","message":"..."}}` and exit nonzero. A returned result with `ok: false` also exits nonzero. `--json` is accepted as a global marker and removed before parsing; repeating it is an error.

No arguments, exactly `--help`/`-h`, and exactly `--version`/`-V` are special invocations. `--help` text identifies version `0.1.0` and the supported 2.1 command surface. `--version` returns JSON. `--config FILE` may prefix a command and is rotated to the command arguments; commands that accept it may also take it in their own options. Default config path is `reliquary.cfg`.

`rel verify` and `phy verify` use ordinary 2.1 admission for files produced by the [offline history migration](../planning/owner-history-migration-plan-2026-11-18.md). Original FRS07 inputs produce a format error.

## Command families

| Family / syntax | Options and defaults | Contract and common diagnostics |
| --- | --- | --- |
| `rel create PATH` | No creation flags | Creates a REL 2.1 with a fresh owner ID and `unspecified` header scope. Requires a new path. The former `--scope` flag returns `invalid_input`. |
| `rel info PATH`, `rel verify PATH` | No flags | Read metadata or fully verify a current REL, including the original owner ID and stored scope. The info command uses a scoped reader and may restore a verified checkpoint. Exactly one path required. |
| `phy create PATH`, `phy info PATH`, `phy verify PATH` | no flags | Create/open/verify current PHY. Exactly one path required. |
| `rel import PATH --source FILE --format chatgpt\|normalized [--start TIME --end TIME] [--phy FILE] [--dry-run] [--process] [--config FILE]` | `--start` and `--end` must appear together. `--dry-run` and `--process` are mutually exclusive. `--config` requires `--process`. | ChatGPT raw exports may use half-open RFC3339 time selection. Normalized source selection is in its manifest. Dry run returns a plan; default stages import; `--process` also runs configured processing. Import diagnostics are stderr. |
| `rel process PATH --until idle\|ready [--config FILE] [--import ID] [--phy FILE] [--max-episodes N]` | config defaults to `reliquary.cfg`; max episodes default 128 and must be 1..=`MAX_PROCESS_EPISODES`; target is required and must be `idle` or `ready` | Processes live work or a staged import. Missing or invalid target and out-of-range episode cap are input errors. |
| `rel work status PATH [--import ID]`, `resume PATH ...`, `stop PATH [--import ID]` | resume accepts process flags plus `--episode`; imported resume defaults to `ready`, otherwise `idle` | Status is read-only. Resume uses the stored workflow plan. `stop` currently returns Unsupported with “this local invocation has no control channel for another process; stop the owning process”; it does not signal a separate process. |
| `rel vectors status PATH --profile HEX`; `rel vectors build PATH [--config FILE] [--limit N]` | build limit defaults 128 | Status requires a 32-byte hex compatibility profile ID. Build invokes configured embedding route. |
| `rel insomnia\|dream\|entity-resolution\|freshness status PATH ...`; `<stage> run PATH ...` | status: `--import ID` for import status; Dream and entity-resolution may use `--phy FILE`; freshness supports `--limit N` default 32. Run accepts processing flags (`--import`, `--until`, `--max-episodes`, `--phy`, `--config`). | Insomnia status can show live or import processing. Dream/entity-resolution status show live stage progress. `run` without import is supported for Insomnia, Dream and entity-resolution; freshness has status only. |
| `rel search PATH --query TEXT --mode lexical\|hybrid\|semantic [--config FILE] [--limit N] [--embedding-route LABEL] [--time-basis source|mentioned --from RFC3339 --before RFC3339]` | config optional; temporal flags are all-or-none and `--before` is exclusive; limit defaults from retrieval policy | Lexical search runs without provider config. Hybrid and semantic search use a compatible configured embedding when required. Semantic mode scans the available Memory vectors exactly and reports `semantic_routing: "global_exact"`. Invalid modes return `--mode must be lexical, hybrid, or semantic`. Pass `--scope archive` with `--mode hybrid` for Archive fragment results. The default scope is `memory`. Temporal results include the normalized filter and matched evidence. |
| `rel query PATH --query TEXT --config FILE [--embedding-route LABEL] [--time-basis source|mentioned --from RFC3339 --before RFC3339]`; `rel interact PATH [--config FILE] [--commands JSONL]` | interact may read commands from stdin; config defaults per runtime rules | Query performs configured retrieval. Interact maintains one local session for the process; see JSONL protocol below. |
| `rel memory list PATH [--cursor JSON] [--limit N]`; `get PATH MEMORY_ID [--revision N]`; `provenance PATH MEMORY_ID`; `history PATH MEMORY_ID` | page limit uses `MAX_MANAGEMENT_PAGE_ITEMS`; history supports `--cut N`. `memory add` requires `--title`, `--content`, `--category`, `--type`; optional `--request-id` requires stable `--created-at-ns`. | Bounded inspection and direct user-origin create. Memory IDs are 64 hex chars. `get` revision is optional. Invalid cursor/ID, stale cut and unauthorized owner produce typed errors. |
| `rel archive conversations PATH [page flags]`; `branches PATH CONVERSATION_ID [page flags]`; `show PATH CONVERSATION_ID LEAF_NODE_ID [page flags]`; `turn PATH CONVERSATION_ID NODE_ID` | page flags: `--cursor JSON`, `--limit N` | Reads bounded conversation/branch/transcript data; archive selectors are exact IDs. |
| `rel graph neighbors PATH NODE`; `rel graph edge PATH SOURCE_NODE TARGET_NODE RELATION` | both accept `--cursor JSON`, `--limit N` | Node is a 64-hex Memory ID, `memory:HEX`, or `entity:HEX`. Relation is `topical`, `factual`, `causal`, `recurrent`, `references`, `duplicate_of`, `supersedes`, `structural_parent`, `entity_association`, or `principal_association`. Graph inspection is distinct from agentic traversal. |
| `rel entity list PATH`, `rel relationship list PATH`, `rel community list PATH`; `entity get PATH ID`, `relationship get PATH ID` | list/detail pages accept `--cursor JSON`, `--limit N` | Bounded semantic inspection. IDs are 64-hex. |
| `rel community recompute PATH [--apply]` | previews with read access by default. `--apply` mounts a writer and recomputes from the current REL | JSON `result` includes eligible and assigned counts, Community count, size histogram, modularity, and membership SHA-256. Apply returns the accepted generation and whether it changed. A stale apply can be retried against the current REL. |
| `rel conversation feed\|status\|finalize\|cancel\|checkpoint\|stream PATH CONVERSATION_ID SUBMISSION_ID --state-dir DIR ...`; `rel conversation episode PATH CONVERSATION_ID BRANCH_ID NODE_ID` | `feed`: `--revision N`, `--content TEXT`; `cancel`: `--revision N`; `finalize`: `--revision N`, `--branch ID`, optional `--parent ID`; `status`/`checkpoint`: no extra flags; `stream`: `--after N` default 0 and `--limit N` default 16, range 1..64. `--state-dir` is required for queue operations. | Operates the local durable conversation queue and stream state. `dispatch`, `reconcile` and `cancel-host` are Unsupported without a registered trusted conversation host. Invalid revision, stream bound or missing operation arguments are typed input errors. |
| `rel dependency list PATH` | read-only source mount | Direct declaration and generation. |
| `rel dependency replace PATH --expected-generation N` | repeatable `--dependency PATH` sets direct dependencies. Repeatable `--mount-rel PATH` supplies extra read-only mounts | Publishes one declaration or returns the existing generation for an identical set. Zero dependency flags clears the set. |
| `rel dependency resolve PATH` | repeatable `--mount-rel PATH` grants read access to those mounts | Ordered owner IDs, with the primary REL last. |
| `rel context inspect\|plan\|checkpoint PATH CONVERSATION_ID LEAF_NODE_ID` | optional `--byte-budget N` defaults to 65536. Repeat `--mount-rel PATH` to supply dependency RELs. `checkpoint` requires `--summary-file FILE` | Bounded transcript and inherited Ego evidence. The summary must be valid UTF-8 and fit the computed budget. |
| `rel source status PATH --import ID`; `rel history memory PATH MEMORY_ID [--cut N]`; `rel history archive PATH [--offset N] [--limit N]`; `rel history proof PATH` | archive offset defaults 0, limit 128 | Read-only import source, historical Memory, archive and proof projections. `phy memory list|get|provenance|history PATH ...` reads PHY memories; `provenance` may take `--rel REL_PATH`, history accepts `--cut N`, and all list/detail page operations accept `--cursor JSON` and `--limit N`. |
| `rel rewrite SOURCE DEST --policy FILE [--dry-run] [--profile HEX ...] [--include-memory HEX ...] [--exclude-memory HEX ...]` | repeatable profile/include/exclude flags replace/extend policy selectors as implemented | `--dry-run` returns plan only. Execution verifies copy input and creates a new destination; no replacement. Invalid policy JSON returns `invalid rewrite policy`. |
| `rel prune memory\|archive\|entity\|relationship SOURCE DEST ID...`; `rel repair SOURCE DEST --keep-profile HEX...`; `rel reclaim SOURCE DEST`; `rel reconcile LEFT RIGHT DEST` | prune requires at least one non-option target; repair requires one or more repeatable `--keep-profile` IDs | Verified copy maintenance; destinations are create-only. Source owners must satisfy quiescence/plan requirements. No command rewrites in place. |
| `config show`; `config verify [--online [--route LABEL]]`; `config credential ...`; `config model set-ROUTE\|clear-ROUTE ...` | config path defaults `reliquary.cfg`; `--route` requires `--online` | Offline verify validates locally only. Online verify sends synthetic probe input. Credentials are not printed. Route options are listed below. |

`[page flags]` means the exact optional pair `--cursor JSON` and `--limit N`; unsupported flags and duplicate nonrepeatable flags fail. See `cli/src/inspection.rs` and `cli/src/semantic_inspection.rs` for operation-specific positional arity.

For Archive hybrid search, run `rel search PATH --mode hybrid --scope archive --query TEXT` with a configured embedding route compatible with the REL's active Archive vectors. The response includes the generation, text coverage, lexical and cosine scores, and the conversation and node IDs for each hit. A passage hit also reports `passage.parent_fragment_id` plus the zero-based UTF-8 offsets `passage.start_byte` in the first turn and `passage.end_byte` in the last turn. The end offset is exclusive. Run `rel archive index PATH` on a writable REL to persist indexes for its active Archive generations. It reports `added_bytes` for each generation and needs only the stored Archive data. Search uses a matching persisted index after reopen. A query on an unindexed generation builds in memory.

The CLI accepts explicit RFC3339 bounds for temporal Archive search. Embedded chat hosts can instead resolve natural-language dates using the Root API with an admitted request time and IANA calendar zone. [Temporal search in the Root API](../public-product-api.md#temporal-search)

## Configuration commands

- `config credential list`
- `config credential add-api-key ID [--stdin]` — otherwise prompts without echo; ID and key are not echoed in JSON. `--stdin` reads at most 64 KiB of one line.
- `config credential login-codex ID` — prints device verification URL and user code to stderr.
- `config credential refresh-codex ID`
- `config credential remove ID`
- `config model set-ROUTE --model MODEL --credential ID [--provider openai-codex|openai-ready] [--url URL] [--reasoning none|minimal|low|medium|high|xhigh|max]`
- `config model set-embedding --model MODEL --credential ID --dimensions N [--provider openai-codex|openai-ready] [--url URL] [--normalization l2|none]`
- Decision routes (`general-decision`, `entity-resolution-decision`, `memory-retrieval-decision`) require `--model`, `--credential` and `--url`.
- For each supported route, `config model clear-ROUTE` removes it. Route names are `general`, `insomnia`, `insomnia-metadata`, `entity-extraction`, `general-decision`, `entity-resolution-decision`, `memory-retrieval-decision`, `entity-resolution`, `dream`, `embedding`.

For a managed Perception session, use `config model set-general-decision --model typesafe/jev-1.13 --credential ID --url URL` with the host's native decision endpoint. The general and embedding routes supply discovery, synthesis and receptors. [Configured runtime](../configured-runtime.md)

## Interactive JSONL

`rel interact PATH [--config FILE] [--commands JSONL]` reads one JSON object per line and emits one reply per request. Records require `request_id` and `command`. Commands are `query`/`refine` with `text`; a JSONL query may also include `temporal_filter` with `basis`, `from` and `before` RFC3339 offset values. Commands also include `traverse` with one to ten `memory_ids`, `show`/`source`/`neighbors` with `memory_id`, plus `status`, `new-turn` and `quit`. Temporal CLI options must be supplied together; the upper `--before` bound is exclusive. Extra fields are rejected. Input lines are bounded at 16 KiB; invalid UTF-8/JSON, missing fields and unknown commands produce per-request errors. Query/refine/traverse share process-local state; ending the process ends that session.

```jsonl
{"request_id":1,"command":"query","text":"How does the service handle movement?","temporal_filter":{"basis":"source","from":"2026-11-20T00:00:00-08:00","before":"2026-11-21T00:00:00-08:00"}}
{"request_id":2,"command":"refine","text":"Focus on server-side requirements"}
{"request_id":3,"command":"new-turn"}
{"request_id":4,"command":"quit"}
```

## Shared parser errors

Declared options are accepted once unless explicitly repeatable. Missing values, duplicate flags, unknown flags and missing positionals fail before a file is opened or state is mutated. Common messages include `missing required positional argument`, `unexpected CLI argument: FLAG`, `duplicate CLI option: FLAG`, and `missing value for FLAG`. Unsupported command families/operations produce `kind: unsupported`; malformed input produces `kind: invalid_input`. The error kind is the machine-facing discriminator; message text is diagnostic, not a versioned enum contract.

## Related docs

- [License for redistributing the CLI](../../README.md#license)
- [Reference index](INDEX.md)
- [Operator workflow](../operator-workflow.md)
- [Configured runtime](../configured-runtime.md)
- [Public product API](../public-product-api.md)

## Notes

The command dispatch and help text are in `cli/src/main.rs`; parsing and per-family behavior are implemented in `cli/src/{parse,commands,config_command,operational,import_command,retrieval_command,inspection,semantic_inspection,conversation,rel_dependencies,context_source}.rs`. Contract coverage is in `cli/tests/cli.rs`, `cli/src/operational_tests.rs` and `cli/tests/g4_corpus.rs`. This page is a human reference; the current `--help` and parser source determine exact behavior on any later revision.
