//! Local redaction for Archive embedding documents and queries.

use regex::{Match, Regex, RegexSet};
use std::{borrow::Cow, net::IpAddr, ops::Range, sync::LazyLock};

const ASSIGNED_VALUE: &str = r#"
    "(?:[^"\r\n\\]|\\.)*" | '(?:[^'\r\n\\]|\\.)*' | [^\s"'&,;}\])<>]+
"#;

const NAME: &str = r"[\p{L}\p{M}][\p{L}\p{M}'’.-]*";
const TYPED_DATE: &str =
    r"(?:[0-9]{4}[-/.][0-9]{1,2}[-/.][0-9]{1,2}|[0-9]{1,2}[-/.][0-9]{1,2}[-/.][0-9]{2,4})";

struct Rule {
    pattern: Regex,
    label: &'static str,
    accepts: fn(&str, Match<'_>) -> bool,
}

impl Rule {
    fn new(pattern: &str, label: &'static str) -> Self {
        Self::checked(pattern, label, |_, _| true)
    }

    fn checked(pattern: &str, label: &'static str, accepts: fn(&str, Match<'_>) -> bool) -> Self {
        Self {
            pattern: Regex::new(pattern).unwrap_or_else(|error| {
                panic!("invalid Archive redaction pattern for {label}: {error}")
            }),
            label,
            accepts,
        }
    }

