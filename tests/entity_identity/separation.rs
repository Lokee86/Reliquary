use super::*;
use reliquary_memory::{
    EntityReviewAuthority, HostAdmissionAdapter, HostAdmissionControl, IdentityEffectiveDecision,
    IdentityPair, IdentitySeparationInspection, IdentitySeparationRequest as Request,
    IdentitySeparationReview, IdentitySeparationState as State,
};

const FIRST: &str = "first-reviewer-credential-for-separation";
const SECOND: &str = "second-reviewer-credential-for-separation";

struct Pair {
    gateway: Gateway,
    a: OwnerId,
    b: OwnerId,
    pair: IdentityPair,
    ac: EntityId,
    bc: EntityId,
    dir: Directory,
}

impl Pair {
    fn new(write_peer: bool) -> Self {
        let dir = Directory::new();
        let ap = dir.0.join("a.rel");
        let bp = dir.0.join("b.rel");
        let (ae, ac) = seed(&ap, [41; 16]);
        let (be, bc) = seed(&bp, [42; 16]);
        let gateway = Gateway::new();
        let a = gateway.mount_rel(&Rel::open(ap).unwrap(), true).unwrap();
        let b = gateway
            .mount_rel(&Rel::open(bp).unwrap(), write_peer)
            .unwrap();
        let pair = IdentityPair {
            local_owner: a.as_bytes(),
            local_entity: ae,
            peer_owner: b.as_bytes(),
            peer_entity: be,
        };
        Self {
            gateway,
            a,
            b,
            pair,
            ac,
            bc,
            dir,
        }
    }

    fn install(
        &self,
        extra: Vec<HostCredentialBinding>,
        operators: &[&str],
    ) -> (
        HostAdmissionAdapter,
        HostAdmissionControl,
        EntityReviewAuthority,
    ) {
        let mut bindings = vec![
            HostCredentialBinding::new(
                "first",
                FIRST,
                [OwnerGrant::ManageRel(self.a), OwnerGrant::ManageRel(self.b)],
            ),
            HostCredentialBinding::new(
                "second",
                SECOND,
                [OwnerGrant::ManageRel(self.a), OwnerGrant::ViewRel(self.b)],
            ),
        ];
        bindings.extend(extra);
        TrustedHostAdmission::new(bindings)
            .install_with_entity_review(&self.gateway, operators)
            .unwrap()
    }

    fn compare(&self, work: &WorkspaceManagement) -> reliquary_memory::IdentityReceipt {
        work.compare_entity_identity(self.pair.local_entity, self.b, self.pair.peer_entity)
            .unwrap()
    }

    fn audit(
        &self,
        work: &WorkspaceManagement,
        revision: Option<u64>,
    ) -> IdentitySeparationInspection {
        work.inspect_entity_separation(
            self.pair.local_entity,
            self.b,
            self.pair.peer_entity,
            revision,
        )
        .unwrap()
        .unwrap()
    }

    fn inspect(&self, work: &WorkspaceManagement) -> reliquary_memory::EntityIdentityInspection {
        work.inspect_entity_identity(self.pair.local_entity, self.b, self.pair.peer_entity, None)
            .unwrap()
            .unwrap()
    }
}

