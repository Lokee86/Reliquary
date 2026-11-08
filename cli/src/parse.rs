use super::*;

pub(super) fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}
pub(super) fn required_option<'a>(args: &'a [String], name: &str) -> Result<&'a str, ProductError> {
    option(args, name).ok_or_else(|| invalid(format!("{name} is required")))
}
pub(super) fn number_option<T: std::str::FromStr>(
    args: &[String],
    name: &str,
) -> Result<Option<T>, ProductError> {
    option(args, name)
        .map(|value| {
            value
                .parse()
                .map_err(|_| invalid(format!("invalid number for {name}")))
        })
        .transpose()
}
pub(super) fn repeated_options<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    args.windows(2)
        .filter(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
        .collect()
}
/// Validate the complete tail of a command before opening a file or mutating state.
/// Each declared option may occur once unless marked repeatable.
pub(super) fn validate_options(
    args: &[String],
    positional_count: usize,
    options: &[(&str, bool)],
) -> Result<(), reliquary_memory::ProductError> {
    if args.len() < positional_count
        || args[..positional_count]
            .iter()
            .any(|arg| arg.starts_with('-'))
    {
        return Err(invalid("missing required positional argument"));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut index = positional_count;
    while index < args.len() {
        let flag = args[index].as_str();
        let Some((_, repeatable)) = options.iter().find(|(name, _)| *name == flag) else {
            return Err(invalid(format!("unexpected CLI argument: {flag}")));
        };
        if !repeatable && !seen.insert(flag) {
            return Err(invalid(format!("duplicate CLI option: {flag}")));
        }
        if matches!(flag, "--dry-run" | "--online" | "--process") {
            index += 1;
        } else {
            if index + 1 >= args.len() || args[index + 1].starts_with('-') {
                return Err(invalid(format!("missing value for {flag}")));
            }
            index += 2;
        }
    }
    Ok(())
}

pub(super) fn one_or_option_path(args: &[String]) -> Result<&Path, reliquary_memory::ProductError> {
    let positional = args
        .iter()
        .take_while(|arg| !arg.starts_with('-'))
        .collect::<Vec<_>>();
    if positional.len() != 1 {
        return Err(invalid("create requires exactly one path"));
    }
    Ok(Path::new(positional[0]))
}
pub(super) fn parse_scope(value: &str) -> Result<OwnerScope, reliquary_memory::ProductError> {
    match value {
        "unspecified" => Ok(OwnerScope::Unspecified),
        "organization" => Ok(OwnerScope::Organization),
        "project" => Ok(OwnerScope::Project),
        "connection" => Ok(OwnerScope::Connection),
        _ => Err(invalid("invalid REL scope")),
    }
}
pub(super) fn parse_hex<const N: usize>(
    value: &str,
    label: &str,
) -> Result<[u8; N], reliquary_memory::ProductError> {
    if value.len() != N * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(format!(
            "{label} must contain {} hexadecimal characters",
            N * 2
        )));
    }
    let mut output = [0; N];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| invalid(format!("{label} contains non-hexadecimal characters")))?;
    }
    Ok(output)
}
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
pub(super) fn invalid(message: impl Into<String>) -> reliquary_memory::ProductError {
    reliquary_memory::ProductError::new(reliquary_memory::ProductErrorKind::InvalidInput, message)
}
