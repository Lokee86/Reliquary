use super::*;

pub(super) enum Part {
    Text(String),
    Unsupported,
}

impl<'de> Deserialize<'de> for Part {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct TextPart;
        impl<'de> Visitor<'de> for TextPart {
            type Value = Part;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a ChatGPT content part")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Part, E> {
                Ok(Part::Text(value.to_owned()))
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Part, M::Error> {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Part::Unsupported)
            }

            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Part, S::Error> {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Part::Unsupported)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Part, E> {
                Ok(Part::Unsupported)
            }

            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Part, E> {
                Ok(Part::Unsupported)
            }

            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Part, E> {
                Ok(Part::Unsupported)
            }

            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Part, E> {
                Ok(Part::Unsupported)
            }

            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Part, E> {
                Ok(Part::Unsupported)
            }
        }
        decoder.deserialize_any(TextPart)
    }
}

pub(super) struct VisibleText {
    pub id: String,
    pub role: String,
    pub text: String,
    pub timestamp_ns: i64,
    pub assets: usize,
}

impl Message {
    pub(super) fn visible_text(
        self,
        coverage: &mut NativeImportCoverage,
    ) -> Result<Option<VisibleText>, NativeImportSourceError> {
        let metadata = self.metadata.unwrap_or_default();
        if metadata.is_visually_hidden_from_conversation == Some(true)
            || self.channel.as_deref() == Some("analysis")
        {
            coverage.omitted_hidden_messages += 1;
            return Ok(None);
        }
        let Some(timestamp) = &self.create_time else {
            coverage.omitted_missing_timestamps += 1;
            return Ok(None);
        };
        let timestamp_ns = timestamp_ns(timestamp.get())?;
        if coverage
            .window
            .is_some_and(|window| !window.includes(timestamp_ns))
        {
            coverage.omitted_outside_window += 1;
            return Ok(None);
        }
        let role = self.author.role;
        let mut assets = metadata.attachments.map_or(0, |items| items.len());
        let visible = matches!(role.as_str(), "user" | "assistant" | "system" | "developer")
            && matches!(self.channel.as_deref(), None | Some("final" | "commentary"))
            && matches!(self.recipient.as_deref(), None | Some("all"))
            && matches!(
                self.content.content_type.as_str(),
                "text" | "multimodal_text"
            );
        let mut text = Vec::new();
        for part in self.content.parts {
            match part {
                Part::Text(part) if visible => text.push(part),
                Part::Unsupported => assets += 1,
                Part::Text(_) => {}
            }
        }
        if text.is_empty() {
            coverage.omitted_unsupported_messages += 1;
            coverage.omitted_asset_references += assets;
            return Ok(None);
        }
        Ok(Some(VisibleText {
            id: self.id,
            role,
            text: text.join("\n"),
            timestamp_ns,
            assets,
        }))
    }
}

/// Convert the JSON decimal directly so binary float rounding cannot move a
/// source turn across a selection boundary or alter its recorded timestamp.
fn timestamp_ns(source: &str) -> Result<i64, NativeImportSourceError> {
    let error = || invalid("invalid ChatGPT timestamp or precision beyond Unix nanoseconds");
    if source.len() > 64 {
        return Err(error());
    }
    let (mantissa, exponent) = source.split_once(['e', 'E']).unwrap_or((source, "0"));
    let exponent = exponent.parse::<i32>().map_err(|_| error())?;
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let fraction = fraction.trim_end_matches('0');
    if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(error());
    }
    let units = format!("{whole}{fraction}")
        .parse::<i128>()
        .map_err(|_| error())?;
    if units == 0 {
        return Ok(0);
    }
    let power = exponent
        .checked_add(9 - fraction.len() as i32)
        .ok_or_else(error)?;
    if power < 0 {
        return Err(error());
    }
    let scale = 10_i128.checked_pow(power as u32).ok_or_else(error)?;
    units
        .checked_mul(scale)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(error)
}

#[cfg(test)]
#[test]
fn converts_source_seconds_without_binary_rounding() {
    assert_eq!(
        timestamp_ns("1780000000.123456789").unwrap(),
        1_780_000_000_123_456_789
    );
    assert_eq!(timestamp_ns("-1.200e-2").unwrap(), -12_000_000);
    assert!(timestamp_ns("1e-10").is_err());
    assert!(timestamp_ns("1e38").is_err());
    assert!(timestamp_ns("\"1\"").is_err());
}
