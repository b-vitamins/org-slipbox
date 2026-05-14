pub(crate) mod assertions;
pub(crate) mod constants;
pub(crate) mod corpus;
pub(crate) mod fixtures;
pub(crate) mod metrics;
pub(crate) mod profile;
pub(crate) mod report;
mod runner;

pub(crate) use runner::main;

#[cfg(test)]
mod tests;
