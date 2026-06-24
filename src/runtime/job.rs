//! Minimal V9 job queue and Promise runtime records.

use std::collections::VecDeque;

use super::{JsValue, Trace, Tracer};

/// Stable handle for a native Promise record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PromiseId(pub u32);

/// Minimal Promise state model shared by V9 runtime and future builtins.
#[derive(Debug, Clone, PartialEq)]
pub enum PromiseState {
    Pending,
    Fulfilled(JsValue),
    Rejected(JsValue),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromiseRecord {
    pub state: PromiseState,
}

impl Default for PromiseRecord {
    fn default() -> Self {
        Self {
            state: PromiseState::Pending,
        }
    }
}

/// Promise reaction work item.  Full reaction lists are future work; V9 starts
/// with deterministic state-transition jobs so async plumbing has one queue.
#[derive(Debug, Clone, PartialEq)]
pub struct PromiseJob {
    pub promise: PromiseId,
    pub reaction: PromiseReaction,
    pub value: JsValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromiseReaction {
    Fulfill,
    Reject,
}

/// Host-observable native jobs used by tests and Test262 host plumbing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeJob {
    PushOutput(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Job {
    PromiseReaction(PromiseJob),
    HostCallback(NativeJob),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JobQueue {
    queue: VecDeque<Job>,
}

impl JobQueue {
    pub fn push(&mut self, job: Job) {
        self.queue.push_back(job);
    }

    pub fn pop(&mut self) -> Option<Job> {
        self.queue.pop_front()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Job> {
        self.queue.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl Trace for PromiseState {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        match self {
            Self::Fulfilled(value) | Self::Rejected(value) => value.trace(tracer),
            Self::Pending => {}
        }
    }
}

impl Trace for PromiseRecord {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        self.state.trace(tracer);
    }
}

impl Trace for PromiseJob {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        self.value.trace(tracer);
    }
}

impl Trace for Job {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        match self {
            Self::PromiseReaction(job) => job.trace(tracer),
            Self::HostCallback(_) => {}
        }
    }
}

impl Trace for JobQueue {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for job in &self.queue {
            job.trace(tracer);
        }
    }
}
