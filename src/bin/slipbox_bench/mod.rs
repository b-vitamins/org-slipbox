pub(crate) mod assertions;
pub(crate) mod constants;
pub(crate) mod corpus;
pub(crate) mod fixtures;
pub(crate) mod metrics;
pub(crate) mod profile;
pub(crate) mod report;
mod runner;
pub(crate) mod workbench;

pub(crate) use runner::main;
pub(crate) use workbench::WorkbenchBench;

#[cfg(test)]
mod tests;
