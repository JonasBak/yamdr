use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("custom block type `{0}` is not implemented")]
    CustomBlockNotImplemented(String),

    #[error("error while reading custom block: `{0}`")]
    CustomBlockRead(String),

    #[error("reader called with unsupported block type: `{0}`")]
    UnsupportedBlockType(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
