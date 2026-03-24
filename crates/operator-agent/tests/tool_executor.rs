use std::sync::Arc;

use operator_agent::tools::ToolExecutor;
use operator_core::{AppInfo, Capability, CapabilitySet, QueryResult, SessionId, TargetId};
use operator_runtime::{RuntimeBuilder, RuntimeConfig};
use operator_testkit::{InMemorySnapshotStore, MockPlatformDriver};
use serde_json::json;

#[tokio::test]
async fn call_wraps_runtime_success_with_structured_output_and_exec_context() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::AppLifecycle]),
    ));
    driver.push_query_result(Ok(QueryResult::Apps(vec![AppInfo {
        bundle_id: Some("com.apple.finder".into()),
        name: "Finder".into(),
        pid: Some(42),
        is_running: true,
    }])));

    let runtime = RuntimeBuilder::new(RuntimeConfig::default())
        .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
        .register_driver(driver.clone())
        .build()
        .await
        .expect("runtime should build");
    let executor = ToolExecutor::new(runtime.core(), runtime.tools().clone());

    let result = executor
        .call(
            &SessionId("sess-1".into()),
            &TargetId("local:macos".into()),
            "list-apps",
            json!({}),
            Some(2_500),
        )
        .await
        .expect("tool call should return a structured result");

    assert!(!result.is_error);
    assert!(result.read_only);
    assert_eq!(result.tool_name, "list-apps");
    assert_eq!(result.arguments, json!({}));
    assert_eq!(
        result.output,
        Some(json!({
            "apps": [{
                "bundle_id": "com.apple.finder",
                "name": "Finder",
                "pid": 42,
                "is_running": true
            }]
        }))
    );
    assert_eq!(result.error, None);

    let calls = driver.query_calls().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1.target, TargetId("local:macos".into()));
    assert_eq!(calls[0].1.session, Some(SessionId("sess-1".into())));
    assert_eq!(calls[0].1.timeout_ms, Some(2_500));
}

#[tokio::test]
async fn call_wraps_runtime_failures_as_structured_errors() {
    let driver = Arc::new(MockPlatformDriver::new(
        "macos",
        CapabilitySet::new([Capability::PointerInput]),
    ));
    let runtime = RuntimeBuilder::new(RuntimeConfig {
        allow_side_effects: false,
        ..RuntimeConfig::default()
    })
    .snapshot_store(Arc::new(InMemorySnapshotStore::new()))
    .register_driver(driver)
    .build()
    .await
    .expect("runtime should build");
    let executor = ToolExecutor::new(runtime.core(), runtime.tools().clone());

    let result = executor
        .call(
            &SessionId("sess-2".into()),
            &TargetId("local:macos".into()),
            "click",
            json!({}),
            None,
        )
        .await
        .expect("tool call should return a structured error result");

    assert!(result.is_error);
    assert!(!result.read_only);
    assert_eq!(result.tool_name, "click");
    assert_eq!(result.output, None);
    let error = result.error.expect("error metadata should be present");
    assert_eq!(error.kind, "tool");
    assert_eq!(
        error.message,
        "tool error: click, message: side effects are disabled by runtime policy"
    );
}
