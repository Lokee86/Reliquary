use reliquary_memory::{
    Gateway, HostCredentialBinding, InspectionQuery, MemoryListFilters, OwnerGrant, OwnerKind,
    Phylactery, ProductErrorKind, Rel, TrustedHostAdmission,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct TestPaths {
    root: PathBuf,
}
impl TestPaths {
    fn new(tag: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "public-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}
impl Drop for TestPaths {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn trusted_path_mount_checks_the_expected_header_before_replay() {
    use std::io::Write;

    let files = TestPaths::new("expected-mount");
    let path = files.path("candidate.rel");
    let candidate = Rel::create(&path).unwrap();
    let expected = Rel::create(files.path("expected.rel")).unwrap().owner_id();
    let len = fs::metadata(&path).unwrap().len();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"x")
        .unwrap();

    let gateway = Gateway::new();
    assert_eq!(
        gateway
            .mount_owner_path(&path, OwnerKind::Rel, true, Some(expected))
            .unwrap_err()
            .kind(),
        ProductErrorKind::Admission,
    );
    assert_eq!(
        gateway
            .mount_owner_path(&path, OwnerKind::Rel, true, Some(candidate.owner_id()))
            .unwrap_err()
            .kind(),
        ProductErrorKind::Verification,
    );

    // Both rejections released the writer lease and installed no mount.
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(len)
        .unwrap();
    assert_eq!(
        gateway
            .mount_owner_path(&path, OwnerKind::Rel, true, Some(candidate.owner_id()))
            .unwrap(),
        candidate.owner_id(),
    );
}

#[test]
#[ignore = "requires RELIQUARY_ARCHIVE_FIXTURE pointing to an existing REL 2.1 with Archive vectors"]
fn archive_search_matches_reference_and_reopens() {
    use reliquary_container::Container;
    use reliquary_vector_store::{
        ArchiveVectorStore, CompatibilityProfileStore, PackedVectorStore, VectorGenerationStore,
    };
    use std::{collections::HashSet, io::Read};

    const K: usize = 10;
    const QUERIES: usize = 20;
    let fixture = PathBuf::from(std::env::var_os("RELIQUARY_ARCHIVE_FIXTURE").unwrap());
    let hash = |path: &std::path::Path| {
        blake3::Hasher::new()
            .update_reader(fs::File::open(path).unwrap())
            .unwrap()
            .finalize()
    };
    let original = hash(&fixture);
    let source_bytes = fs::metadata(&fixture).unwrap().len();
    let temp = TestPaths::new("archive-exact");
    let copy = temp.path("owner.rel");
    fs::copy(&fixture, &copy).unwrap();
    assert_eq!(hash(&copy), original);

    // Read the accepted source independently of the new search path.
    let (generation, fragments, dimensions, values, sources) = {
        let mut container = Container::open_read_only(&copy).unwrap();
        let archive = reliquary_archive::Archive::open(&container).unwrap();
        let packed = PackedVectorStore::open(&container).unwrap();
        let profiles = CompatibilityProfileStore::open(&container).unwrap();
        let archives = ArchiveVectorStore::open(&container, &packed).unwrap();
        let generations = VectorGenerationStore::open(&container, &profiles, &archives).unwrap();
        let generation = generations
            .current_generations()
            .filter(|g| profiles.get_active(g.compatibility_profile_id).is_some())
            .max_by_key(|g| {
                (
                    archives.info(g.archive_vector_id).unwrap().rows,
                    g.vector_version,
                )
            })
            .expect("Archive fixture must have an active vector generation");
        let mapping = archives.get(generation.archive_vector_id).unwrap();
        let sources = mapping
            .fragment_ids
            .iter()
            .map(|id| {
                let (fragment, _) = archive
                    .fragment_vector_source(
                        reliquary_archive::FragmentId(id.0),
                        generation.source_archive_version,
                    )
                    .unwrap();
                let text = match archive.bounded_conversation_range(
                    &fragment.conversation_id,
                    &fragment.start_node_id,
                    &fragment.end_node_id,
                    reliquary_archive::MAX_EVIDENCE_TURNS,
                    reliquary_archive::MAX_EVIDENCE_RESULT_BYTES,
                ) {
                    Ok(turns) => turns
                        .iter()
                        .map(|turn| {
                            archive.content_for_node(&fragment.conversation_id, &turn.node_id)
                        })
                        .collect::<Option<Vec<_>>>()
                        .map(|texts| texts.join("\n")),
                    Err(
                        reliquary_archive::ArchiveError::MissingContent
                        | reliquary_archive::ArchiveError::EvidenceLimitExceeded,
                    ) => None,
                    Err(error) => panic!("invalid Archive fragment range: {error}"),
                };
                (fragment.clone(), text)
            })
            .collect::<Vec<_>>();
        let matrix = packed
            .get(&mut container, mapping.packed_vector_id)
            .unwrap();
        let values = matrix
            .bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .copied()
            .map(f32::from_le_bytes)
            .collect::<Vec<_>>();
        (
            generation,
            mapping.fragment_ids,
            matrix.schema().dimensions as usize,
            values,
            sources,
        )
    };
    let rows = values.chunks_exact(dimensions).collect::<Vec<_>>();
    assert_eq!(rows.len(), fragments.len());
    assert!(rows.len() >= QUERIES);
    let queries = &rows[rows.len() - QUERIES..];
    let dot = |a: &[f32], b: &[f32]| {
        a.iter()
            .zip(b)
            .map(|(&x, &y)| f64::from(x) * f64::from(y))
            .sum::<f64>()
    };
    let norms = rows
        .iter()
        .map(|row| dot(row, row).sqrt())
        .collect::<Vec<_>>();
    let expected = queries
        .iter()
        .map(|query| {
            let query_norm = dot(query, query).sqrt();
            let mut scored = rows
                .iter()
                .zip(&norms)
                .enumerate()
                .filter(|(_, (_, norm))| **norm > 0.0)
                .map(|(ordinal, (row, norm))| {
                    let score = (dot(row, query) / (norm * query_norm)).clamp(-1.0, 1.0);
                    (ordinal, score)
                })
                .collect::<Vec<_>>();
            scored.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            scored.truncate(K);
            scored
        })
        .collect::<Vec<_>>();

    // Pair each benchmark vector with up to 12 terms from its source fragment.
    let query_terms = |text: &str| {
        reliquary_retrieval::lexical_terms(text)
            .into_iter()
            .filter(|term| term.len() <= 32)
            .take(12)
            .collect::<Vec<_>>()
    };
    let fallback = sources
        .iter()
        .filter_map(|(_, text)| text.as_deref())
        .map(query_terms)
        .find(|terms| !terms.is_empty())
        .expect("Archive fixture has readable keyword text");
    let hybrid_queries = sources[sources.len() - QUERIES..]
        .iter()
        .map(|(_, text)| {
            let terms = text.as_deref().map(query_terms).unwrap_or_default();
            if terms.is_empty() {
                fallback.clone()
            } else {
                terms
            }
        })
        .collect::<Vec<_>>();
    let query_text = hybrid_queries
        .iter()
        .map(|terms| terms.join(" "))
        .collect::<Vec<_>>();

    // The full scan uses the pre-index whole-word and substring rules.
    let lexical_sources = sources
        .iter()
        .map(|(_, text)| {
            text.as_ref().map(|text| {
                let words = text
                    .split(|ch: char| !ch.is_alphanumeric())
                    .filter(|word| !word.is_empty())
                    .map(str::to_lowercase)
                    .collect::<HashSet<_>>();
                (words, text.to_lowercase())
            })
        })
        .collect::<Vec<_>>();
    let expected_hybrid: Vec<_> = hybrid_queries
        .iter()
        .zip(queries)
        .map(|(terms, vector)| {
            let query_norm = dot(vector, vector).sqrt();
            let mut hits = lexical_sources
                .iter()
                .zip(rows.iter().zip(&norms))
                .enumerate()
                .filter_map(|(ordinal, (text, (row, norm)))| {
                    let lexical = text.as_ref().and_then(|(words, lower)| {
                        let counts = terms
                            .iter()
                            .filter(|term| words.contains(*term))
                            .map(|term| lower.matches(term).take(4).count());
                        let (matched, frequency) =
                            counts.fold((0, 0), |(matched, frequency), count| {
                                (matched + usize::from(count > 0), frequency + count)
                            });
                        (matched > 0).then(|| {
                            let coverage = matched as f64 / terms.len() as f64;
                            let density = (frequency as f64 / (terms.len() * 2) as f64).min(1.0);
                            0.85 * coverage + 0.15 * density
                        })
                    });
                    let cosine = (dot(row, vector) / (norm * query_norm)).clamp(-1.0, 1.0);
                    let semantic = (cosine > 0.0).then_some(cosine);
                    let score = match (lexical, semantic) {
                        (Some(a), Some(b)) => 0.45 * a + 0.55 * b,
                        (Some(score), None) | (None, Some(score)) => score,
                        (None, None) => return None,
                    };
                    Some((ordinal, lexical, semantic, score))
                })
                .collect::<Vec<_>>();
            assert!(hits.iter().any(|(_, lex, _, _)| lex.is_some()));
            hits.sort_unstable_by(|a, b| b.3.total_cmp(&a.3).then_with(|| a.0.cmp(&b.0)));
            hits.truncate(K);
            hits
        })
        .collect();

    let mut first = Vec::new();
    let mut first_hybrid = Vec::new();
    let profile = generation.compatibility_profile_id;
    let mut expected_copy = original;
    let mut added_bytes = 0;
    for reopened in [false, true] {
        if reopened {
            let rel = Rel::open(&copy).unwrap();
            let gateway = Gateway::new();
            let owner = gateway.mount_rel(&rel, true).unwrap();
            let token = "synthetic-archive-index-publication-token";
            let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
                "archive-indexer",
                token,
                [OwnerGrant::OperateRel(owner)],
            )])
            .install(&gateway)
            .unwrap();
            let instance = admission.authenticate(token).unwrap();
            let published = instance.publish_archive_lexical_indexes(owner).unwrap();
            assert!(
                published
                    .iter()
                    .any(|index| index.generation_id == generation.id.0)
            );
            added_bytes = published.iter().map(|index| index.added_bytes).sum::<u64>();
            let retry = instance.publish_archive_lexical_indexes(owner).unwrap();
            assert_eq!(published.len(), retry.len());
            assert!(retry.iter().all(|index| index.added_bytes == 0));
            drop(instance);
            drop(admission);
            drop(gateway);
            assert_eq!(
                fs::metadata(&copy).unwrap().len(),
                source_bytes + added_bytes
            );
            let prefix = blake3::Hasher::new()
                .update_reader(fs::File::open(&copy).unwrap().take(source_bytes))
                .unwrap()
                .finalize();
            assert_eq!(prefix, original);
            expected_copy = hash(&copy);
        }
        let rel = Rel::open(&copy).unwrap();
        let gateway = Gateway::new();
        let owner = gateway.mount_rel(&rel, false).unwrap();
        let other = Rel::create(temp.path("other.rel")).unwrap();
        let other_owner = gateway.mount_rel(&other, false).unwrap();
        let token = "synthetic-archive-vector-read-admission-token";
        let (admission, control) = TrustedHostAdmission::new([HostCredentialBinding::new(
            "archive-reader",
            token,
            [OwnerGrant::ViewRel(owner)],
        )])
        .install(&gateway)
        .unwrap();
        let instance = admission.authenticate(token).unwrap();
        assert_eq!(
            instance
                .publish_archive_lexical_indexes(owner)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized
        );

        for (i, (query, expected)) in queries.iter().zip(&expected).enumerate() {
            let actual = instance
                .search_archive_vectors(owner, profile, query, K)
                .unwrap();
            assert_eq!(actual.profile_id, profile);
            assert_eq!(actual.generation_id, generation.id.0);
            assert_eq!(
                actual.source_archive_version,
                generation.source_archive_version
            );
            assert_eq!(actual.hits.len(), expected.len());
            for (hit, &(ordinal, score)) in actual.hits.iter().zip(expected) {
                assert_eq!(hit.ordinal, ordinal);
                assert_eq!(hit.fragment_id, fragments[ordinal].0);
                assert!((hit.score - score).abs() <= 1e-15);
            }
            if reopened {
                assert_eq!(actual, first[i]);
            } else {
                first.push(actual);
            }
        }
        for (i, (text, vector)) in query_text.iter().zip(queries).enumerate() {
            let actual = instance
                .search_archive_hybrid(owner, profile, text, vector, K)
                .unwrap();
            assert_eq!(actual.profile_id, profile);
            assert_eq!(actual.generation_id, generation.id.0);
            assert_eq!(
                actual.source_archive_version,
                generation.source_archive_version
            );
            assert_eq!(actual.fragments_scored, rows.len());
            assert_eq!(
                actual.fragments_with_text,
                sources.iter().filter(|(_, text)| text.is_some()).count()
            );
            assert_eq!(actual.hits.len(), expected_hybrid[i].len());
            for (hit, &(ordinal, lexical, semantic, score)) in
                actual.hits.iter().zip(&expected_hybrid[i])
            {
                let (source, _) = &sources[ordinal];
                assert_eq!(hit.ordinal, ordinal);
                assert_eq!(hit.fragment_id, fragments[ordinal].0);
                assert_eq!(hit.conversation_id, source.conversation_id);
                assert_eq!(hit.start_node_id, source.start_node_id);
                assert_eq!(hit.end_node_id, source.end_node_id);
                assert_eq!(hit.lexical_score, lexical);
                match (hit.semantic_score, semantic) {
                    (Some(actual), Some(expected)) => assert!((actual - expected).abs() <= 1e-15),
                    (None, None) => {}
                    _ => panic!("semantic score presence differs"),
                }
                assert!((hit.score - score).abs() <= 1e-15);
            }
            if reopened {
                assert_eq!(actual, first_hybrid[i]);
            } else {
                first_hybrid.push(actual);
            }
        }
        assert_eq!(
            instance
                .search_archive_hybrid(other_owner, profile, &query_text[0], queries[0], K)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized
        );
        assert_eq!(
            instance
                .search_archive_vectors(other_owner, profile, queries[0], K)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unauthorized
        );
        assert_eq!(
            instance
                .search_archive_vectors(
                    owner,
                    reliquary_memory::CompatibilityProfileId([0; 32]),
                    queries[0],
                    K
                )
                .unwrap_err()
                .kind(),
            ProductErrorKind::Unavailable
        );
        assert_eq!(
            instance
                .search_archive_vectors(owner, profile, &vec![1.0; dimensions + 1], K)
                .unwrap_err()
                .kind(),
            ProductErrorKind::InvalidInput
        );
        assert_eq!(
            instance
                .search_archive_vectors(owner, profile, queries[0], 0)
                .unwrap_err()
                .kind(),
            ProductErrorKind::InvalidInput
        );
        control.revoke(token).unwrap();
        assert_eq!(
            instance
                .search_archive_hybrid(owner, profile, &query_text[0], queries[0], K)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Revoked
        );
        assert_eq!(
            instance
                .search_archive_vectors(owner, profile, queries[0], K)
                .unwrap_err()
                .kind(),
            ProductErrorKind::Revoked
        );
        drop(instance);
        drop(control);
        drop(admission);
        drop(gateway);
        fs::remove_file(other.path()).unwrap();
        assert_eq!(fs::read_dir(&temp.root).unwrap().count(), 1);
    }

    assert_eq!(hash(&fixture), original);
    assert_eq!(hash(&copy), expected_copy);
    println!(
        "Archive top-{K} matched {QUERIES} exact and {} hybrid reference queries before and after index publication and reopen across {} vectors of {dimensions} dimensions. Index publication added {added_bytes} bytes.",
        query_text.len(),
        rows.len()
    );
}

