use anyhow::Result;
use std::time::Duration;

#[derive(Debug)]
pub enum TransactionError {
    ValidationFailed(String),
    MutationFailed(String),
    VerificationFailed(String),
}

pub struct Transaction<'a, B, S> {
    backend: &'a B,
    target_state: S,
}

