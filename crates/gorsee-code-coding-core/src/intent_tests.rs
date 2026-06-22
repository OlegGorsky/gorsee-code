use crate::{route_intent, CodingIntent};

#[test]
fn greeting_routes_to_chat_without_tools() {
    let decision = route_intent("Привет, как дела?");
    assert_eq!(decision.intent, CodingIntent::Chat);
    assert!(!decision.requires_tools);
}

#[test]
fn write_requests_route_to_edit() {
    for text in [
        "напиши простой бот в телеграм",
        "создай файл конфигурации",
        "исправь тесты",
        "implement the diff panel",
    ] {
        let decision = route_intent(text);
        assert_eq!(decision.intent, CodingIntent::Edit, "{text}");
        assert!(decision.requires_write);
        assert!(decision.requires_approval);
    }
}

#[test]
fn verification_and_review_are_read_or_command_modes() {
    assert_eq!(
        route_intent("проверь почему тест падает").intent,
        CodingIntent::Test
    );
    assert_eq!(route_intent("покажи diff").intent, CodingIntent::Review);
    assert_eq!(
        route_intent("сделай аудит проекта").intent,
        CodingIntent::Review
    );
}

#[test]
fn slash_commands_do_not_become_chat_turns() {
    let decision = route_intent("/project /home/oleg");
    assert_eq!(decision.intent, CodingIntent::Inspect);
    assert!(!decision.requires_tools);
}
