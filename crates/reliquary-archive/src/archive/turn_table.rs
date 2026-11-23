use super::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::num::NonZeroU64;

#[derive(Deserialize, Serialize)]
pub(super) struct AcceptedTurn {
    pub node: Node,
    pub archive_version: u64,
    pub activity: Option<NonZeroU64>,
    pub attachments: Box<[SourceAttachment]>,
    pub prune: Option<Box<[u8; 32]>>,
}

/// Accepted turn order and details have one home. The borrowed lookup
/// indexes positions in that table and can be rebuilt from a checkpoint.
#[derive(Default)]
pub(super) struct TurnTable {
    ordered: Vec<AcceptedTurn>,
    by_conversation: HashMap<Box<str>, HashMap<Box<str>, usize>>,
}

impl TurnTable {
    pub fn len(&self) -> usize {
        self.ordered.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &AcceptedTurn> {
        self.ordered.iter()
    }

    pub fn contains_conversation(&self, conversation: &str) -> bool {
        self.by_conversation.contains_key(conversation)
    }

    pub fn get(&self, conversation: &str, id: &str) -> Option<&AcceptedTurn> {
        let index = *self.by_conversation.get(conversation)?.get(id)?;
        self.ordered.get(index)
    }

    pub fn reserve(&mut self, conversation: &str) -> Result<(), ArchiveError> {
        self.ordered
            .try_reserve(1)
            .map_err(|_| ArchiveError::IndexCapacityExhausted)?;
        if let Some(nodes) = self.by_conversation.get_mut(conversation) {
            nodes.try_reserve(1)
        } else {
            self.by_conversation.try_reserve(1)
        }
        .map_err(|_| ArchiveError::IndexCapacityExhausted)
    }

    pub fn push(&mut self, turn: AcceptedTurn) {
        let previous = Self::index(&mut self.by_conversation, &turn.node, self.ordered.len());
        debug_assert!(previous.is_none());
        self.ordered.push(turn);
    }

    fn index(
        conversations: &mut HashMap<Box<str>, HashMap<Box<str>, usize>>,
        node: &Node,
        position: usize,
    ) -> Option<usize> {
        if let Some(nodes) = conversations.get_mut(node.conversation_id.as_str()) {
            nodes.insert(node.id.as_str().into(), position)
        } else {
            conversations.insert(
                node.conversation_id.as_str().into(),
                HashMap::from([(node.id.as_str().into(), position)]),
            );
            None
        }
    }
}

impl Serialize for TurnTable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.ordered.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TurnTable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ordered = Vec::<AcceptedTurn>::deserialize(deserializer)?;
        let mut by_conversation = HashMap::new();
        let mut previous_version = 0;
        for (position, record) in ordered.iter().enumerate() {
            if record.archive_version <= previous_version
                || Self::index(&mut by_conversation, &record.node, position).is_some()
            {
                return Err(serde::de::Error::custom(
                    "invalid accepted Archive turn order",
                ));
            }
            previous_version = record.archive_version;
        }
        Ok(Self {
            ordered,
            by_conversation,
        })
    }
}