#[test]
fn current_rel_and_phylactery_reopen_with_stable_identity_and_kind_checks() {
    let temp = TestPaths::new("owners");
    let rel_path = temp.path("owner.rel");
    let phy_path = temp.path("owner.phy");
    let rel = Rel::create(&rel_path).unwrap();
    let phy = Phylactery::create(&phy_path).unwrap();
    assert_eq!(Rel::open(&rel_path).unwrap().owner_id(), rel.owner_id());
    assert_eq!(
        Phylactery::open(&phy_path).unwrap().owner_id(),
        phy.owner_id()
    );
    assert_eq!(rel.info().unwrap().format, "2.1");
    assert_eq!(
        Rel::open(&phy_path).err().unwrap().kind(),
        ProductErrorKind::WrongFormat
    );
    assert_eq!(
        Phylactery::open(&rel_path).err().unwrap().kind(),
        ProductErrorKind::WrongFormat
    );
}

#[test]
fn trusted_host_detaches_only_the_selected_read_only_phy() {
    let temp = TestPaths::new("detach-phy");
    let rel = Rel::create(temp.path("owner.rel")).unwrap();
    let writable = Phylactery::create(temp.path("writable.phy")).unwrap();
    let read_only = Phylactery::create(temp.path("view.phy")).unwrap();
    let gateway = Gateway::new();
    let rel_id = gateway.mount_rel(&rel, true).unwrap();
    let writable_id = gateway.mount_phylactery(&writable, true).unwrap();
    let read_id = gateway.mount_phylactery(&read_only, false).unwrap();
    let token = "synthetic-phy-detach-admission-token";
    let (admission, control) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "operator",
        token,
        [
            OwnerGrant::ViewRel(rel_id),
            OwnerGrant::ViewPhylactery(writable_id),
            OwnerGrant::ViewPhylactery(read_id),
        ],
    )])
    .install(&gateway)
    .unwrap();
    let instance = admission.authenticate(token).unwrap();
    let inspect = |owner, kind| {
        instance.inspect_owner(
            owner,
            kind,
            &InspectionQuery::MemorySummaries(MemoryListFilters::default()),
            None,
            1,
        )
    };
    let original = fs::read(read_only.path()).unwrap();

    assert!(control.detach_read_only_phylactery(rel_id).is_err());
    assert!(control.detach_read_only_phylactery(writable_id).is_err());
    control.detach_read_only_phylactery(read_id).unwrap();
    assert!(inspect(read_id, OwnerKind::Phylactery).is_err());
    assert!(inspect(rel_id, OwnerKind::Rel).is_ok());
    assert!(inspect(writable_id, OwnerKind::Phylactery).is_ok());

    assert_eq!(
        gateway.mount_phylactery(&read_only, false).unwrap(),
        read_id
    );
    assert_eq!(fs::read(read_only.path()).unwrap(), original);
}

