# Gorsee Code: план зрелого coding core и оркестратора

Дата фиксации: 2026-06-22  
Статус: source of truth для следующего этапа реализации.

## 1. Scope lock

Этот этап не про новый визуал TUI. Текущие дерево, редактор, мышь, меню, раскладка и стиль считаются клиентским слоем и не переписываются без отдельной задачи.

Цель этапа: сделать Gorsee Code полноценной средой для кодинга проектов. Пользователь выбирает рабочую папку, открывает сессию, пишет задачу, а система планирует работу, использует tools, меняет файлы, показывает diff, запускает проверки и возвращает чистый итог.

Обязательный путь coding turn:

```text
workspace -> session -> user turn -> intent -> plan -> tools
  -> file/git changes -> diff -> verification -> clean final answer
```

## 2. Инварианты зрелой coding-среды

- Простое сообщение вроде `привет` не запускает матрицу агентов.
- Сессия проекта не становится `завершена` после одного ответа; завершается только turn/run.
- Raw events не отображаются как реплики чата.
- `tool_requested`, `tool_finished`, `artifact_created`, `session_finished`, raw JSON и полный stderr/stdout не попадают в чат как сообщения.
- Coding request меняет файлы через tools, а не печатает весь код в ответ.
- Diff и verification являются структурными данными.
- TUI, CLI и ACP используют одно ядро, без дублирования агентной логики.
- Все write/command/MCP действия проходят через единый safety/approval слой.

## 3. Целевая архитектура

```text
TUI / CLI / ACP client
  -> CodingOrchestrator
      -> LCP: Local Coding Protocol
      -> IntentRouter
      -> PlanningEngine
      -> ExecutionEngine
      -> ToolRuntime
      -> McpRuntime
      -> DiffEngine / GitEngine
      -> VerificationEngine
      -> SessionStore / TurnStore
      -> TranscriptMapper
  -> clean client state
```

Главное правило: TUI отображает готовое состояние, а не чинит симптомы runtime. Поведение принадлежит `CodingOrchestrator`.

## 4. Базовые сущности

### Workspace

Рабочая папка проекта: absolute root path, git metadata, project instructions, MCP config, agent/model config, protected paths, session list, context/cache metadata.

### Session

Долгоживущая сущность внутри workspace: id, root, active turn, status, timestamps, context summary, token usage, linked artifacts.

Статусы session: `ready`, `running`, `waiting_approval`, `failed`. Нормальный ответ ассистента возвращает session в `ready`, а не в `finished`.

### Turn

Один пользовательский запрос: user message, intent, plan, selected agents, tool calls, file changes, diff, verification, final answer, usage records.

### Transcript

Чистая модель для клиента:

- `user_message`;
- `assistant_message`;
- `thinking`;
- `tool_summary`;
- `diff_ready`;
- `approval_needed`;
- `verification_result`;
- `error_summary`.

## 5. LCP: Local Coding Protocol

LCP - внутренний контракт между клиентами и coding core.

Запрос содержит `workspace`, optional `session_id`, `message`, `client` и `mode`. Ответ содержит `session_id`, `turn_id`, `intent`, `status`, `transcript`, `diff`, `verification` и `usage`.

Новая возможность добавляется в LCP/core, а не отдельно в TUI, CLI или ACP.

## 6. ACP adapter

ACP нужен как внешний agent-client-protocol слой. Он не содержит агентной логики.

Обязанности ACP adapter:

- маппить внешние session/turn на наши `Workspace`, `Session`, `Turn`;
- отправлять progress, approvals, tool summaries, diff, verification и final answer;
- использовать тот же `CodingOrchestrator`, что TUI и CLI;
- не дублировать agent runtime.

Критерий: задача, работающая в TUI, должна работать через ACP без отдельной реализации.

## 7. IntentRouter

IntentRouter заменяет грубую эвристику `is_simple_chat`.

Контракт результата:

```json
{
  "intent": "chat | inspect | edit | test | review | release",
  "confidence": 0.95,
  "requires_tools": true,
  "requires_write": false,
  "requires_approval": false,
  "reason": "human-readable reason"
}
```

Маршрутизация:

