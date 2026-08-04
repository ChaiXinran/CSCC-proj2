use std::collections::VecDeque;

use super::{ArrayBufferId, JsValue};

const WAIT_MARKER_PREFIX: &str = "\0agent-wait:";

#[derive(Debug, Clone)]
struct AgentWorker {
    receiver: Option<JsValue>,
    left: bool,
}

#[derive(Debug, Clone)]
struct AgentWaiter {
    buffer: ArrayBufferId,
    index: usize,
    timeout_ms: f64,
    report_template: Option<String>,
    completed: bool,
}

/// Per-runtime cooperative Test262 agent state.
///
/// Test262 variants each own a NativeContext, so this state cannot leak across
/// cases even when the outer runner uses multiple jobs.
#[derive(Debug, Default)]
pub(crate) struct AgentManager {
    workers: Vec<AgentWorker>,
    current_worker: Option<usize>,
    reports: VecDeque<String>,
    waiters: Vec<AgentWaiter>,
    monotonic_ms: f64,
}

impl AgentManager {
    pub(crate) fn start_worker(&mut self) -> usize {
        let id = self.workers.len();
        self.workers.push(AgentWorker {
            receiver: None,
            left: false,
        });
        self.current_worker = Some(id);
        id
    }

    pub(crate) fn finish_start(&mut self) {
        self.current_worker = None;
    }

    pub(crate) fn set_receiver(&mut self, receiver: JsValue) -> bool {
        let Some(worker) = self.current_worker.and_then(|id| self.workers.get_mut(id)) else {
            return false;
        };
        worker.receiver = Some(receiver);
        true
    }

    pub(crate) fn receivers(&self) -> Vec<(usize, JsValue)> {
        self.workers
            .iter()
            .enumerate()
            .filter(|(_, worker)| !worker.left)
            .filter_map(|(id, worker)| worker.receiver.clone().map(|receiver| (id, receiver)))
            .collect()
    }

    pub(crate) fn enter_worker(&mut self, id: usize) {
        self.current_worker = Some(id);
    }

    pub(crate) fn leave_worker_call(&mut self) {
        self.current_worker = None;
    }

    pub(crate) fn mark_leaving(&mut self) {
        if let Some(worker) = self.current_worker.and_then(|id| self.workers.get_mut(id)) {
            worker.left = true;
        }
    }

    pub(crate) fn is_worker(&self) -> bool {
        self.current_worker.is_some()
    }

    pub(crate) fn register_wait(
        &mut self,
        buffer: ArrayBufferId,
        index: usize,
        timeout_ms: f64,
    ) -> String {
        let id = self.waiters.len();
        self.waiters.push(AgentWaiter {
            buffer,
            index,
            timeout_ms,
            report_template: None,
            completed: false,
        });
        if timeout_ms.is_finite() {
            self.monotonic_ms += timeout_ms;
        }
        format!("{WAIT_MARKER_PREFIX}{id}")
    }

    pub(crate) fn report(&mut self, report: String) {
        if let Some(marker_start) = report.find(WAIT_MARKER_PREFIX)
            && let Some(id) = report[marker_start + WAIT_MARKER_PREFIX.len()..]
                .parse::<usize>()
                .ok()
            && let Some(waiter) = self.waiters.get_mut(id)
        {
            waiter.report_template = Some(report);
            return;
        }
        self.reports.push_back(report);
    }

    pub(crate) fn notify(&mut self, buffer: ArrayBufferId, index: usize, count: usize) -> usize {
        let mut notified = 0;
        for waiter in &mut self.waiters {
            if notified >= count {
                break;
            }
            if waiter.buffer == buffer && waiter.index == index && !waiter.completed {
                waiter.completed = true;
                if waiter.timeout_ms.is_finite() {
                    self.monotonic_ms = (self.monotonic_ms - waiter.timeout_ms).max(0.0);
                }
                if let Some(template) = waiter.report_template.take() {
                    self.reports.push_back(replace_wait_marker(&template, "ok"));
                }
                notified += 1;
            }
        }
        notified
    }

    pub(crate) fn sleep(&mut self, duration_ms: f64) {
        for waiter in &mut self.waiters {
            if !waiter.completed && waiter.timeout_ms <= duration_ms {
                waiter.completed = true;
                if let Some(template) = waiter.report_template.take() {
                    self.reports
                        .push_back(replace_wait_marker(&template, "timed-out"));
                }
            }
        }
    }

    pub(crate) fn get_report(&mut self) -> Option<String> {
        if self.reports.is_empty()
            && let Some(waiter) = self.waiters.iter_mut().find(|waiter| {
                !waiter.completed
                    && waiter.timeout_ms.is_finite()
                    && waiter.report_template.is_some()
            })
        {
            waiter.completed = true;
            let template = waiter
                .report_template
                .take()
                .expect("filtered pending waiter report");
            self.reports
                .push_back(replace_wait_marker(&template, "timed-out"));
        }
        self.reports.pop_front()
    }

    pub(crate) fn monotonic_now(&self) -> f64 {
        self.monotonic_ms
    }

    pub(crate) fn roots(&self) -> impl Iterator<Item = &JsValue> + '_ {
        self.workers
            .iter()
            .filter_map(|worker| worker.receiver.as_ref())
    }
}

fn replace_wait_marker(template: &str, status: &str) -> String {
    let Some(start) = template.find(WAIT_MARKER_PREFIX) else {
        return template.to_string();
    };
    let end = template[start..]
        .find(|character: char| character.is_whitespace())
        .map_or(template.len(), |offset| start + offset);
    format!("{}{}{}", &template[..start], status, &template[end..])
}
