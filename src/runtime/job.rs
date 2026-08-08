//! Minimal V9 job queue and Promise runtime records.

use std::collections::VecDeque;

use super::{JsValue, StableId, Trace, Tracer};

/// Stable handle for a native Promise record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PromiseId(pub u32);
impl StableId for PromiseId {
    fn from_u32(value: u32) -> Self {
        Self(value)
    }
    fn to_u32(self) -> u32 {
        self.0
    }
}

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
    pub reactions: Vec<PromiseThenReaction>,
}

impl Default for PromiseRecord {
    fn default() -> Self {
        Self {
            state: PromiseState::Pending,
            reactions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromiseThenReaction {
    pub result_promise: Option<PromiseId>,
    pub resolve: JsValue,
    pub reject: JsValue,
    pub on_fulfilled: Option<JsValue>,
    pub on_rejected: Option<JsValue>,
    pub finally: bool,
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

#[derive(Debug, Clone, PartialEq)]
pub struct PromiseCallbackJob {
    pub result_promise: Option<PromiseId>,
    pub resolve: JsValue,
    pub reject: JsValue,
    pub on_fulfilled: Option<JsValue>,
    pub on_rejected: Option<JsValue>,
    pub fulfilled: bool,
    pub value: JsValue,
    pub finally: bool,
}

/// Job that resolves a promise by calling a thenable's `then` method.
/// Used by `PromiseResolveThenableJob` in the spec.
/// Stores the promise as a `JsValue` (Object) so the VM can resolve it.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolveThenableJob {
    pub promise_to_resolve: JsValue,
    pub thenable: JsValue,
    pub then: JsValue,
}

/// Host-observable native jobs used by tests and Test262 host plumbing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeJob {
    PushOutput(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Job {
    PromiseReaction(PromiseJob),
    PromiseCallback(PromiseCallbackJob),
    PromiseResolveThenable(ResolveThenableJob),
    HostCallback(NativeJob),
}

impl Job {
    pub(crate) fn root_values(&self) -> Vec<JsValue> {
        match self {
            Self::PromiseReaction(job) => vec![job.value.clone()],
            Self::PromiseCallback(job) => {
                let mut roots = vec![job.value.clone(), job.resolve.clone(), job.reject.clone()];
                roots.extend(job.on_fulfilled.iter().cloned());
                roots.extend(job.on_rejected.iter().cloned());
                roots
            }
            Self::PromiseResolveThenable(job) => vec![
                job.promise_to_resolve.clone(),
                job.thenable.clone(),
                job.then.clone(),
            ],
            Self::HostCallback(_) => Vec::new(),
        }
    }

    pub(crate) fn root_promises(&self) -> Vec<PromiseId> {
        match self {
            Self::PromiseReaction(job) => vec![job.promise],
            Self::PromiseCallback(job) => job.result_promise.into_iter().collect(),
            Self::PromiseResolveThenable(_) | Self::HostCallback(_) => Vec::new(),
        }
    }
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

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
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
        for reaction in &self.reactions {
            reaction.trace(tracer);
        }
    }
}

impl Trace for PromiseThenReaction {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(value) = &self.on_fulfilled {
            value.trace(tracer);
        }
        if let Some(value) = &self.on_rejected {
            value.trace(tracer);
        }
        self.resolve.trace(tracer);
        self.reject.trace(tracer);
    }
}

impl Trace for PromiseJob {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        self.value.trace(tracer);
    }
}

impl Trace for ResolveThenableJob {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        self.thenable.trace(tracer);
        self.then.trace(tracer);
    }
}

impl Trace for PromiseCallbackJob {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        self.value.trace(tracer);
        if let Some(value) = &self.on_fulfilled {
            value.trace(tracer);
        }
        if let Some(value) = &self.on_rejected {
            value.trace(tracer);
        }
        self.resolve.trace(tracer);
        self.reject.trace(tracer);
    }
}

impl Trace for Job {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        match self {
            Self::PromiseReaction(job) => job.trace(tracer),
            Self::PromiseCallback(job) => job.trace(tracer),
            Self::PromiseResolveThenable(job) => job.trace(tracer),
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