- `chat`: приветствия, общие вопросы, уточнения;
- `inspect`: посмотреть код, найти причину, объяснить архитектуру;
- `edit`: создать, изменить, исправить, реализовать;
- `test`: запустить проверки, воспроизвести баг, разобраться с падением;
- `review`: показать diff, проверить изменения, найти риски;
- `release`: commit, tag, GitHub, npm, publish.

## 8. PlanningEngine

План должен быть структурой turn, а не декоративным текстом в чате.

Минимальный контракт:

```json
{
  "goal": "string",
  "summary": "string",
  "steps": [
    {
      "id": "inspect_repo",
      "kind": "read | search | edit | command | verify",
      "description": "string",
      "expected_tools": ["files:read", "files:search"],
      "risk": "read | write | command"
    }
  ],
  "files_to_inspect": [],
  "files_to_modify": [],
  "verification": [],
  "agents": []
}
```

Правила:

- для `chat` структурный plan не нужен;
- для `edit`, `test`, `release` plan обязателен;
- TUI показывает короткий summary, не raw JSON;
- plan сохраняется в artifacts/session store.

## 9. ExecutionEngine и tools

ExecutionEngine исполняет план через controlled tool runtime.

Обязательное поведение:

- read/search перед write, если файл не был открыт заранее;
- изменения файлов идут через controlled edit/write tools;
- после write/edit формируется diff;
- после diff запускается verification, если есть применимые команды;
- ошибки команд попадают в structured verification/error summary;
- final answer не содержит полный dump кода, если файлы были созданы или изменены.

Минимальные capabilities: `files:list`, `files:read`, `files:search`, `files:write`, `files:edit`, `git:status`, `git:diff`, `git:changed_files`, `command:run`, `tests:run`, `context:repo_map`, `mcp:call`.

## 10. DiffEngine / GitEngine

Diff/Git слой должен быть структурным. Сырой `git diff` допустим только как fallback.

Рекомендуемые библиотеки:

- `git2` для статуса, HEAD blobs, changed files и repo metadata;
- `similar` для line/word/grapheme diff и unified diff.

Контракт diff содержит `files[]` с `path`, `status`, `additions`, `deletions`, `hunks` и `summary` с количеством файлов, добавлений и удалений.

TUI показывает summary и открывает detailed diff panel по запросу.

## 11. MCP runtime

MCP - внешний tool/context слой. Рекомендуемая Rust-библиотека: `rmcp`.

Задачи:

- читать MCP configs из проекта и пользователя;
- подключать MCP servers;
- регистрировать MCP tools/resources в общем registry;
- пропускать опасные MCP calls через safety/approval;
- отдавать MCP result в ExecutionEngine, а не напрямую в TUI.

## 12. Агентная матрица

| Intent | Агенты | Поведение |
| --- | --- | --- |
| `chat` | Assistant или Architect | один короткий ответ без tools |
| `inspect` | Architect, optional Scout | читает и объясняет, без write |
| `edit` | Architect, Coder, Validator | план, изменения, diff, проверки |
| `test` | Validator, optional Coder | запуск, разбор, исправление |
| `review` | Validator, Summarizer | анализ diff/git state |
| `release` | Validator, Summarizer | gated commands после approval |

`Summarizer` и вся матрица не запускаются на каждую короткую реплику.

## 13. Фазы внедрения

| Фаза | Результат | Acceptance |
| --- | --- | --- |
| 1. Core routing и clean transcript | `CodingOrchestrator`, `IntentRouter`, lifecycle `Session`/`Turn`, `TranscriptMapper` | `привет` дает один ответ; история не исчезает; session остается `ready`; чат не показывает raw events |
| 2. Planning и execution contract | структурный plan, execution step state, controlled tools, запрет code dump | `создай файл ...` реально создает файл; итог кратко перечисляет файлы и проверки |
| 3. Diff/Git и verification | `git2` status, `similar` diff model, verification artifacts, structured diff/test events | есть diff summary; validator видит changed files; test failures не засоряют чат |
| 4. MCP | config discovery, servers/tools/resources, calls через safety layer | `/mcp` показывает реальные tools; разрешенный MCP tool вызывается; опасные calls требуют approval |
| 5. ACP | adapter поверх `CodingOrchestrator`, stdio/server surface, progress/approval/diff/verification/final events | ACP client создает turn; TUI и ACP используют одну session model |
| 6. Release hardening | smoke TUI/CLI/ACP, README, npm/GitHub release после проверок | свежая версия запускает тот же TUI; нет regression по мыши, дереву, редактору и меню |