#[test]
fn confirmed_pair_keeps_the_worker_judgment_and_undo_restores_it() {
    let f = Pair::new(true);
    let (admission, _, authority) = f.install(vec![], &["first", "second"]);
    let first = admission.authenticate(FIRST).unwrap();
    let second = admission.authenticate(SECOND).unwrap();
    let user = authority.bind(&first).unwrap();
    let other = authority.bind(&second).unwrap();
    let work = first.workspace_management(f.a).unwrap();
    let peer = first.workspace_management(f.b).unwrap();
    let script = Arc::new(Script::default());
    f.gateway
        .set_runtime(Some(&runtime(script.clone())))
        .unwrap();
    evidence(&work, f.pair.local_entity, f.ac);
    assert_eq!(f.compare(&work).outcome, IdentityOutcome::Unresolved);
    let first_action = user.prepare(f.pair, Request::Confirm).unwrap();
    let id = first_action.preview().action_id;
    assert_eq!(user.commit(&first_action).unwrap().state, State::Applies);

    evidence(&peer, f.pair.peer_entity, f.bc);
    wait_revision(&work, f.pair.local_entity, f.b, f.pair.peer_entity, 2);
    let revised = f.inspect(&work);
    assert_eq!(revised.receipt.outcome, IdentityOutcome::SameIdentity);
    assert_eq!(
        revised.effective,
        IdentityEffectiveDecision::ConfirmedSeparate
    );
    let later = user.prepare(f.pair, Request::Confirm).unwrap();
    assert!(matches!(
        later.preview().review,
        IdentitySeparationReview::Confirm {
            disagrees: true,
            ..
        }
    ));
    let stale_undo = other
        .prepare(f.pair, Request::Undo { target_revision: 1 })
        .unwrap();
    let calls = script.calls.load(Ordering::SeqCst);
    assert_eq!(user.commit(&later).unwrap().event.revision, 2);
    assert_eq!(
        other.commit(&stale_undo).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    assert_eq!(user.commit(&first_action).unwrap().state, State::Historical);
    let undo = other
        .prepare(f.pair, Request::Undo { target_revision: 2 })
        .unwrap();
    assert!(
        matches!(&undo.preview().review, IdentitySeparationReview::Undo { confirmation } if confirmation.operator_id == "first")
    );
    let withdrawn = other.commit(&undo).unwrap();
    assert_eq!(withdrawn.state, State::Withdrawn);
    assert_eq!(withdrawn.event.operator_id, "second");
    assert_eq!(withdrawn.withdrawn_confirmation.unwrap().revision, 2);
    work.quiesce().unwrap();
    work.reopen().unwrap();
    assert_eq!(
        f.inspect(&work).effective,
        IdentityEffectiveDecision::Evaluated(IdentityOutcome::SameIdentity)
    );
    assert_eq!(f.audit(&work, Some(1)).state, State::Historical);
    assert_eq!(f.audit(&work, Some(2)).event.operator_id, "first");
    assert_eq!(f.audit(&work, None).event.operator_id, "second");
    let reconnect = admission.authenticate(FIRST).unwrap();
    let recovered = authority
        .bind(&reconnect)
        .unwrap()
        .recover(f.pair, id)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.state, State::Historical);
    assert_eq!(recovered.event.operator_id, "first");
    let renewed = user.prepare(f.pair, Request::Confirm).unwrap();
    let revision = user.commit(&renewed).unwrap().event.revision;
    f.gateway.set_runtime(None).unwrap();
    let pending = f.inspect(&work);
    assert_eq!(pending.state, IdentityComparisonState::Pending);
    assert_eq!(
        pending.effective,
        IdentityEffectiveDecision::ConfirmedSeparate
    );
    let undo = other
        .prepare(
            f.pair,
            Request::Undo {
                target_revision: revision,
            },
        )
        .unwrap();
    other.commit(&undo).unwrap();
    assert_eq!(
        f.inspect(&work).effective,
        IdentityEffectiveDecision::Pending
    );
    assert_eq!(script.calls.load(Ordering::SeqCst), calls);
}