#[test]
fn trusted_management_requires_manage_grant_and_revocation_fences_reads() {
    let temp = TestPaths::new("management");
    let rel = Rel::create(temp.path("owner.rel")).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let manage_token = "m".repeat(48);
    let view_token = "v".repeat(48);
    let admission = TrustedHostAdmission::new([
        HostCredentialBinding::new("manager", &manage_token, [OwnerGrant::ManageRel(owner)]),
        HostCredentialBinding::new("viewer", &view_token, [OwnerGrant::ViewRel(owner)]),
    ]);
    let (adapter, control) = admission.install(&gateway).unwrap();
    let manager = adapter.authenticate(&manage_token).unwrap();
    let viewer = adapter.authenticate(&view_token).unwrap();
    let view = viewer.workspace_management(owner).unwrap();
    assert_eq!(
        view.create_memory(
            "title".into(),
            "body".into(),
            "fact".into(),
            "note".into(),
            1
        )
        .unwrap_err()
        .kind(),
        ProductErrorKind::Unauthorized
    );
    let management = manager.workspace_management(owner).unwrap();
    let memory = management
        .create_memory(
            "title".into(),
            "body".into(),
            "fact".into(),
            "note".into(),
            1,
        )
        .unwrap();
    assert_eq!(management.statistics().unwrap().memories.memories, 1);
    assert_eq!(
        management
            .provenance(reliquary_memory::MemoryId(parse_hex(&memory.id)))
            .unwrap()
            .principal_id
            .as_deref(),
        Some("manager")
    );
    management.quiesce().unwrap();
    assert_eq!(management.reconcile_quiesced().unwrap().owner_id, owner);
    management.reopen().unwrap();
    assert_eq!(
        management
            .memory(reliquary_memory::MemoryId(parse_hex(&memory.id)))
            .unwrap()
            .content,
        "body"
    );
    control.revoke(&manage_token).unwrap();
    assert_eq!(
        management.health().unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
    assert_eq!(
        management.quiesce().unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
}

#[test]
fn host_grants_do_not_cross_rel_and_phylactery_boundaries() {
    let temp = TestPaths::new("privacy");
    let rel = Rel::create(temp.path("owner.rel")).unwrap();
    let phy = Phylactery::create(temp.path("owner.phy")).unwrap();
    let gateway = Gateway::new();
    let rel_owner = gateway.mount_rel(&rel, false).unwrap();
    let phy_owner = gateway.mount_phylactery(&phy, false).unwrap();
    let token = "r".repeat(48);
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "rel-only",
        &token,
        [OwnerGrant::ViewRel(rel_owner)],
    )])
    .install(&gateway)
    .unwrap();
    let instance = adapter.authenticate(&token).unwrap();
    assert_eq!(instance.rel_info(rel_owner).unwrap().owner_id, rel_owner);
    assert_eq!(
        instance.phylactery_info(phy_owner).unwrap_err().kind(),
        ProductErrorKind::Unauthorized
    );
    assert_eq!(
        instance
            .open_conversation_queue(temp.path("queue"), rel_owner)
            .err()
            .unwrap()
            .kind(),
        ProductErrorKind::Unauthorized
    );
}