Каждая фаза должна оставлять продукт в рабочем состоянии. Нельзя выпускать промежуточную версию, где чат снова показывает process lines или ломает базовый сценарий сообщения.

## 14. Обязательные smoke-сценарии

Перед релизом должны проходить сценарии:

- открыть `gcode tui`, выбрать рабочую папку, увидеть файлы этой папки;
- отправить `привет` и получить обычный ответ без process lines;
- отправить coding request на создание файла и увидеть созданный файл;
- открыть diff по изменению;
- запустить verification;
- продолжить ту же session вторым сообщением без потери истории;
- вызвать `/`, `@`, `/mcp`, `/limits`, `/sessions`;
- проверить ACP stdio smoke.

## 15. Команды проверки

Перед утверждением или релизом:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Дополнительно: ручной smoke `gcode tui`, CLI smoke для `gcode`, ACP stdio smoke, проверка npm package metadata.

## 16. Что считается неготовым

Реализация не готова, если есть хотя бы один пункт:

- чат показывает raw event names или нумерованные process строки;
- сессия после ответа помечается как завершенная;
- обычный chat запускает несколько агентов;
- coding task печатает большой исходник вместо изменения файлов;
- diff существует только как stdout shell;
- MCP является фиктивным списком без runtime вызова;
- ACP содержит отдельную агентную логику;
- TUI напрямую чинит симптомы вместо получения clean transcript;
- release делается без полного `fmt/check/test/clippy`.

## 17. Главный принцип

```text
orchestrator owns behavior;
clients render state;
tools change projects;
diff and verification prove work.
```

## 18. Зафиксированный план зрелого coding core

Этот раздел фиксирует текущий рабочий план как контракт реализации. Он важнее косметических правок TUI: визуал, дерево, редактор и мышь не переписываются, пока не сломан сам клиентский слой. Главная задача - сделать реальную среду кодинга с оркестратором, tools, diff, verification, ACP, MCP и управляемыми сессиями.

### 18.1. Целевой пользовательский сценарий

Пользователь выбирает рабочую папку проекта, открывает или продолжает сессию, пишет задачу и получает результат как в профессиональном coding agent:

1. сообщение пользователя сохраняется в истории;
2. orchestrator определяет intent;
3. для coding-задачи создается структурный план;
4. ExecutionEngine вызывает tools для чтения, редактирования, команд, тестов, MCP и git;
5. изменения применяются в файлах проекта, а не печатаются большим куском кода в чат;
6. DiffEngine формирует структурный diff;
7. VerificationEngine фиксирует проверки;
8. чат показывает чистый ответ, summary tools, diff и verification без raw runtime events;
9. session остается живой и готовой к следующему turn.

### 18.2. Модульный порядок внедрения

| Порядок | Модуль | Что должно появиться | Критерий готовности |
| --- | --- | --- | --- |
| 1 | LCP contract | единые `CodingRequest` и `CodingResponse` для TUI/CLI/ACP | клиенты используют `session_id`, `turn_id`, `intent`, `transcript`, `diff`, `verification`, `usage`, `approvals`, а не raw events |
| 2 | TranscriptMapper | чистая пользовательская лента | в чате нет `#0001`, `tool_requested`, `artifact_created`, `session_finished`, raw JSON и stderr/stdout dump |
| 3 | Session/Turn lifecycle | долгоживущие sessions и отдельные turns | обычный ответ возвращает session в `ready`, не в `finished`; история не затирается |
| 4 | IntentRouter | нормальная маршрутизация chat/inspect/edit/test/review/release | `привет` не запускает матрицу агентов и не списывает бюджет как coding turn |
| 5 | PlanningEngine | структурный план для coding tasks | план сохраняется как artifact/state и исполняется, а не просто выводится текстом |
| 6 | ExecutionEngine | controlled execution через tool registry | read/search/write/edit/command/test/MCP/git проходят через один safety/approval слой |
| 7 | DiffEngine/GitEngine | структурные diff/git summaries | после write-turn есть changed files, hunks, additions/deletions и связь с verification |
| 8 | VerificationEngine | проверки как данные turn | test/check/clippy failures показываются кратко в чате и подробно в artifact/detail view |
| 9 | MCP runtime | реальные MCP servers/tools/resources | `/mcp` показывает inventory, разрешенные calls выполняются, опасные calls требуют approval |
| 10 | ACP adapter | внешний protocol поверх того же core | ACP не содержит отдельную агентную логику; TUI/CLI/ACP расходятся только в отображении |
| 11 | Context/cache/usage | честный учет контекста, лимитов и API cache | UI показывает реальные токены/лимиты; cache metadata не смешивается с расходом turn |
| 12 | Release hardening | smoke и regression suite | релиз разрешен только после green fmt/check/test/clippy и smoke TUI/CLI/ACP |

