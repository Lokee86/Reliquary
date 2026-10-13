//! Disposable N5 proposals derived from immutable accepted first-publication proof.
use super::*;

pub(super) fn inspect(cva: &mut Cva) -> Value {
    let ids: BTreeSet<_> = cva.memory_ids().into_iter().collect();
    let mut recipes = BTreeMap::<String, Value>::new();
    let mut conflicts = BTreeSet::new();
    let mut scan = crate::Container::open_read_only_for_scan(cva.container.path()).unwrap();
    scan.visit_payloads::<CvaError>(|_, payload| {
        let Some(completion) = crate::insomnia::completion::decode_completion(payload).map_err(|e|fail(e))? else {
            return Ok(());
        };
        let Some(episode) = cva.episode(completion.episode_id).cloned() else { return Ok(()); };
        if episode.origin != crate::EpisodeOrigin::Import { return Ok(()); }
        let turns = cva.episode_turns(episode.id)?;
        let position = turns.iter().filter_map(|t|cva.activity_position_for_turn(&episode.conversation_id,&t.node_id)).max();
        for record in completion.records.iter().filter(|r|r.revision==1 && ids.contains(&r.id)) {
            let key = hex(&record.id.0);
            let first = cva.memory_revision(record.id,1)?;
            if cva.memories.first_mutation_id(record.id) != Some(record.mutation_id.as_str()) { continue; }
            if first.source_episode_id != Some(episode.id) || record.source_episode_id != Some(episode.id)
                || crate::memory_model::memory_body_id(&first.title,&first.content) != record.body_id
                || first.source_node_id != record.source_node_id
                || record.source_node_id.as_ref().is_some_and(|node| !turns.iter().any(|t|t.node_id==*node))
                || position.is_none() {
                conflicts.insert(key.clone()); recipes.remove(&key); continue;
            }
            if conflicts.contains(&key) { continue; }
            let recipe = json!({"episode":hex(&episode.id.0),"mutation_id":record.mutation_id,
                "first_revision":proof(&first),"body":hex(&record.body_id.0),"availability":position});
            if recipes.get(&key).is_some_and(|old|old!=&recipe) {
                conflicts.insert(key.clone()); recipes.remove(&key);
            } else { recipes.insert(key,recipe); }
        }
        Ok(())
    }).unwrap();
    json!({"proven":recipes,"conflicting":conflicts})
}
