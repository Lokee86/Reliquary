//! Read-only historical source evidence over the reviewed V01 copy population.
#[path = "historical_state/archive_bindings.rs"]
mod archive_bindings;
#[path = "historical_state/carrier.rs"]
mod carrier;
#[path = "historical_state/clock_neutral.rs"]
mod clock_neutral;
#[path = "historical_state/freshness_cuts.rs"]
mod freshness_cuts;
#[path = "historical_state/insomnia.rs"]
mod insomnia;
#[path = "historical_state/membership.rs"]
mod membership;
#[path = "historical_state/native.rs"]
mod native;
#[path = "historical_state/prefix_freshness.rs"]
mod prefix_freshness;
#[path = "historical_state/project.rs"]
mod project;
#[path = "historical_state/row_commitment.rs"]
pub(crate) mod row_commitment;
#[path = "historical_state/vector_objects.rs"]
mod vector_objects;

use super::*;
use crate::FileKind;
use crate::f03_source_witness::{DirectoryGuard, HashedFile};
use serde_json::{Value, json};

const SCHEMA: &str = "reliquary-f03-historical-state-v2";
const COVERAGE: &str = "source-coverage.json";
const MANIFEST: &str = "source-witness-manifest.json";
const MAX_HISTORICAL_ARTIFACTS: usize = 9216;

struct ArtifactSink<'a> {
    output: Option<&'a Path>,
    resume: bool,
    digests: BTreeMap<String, String>,
    total_bytes: u64,
    guards: Vec<Arc<File>>,
}

impl ArtifactSink<'_> {
    fn store(&mut self, name: String, value: &Value) -> ToolResult<String> {
        let bytes = super::super::serialize_with_newline(value)?;
        self.total_bytes = self
            .total_bytes
            .checked_add(bytes.len() as u64)
            .filter(|total| *total <= MAX_TOTAL_BATCH_BYTES)
            .ok_or("historical artifact byte limit exceeded")?;
        let sha256 = sha256_bytes(&bytes);
        if self.digests.len() >= MAX_HISTORICAL_ARTIFACTS || self.digests.contains_key(&name) {
            return Err("historical artifact count limit or duplicate name".into());
        }
        if let Some(output) = self.output {
            let path = output.join(&name);
            let _ = save_immutable_json(&path, value, self.resume)?;
            self.guards.push(verify_immutable_output(
                &path,
                &bytes,
                "historical witness artifact",
            )?);
        }
        self.digests.insert(name, sha256.clone());
        Ok(sha256)
    }
}

struct BatchWriter<'a, 'b> {
    sink: &'a mut ArtifactSink<'b>,
    copy_id: &'a str,
    source_sha256: &'a str,
    stream: &'static str,
    previous: String,
    rows: Vec<Value>,
    count: u64,
    index: u64,
}

impl<'a, 'b> BatchWriter<'a, 'b> {
    fn new(sink: &'a mut ArtifactSink<'b>, copy: &'a VerifiedCopy, stream: &'static str) -> Self {
        Self {
            sink,
            copy_id: &copy.source_id,
            source_sha256: &copy.expected.sha256,
            stream,
            previous: INITIAL_BATCH_PREDECESSOR.to_string(),
            rows: Vec::with_capacity(BATCH_FRAME_LIMIT),
            count: 0,
            index: 0,
        }
    }