### 18.3. Первый инженерный срез

Первый срез должен закрыть самые болезненные симптомы без переписывания визуала:

1. завершить `CodingResponse` в LCP и подключить его в ACP/CLI/TUI state;
2. сделать так, чтобы чат строился только из `TranscriptMapper`;
3. запретить клиентам самостоятельно показывать raw events как сообщения;
4. починить lifecycle: `session.ready/running/waiting_approval/failed`, без `session finished` после обычного turn;
5. закрепить regression tests на `привет`, второй turn в той же session, отсутствие raw process lines и чистый assistant answer.

Definition of done:

- `gcode route привет` возвращает один нормальный ответ;
- TUI после `привет` показывает сообщение пользователя и ответ ассистента, без технических строк;
- повторное сообщение не стирает старую историю;
- usage не имитирует заполненную шкалу при маленьком расходе токенов;
- ACP получает тот же LCP response, что и TUI/CLI.

### 18.4. Второй инженерный срез

Второй срез превращает систему из чат-ответчика в coding agent:

1. PlanningEngine создает шаги для inspect/edit/test/review/release;
2. ExecutionEngine исполняет шаги через tool registry;
3. файл перед write должен быть прочитан или явно создан как новый;
4. write/edit формирует diff artifact;
5. Validator запускает verification только через command-risk tool и approval;
6. final answer перечисляет измененные файлы, diff status и verification status, а не вставляет весь исходник.

Definition of done:

- запрос “создай файл ...” реально создает файл в workspace;
- запрос “исправь ...” меняет существующий файл через controlled edit/write;
- после изменения доступен diff summary;
- при отказе от test approval turn завершается со `verification: skipped`, а не зависает;
- чат не показывает полный stdout/stderr как основную переписку.

### 18.5. Третий инженерный срез

Третий срез подключает внешние протоколы и расширяемость:

1. MCP config discovery из user/workspace config;
2. `rmcp` или совместимый runtime для подключения servers/tools/resources;
3. MCP tools входят в общий registry и safety policy;
4. ACP stdio/server использует один `CodingOrchestrator`;
5. approvals, progress, diff, verification и final answer отдаются через LCP/ACP mapping.

Definition of done:

- `/mcp` показывает реальные подключенные MCP tools;
- безопасный MCP read call выполняется;
- опасный MCP/write/command call создает approval;
- ACP smoke проходит на простом chat и coding turn;
- исправление в core автоматически отражается в TUI, CLI и ACP.

### 18.6. Stop-ship условия

Релиз запрещен, если выполняется хотя бы одно условие:

- чат показывает raw runtime/process/tool строки;
- обычный chat запускает несколько агентов или coding pipeline;
- session помечается завершенной после нормального ответа;
- старые сообщения исчезают после нового turn;
- coding task печатает большой код в ответ вместо изменения файлов;
- diff или verification доступны только как shell stdout;
- MCP/ACP являются декоративными заглушками;
- usage/limits показывают неверные проценты;
- TUI, CLI и ACP имеют разные правила orchestration;
- не пройдены `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`.

### 18.7. Релизный контракт

Перед публикацией версии нужно зафиксировать в release notes:

- какие модули core реально работают;
- какие команды и протоколы покрыты smoke;
- какие сценарии TUI проверены вручную;
- какие known limitations остаются;
- что не является готовой возможностью и не должно рекламироваться.

Нельзя писать в README или npm/GitHub release, что Gorsee Code является полноценной coding-средой, пока не выполнены stop-ship условия и acceptance из разделов 18.3-18.5.
