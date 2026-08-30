//! What can go wrong reading a pack.

use crate::meta::MetadataError;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum DatapackError {
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not read the archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("pack '{0}' has no pack.mcmeta")]
    NoMetadata(String),
    #[error("pack '{pack}': {source}")]
    Metadata {
        pack: String,
        #[source]
        source: MetadataError,
    },
}

impl DatapackError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
