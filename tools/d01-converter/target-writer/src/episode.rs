use crate::transfer::{decimal, decode_hex};
use reliquary_archive::{Episode, EpisodeBoundary, EpisodeId, EpisodeOrigin};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpisodeInput {
    id: String,
    conversation: String,
    start: String,
    end: String,
    origin: Origin,
    boundary: Boundary,
    source_through_ns: String,
    finalized_at_ns: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Origin {
    Live,
    Import,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Boundary {
    Size,
    Inactivity,
    CreateMemory,
    ImportEnd,
    Explicit,
}

impl TryFrom<EpisodeInput> for Episode {
    type Error = &'static str;

    fn try_from(input: EpisodeInput) -> Result<Self, Self::Error> {
        Ok(Episode {
            id: EpisodeId(decode_hex(&input.id)?),
            conversation_id: input.conversation,
            start_node_id: input.start,
            end_node_id: input.end,
            origin: match input.origin {
                Origin::Live => EpisodeOrigin::Live,
                Origin::Import => EpisodeOrigin::Import,
            },
            boundary: match input.boundary {
                Boundary::Size => EpisodeBoundary::Size,
                Boundary::Inactivity => EpisodeBoundary::Inactivity,
                Boundary::CreateMemory => EpisodeBoundary::CreateMemory,
                Boundary::ImportEnd => EpisodeBoundary::ImportEnd,
                Boundary::Explicit => EpisodeBoundary::Explicit,
            },
            source_through_ns: decimal(&input.source_through_ns)?,
            finalized_at_ns: decimal(&input.finalized_at_ns)?,
        })
    }
}