#[test]
#[ignore = "requires the verified offline migration candidates"]
fn migrated_pair_preserves_private_memory_grants() {
    use reliquary_memory::{InspectionQuery, MemoryId, OwnerKind};
    use std::env;

    let rel = Rel::open(env::var_os("RELIQUARY_MIGRATION_REL").unwrap()).unwrap();
    let phy = Phylactery::open(env::var_os("RELIQUARY_MIGRATION_PHY").unwrap()).unwrap();
    let id = MemoryId(parse_hex(&env::var("RELIQUARY_MIGRATION_MEMORY").unwrap()));
    let gateway = Gateway::new();
    let rel_owner = gateway.mount_rel(&rel, false).unwrap();
    let phy_owner = gateway.mount_phylactery(&phy, false).unwrap();
    let rel_only = "r".repeat(48);
    let paired = "p".repeat(48);
    let (adapter, _) = TrustedHostAdmission::new([
        HostCredentialBinding::new("rel-only", &rel_only, [OwnerGrant::ViewRel(rel_owner)]),
        HostCredentialBinding::new(
            "paired",
            &paired,
            [
                OwnerGrant::ViewRel(rel_owner),
                OwnerGrant::ViewPhylactery(phy_owner),
            ],
        ),
    ])
    .install(&gateway)
    .unwrap();
    let query = InspectionQuery::Memory {
        id,
        revision: Some(1),
    };
    let inspect = |token| {
        adapter.authenticate(token).unwrap().inspect_owner(
            phy_owner,
            OwnerKind::Phylactery,
            &query,
            None,
            1,
        )
    };
    assert_eq!(
        inspect(&rel_only).unwrap_err().kind(),
        ProductErrorKind::Unauthorized
    );
    assert_eq!(inspect(&paired).unwrap().items.len(), 1);
}

