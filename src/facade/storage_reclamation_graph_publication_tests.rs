use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{GraphRelationKind, GraphRelationOrigin, SemanticGraphRelationKind};

#[test]
fn native_graph_relocation_and_scan_validate_shape_and_preserve_exact_payload() {
    let row = GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::User,
    };
    let envelope = |body: &[u8]| {
        publication_envelope::encode(PublicationEnvelope {
            global_version: 5,
            clock: PublicationClock::Allocate {
                transaction_time_ns: None,
            },
            body,
        })
        .unwrap()
    };
    let object = crate::ObjectRef::from_legacy_bytes([9; 16]);
    for transaction in [
        GraphTransaction::Single(row),
        GraphTransaction::Batch(vec![row]),
    ] {
        let body = publication::encode(&GraphPublication {
            graph_version: 1,
            transaction,
        })
        .unwrap();
        let mut orphan_scan = crate::maintenance::reclamation_scan::ReclamationScan::default();
        orphan_scan.ingest(object, &body).unwrap();
        let orphan_plan = orphan_scan.finish();
        assert_eq!(orphan_plan.unpublished_backing_chunks, 1);
        assert!(orphan_plan.drop.contains(&object));
        let bytes = envelope(&body);
        let mut scan = crate::maintenance::reclamation_scan::ReclamationScan::default();
        scan.ingest(object, &bytes).unwrap();
        assert_eq!(
            super::relocate_payload(&bytes, &Default::default()).unwrap(),
            bytes
        );
        let retained = {
            let mut scan = crate::maintenance::reclamation_scan::ReclamationScan::default();
            scan.ingest(object, &bytes).unwrap();
            scan.finish()
        };
        assert!(!retained.drop.contains(&object));
        assert_eq!(retained.unpublished_backing_chunks, 0);
        for end in 8..body.len() {
            let malformed = envelope(&body[..end]);
            assert!(scan.ingest(object, &malformed).is_err());
            assert!(super::relocate_payload(&malformed, &Default::default()).is_err());
        }
        let mut trailing = body.clone();
        trailing.push(0);
        assert!(scan.ingest(object, &envelope(&trailing)).is_err());
        assert!(super::relocate_payload(&envelope(&trailing), &Default::default()).is_err());
    }
    let foreign = envelope(b"FOREIGN1opaque");
    assert!(
        crate::maintenance::reclamation_scan::ReclamationScan::default()
            .ingest(object, &foreign)
            .is_err()
    );
    assert!(super::relocate_payload(&foreign, &Default::default()).is_err());
}