    fn field(name: &str, value: &str, label: &'static str) -> Self {
        Self::new(
            &format!(r#"(?x)\b(?i:{name})["']?[\x20\t]*[:=][\x20\t]*(?P<value>{value})"#),
            label,
        )
    }
}

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    vec![
        // Scan the whole key block, including a key copied without its closing marker.
        Rule::new(
            r"(?s)-----BEGIN (?:[A-Z0-9]+[\x20-])*PRIVATE KEY(?: BLOCK)?-----.*?(?:-----END (?:[A-Z0-9]+[\x20-])*PRIVATE KEY(?: BLOCK)?-----|\z)",
            "[PRIVATE_KEY]",
        ),
        Rule::new(
            r"(?x)
                \b(?:
                    sk-[A-Za-z0-9_-]{20,}
                    | gh[pousr]_[A-Za-z0-9]{24,}
                    | github_pat_[A-Za-z0-9_]{26,}
                    | gl(?:pat|rt|dt|cbt)-[A-Za-z0-9_-]{16,}
                    | (?:AKIA|ASIA)[A-Z0-9]{16}
                    | xox[aboprs]-[A-Za-z0-9-]{12,}
                    | AIza[A-Za-z0-9_-]{30,}
                    | (?:sk|rk)_(?:live|test)_[A-Za-z0-9]{12,}
                    | (?:npm_|hf_|pypi-|xai-|dapi|sbp_)[A-Za-z0-9_-]{20,}
                    | ya29\.[A-Za-z0-9_-]{20,}
                    | SG\.[A-Za-z0-9_-]{14,}\.[A-Za-z0-9_-]{20,}
                )\b
            ",
            "[API_KEY]",
        ),
        Rule::new(
            r"\beyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*(?:\.[A-Za-z0-9_-]+){0,2}",
            "[TOKEN]",
        ),
        Rule::new(
            r#"(?ix)
                \bauthorization["']?[\x20\t]*[:=][\x20\t]*["']?
                (?:basic|bearer|token)[\x20\t]+(?P<value>[A-Za-z0-9._~+/=-]+)
            "#,
            "[TOKEN]",
        ),
        Rule::new(
            r"(?i)\bbearer[\x20\t]+(?P<value>[A-Za-z0-9._~+/=-]{20,})",
            "[TOKEN]",
        ),
        Rule::new(r"\b(?:bot)?[0-9]{8,12}:[A-Za-z0-9_-]{30,}\b", "[TOKEN]"),
        Rule::new(
            r#"(?i)\b[a-z][a-z0-9+.-]*://(?P<value>[^/\s:'"@<>,]+:[^/\s'"@<>,]+)@"#,
            "[CREDENTIALS]",
        ),
        Rule::new(
            r#"(?im)\b(?:set-cookie|cookie)["']?[\x20\t]*:[\x20\t]*["']?(?P<value>[A-Za-z0-9_.-]+=[^\r\n"']+)"#,
            "[COOKIE]",
        ),
        Rule::new(
            r#"(?ix)
                [?\x23&](?:api[-_]?key|key|access[-_]?token|refresh[-_]?token|token|password|secret
                    | sig(?:nature)?|x-amz-(?:credential|signature|security-token)|x-goog-(?:credential|signature))
                =(?P<value>[^\s"'&,;<>\])\x23]+)
            "#,
            "[SECRET]",
        ),
        Rule::new(
            &format!(
                r#"(?ix)
                    \b(?:[a-z0-9]+[_.-])*
                    (?:api[\x20_-]?key|access[\x20_-]?key(?:[_-]?id)?|(?:client|signing|secret)[\x20_-]?(?:key|secret)
                        | (?:access|refresh|auth|session|oauth|bearer|bot|id)[_-]?token|token
                        | password|passwd|passphrase|credentials?|secret|private[_-]?key|connection[_-]?string)
                    ["']?[\x20\t]*[:=][\x20\t]*(?P<value>{ASSIGNED_VALUE})
                "#
            ),
            "[SECRET]",
        ),
        Rule::new(
            &format!(
                r"(?ix)(?:^|[\x20\t])--(?:api-key|token|password|passwd|client-secret|access-token|secret-key)
                    (?:=|[\x20\t]+)(?P<value>{ASSIGNED_VALUE})"
            ),
            "[SECRET]",
        ),
        Rule::new(
            r#"(?imx)^[\x20\t]*(?:[-*][\x20\t]+)?(?:[a-z0-9]+[_-])*
                (?:password|passwd|passphrase|(?:seed|recovery)[\x20_-]+phrase)
                [\x20\t]*[:=][\x20\t]*(?P<value>[^"'\s\[][^\r\n;]*)"#,
            "[SECRET]",
        ),
        Rule::checked(
            r"(?i)[\p{L}\p{N}.!#$%&'*+/=?^_\x60{|}~\-]+@(?:[\p{L}\p{N}](?:[\p{L}\p{N}-]*[\p{L}\p{N}])?\.)+(?:xn--[\p{L}\p{N}-]{2,59}|\p{L}{2,63})",
            "[EMAIL]",
            |text, matched| {
                !text[matched.end()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '-' || c == '_')
            },
        ),
        Rule::checked(
            r"(?ix)
                (?:
                    (?:\+|00)[0-9](?:[\x20\t().-]*[0-9]){7,14}
                    | (?:1[\x20\t.-]*)?
                      (?:\([0-9]{3}\)|[0-9]{3})[\x20\t.-]*[0-9]{3}[\x20\t.-]*[0-9]{4}
                    | [0-9]{10,15}
                )
                (?:[\x20\t]*(?:ext\.?|extension|x)[\x20\t]*[0-9]{1,6})?
            ",
            "[PHONE]",
            identifier_boundary,
        ),
        Rule::field(
            r"(?:phone|mobile|telephone|fax)(?:[\x20_-]+(?:no\.?|number))?",
            r#"["']?\+?\(?[0-9](?:[\x20\t().-]*[0-9]){6,14}\)?"#,
            "[PHONE]",
        ),
        Rule::new(
            r"(?i)\bs-1-[0-9]{1,15}(?:-[0-9]{1,10}){3,15}\b",
            "[SECURITY_ID]",
        ),
        Rule::new(r"\b[0-9]{3}[-\x20][0-9]{2}[-\x20][0-9]{4}\b", "[SSN]"),
        Rule::field(
            r"(?:social[\x20_-]+security(?:[\x20_-]+(?:no\.?|number))?|ssn|itin)",
            r#"["']?[0-9]{3}[-\x20]?[0-9]{2}[-\x20]?[0-9]{4}\b"#,
            "[SSN]",
        ),
        Rule::checked(
            r"\b[0-9]{3}[-\x20][0-9]{3}[-\x20][0-9]{3}\b",
            "[GOVERNMENT_ID]",
            |_, matched| luhn(matched.as_str()),
        ),
        Rule::field(
            r"(?:sin|social[\x20_-]+insurance[\x20_-]+number|passport(?:[\x20_-]+(?:no\.?|number|id))?|(?:driver['’]?s?[\x20_-]+licen[cs]e|government[\x20_-]+id|tax[\x20_-]+id|national[\x20_-]+id|health[\x20_-]+card|medicare[\x20_-]+id|nhs[\x20_-]+number|medical[\x20_-]+record[\x20_-]+number|patient[\x20_-]+id)(?:[\x20_-]+(?:no\.?|number))?)",
            r#"["']?(?:[A-Za-z0-9][A-Za-z0-9-]{4,21}|[0-9]{3}(?:[\x20-][0-9]{3}){2})\b"#,
            "[GOVERNMENT_ID]",
        ),
        Rule::field(
            r"dob|birthdate|date[\x20_-]+of[\x20_-]+birth",
            &format!(r#"["']?{TYPED_DATE}\b"#),
            "[DATE_OF_BIRTH]",
        ),
        Rule::checked(
            r"\b(?:[0-9]{4}(?:[\x20-][0-9]{4}){3}|[0-9]{4}[\x20-][0-9]{6}[\x20-][0-9]{5}|[0-9]{13,19})\b",
            "[CREDIT_CARD]",
            |_, matched| luhn(matched.as_str()),
        ),
        Rule::checked(
            r"\b[A-Z]{2}[0-9]{2}(?:[\x20]?[A-Z0-9]{4}){2,7}(?:[\x20]?[A-Z0-9]{1,2})?\b",
            "[BANK_ACCOUNT]",
            |_, matched| valid_iban(matched.as_str()),
        ),
        Rule::field(
            r"(?:(?:bank[\x20_-]+)?account|routing|aba[\x20_-]+routing|card)[\x20_-]+(?:no\.?|number)|cvv2?|cvc2?|security[\x20_-]+code|pin",
            r#"["']?[0-9](?:[\x20-]?[0-9]){2,22}\b"#,
            "[FINANCIAL_ID]",
        ),
        Rule::checked(
            r"(?ix)(?:\b[0-9]{1,3}(?:\.[0-9]{1,3}){3}\b|(?:[0-9a-f]{0,4}:){2,8}[0-9a-f]{0,4}(?:\.[0-9]{1,3}){0,3})",
            "[IP_ADDRESS]",
            |text, matched| {
                identifier_boundary(text, matched) && matched.as_str().parse::<IpAddr>().is_ok()
            },
        ),
        Rule::new(
            r"(?i)\b[0-9a-f]{2}(?::[0-9a-f]{2}){5}\b|\b[0-9a-f]{2}(?:-[0-9a-f]{2}){5}\b",
            "[MAC_ADDRESS]",
        ),
        Rule::field(
            r"(?:legal|full|first|last|given|family|contact|patient|customer|recipient|account[_\x20-]+holder)[\x20_-]+name",
            &format!(r#"["']?{NAME}(?:[\x20\t]+{NAME}){{0,4}}"#),
            "[PERSON_NAME]",
        ),
        Rule::new(
            &format!(
                r"(?x)\b(?i:my[\x20\t]+name[\x20\t]+is|(?:mr|mrs|ms|dr)\.)[\x20\t]+
                   (?P<value>\p{{Lu}}[\p{{L}}\p{{M}}'’.-]*(?:[\x20\t]+\p{{Lu}}[\p{{L}}\p{{M}}'’.-]*){{0,3}})"
            ),
            "[PERSON_NAME]",
        ),
        Rule::field(
            r"(?:home|street|mailing|shipping|billing|delivery|residential|postal)[\x20_-]+address",
            r#"(?:"(?:[^"\r\n\\]|\\.)+"|'(?:[^'\r\n\\]|\\.)+'|[^\r\n;}\]>]+)"#,
            "[ADDRESS]",
        ),
        Rule::new(
            r"(?ix)
                \b(?:p\.?[\x20\t]*o\.?[\x20\t]*box[\x20\t]+[0-9]{1,8}|
                    [0-9]{1,7}[a-z]?[\x20\t]+(?:[\p{L}0-9.'’-]+[\x20\t]+){1,7}
                    (?:street|st|avenue|ave|road|rd|boulevard|blvd|lane|ln|drive|dr|
                       court|ct|place|pl|circle|cir|terrace|ter|way|crescent|cres|close|highway|hwy)
                    (?:\b\.)?
                    (?:[\x20\t,]+(?:apt\.?|unit|suite|\x23)[\x20\t]*[a-z0-9-]{1,12})?
                )\b
            ",
            "[ADDRESS]",
        ),
    ]
});

static CANDIDATE_RULES: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new(RULES.iter().map(|rule| rule.pattern.as_str()))
        .expect("valid Archive redaction pattern set")
});

/// Prepare an Archive query or document body for embedding inference.
pub fn redact_embedding_text(text: &str) -> Cow<'_, str> {
    redact_embedding_range(text, 0..text.len())
}

/// Detect sensitive spans in the original turn before clipping to a passage.
pub(crate) fn redact_embedding_range(text: &str, range: Range<usize>) -> Cow<'_, str> {
    let mut matches = Vec::new();
    for index in CANDIDATE_RULES.matches(text).iter() {
        let rule = &RULES[index];
        for captures in rule.pattern.captures_iter(text) {
            let captured = captures.name("value");
            let matched = captured.unwrap_or_else(|| captures.get(0).expect("full match"));
            if matched.start() >= range.end {
                break;
            }
            if range.start < matched.end() && (rule.accepts)(text, matched) {
                let span = unquoted_range(matched);
                if span.start < range.end && range.start < span.end {
                    matches.push((span, rule.label));
                }
            }
        }
    }
    if matches.is_empty() {
        return Cow::Borrowed(&text[range]);
    }
    matches.sort_by_key(|(span, _)| (span.start, std::cmp::Reverse(span.end)));
    let mut redacted = String::with_capacity(range.len());
    let mut cursor = range.start;
    for (matched, label) in matches {
        let start = matched.start.max(range.start);
        let end = matched.end.min(range.end);
        if end <= cursor {
            continue;
        }
        if start >= cursor {
            redacted.push_str(&text[cursor..start]);
            redacted.push_str(label);
        }
        cursor = end;
    }
    redacted.push_str(&text[cursor..range.end]);
    Cow::Owned(redacted)
}

fn unquoted_range(matched: Match<'_>) -> Range<usize> {
    let candidate = matched.as_str().as_bytes();
    if candidate.first().is_some_and(|c| matches!(c, b'"' | b'\'')) {
        let closed = candidate.len() >= 2 && candidate.first() == candidate.last();
        return matched.start() + 1..matched.end() - usize::from(closed);
    }
    matched.range()
}

fn identifier_boundary(text: &str, number: Match<'_>) -> bool {
    let is_identifier = |c: char| c.is_ascii_alphanumeric() || c == '_';
    !text[..number.start()]
        .chars()
        .next_back()
        .is_some_and(is_identifier)
        && !text[number.end()..]
            .chars()
            .next()
            .is_some_and(is_identifier)
}

fn luhn(candidate: &str) -> bool {
    let digits = candidate.bytes().rev().filter(u8::is_ascii_digit);
    let sum = digits
        .enumerate()
        .map(|(index, digit)| {
            let digit = u32::from(digit - b'0');
            if index % 2 == 0 {
                digit
            } else {
                let doubled = digit * 2;
                doubled % 10 + doubled / 10
            }
        })
        .sum::<u32>();
    sum > 0 && sum % 10 == 0
}

fn valid_iban(candidate: &str) -> bool {
    let mut iban = candidate.bytes().filter(|&c| c != b' ');
    let prefix = [iban.next(), iban.next(), iban.next(), iban.next()];
    let mut digits = 0;
    let mut remainder = 0;
    for c in iban.chain(prefix.into_iter().flatten()) {
        digits += 1;
        remainder = match c {
            b'0'..=b'9' => (remainder * 10 + u32::from(c - b'0')) % 97,
            b'A'..=b'Z' => (remainder * 100 + u32::from(c - b'A') + 10) % 97,
            _ => return false,
        };
    }
    (15..=34).contains(&digits) && remainder == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_pii_formats() {
        let cases = [
            (
                "Email mailto:dev+tag@example.invalid, café@ejemplo.es, or call (604) 555-0123 x4, \
                 604.555.0123, 6045550123, or +44 (0)20 7946 0958.",
                "Email mailto:[EMAIL], [EMAIL], or call [PHONE], [PHONE], [PHONE], or [PHONE].",
            ),
            (
                "SSN: 123-45-6789 | SIN: 046 454 286",
                "SSN: [SSN] | SIN: [GOVERNMENT_ID]",
            ),
            ("card 4111-1111-1111-1111", "card [CREDIT_CARD]"),
            ("IBAN DE89 3704 0044 0532 0130 00", "IBAN [BANK_ACCOUNT]"),
            (
                "host 192.0.2.15 or [2001:db8::5]",
                "host [IP_ADDRESS] or [[IP_ADDRESS]]",
            ),
            (
                "passport number: Z0001234",
                "passport number: [GOVERNMENT_ID]",
            ),
            ("dob: 2000-02-29", "dob: [DATE_OF_BIRTH]"),
            (
                "my name is Irène D’Example and this is my test",
                "my name is [PERSON_NAME] and this is my test",
            ),
            (
                "Shipping address: \"1 Example Road, Testville\"",
                "Shipping address: \"[ADDRESS]\"",
            ),
            ("meet at 9 Example Road Suite 12", "meet at [ADDRESS]"),
        ];
        for (input, expected) in cases {
            assert_eq!(redact_embedding_text(input), expected);
        }
    }

    #[test]
    fn redacts_credentials_in_chat_and_code() {
        let cases = [
            (
                "OPENROUTER_API_KEY=synthetic-embedding-key",
                "OPENROUTER_API_KEY=[SECRET]",
            ),
            (
                r#"{"password":"synthetic\"quoted","region":"local"}"#,
                r#"{"password":"[SECRET]","region":"local"}"#,
            ),
            (
                "Authorization: Basic ZmFrZTp2YWx1ZQ==",
                "Authorization: Basic [TOKEN]",
            ),
            ("Bearer synthetic-session-token", "Bearer [TOKEN]"),
            (
                "recovery phrase: example secret words",
                "recovery phrase: [SECRET]",
            ),
            (
                "curl --password 'fictional value'",
                "curl --password '[SECRET]'",
            ),
            ("eyJhbGciOiJzIn0.eyJpZCI6Ingi.demo_signature", "[TOKEN]"),
            ("sk-or-v1-0synthetic0placeholder0", "[API_KEY]"),
            (
                "postgres://testuser:pass%40word@db.invalid/path",
                "postgres://[CREDENTIALS]/path",
            ),
            (
                "https://files.invalid/asset?X-Amz-Signature=demo&download=1",
                "https://files.invalid/asset?X-Amz-Signature=[SECRET]&download=1",
            ),
            (
                "Cookie: session=synthetic; path=/\nOK",
                "Cookie: [COOKIE]\nOK",
            ),
            (
                "-----BEGIN OPENSSH PRIVATE KEY-----\nsynthetic-material\n-----END OPENSSH PRIVATE KEY-----\nOK",
                "[PRIVATE_KEY]\nOK",
            ),
            (
                "-----BEGIN PRIVATE KEY-----\nsynthetic-partial",
                "[PRIVATE_KEY]",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(redact_embedding_text(input), expected);
        }
    }

    #[test]
    fn masks_windows_account_security_identifiers() {
        let text = "Principal S-1-5-21-1111111111-2222222222-3333333333-1001 and system S-1-5-18";
        assert_eq!(
            redact_embedding_text(text),
            "Principal [SECURITY_ID] and system S-1-5-18"
        );
    }

    #[test]
    fn retains_original_text_without_a_match() {
        let text = "Archive vector version v2026112400 and 1.2.3, std::fmt::Write, forge build b98a213e, invalid card checksum 4111-1111-1111-1112";
        assert!(matches!(redact_embedding_text(text), Cow::Borrowed(_)));
    }

    #[test]
    fn clips_sensitive_spans_at_each_passage_edge() {
        let input =
            "é dev+tag@example.invalid before 1-604-555-0123 after password='example secret' end";
        let email_edge = input.find('@').unwrap();
        let phone_edge = input.find("555").unwrap();
        let secret_edge = input.find(" secret").unwrap();
        assert_eq!(redact_embedding_range(input, 0..email_edge), "é [EMAIL]");
        assert_eq!(
            redact_embedding_range(input, email_edge..phone_edge),
            "[EMAIL] before [PHONE]"
        );
        assert_eq!(
            redact_embedding_range(input, phone_edge..secret_edge),
            "[PHONE] after password='[SECRET]"
        );
        assert_eq!(
            redact_embedding_range(input, secret_edge..input.len()),
            "[SECRET]' end"
        );
    }
}