fn parse_hex(value: &str) -> [u8; 32] {
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    bytes
}
#[test]
fn public_copy_workflows_reopen_and_do_not_overwrite() {
    use reliquary_memory::{ReconciliationMode, RewritePolicy, execute_rewrite, reconcile_rel};

    let temp = TestPaths::new("copies");
    let source = temp.path("original.rel");
    let identical = temp.path("identical.rel");
    let rewritten = temp.path("rewritten.rel");
    let merged = temp.path("merged.rel");
    let rel = Rel::create(&source).unwrap();
    let owner_id = rel.owner_id();
    let plan = rel.plan_rewrite(RewritePolicy::default()).unwrap();
    let before = fs::read(&source).unwrap();
    fs::copy(&source, &identical).unwrap();

    let report = execute_rewrite(&plan, &source, &rewritten).unwrap();
    let fresh = Rel::open(&rewritten).unwrap();
    assert_eq!(fresh.owner_id().as_bytes(), report.destination_owner_id);
    assert_ne!(fresh.owner_id(), owner_id);
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(
        execute_rewrite(&plan, &source, &rewritten)
            .unwrap_err()
            .kind(),
        ProductErrorKind::PublicationCollision
    );

    let merge = reconcile_rel(&source, &identical, &merged).unwrap();
    assert_eq!(merge.mode, ReconciliationMode::ExactCopy);
    assert_eq!(fs::read(&merged).unwrap(), before);
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(
        reconcile_rel(&source, &identical, &merged)
            .unwrap_err()
            .kind(),
        ProductErrorKind::PublicationCollision
    );
}

