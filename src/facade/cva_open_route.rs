#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenRecordRoute {
    Archive,
    Memory,
    Graph,
    ContainerVersion,
    Fallback,
}

pub(crate) fn classify_open_record(payload: &[u8]) -> OpenRecordRoute {
    let Some(magic) = payload.get(..8) else {
        return OpenRecordRoute::Fallback;
    };

    match magic {
        b"CVACONT1" | b"CVANODE1" | b"CVABRCH1" | b"CVAFRAG1" | b"CVAFILE1" | b"CVAAFMT2"
        | b"CVAAREC1" | b"CVACONV1" | b"CVACONV2" | b"CVAEPIS1" | b"CVAFMEM1" | b"CVATURN1" => {
            OpenRecordRoute::Archive
        }

        b"CVAMEMF2" | b"CVAMBDY1" | b"CVAMEMR2" | b"CVAMEMR3" | b"CVAMEMR4" | b"CVAMEMR5"
        | b"CVAMEMR6" | b"CVAMEMR7" | b"CVAMEMV1" | b"CVAMRTE1" | b"CVAMRTE2" => {
            OpenRecordRoute::Memory
        }

        b"CVAGFMT1" | b"CVAGNODE" | b"CVAGNOD2" | b"CVAGVER1" | b"CVAGMUT1" | b"CVAGMUT2"
        | b"CVAGBAT1" | b"CVAGBAT2" => OpenRecordRoute::Graph,

        b"CVAVERS1" | b"CVAVERS2" => OpenRecordRoute::ContainerVersion,

        _ => OpenRecordRoute::Fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenRecordRoute, classify_open_record};

    #[test]
    fn routes_known_high_volume_record_families() {
        let cases = [
            (b"CVACONT1".as_slice(), OpenRecordRoute::Archive),
            (b"CVAAREC1".as_slice(), OpenRecordRoute::Archive),
            (b"CVATURN1".as_slice(), OpenRecordRoute::Archive),
            (b"CVAMEMF2".as_slice(), OpenRecordRoute::Memory),
            (b"CVAMEMR7".as_slice(), OpenRecordRoute::Memory),
            (b"CVAMRTE2".as_slice(), OpenRecordRoute::Memory),
            (b"CVAGFMT1".as_slice(), OpenRecordRoute::Graph),
            (b"CVAGBAT2".as_slice(), OpenRecordRoute::Graph),
            (b"CVAVERS2".as_slice(), OpenRecordRoute::ContainerVersion),
        ];

        for (payload, expected) in cases {
            assert_eq!(classify_open_record(payload), expected);
        }
    }

    #[test]
    fn insomnia_completion_and_unknown_records_stay_on_fallback() {
        assert_eq!(
            classify_open_record(b"CVAINSC5payload"),
            OpenRecordRoute::Fallback
        );
        assert_eq!(
            classify_open_record(b"CVAPEP01payload"),
            OpenRecordRoute::Fallback
        );
        assert_eq!(
            classify_open_record(b"CVACMP1\0payload"),
            OpenRecordRoute::Fallback
        );
        assert_eq!(
            classify_open_record(b"UNKNOWN!payload"),
            OpenRecordRoute::Fallback
        );
        assert_eq!(classify_open_record(b"tiny"), OpenRecordRoute::Fallback);
    }
}
