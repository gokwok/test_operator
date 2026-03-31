use operator_agent::{
    planner::{FinishGate, FinishGateVerdict},
    AgentSessionState,
};
use operator_core::{SessionId, TargetId};

fn sample_state() -> AgentSessionState {
    let mut state = AgentSessionState::new(
        SessionId("sess-note".into()),
        TargetId("macos".into()),
        "Open Finder",
    );
    state.start_turn();
    state.start_step();
    state
}

#[test]
fn finish_gate_not_ok_feedback_appends_notes_for_the_same_task() {
    let finish_gate = FinishGate::new();
    let mut state = sample_state();

    finish_gate.record_feedback(
        &mut state,
        &FinishGateVerdict::NotOk {
            reason: "The transcript never confirmed Finder became frontmost.".into(),
        },
    );
    finish_gate.record_feedback(
        &mut state,
        &FinishGateVerdict::NotOk {
            reason: "The finish summary skipped the final observe.".into(),
        },
    );
    finish_gate.record_feedback(
        &mut state,
        &FinishGateVerdict::Ok {
            reason: "The task is complete.".into(),
        },
    );

    assert_eq!(
        state.notes,
        vec![
            "The transcript never confirmed Finder became frontmost.",
            "The finish summary skipped the final observe.",
        ]
    );
}

#[test]
fn finish_gate_feedback_is_cleared_when_bootstrapping_a_new_task() {
    let finish_gate = FinishGate::new();
    let mut state = sample_state();

    finish_gate.record_feedback(
        &mut state,
        &FinishGateVerdict::NotOk {
            reason: "Need another observe before finishing.".into(),
        },
    );
    assert_eq!(state.notes, vec!["Need another observe before finishing."]);

    state.bootstrap_task("Open Safari");

    assert_eq!(state.task, "Open Safari");
    assert!(state.notes.is_empty());
    assert_eq!(state.turn_index, 0);
    assert_eq!(state.step_index, 0);
    assert_eq!(state.parse_attempts, 0);
}