    fn push(&mut self, row: Value) -> ToolResult<()> {
        self.rows.push(row);
        self.count += 1;
        if self.rows.len() == BATCH_FRAME_LIMIT {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> ToolResult<()> {
        if self.rows.is_empty() {
            return Ok(());
        }
        let batch = json!({
            "schema_id": SCHEMA,
            "copy_id": self.copy_id,
            "source_sha256": self.source_sha256,
            "stream": self.stream,
            "batch_index": self.index.to_string(),
            "previous_sha256": self.previous,
            "first_ordinal": (self.count - self.rows.len() as u64).to_string(),
            "rows": self.rows,
        });
        let name = format!("{}-{}-{:06}.json", self.copy_id, self.stream, self.index);
        self.previous = self.sink.store(name, &batch)?;
        self.index += 1;
        self.rows.clear();
        Ok(())
    }

    fn finish(mut self) -> ToolResult<Value> {
        self.flush()?;
        Ok(json!({
            "rows": self.count.to_string(),
            "batches": self.index.to_string(),
            "last_batch_sha256": self.previous,
        }))
    }
}

fn observe_copy(
    copy: &VerifiedCopy,
    sink: &mut ArtifactSink<'_>,
    exhaustive_freshness: bool,
) -> ToolResult<Value> {
    verify_copy_stable(copy)?;
    let input = copy
        .held_file
        .try_clone()
        .map_err(|_| "source clone failed")?;
    let mut container = Container::open_read_only_for_scan_from_file(&copy.path, input)
        .map_err(|_| "historical reader rejected the source header")?;
    if container.backing_file_identity().ok().flatten() != Some(copy.snapshot.file_identity) {
        return Err("historical reader did not retain the V01-bound file identity".into());
    }
    let header = header_observation(&container);
    let identity = container
        .identity()
        .ok_or("historical source has no typed owner header")?;
    let uuid = container
        .owner_uuid()
        .ok_or("historical source has no owner UUID")?;
    let mut frames = BatchWriter::new(sink, copy, "frames");
    let mut carriers = BTreeMap::new();
    let mut counts = BTreeMap::new();
    let mut cursor = container
        .physical_cursor_for_source_witness(MAX_FRAME_PAYLOAD_BYTES, MAX_CLOCK_STATE_ENTRIES)
        .map_err(|_| "historical source cursor could not be opened")?;
    let mut last_end = container.header_len;
    let mut last_clock = 0;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|_| "historical source frame rejected")?
    {
        let (carrier, group, details) = carrier::describe(&frame.payload, identity.file_kind)?;
        *carriers.entry(carrier).or_insert(0_u64) += 1;
        *counts.entry(group).or_insert(0_u64) += 1;
        frames.push(json!({
            "offset": last_end.to_string(),
            "end_offset": frame.end.to_string(),
            "payload_sha256": sha256_bytes(&frame.payload),
            "latest_global": frame.latest_global.to_string(),
            "carrier": carrier,
            "group": group,
            "details": details,
        }))?;
        last_end = frame.end;
        last_clock = frame.latest_global;
    }
    if last_end != copy.expected.byte_length {
        return Err("historical scan did not end at the bound source length".into());
    }
    let clock = cursor
        .finish_open_scan()
        .map_err(|_| "historical clock rejected")?;
    container.apply_scan_clock(clock);
    if container.latest_version() != last_clock {
        return Err("historical scan clock does not match final physical frame".into());
    }
    let frame_stream = frames.finish()?;
    verify_copy_stable(copy)?;
    let native = match identity.file_kind {
        FileKind::Reliquary => native::rel(copy, sink, container, exhaustive_freshness),
        FileKind::Phylactery => native::phy(copy, sink, container),
        _ => Err("historical source has unsupported owner kind".into()),
    }?;
    verify_copy_stable(copy)?;

    let carriers: BTreeMap<_, _> = carriers
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect();
    let groups: BTreeMap<_, _> = super::super::SEMANTIC_GROUPS.into_iter()
        .map(|group| {
            let observed = counts.get(group).copied().unwrap_or_default();
            (group, json!({
                "physical_carrier_count": observed.to_string(),
                "semantic_presence": if observed > 0 { "observed_carrier" } else { "unaudited" },
                "semantic_completeness": "incomplete",
            }))
        }).collect();
    Ok(json!({
        "copy_id": copy.source_id,
        "source_sha256": copy.expected.sha256,
        "source_byte_length": copy.expected.byte_length.to_string(),
        "owner_uuid": lower_hex(&uuid),
        "header_observation": header,
        "frame_stream": frame_stream,
        "physical_carrier_counts": carriers,
        "groups": groups,
        "native": native,
    }))
}

fn run(
    options: super::super::Options,
    exhaustive_freshness: bool,
    prefix_plan: Option<&Path>,
) -> ToolResult<Value> {
    if prefix_plan.is_some() && (options.verify_only || options.resume || exhaustive_freshness) {
        return Err(
            "Freshness prefix replay requires a fresh protected output directory and a single mode"
                .into(),
        );
    }
    let (specs, binding) = production_v01_inputs()?;
    let (_, receipt, receipt_guard) = super::super::read_bounded_snapshot(
        &options.expected_custody,
        super::super::MAX_CUSTODY_BYTES,
        "expected V01 receipt",
    )?;
    if receipt.sha256 != V01_CUSTODY_RECEIPT_SHA256 {
        return Err("expected custody does not match the reviewed V01 receipt".into());
    }

    ensure_no_reparse_components(&options.source_root, "source root")?;
    let source_root =
        fs::canonicalize(&options.source_root).map_err(|_| "source root is unavailable")?;
    let inventory = super::super::collect_source_inventory(&source_root)?;
    verify_exact_layout(&specs, &inventory)?;
    let copies = bind_sources(&source_root, &specs, &inventory)?;
    let output = resolve_output_path(&options.output, &source_root)?;
    let mut directory_guards = Vec::new();
    if !options.verify_only {
        let parent = output.parent().ok_or("output has no parent")?;
        directory_guards = open_output_ancestor_guards(parent)?;
        if options.resume {
            ensure_no_reparse_components(&output, "historical output")?;
        } else {
            fs::create_dir(&output).map_err(|_| "fresh historical output could not be created")?;
        }
        directory_guards.push(open_directory_guard(&output, "historical output")?);
        ensure_directory_guards(&directory_guards)?;
    }

    let mut sink = ArtifactSink {
        output: (!options.verify_only).then_some(output.as_path()),
        resume: options.resume,
        digests: BTreeMap::new(),
        total_bytes: 0,
        guards: Vec::new(),
    };
    if let Some(plan) = prefix_plan {
        let report = prefix_freshness::observe(plan, &copies, &output)?;
        let _final_inventory = verify_inputs(
            &options,
            &copies,
            &source_root,
            &specs,
            &inventory,
            &receipt,
            &directory_guards,
        )?;
        sink.store("freshness-prefix-proofs.json".into(), &report)?;
        verify_output_membership(&output, &sink.digests.keys().cloned().collect())?;
        let _held = (receipt_guard, sink.guards);
        return Ok(report);
    }
    let mut sources = Vec::with_capacity(copies.len());
    let mut source_file_stats = BTreeMap::new();
    for copy in &copies {
        eprintln!("scanning {}", copy.source_id);
        sources.push(
            observe_copy(copy, &mut sink, exhaustive_freshness)
                .map_err(|e| format!("{}: {e}", copy.source_id))?,
        );
        source_file_stats.insert(&copy.source_id, &copy.snapshot.modified_unix_ns);
    }

    let coverage = json!({
        "schema_id": SCHEMA,
        "status": "INCOMPLETE",
        "producer_role": "source_reader",
        "authentic_admission": false,
        "semantic_history_assessed": true,
        "physical_carrier_completeness": "exhaustive_for_v01_copies",
        "global_ordering_completeness": "all_global_coordinates",
        "freshness_activity_requested": exhaustive_freshness,
        "semantic_completeness": "incomplete",
        "sidecar_discovery_scope": "exact_v01_copy_tree_only",
        "gaps": [
            "External operational sidecar inventory and independent native scope authentication",
            "Common-schema evidence for non-Archive families",
            "Clock-neutral as-of membership and full whole-owner historical projection",
        ],
        "binding_provenance": binding,
        "sources": sources,
    });
    let coverage_sha256 = sink.store(COVERAGE.into(), &coverage)?;
    let binary = env::current_exe().map_err(|_| "producer binary is unavailable")?;
    let binary = hash_file_snapshot(&binary)?;
    let producer_sha256 = sha256_bytes(
        &[
            include_bytes!("historical_state.rs").as_slice(),
            include_bytes!("../f03_preconversion_physical_census.rs").as_slice(),
            include_bytes!("historical_state/archive_bindings.rs").as_slice(),
            include_bytes!("historical_state/freshness_cuts.rs").as_slice(),
            include_bytes!("historical_state/prefix_freshness.rs").as_slice(),
            include_bytes!("historical_state/membership.rs").as_slice(),
            include_bytes!("historical_state/carrier.rs").as_slice(),
            include_bytes!("historical_state/native.rs").as_slice(),
            include_bytes!("historical_state/row_commitment.rs").as_slice(),
            include_bytes!("historical_state/vector_objects.rs").as_slice(),
            include_bytes!("historical_state/project.rs").as_slice(),
            include_bytes!("historical_state/clock_neutral.rs").as_slice(),
            include_bytes!("historical_state/insomnia.rs").as_slice(),
        ]
        .concat(),
    );
    let manifest = json!({
        "schema_id": SCHEMA,
        "status": "INCOMPLETE",
        "producer_role": "source_reader",
        "authentic_admission": false,
        "base_reader_commit": BASE_READER_COMMIT,
        "reader_source_sha256": reader_source_sha256(),
        "producer_code_sha256": producer_sha256,
        "producer_binary_sha256": binary.sha256,
        "cargo_lock_sha256": sha256_bytes(include_bytes!("../../Cargo.lock")),
        "expected_custody_sha256": receipt.sha256,
        "source_coverage_sha256": coverage_sha256,
        "source_file_mtimes_unix_ns": source_file_stats,
        "artifact_sha256": sink.digests,
    });
    sink.store(MANIFEST.into(), &manifest)?;
    let final_inventory = verify_inputs(
        &options,
        &copies,
        &source_root,
        &specs,
        &inventory,
        &receipt,
        &directory_guards,
    )?;
    if let Some(output) = sink.output {
        verify_output_membership(output, &sink.digests.keys().cloned().collect())?;
    }
    let _held = (receipt_guard, sink.guards, final_inventory);
    Ok(json!({
        "schema_id": SCHEMA,
        "status": "INCOMPLETE",
        "source_count": copies.len().to_string(),
        "source_coverage_sha256": coverage_sha256,
        "authentic_admission": false,
    }))
}

fn verify_inputs(
    options: &super::super::Options,
    copies: &[VerifiedCopy],
    source_root: &Path,
    specs: &[CopySpec],
    inventory: &SourceTreeInventory,
    receipt: &HashedFile,
    directory_guards: &[DirectoryGuard],
) -> ToolResult<SourceTreeInventory> {
    for copy in copies {
        verify_copy_stable(copy)?;
    }
    if !snapshots_match(&hash_file_snapshot(&options.expected_custody)?, receipt) {
        return Err("V01 receipt changed during historical observation".into());
    }
    let final_inventory = verify_source_inventory(source_root, inventory)?;
    verify_exact_layout(specs, &final_inventory)?;
    ensure_directory_guards(directory_guards)?;
    Ok(final_inventory)
}

pub fn cli_main() -> i32 {
    let mut arguments: Vec<_> = env::args().skip(1).collect();
    let prefix_plan = if arguments
        .first()
        .is_some_and(|value| value == "--prefix-freshness")
    {
        if arguments.len() < 2 {
            eprintln!("--prefix-freshness needs the independently verified plan path");
            return 1;
        }
        arguments.remove(0);
        Some(PathBuf::from(arguments.remove(0)))
    } else {
        None
    };
    let exhaustive_freshness = arguments
        .first()
        .is_some_and(|value| value == "--exhaustive-freshness");
    if exhaustive_freshness {
        arguments.remove(0);
    }
    match super::super::parse_options(arguments.into_iter()) {
        Ok(None) => {
            println!(
                "f03_source_witness [--exhaustive-freshness | --prefix-freshness PLAN] --source-root PATH --expected-custody V01_RECEIPT --output PATH [--resume | --verify-only]"
            );
            0
        }
        Ok(Some(options)) => match run(options, exhaustive_freshness, prefix_plan.as_deref()) {
            Ok(report) => {
                println!("{report}");
                if prefix_plan.is_some() { 0 } else { 2 }
            }
            Err(error) => {
                eprintln!("F03 historical source witness failed: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
