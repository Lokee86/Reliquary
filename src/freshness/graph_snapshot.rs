use crate::graph_codec::GraphVersionPayload;
use crate::{GraphError, ObjectRef};

pub(crate) fn publication_version(
    object: ObjectRef,
    payload: &[u8],
) -> Result<Option<GraphVersionPayload>, GraphError> {
    crate::graph_compact_codec::publication::version_at(object, payload)
}
