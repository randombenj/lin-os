use core::fmt;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct NetworkConfigurationError {
    details: String,
}

impl NetworkConfigurationError {
    pub fn new(msg: String) -> NetworkConfigurationError {
        NetworkConfigurationError{details: msg}
    }
}

impl fmt::Display for NetworkConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.details)
    }
}

impl Error for NetworkConfigurationError {
    fn description(&self) -> &str {
        &self.details
    }
}
