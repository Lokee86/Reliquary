pub(crate) use reliquary_import::PreparedImport;

pub(crate) fn prepare(input: &super::NativeImport) -> Result<PreparedImport, super::ImportError> {
    reliquary_import::prepare(input).map_err(Into::into)
}

pub(crate) fn digest<T: serde::Serialize>(value: &T) -> Result<String, super::ImportError> {
    reliquary_import::digest(value).map_err(Into::into)
}
