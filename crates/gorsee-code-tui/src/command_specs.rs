#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: &'static str,
    pub description: &'static str,
}

pub fn command_specs() -> &'static [CommandSpec] {
    &[
        CommandSpec {
            name: "agents",
            description: "показать активных агентов",
        },
        CommandSpec {
            name: "models",
            description: "выбрать и посмотреть модели",
        },
        CommandSpec {
            name: "models recommend --task",
            description: "подобрать модель под задачу",
        },
        CommandSpec {
            name: "limits",
            description: "показать лимиты аккаунта",
        },
        CommandSpec {
            name: "diff",
            description: "показать текущий diff",
        },
        CommandSpec {
            name: "sessions",
            description: "показать сохраненные сессии",
        },
        CommandSpec {
            name: "instructions",
            description: "показать проектные инструкции",
        },
        CommandSpec {
            name: "skills",
            description: "показать навыки кодинга",
        },
        CommandSpec {
            name: "mcp",
            description: "показать доступные MCP/tools",
        },
        CommandSpec {
            name: "approvals",
            description: "показать ожидающие подтверждения",
        },
        CommandSpec {
            name: "capabilities",
            description: "показать возможности моделей",
        },
        CommandSpec {
            name: "checkpoint",
            description: "сохранить состояние сессии",
        },
        CommandSpec {
            name: "context",
            description: "показать контекст проекта",
        },
        CommandSpec {
            name: "doctor",
            description: "проверить локальную настройку",
        },
        CommandSpec {
            name: "files",
            description: "показать файлы workspace",
        },
        CommandSpec {
            name: "hooks",
            description: "показать safety hooks",
        },
        CommandSpec {
            name: "route",
            description: "показать маршрут агентов",
        },
        CommandSpec {
            name: "timeline",
            description: "показать последние события",
        },
        CommandSpec {
            name: "terminal",
            description: "выполнить shell-команду в проекте",
        },
        CommandSpec {
            name: "approve",
            description: "подтвердить tool call",
        },
        CommandSpec {
            name: "deny",
            description: "отклонить tool call",
        },
        CommandSpec {
            name: "pause",
            description: "поставить сессию на паузу",
        },
        CommandSpec {
            name: "resume",
            description: "продолжить сессию",
        },
        CommandSpec {
            name: "quit",
            description: "закрыть TUI",
        },
    ]
}
