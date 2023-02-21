//! Parser for APT source metadata

pub mod contents;

pub trait Filter {
    fn filter_bytes(&self, input: &[u8]) -> bool;
}

#[derive(Clone, Debug, Default)]
pub struct AcceptAllFilter {}

impl Filter for AcceptAllFilter {
    fn filter_bytes(&self, _input: &[u8]) -> bool {
        true
    }
}

impl AcceptAllFilter {
    pub fn new() -> Self {
        Self {}
    }
}
