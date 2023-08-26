#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Error {
    Put,
    Get,
    List,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Error::Put => "ERR usage: PUT file length newline data\n",
            Error::Get => "ERR usage: GET file rev\n",
            Error::List => "ERR usage: LIST dir\n",
        };

        write!(f, "{msg}")
    }
}
