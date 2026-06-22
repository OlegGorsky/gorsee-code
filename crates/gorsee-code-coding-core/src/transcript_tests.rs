use gorsee_code_core::{Event, EventKind};
use serde_json::json;

use crate::{TranscriptEventKind, TranscriptMapper};

#[test]
fn transcript_keeps_chat_and_hides_raw_process_events() {
    let events = vec![
        event(
            1,
            EventKind::SessionStarted,
            None,
            json!({"objective":"привет"}),
        ),
        event(2, EventKind::AgentStarted, Some("architect"), json!({})),
        event(
            3,
            EventKind::ToolRequested,
            Some("architect"),
            json!({"name":"read_file"}),
        ),
        event(
            4,
            EventKind::ToolFinished,
            Some("architect"),
            json!({"name":"read_file","output":"done"}),
        ),
        event(
            5,
            EventKind::AgentMessage,
            Some("architect"),
            json!({"message":"Привет! Чем помочь?"}),
        ),
        event(6, EventKind::SessionFinished, None, json!({})),
    ];

    let transcript = TranscriptMapper.map_events(&events);

    assert_eq!(
        transcript
            .iter()
            .map(|event| &event.kind)
            .collect::<Vec<_>>(),
        vec![
            &TranscriptEventKind::UserMessage,
            &TranscriptEventKind::AssistantMessage,
        ]
    );
    assert_eq!(transcript[0].summary, "привет");
    assert_eq!(transcript[1].summary, "Привет! Чем помочь?");
}

#[test]
fn workspace_ready_bootstrap_is_not_a_chat_message() {
    let transcript = TranscriptMapper.map_events(&[event(
        1,
        EventKind::SessionStarted,
        None,
        json!({"objective":"workspace_ready"}),
    )]);
    assert!(transcript.is_empty());
}

#[test]
fn approval_request_is_visible_without_raw_tool_event() {
    let transcript = TranscriptMapper.map_events(&[event(
        1,
        EventKind::ToolRequested,
        Some("coder"),
        json!({"name":"apply_patch","approval_id":"approve-1"}),
    )]);

    assert_eq!(transcript.len(), 1);
    assert_eq!(transcript[0].kind, TranscriptEventKind::ApprovalNeeded);
    assert_eq!(
        transcript[0].summary,
        "требуется подтверждение: apply_patch"
    );
    assert_eq!(transcript[0].detail.as_deref(), Some("approve-1"));
}

#[test]
fn invalid_model_response_is_humanized() {
    let transcript = TranscriptMapper.map_events(&[event(
        1,
        EventKind::Error,
        Some("architect"),
        json!({"error":"invalid model response: expected `,` or `}` at line 1 column 42: {\"final_answer\":\"...\"}"}),
    )]);
    assert_eq!(
        transcript[0].summary,
        "Модель вернула ответ в неподдерживаемом формате."
    );
    assert!(transcript[0]
        .detail
        .as_deref()
        .unwrap()
        .contains("invalid model response"));
}

#[test]
fn failed_tool_finished_is_kept_out_of_user_transcript() {
    let transcript = TranscriptMapper.map_events(&[event(
        1,
        EventKind::ToolFinished,
        Some("validator"),
        json!({
            "name":"run_command",
            "status":"failed",
            "output":"cargo check failed\nexit_status=101"
        }),
    )]);

    assert!(transcript.is_empty());
}

#[test]
fn transcript_only_shows_latest_agent_message() {
    let events = vec![
        event(
            1,
            EventKind::SessionStarted,
            None,
            json!({"objective":"создай файл"}),
        ),
        event(
            2,
            EventKind::AgentMessage,
            Some("architect"),
            json!({"message":"внутренний план"}),
        ),
        event(
            3,
            EventKind::AgentMessage,
            Some("coder"),
            json!({"message":"внутренняя реализация"}),
        ),
        event(
            4,
            EventKind::AgentMessage,
            Some("validator"),
            json!({"message":"Готово: файл создан, проверки прошли."}),
        ),
        event(5, EventKind::TurnFinished, None, json!({})),
    ];

    let transcript = TranscriptMapper.map_events(&events);

    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript[0].summary, "создай файл");
    assert_eq!(
        transcript[1].summary,
        "Готово: файл создан, проверки прошли."
    );
}

#[test]
fn transcript_does_not_show_agent_message_before_turn_finishes() {
    let events = vec![
        event(
            1,
            EventKind::TurnStarted,
            None,
            json!({"objective":"создай файл"}),
        ),
        event(
            2,
            EventKind::AgentMessage,
            Some("architect"),
            json!({"message":"рабочий вывод для следующих агентов"}),
        ),
        event(
            3,
            EventKind::ToolRequested,
            Some("coder"),
            json!({"name":"apply_patch","approval_id":"approve-1"}),
        ),
    ];

    let transcript = TranscriptMapper.map_events(&events);

    assert_eq!(
        transcript
            .iter()
            .map(|event| &event.kind)
            .collect::<Vec<_>>(),
        vec![
            &TranscriptEventKind::UserMessage,
            &TranscriptEventKind::ApprovalNeeded,
        ]
    );
}

#[test]
fn transcript_keeps_latest_agent_message_for_each_turn() {
    let events = vec![
        event(
            1,
            EventKind::TurnStarted,
            None,
            json!({"objective":"привет"}),
        ),
        event(
            2,
            EventKind::AgentMessage,
            Some("architect"),
            json!({"message":"думаю"}),
        ),
        event(
            3,
            EventKind::AgentMessage,
            Some("architect"),
            json!({"message":"Привет!"}),
        ),
        event(4, EventKind::TurnFinished, None, json!({})),
        event(
            5,
            EventKind::TurnStarted,
            None,
            json!({"objective":"как дела?"}),
        ),
        event(
            6,
            EventKind::AgentMessage,
            Some("architect"),
            json!({"message":"готовлю ответ"}),
        ),
        event(
            7,
            EventKind::AgentMessage,
            Some("architect"),
            json!({"message":"Все нормально, чем помочь?"}),
        ),
        event(8, EventKind::TurnFinished, None, json!({})),
    ];

    let transcript = TranscriptMapper.map_events(&events);

    assert_eq!(
        transcript
            .iter()
            .map(|event| event.summary.as_str())
            .collect::<Vec<_>>(),
        vec![
            "привет",
            "Привет!",
            "как дела?",
            "Все нормально, чем помочь?",
        ]
    );
}

fn event(
    sequence: u64,
    kind: EventKind,
    agent_id: Option<&str>,
    payload: serde_json::Value,
) -> Event {
    Event::new(sequence, "s", agent_id.map(str::to_string), kind, payload)
}
