use agentjs::{ExecutionOptions, FailureKind, Runtime, RuntimeConfig};

#[test]
fn agent_render_collects_json_alongside_logs_and_value() {
    let mut runtime = Runtime::new(RuntimeConfig::default()).unwrap();
    let report = runtime
        .eval_agent(
            r#"
                console.log("Native runtime checked 3 modules");
                agent.render({
                    type: "panel",
                    title: "Test Result",
                    children: [{ type: "text", text: "92.56%" }]
                });
                "92.56%";
            "#,
            ExecutionOptions::default(),
        )
        .unwrap();

    assert_eq!(report.value, "92.56%");
    assert_eq!(report.output, ["Native runtime checked 3 modules"]);
    assert_eq!(report.render_events.len(), 1);
    assert_eq!(
        report.render_events[0].payload,
        r#"{"type":"panel","title":"Test Result","children":[{"type":"text","text":"92.56%"}]}"#
    );
}

#[test]
fn agent_render_rejects_unsupported_tree_types() {
    let mut runtime = Runtime::new(RuntimeConfig::default()).unwrap();
    let failure = runtime
        .eval_agent(
            r#"agent.render({ type: "html", value: "<script>" });"#,
            ExecutionOptions::default(),
        )
        .unwrap_err();

    assert_eq!(failure.kind, FailureKind::Type);
    assert!(failure.message.contains("unsupported RenderTree type"));
}

#[test]
fn agent_render_enforces_depth_and_size_limits() {
    let config = RuntimeConfig {
        render_tree_depth_limit: 2,
        render_tree_byte_limit: 48,
        ..RuntimeConfig::default()
    };
    let mut runtime = Runtime::new(config).unwrap();
    let depth_failure = runtime
        .eval_agent(
            r#"agent.render({ type: "panel", child: { nested: { value: true } } });"#,
            ExecutionOptions::default(),
        )
        .unwrap_err();
    assert_eq!(depth_failure.kind, FailureKind::RuntimeLimit);
    assert!(depth_failure.message.contains("depth limit"));

    let size_failure = runtime
        .eval_agent(
            r#"agent.render({ type: "text", text: "this payload is deliberately too large" });"#,
            ExecutionOptions::default(),
        )
        .unwrap_err();
    assert_eq!(size_failure.kind, FailureKind::RuntimeLimit);
    assert!(size_failure.message.contains("byte limit"));
}

#[test]
fn agent_host_object_is_frozen() {
    let mut runtime = Runtime::new(RuntimeConfig::default()).unwrap();
    let report = runtime
        .eval_agent(
            r#"
                agent.render = 1;
                agent.extra = true;
                typeof agent.render + ":" + String(agent.extra);
            "#,
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(report.value, "function:undefined");
}
