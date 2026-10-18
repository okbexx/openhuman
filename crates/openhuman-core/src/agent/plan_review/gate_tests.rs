use super::*;

#[tokio::test(start_paused = true)]
async fn approve_resolves_parked_turn() {
    let gate = PlanReviewGate::new(Duration::from_secs(5));
    let parked = gate.request_review(
        Some("t1".into()),
        Some("c1".into()),
        "Ship it".into(),
        vec!["step one".into()],
        Some("call-1".into()),
    );
    tokio::pin!(parked);
    assert!(futures::poll!(parked.as_mut()).is_pending());
    assert!(gate.decide_by_thread("t1", PlanReviewResolution::Approve));
    assert_eq!(parked.await, PlanReviewResolution::Approve);
}

#[tokio::test(start_paused = true)]
async fn revise_carries_feedback_back() {
    let gate = PlanReviewGate::new(Duration::from_secs(5));
    let parked = gate.request_review(Some("t2".into()), None, "Plan".into(), vec![], None);
    tokio::pin!(parked);
    assert!(futures::poll!(parked.as_mut()).is_pending());
    assert!(gate.decide_by_thread(
        "t2",
        PlanReviewResolution::Revise {
            feedback: "add a test step".into(),
        },
    ));
    assert_eq!(
        parked.await,
        PlanReviewResolution::Revise {
            feedback: "add a test step".into(),
        }
    );
}

#[tokio::test(start_paused = true)]
async fn timeout_fails_closed_to_reject() {
    let gate = PlanReviewGate::new(Duration::from_millis(40));
    let resolution = gate
        .request_review(Some("t3".into()), None, "Plan".into(), vec![], None)
        .await;
    assert_eq!(resolution, PlanReviewResolution::Reject);
    // The waiter is cleaned up after timeout.
    assert!(!gate.decide_by_thread("t3", PlanReviewResolution::Approve));
}

#[tokio::test]
async fn decide_unknown_request_is_false() {
    let gate = PlanReviewGate::new(Duration::from_secs(5));
    assert!(!gate.decide("nope", PlanReviewResolution::Approve));
}

#[tokio::test]
async fn cancelled_park_cleans_up_waiter() {
    // A parked review whose future is dropped (turn cancel) before it
    // resolves must not leak its waiter / thread mapping.
    let gate = PlanReviewGate::new(Duration::from_secs(30));
    let mut parked =
        Box::pin(gate.request_review(Some("t-drop".into()), None, "Plan".into(), vec![], None));
    assert!(futures::poll!(parked.as_mut()).is_pending());
    assert!(gate.parked_review_for_thread("t-drop").is_some());
    drop(parked);
    // The drop guard removed the entry, so there is nothing left to decide.
    assert!(!gate.decide_by_thread("t-drop", PlanReviewResolution::Approve));
}

#[tokio::test(start_paused = true)]
async fn parked_review_for_thread_carries_tool_call_id_and_expiry() {
    let gate = PlanReviewGate::new(Duration::from_secs(5));
    let parked = gate.request_review(
        Some("t-replay".into()),
        Some("c-replay".into()),
        "Ship it".into(),
        vec!["step one".into()],
        Some("call-replay".into()),
    );
    tokio::pin!(parked);
    assert!(futures::poll!(parked.as_mut()).is_pending());

    let row = gate
        .parked_review_for_thread("t-replay")
        .expect("review should be parked");
    assert_eq!(row.thread_id, Some("t-replay".to_string()));
    assert_eq!(row.client_id, Some("c-replay".to_string()));
    assert_eq!(row.tool_call_id, Some("call-replay".to_string()));
    assert!(row.expires_at.is_some());
    assert_eq!(row.steps, vec!["step one".to_string()]);

    assert!(gate.decide_by_thread("t-replay", PlanReviewResolution::Approve));
    assert_eq!(parked.await, PlanReviewResolution::Approve);

    // Decided reviews are no longer parked.
    assert!(gate.parked_review_for_thread("t-replay").is_none());
}

#[tokio::test]
async fn parked_review_for_thread_is_none_when_nothing_is_parked() {
    let gate = PlanReviewGate::new(Duration::from_secs(5));
    assert!(gate.parked_review_for_thread("no-such-thread").is_none());
}
