use core::fmt;

#[derive(Debug, Clone)]
pub struct NetworkError {
    pub message: String,
    pub err: nix::Error,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "mount error: {} -> {}", self.message, self.err)
    }
}
