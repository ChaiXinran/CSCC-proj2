use agentjs::runtime::{
    Job, JsValue, NativeContext, NativeJob, PromiseJob, PromiseReaction, PromiseState,
};

#[test]
fn job_queue_drains_fifo() {
    let mut context = NativeContext::default();

    context
        .enqueue_job(Job::HostCallback(NativeJob::PushOutput("first".into())))
        .expect("enqueue first job");
    context
        .enqueue_job(Job::HostCallback(NativeJob::PushOutput("second".into())))
        .expect("enqueue second job");

    assert_eq!(context.pending_job_count(), 2);
    context.drain_jobs().expect("drain jobs");

    assert_eq!(context.pending_job_count(), 0);
    assert_eq!(context.take_output(), vec!["first", "second"]);
}

#[test]
fn promise_state_transitions_once() {
    let mut context = NativeContext::default();
    let promise = context.create_promise().expect("create promise");

    assert_eq!(context.promise_state(promise), Some(PromiseState::Pending));
    assert!(
        context
            .fulfill_promise(promise, JsValue::Number(1.0))
            .expect("fulfill promise")
    );
    assert_eq!(
        context.promise_state(promise),
        Some(PromiseState::Fulfilled(JsValue::Number(1.0)))
    );
    assert!(
        !context
            .reject_promise(promise, JsValue::String("late".into()))
            .expect("late reject should be ignored")
    );
    assert_eq!(
        context.promise_state(promise),
        Some(PromiseState::Fulfilled(JsValue::Number(1.0)))
    );
}

#[test]
fn promise_reaction_jobs_settle_during_drain() {
    let mut context = NativeContext::default();
    let promise = context.create_promise().expect("create promise");

    context
        .enqueue_job(Job::PromiseReaction(PromiseJob {
            promise,
            reaction: PromiseReaction::Fulfill,
            value: JsValue::String("ok".into()),
        }))
        .expect("enqueue promise job");

    assert_eq!(context.promise_state(promise), Some(PromiseState::Pending));
    context.drain_jobs().expect("drain jobs");
    assert_eq!(
        context.promise_state(promise),
        Some(PromiseState::Fulfilled(JsValue::String("ok".into())))
    );
}

#[test]
fn array_iterator_reads_values_in_order() {
    let mut context = NativeContext::default();
    let array = context
        .create_array(vec![JsValue::Number(1.0), JsValue::Number(2.0)])
        .expect("create array");
    let mut iterator = context.get_iterator(array).expect("array iterator");

    assert_eq!(
        context.iterator_next(&mut iterator).expect("first"),
        Some(JsValue::Number(1.0))
    );
    assert_eq!(
        context.iterator_next(&mut iterator).expect("second"),
        Some(JsValue::Number(2.0))
    );
    assert_eq!(context.iterator_next(&mut iterator).expect("done"), None);
    assert!(iterator.is_done());
}

#[test]
fn string_iterator_reads_codepoints_in_order() {
    let mut context = NativeContext::default();
    let mut iterator = context
        .get_iterator(JsValue::String("aβ".into()))
        .expect("string iterator");

    assert_eq!(
        context.iterator_next(&mut iterator).expect("first"),
        Some(JsValue::String("a".into()))
    );
    assert_eq!(
        context.iterator_next(&mut iterator).expect("second"),
        Some(JsValue::String("β".into()))
    );
    context
        .iterator_close(&mut iterator)
        .expect("close iterator");
    assert_eq!(context.iterator_next(&mut iterator).expect("closed"), None);
}
