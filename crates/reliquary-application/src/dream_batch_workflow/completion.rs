use super::*;
#[test]
fn uncertain_frontier_withholds_every_effect_and_cannot_be_settled_as_empty() {
    for rel in [true, false] {
        let (path, owner, source, candidates) = fixture(rel);
        begin(&path, owner, rel, source, candidates.clone());
        let ids: Vec<_> = candidates.iter().map(|c| c.id).collect();
        let held = draft(
            &path,
            owner,
            rel,
            source.id,
            &ids,
            "duplicate_of",
            "uncertain",
        );
        assert_eq!(held.pending_pairs().len(), 2);
        let before = fs::read(&path.0).unwrap();
        assert_eq!(
            publish(&path, owner, rel, [32; 32], &held),
            Err(DreamWorkflowError::IncompleteFrontier)
        );
        assert!(settle(&path, owner, rel, source.id).is_err());
        assert_eq!(before, fs::read(&path.0).unwrap());
        let accepted = draft(&path, owner, rel, source.id, &ids, "duplicate_of", "yes");
        assert!(publish(&path, owner, rel, [32; 32], &accepted).unwrap());
        assert!(settle(&path, owner, rel, source.id).unwrap());
    }
}
#[test]
fn verified_rejection_is_a_terminal_receipt_without_a_fabricated_relation() {
    for rel in [true, false] {
        let (path, owner, source, candidates) = fixture(rel);
        begin(&path, owner, rel, source, candidates.clone());
        let rejected = draft(
            &path,
            owner,
            rel,
            source.id,
            &candidates.iter().map(|c| c.id).collect::<Vec<_>>(),
            "duplicate_of",
            "no",
        );
        assert!(rejected.pending_pairs().is_empty());
        assert!(publish(&path, owner, rel, [33; 32], &rejected).unwrap());
        assert!(settle(&path, owner, rel, source.id).unwrap());
        if rel {
            assert_eq!(
                RelSession::open_path(&path.0, &graph(owner), [], [])
                    .unwrap()
                    .graph_version(),
                0
            );
        } else {
            assert_eq!(
                PhySession::open_path(&path.0, &graph(owner), [])
                    .unwrap()
                    .graph_version(),
                0
            );
        }
    }
}

struct NoModel;
impl GeneralEndpoint for NoModel {
    fn model(&self) -> &str {
        "must-not-run"
    }
    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        panic!("an empty frontier must not invoke a model")
    }
}
#[test]
fn source_only_frontier_needs_no_embedding_or_model_and_reopens_without_volatile_admission() {
    for rel in [true, false] {
        let (path, owner, source, _) = fixture(rel);
        let prepared = {
            let r;
            let p;
            let view = if rel {
                r = RelSession::open_path(&path.0, &graph(owner), [], []).unwrap();
                EntityOwnerSession::Rel(&r)
            } else {
                p = PhySession::open_path(&path.0, &graph(owner), []).unwrap();
                EntityOwnerSession::Phy(&p)
            };
            prepare_explicit_dream_frontier(&view, owner, source.id, &[]).unwrap()
        };
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            begin_prepared_dream_frontier(
                &mut DreamOwnerWriter::Rel(&mut w),
                owner,
                "initial",
                &prepared,
                501,
            )
            .unwrap();
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            begin_prepared_dream_frontier(
                &mut DreamOwnerWriter::Phy(&mut w),
                owner,
                "initial",
                &prepared,
                501,
            )
            .unwrap();
        }
        let proposal = {
            let r;
            let p;
            let view = if rel {
                r = RelSession::open_path(&path.0, &graph(owner), [], []).unwrap();
                EntityOwnerSession::Rel(&r)
            } else {
                p = PhySession::open_path(&path.0, &graph(owner), []).unwrap();
                EntityOwnerSession::Phy(&p)
            };
            draft_dream_batch_organization(
                &view,
                owner,
                source.id,
                &[],
                &DreamClassifier::new(NoModel),
                None::<&DreamVerifier<NoModel>>,
                DreamVerificationPolicy::default(),
            )
            .unwrap()
        };
        assert!(proposal.classifications().is_empty());
        assert_eq!(proposal.plan().lifecycle_intents.len(), 1);
        assert!(publish(&path, owner, rel, [35; 32], &proposal).unwrap());
        assert!(settle(&path, owner, rel, source.id).unwrap());
        let before = fs::read(&path.0).unwrap();
        if rel {
            let mut w = RelWriter::open_path(&path.0, graph(owner), [], []).unwrap();
            begin_prepared_dream_frontier(
                &mut DreamOwnerWriter::Rel(&mut w),
                owner,
                "initial",
                &prepared,
                501,
            )
            .unwrap();
        } else {
            let mut w = PhyWriter::open_path(&path.0, &graph(owner), []).unwrap();
            begin_prepared_dream_frontier(
                &mut DreamOwnerWriter::Phy(&mut w),
                owner,
                "initial",
                &prepared,
                501,
            )
            .unwrap();
        }
        assert!(!settle(&path, owner, rel, source.id).unwrap());
        assert_eq!(before, fs::read(&path.0).unwrap());
    }
}
