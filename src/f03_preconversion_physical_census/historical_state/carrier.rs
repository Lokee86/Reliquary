//! Concrete carrier catalog for the six V01-bound source files.
use crate::FileKind;
use crate::container::{grouped_clock, publication_envelope};
use serde_json::{Value, json};

pub(super) fn describe(
    payload: &[u8],
    owner: FileKind,
) -> Result<(&'static str, &'static str, Value), String> {
    use publication_envelope::PublicationClock;
    if let Some(envelope) =
        publication_envelope::decode(payload).map_err(|_| "invalid publication envelope")?
    {
        let (body, group) = match envelope.body.get(..8) {
            Some(b"CVABRC02") => ("CVABRC02", "archive_echo"),
            Some(b"CVACON04") => ("CVACON04", "archive_echo"),
            Some(b"CVAEPI02") => ("CVAEPI02", "archive_echo"),
            Some(b"CVAFRA02") => ("CVAFRA02", "archive_echo"),
            Some(b"CVATUR03") => ("CVATUR03", "archive_echo"),
            Some(b"CVAENC02") => ("CVAENC02", "entity_relationship"),
            Some(b"CVAEND02") => ("CVAEND02", "entity_relationship"),
            Some(b"CVAETM02") => ("CVAETM02", "entity_relationship"),
            Some(b"CVAGCB02") => ("CVAGCB02", "graph_community"),
            Some(b"CVAGCM02") => ("CVAGCM02", "graph_community"),
            Some(b"CVAMC002") => ("CVAMC002", "memory"),
            Some(b"CVAMD002") => ("CVAMD002", "memory"),
            _ => return Err("unclassified publication body".into()),
        };
        if group == "archive_echo" && owner != FileKind::Reliquary {
            return Err("Archive publication in a non-REL owner".into());
        }
        let (clock, timestamp) = match envelope.clock {
            PublicationClock::Allocate {
                transaction_time_ns,
            } => ("allocate", transaction_time_ns.map(|v| v.to_string())),
            PublicationClock::BindReserved => ("bind_reserved", None),
        };
        return Ok((
            body,
            group,
            json!({
                "outer_carrier": "CVAPUB01",
                "global_version": envelope.global_version.to_string(),
                "clock": clock,
                "transaction_time_ns_in_envelope": timestamp,
            }),
        ));
    }

    let (carrier, group, rel_only) = match payload.get(..8) {
        Some(b"CVAVERS1") => ("CVAVERS1", "container_lineage", false),
        Some(b"CVAVERS2") => ("CVAVERS2", "container_lineage", false),
        Some(b"CVARELM1") => ("CVARELM1", "container_lineage", true),
        Some(b"CVAAFMT2") => ("CVAAFMT2", "archive_echo", true),
        Some(b"CVACONT1") => ("CVACONT1", "archive_echo", true),
        Some(b"CVACNAM1") => ("CVACNAM1", "graph_community", false),
        Some(b"CVACOMM1") => ("CVACOMM1", "graph_community", false),
        Some(b"CVAGFMT1") => ("CVAGFMT1", "graph_community", false),
        Some(b"CVAGNOD2") => ("CVAGNOD2", "graph_community", false),
        Some(b"CVAGNODE") => ("CVAGNODE", "graph_community", false),
        Some(b"CVAMBDY1") => ("CVAMBDY1", "memory", false),
        Some(b"CVAMEMF2") => ("CVAMEMF2", "memory", false),
        Some(b"CVAMRTE2") => ("CVAMRTE2", "memory", false),
        Some(b"CVAENTF1") => ("CVAENTF1", "entity_relationship", false),
        Some(b"CVAERL01") => ("CVAERL01", "entity_relationship", false),
        Some(b"CVAERS03") => ("CVAERS03", "entity_relationship", false),
        Some(b"CVAFRP01") => ("CVAFRP01", "freshness", true),
        Some(b"CVAFRS02") => ("CVAFRS02", "freshness", true),
        Some(b"CVAFRS07") => ("CVAFRS07", "freshness", true),
        Some(b"CVADRM01") => ("CVADRM01", "insomnia_dream_import", false),
        Some(b"CVADRP02") => ("CVADRP02", "insomnia_dream_import", false),
        Some(b"CVDREAM1") => ("CVDREAM1", "insomnia_dream_import", true),
        Some(b"CVDREAM2") => ("CVDREAM2", "insomnia_dream_import", true),
        Some(b"CVAINSC8") => ("CVAINSC8", "insomnia_dream_import", true),
        Some(b"CVAINSF1") => ("CVAINSF1", "insomnia_dream_import", true),
        Some(b"CVAINSA1") => ("CVAINSA1", "insomnia_dream_import", true),
        Some(b"CVAIEPS1") => ("CVAIEPS1", "insomnia_dream_import", true),
        Some(b"CVAIEPC1") => ("CVAIEPC1", "insomnia_dream_import", true),
        Some(b"CVAINSW1") => ("CVAINSW1", "insomnia_dream_import", true),
        Some(b"CVAAVEC1") => ("CVAAVEC1", "vectors", true),
        Some(b"CVAAVFM1") => ("CVAAVFM1", "vectors", true),
        Some(b"CVACPFM1") => ("CVACPFM1", "vectors", false),
        Some(b"CVACPRO1") => ("CVACPRO1", "vectors", false),
        Some(b"CVACPRT1") => ("CVACPRT1", "vectors", false),
        Some(b"CVAMVEC1") => ("CVAMVEC1", "vectors", false),
        Some(b"CVAMVFM1") => ("CVAMVFM1", "vectors", false),
        Some(b"CVAPVEC1") => ("CVAPVEC1", "vectors", false),
        Some(b"CVAPVFM1") => ("CVAPVFM1", "vectors", false),
        Some(b"CVAVGEN2") => ("CVAVGEN2", "vectors", true),
        Some(b"CVAVGFM2") => ("CVAVGFM2", "vectors", true),
        Some(b"CVAVGRC2") => ("CVAVGRC2", "vectors", true),
        Some(b"PRJCOR02") => ("PRJCOR02", "external_project_operational", true),
        Some(b"PRJFILE1") => ("PRJFILE1", "external_project_operational", true),
        _ => return Err("unclassified top-level carrier".into()),
    };
    if rel_only && owner != FileKind::Reliquary {
        return Err("REL-only carrier in a non-REL owner".into());
    }
    let details = grouped_clock::insomnia_range(payload)
        .map_err(|_| "invalid grouped publication clock")?
        .map(|(start, count, time)| {
            json!({
                "global_start": start.to_string(),
                "global_count": count.to_string(),
                "transaction_time_ns": time.map(|v| v.to_string()),
            })
        })
        .unwrap_or(Value::Null);
    Ok((carrier, group, details))
}