#[test]
fn public_management_memory_history_and_proof_survive_reopen() {
    use reliquary_memory::{MemoryHistoryLookup, MemoryHistoryStatus, MemoryId};
    let temp = TestPaths::new("history");
    let path = temp.path("owner.rel");
    let rel = Rel::create(&path).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let token = "h".repeat(48);
    let (adapter, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "manager",
        &token,
        [OwnerGrant::ManageRel(owner)],
    )])
    .install(&gateway)
    .unwrap();
    let management = adapter
        .authenticate(&token)
        .unwrap()
        .workspace_management(owner)
        .unwrap();
    let memory = management
        .create_memory(
            "A title".into(),
            "A body".into(),
            "fact".into(),
            "project".into(),
            42,
        )
        .unwrap();
    management.quiesce().unwrap();
    let reopened = Rel::open(&path).unwrap();
    let id = MemoryId(parse_hex(&memory.id));
    let history = reopened.memory_history(id).unwrap();
    let MemoryHistoryLookup::Found(history) = history else {
        panic!("missing published history");
    };
    assert!(matches!(history.status, MemoryHistoryStatus::Available));
    assert_eq!(history.history.revisions.len(), 1);
    assert!(
        !reopened
            .current_historical_proof()
            .unwrap()
            .digest
            .is_empty()
    );
}
#[test]
fn public_host_commits_conversation_and_management_reads_bounded_transcript() {
    use reliquary_memory::{ConversationSubmissionId, FinalizeCapture, QueueState, SubmissionId};
    let temp = TestPaths::new("conversation");
    let rel = Rel::create(temp.path("owner.rel")).unwrap();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true).unwrap();
    let token = "q".repeat(48);
    let (adapter, control) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "operator",
        &token,
        [OwnerGrant::OperateRel(owner), OwnerGrant::ManageRel(owner)],
    )])
    .install(&gateway)
    .unwrap();
    let instance = adapter.authenticate(&token).unwrap();
    let mut queue = instance
        .open_conversation_queue(temp.path("queue"), owner)
        .unwrap();
    let submission = SubmissionId::new("submission-1").unwrap();
    let id = ConversationSubmissionId::new("conversation-1", submission.clone()).unwrap();
    instance
        .submit_conversation(
            &mut queue,
            "conversation-1",
            submission,
            "visible committed content",
            0,
        )
        .unwrap();
    let status = instance
        .finalize_conversation_submission(
            &mut queue,
            &id,
            1,
            FinalizeCapture {
                branch_id: "main".into(),
                parent_node_id: None,
                committed_timestamp_ns: 42,
                verified_principal_id: "operator".into(),
            },
        )
        .unwrap();
    assert_eq!(status.state, QueueState::Committed);
    let management = instance.workspace_management(owner).unwrap();
    let page = management.conversations(0, 4).unwrap();
    assert_eq!(page.items[0].conversation_id, "conversation-1");
    management
        .select_conversation("conversation-1", "submission-1")
        .unwrap();
    let transcript = management.transcript(0, 4).unwrap();
    assert_eq!(transcript.items[0].content, "visible committed content");
    management
        .set_title("conversation-1", "Updated title")
        .unwrap();
    assert_eq!(
        management.conversations(0, 4).unwrap().items[0]
            .title
            .as_deref(),
        Some("Updated title")
    );
    assert!(management.conversations(0, usize::MAX).is_err());
    control.revoke(&token).unwrap();
    assert_eq!(
        management.transcript(0, 4).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
}
