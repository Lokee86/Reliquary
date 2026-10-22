//! Literal current Begin fixtures, separate from physical-prefix writer capture.
pub(crate) fn explicit(owner: [u8; 16], pass: &crate::InitialDreamPass) -> Vec<u8> {
    crate::dream_begin_codec::encode(&crate::dream_begin_codec::BeginRecord {
        owner_uuid: owner,
        pass_id: pass.pass_id.clone(),
        source_id: pass.source_id,
        turn: pass.turn,
        graph_version_before: pass.graph_version_before,
        population: crate::dream_begin_codec::BeginPopulation::ExplicitSet(
            pass.preexisting_ids.clone(),
        ),
    })
    .unwrap()
}