#[test]
fn operator_approval_is_session_bound_revocable_and_requires_both_grants() {
    let f = Pair::new(false);
    let peer_before = fs::read(f.dir.0.join("b.rel")).unwrap();
    const VIEW: &str = "separation-both-read-only-credential";
    const LOCAL: &str = "separation-no-peer-grant-credential";
    let (admission, control, authority) = f.install(
        vec![
            HostCredentialBinding::new(
                "view",
                VIEW,
                [OwnerGrant::ViewRel(f.a), OwnerGrant::ViewRel(f.b)],
            ),
            HostCredentialBinding::new("local", LOCAL, [OwnerGrant::ManageRel(f.a)]),
        ],
        &["first", "view", "local"],
    );
    let manager = admission.authenticate(FIRST).unwrap();
    let user = authority.bind(&manager).unwrap();
    let work = manager.workspace_management(f.a).unwrap();
    f.compare(&work);
    let prepared = user.prepare(f.pair, Request::Confirm).unwrap();
    let id = prepared.preview().action_id;
    let ordinary = admission.authenticate(SECOND).unwrap();
    assert!(
        matches!(authority.bind(&ordinary), Err(e) if e.kind() == ProductErrorKind::Unauthorized)
    );
    let (other_admission, _, _) = f.install(vec![], &["first"]);
    let foreign = other_admission.authenticate(FIRST).unwrap();
    assert!(
        matches!(authority.bind(&foreign), Err(e) if e.kind() == ProductErrorKind::Unauthorized)
    );
    let new_login = admission.authenticate(FIRST).unwrap();
    assert!(
        authority
            .bind(&new_login)
            .unwrap()
            .commit(&prepared)
            .is_err()
    );
    for token in [VIEW, LOCAL] {
        let limited = admission.authenticate(token).unwrap();
        let reviewer = authority.bind(&limited).unwrap();
        assert!(
            matches!(reviewer.prepare(f.pair, Request::Confirm), Err(e) if e.kind() == ProductErrorKind::Unauthorized)
        );
        assert_eq!(
            reviewer.recover(f.pair, id).unwrap_err().kind(),
            ProductErrorKind::Unauthorized
        );
    }
    work.create_memory(
        "Other".into(),
        "Unrelated write".into(),
        "fact".into(),
        "note".into(),
        44,
    )
    .unwrap();
    let (one, two) = std::thread::scope(|s| {
        let one = s.spawn(|| user.commit(&prepared).unwrap());
        let two = s.spawn(|| user.commit(&prepared).unwrap());
        (one.join().unwrap(), two.join().unwrap())
    });
    assert_eq!(one.event, two.event);
    assert_eq!(one.event.revision, 1);
    assert!(
        work.inspect_entity_separation(f.pair.local_entity, f.b, f.pair.peer_entity, Some(2))
            .unwrap()
            .is_none()
    );
    let limited = admission
        .authenticate(LOCAL)
        .unwrap()
        .workspace_management(f.a)
        .unwrap();
    assert_eq!(
        limited
            .inspect_entity_separation(f.pair.local_entity, f.b, f.pair.peer_entity, None)
            .unwrap_err()
            .kind(),
        ProductErrorKind::Unauthorized
    );
    let undo = user
        .prepare(f.pair, Request::Undo { target_revision: 1 })
        .unwrap();
    control.revoke(FIRST).unwrap();
    assert_eq!(
        user.commit(&undo).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
    assert_eq!(
        user.commit(&prepared).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
    assert_eq!(
        user.recover(f.pair, id).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
    assert_eq!(fs::read(f.dir.0.join("b.rel")).unwrap(), peer_before);
}

#[test]
fn review_checks_the_displayed_receipt_and_the_reviewed_peer_history() {
    let f = Pair::new(false);
    let (admission, _, authority) = f.install(vec![], &["first"]);
    let first = admission.authenticate(FIRST).unwrap();
    let user = authority.bind(&first).unwrap();
    let work = first.workspace_management(f.a).unwrap();
    let peer = first.workspace_management(f.b).unwrap();
    f.compare(&work);
    let before_evidence = user.prepare(f.pair, Request::Confirm).unwrap();
    evidence(&work, f.pair.local_entity, f.ac);
    assert_eq!(f.compare(&work).revision, 2);
    assert_eq!(
        user.commit(&before_evidence).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    let before_remount = user.prepare(f.pair, Request::Confirm).unwrap();
    peer.quiesce().unwrap();
    peer.reopen().unwrap();
    assert_eq!(
        user.commit(&before_remount).unwrap_err().kind(),
        ProductErrorKind::Stale
    );
    let approved = user.prepare(f.pair, Request::Confirm).unwrap();
    user.commit(&approved).unwrap();
    peer.quiesce().unwrap();
    peer.reopen().unwrap();
    assert_eq!(f.audit(&work, None).state, State::Applies);

    // Same owner and Entity ID; the original reviewed Entity revision is absent.
    let replacement = f.dir.0.join("replacement.rel");
    let (replacement_id, _) = seed_entities(
        &replacement,
        f.b.as_bytes(),
        [
            ("person", PERSON, "application"),
            ("contract", CONTRACT, "artifact"),
        ],
    );
    assert_eq!(replacement_id, f.pair.peer_entity);
    peer.quiesce().unwrap();
    f.gateway
        .mount_rel(&Rel::open(replacement).unwrap(), false)
        .unwrap();
    assert_eq!(f.audit(&work, None).state, State::Historical);
    assert_ne!(
        f.inspect(&work).effective,
        IdentityEffectiveDecision::ConfirmedSeparate
    );
    let undo = user
        .prepare(f.pair, Request::Undo { target_revision: 1 })
        .unwrap();
    assert_eq!(user.commit(&undo).unwrap().state, State::Withdrawn);
    work.quiesce().unwrap();
    f.gateway
        .mount_rel(&Rel::open(f.dir.0.join("a.rel")).unwrap(), false)
        .unwrap();
    assert_eq!(
        user.prepare(f.pair, Request::Confirm).unwrap_err().kind(),
        ProductErrorKind::WriterUnavailable
    );
}
