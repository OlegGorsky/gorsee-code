# Gorsee Code

Gorsee Code - локальное рабочее пространство для кодинговых агентов на базе
NeuroGate. Проект объединяет терминальный интерфейс, CLI-команды, систему
сессий, модели, лимиты, подтверждения tool calls и проектные настройки в одном
инструменте `gcode`.

Главная идея: запустить `gcode` внутри рабочей папки, выбрать проект, написать
задачу обычным языком и получить управляемую агентную сессию с историей,
контекстом, диффами, подтверждениями и сохранением состояния.

## Для чего нужен Gorsee Code

Gorsee Code задуман как локальный command center для разработки:

- вести диалог с агентом прямо в терминале;
- запускать кодинговые задачи через NeuroGate-модели;
- работать внутри конкретной папки проекта, а не в абстрактном чате;
- видеть файлы, диффы, сессии, модели, лимиты и подтверждения;
- хранить историю агентных запусков в `.gorsee-code/sessions`;
- настраивать модели и токеновые бюджеты под роли агентов;
- запускать быстрые CLI-команды без открытия TUI;
- безопасно подтверждать рискованные действия перед записью файлов или запуском команд.

## Возможности

### TUI-интерфейс

`gcode` без аргументов и `gcode tui` открывают полноэкранный терминальный
интерфейс:

- центральная лента сообщений и результатов агента;
- строка ввода задач и команд;
- выбор рабочей папки проекта;
- дерево файлов проекта;
- просмотр текущего diff;
- список и переключение сессий;
- управление моделями;
- проектные инструкции `AGENTS.md`, `GORSEE.md`, `README.md`;
- список skills и проектные overrides;
- MCP-конфиги `.mcp.json`, `.cursor/mcp.json`, `.codex/mcp.json`;
- live-лимиты аккаунта и локальный бюджет сессии;
- контекст по агентам: модель, токены, процент использования;
- подтверждения tool calls;
- команды через `/` и упоминание файлов через `@`.

### Агентная матрица

По умолчанию используется несколько ролей:

- `architect` - планирование и архитектурные решения;
- `scout` - чтение, поиск и карта репозитория;
- `coder` - изменения кода, патчи и тесты;
- `validator` - проверка, diff, тесты и качество;
- `summarizer` - сжатие контекста и итоговые сводки.

Для простого короткого сообщения в интерактивном чате Gorsee Code использует
основного агента с уменьшенным reasoning и бюджетом, чтобы не запускать всю
матрицу без необходимости.

### Сессии и история

Каждый запуск задачи сохраняется в `.gorsee-code/sessions/<session-id>`:

- `manifest.json` - статус, агенты, цель, рабочая папка;
- `events.jsonl` - события сессии;
- `approvals.jsonl` - ожидающие или завершенные подтверждения;
- `execution.json` - состояние выполнения, если сессия остановилась на approval.

Сессии можно смотреть, ставить на паузу, продолжать, экспортировать и
проигрывать заново через CLI.

### Безопасность

Базовая политика консервативная:

- чтение, поиск и тесты внутри workspace разрешены;
- запись файлов, патчи, shell-команды и сетевые действия требуют подтверждения;
- удаление и доступ за пределы workspace ограничиваются;
- выводы, события, gateway payloads и терминальный вывод проходят через redaction helpers.

Дополнительно можно защитить важные пути через `gcode protect`.

## Установка

Требуется Node.js 16+.

```bash
npm install -g @gorsee/code
gcode
```

Во время установки npm скачивает готовый бинарник из GitHub Releases под вашу
платформу.

Поддерживаемые артефакты:

- Linux x64;
- Linux arm64;
- macOS Apple Silicon;
- macOS Intel;
- Windows x64.

Если после обновления запускается старая версия, проверьте порядок `PATH`:

```bash
which -a gcode
gcode --version
```

## Первый запуск

Откройте папку проекта и запустите:

```bash
cd /path/to/project
gcode
```

Если ключ NeuroGate еще не найден, TUI попросит ввести API key и сохранит его
локально. Также можно заранее передать ключ через переменную окружения:

```bash
export NEUROGATE_API_KEY="..."
gcode
```

Альтернативный вариант - сохранить ключ командой:

```bash
gcode auth set "$NEUROGATE_API_KEY"
gcode auth status
```

Gorsee Code читает ключ из:

- `NEUROGATE_API_KEY`;
- `GORSEE_NEUROGATE_API_KEY`;
- `.gorsee-code/auth.json` внутри текущего проекта.

## Файлы проекта

`gcode init` создает локальную конфигурацию:

```bash
gcode init
```

После инициализации появляются:

```text
gorsee-code.toml
.gorsee-code/
```

`gorsee-code.toml` хранит проектное имя, модели агентов, лимиты, защищенные
пути и настройки NeuroGate endpoint. `.gorsee-code/` хранит auth, сессии,
skills и локальное состояние.

## Быстрые сценарии

Запустить TUI:

```bash
gcode
gcode tui
```

Проверить окружение:

```bash
gcode doctor
```

Запустить задачу без TUI:

```bash
gcode exec "проверь репозиторий и запусти тесты"
```

Посмотреть маршрут агентов без live-запуска:

```bash
gcode route "исправить баг авторизации и проверить тестами"
```

Показать файлы и текущий diff:

```bash
gcode files
gcode diff
```

Выбрать модель для агента:

```bash
gcode models
gcode models recommend --task "frontend bugfix"
gcode models set --agent coder --model kimi-k2.6
```

Посмотреть лимиты аккаунта:

```bash
gcode limits
gcode limits --json
gcode limits watch --once
```

## Команды CLI

### Проект и настройка

| Команда | Что делает |
| --- | --- |
| `gcode` | открывает интерактивный TUI |
| `gcode tui` | открывает интерактивный TUI явно |
| `gcode init` | создает `gorsee-code.toml` и `.gorsee-code/` |
| `gcode setup` | выполняет init и сохраняет ключ из env, если он задан |
| `gcode doctor` | проверяет config, auth и live endpoints NeuroGate |
| `gcode reset --yes` | удаляет локальную конфигурацию и состояние проекта |
| `gcode uninstall --user-data keep` | удаляет project config, но оставляет `.gorsee-code/` |
| `gcode uninstall --user-data remove` | удаляет project config и пользовательское состояние |

### Авторизация

| Команда | Что делает |
| --- | --- |
| `gcode auth set <key>` | сохраняет NeuroGate API key локально |
| `gcode auth set` | берет ключ из `NEUROGATE_API_KEY`, если аргумент не указан |
| `gcode auth status` | показывает источник auth и статус ключа |

### Модели и агенты

| Команда | Что делает |
| --- | --- |
| `gcode agents` | показывает стандартную матрицу агентов |
| `gcode models` | показывает live-модели NeuroGate или локальную матрицу |
| `gcode models benchmark` | выводит модели с cost/credit multiplier |
| `gcode models recommend --task "<задача>"` | рекомендует агента и модель под задачу |
| `gcode models set --agent <agent> --model <model>` | записывает модель агента в `gorsee-code.toml` |
| `gcode capabilities` | показывает live capabilities моделей или настроенную матрицу |

### Задачи, skills и выполнение

| Команда | Что делает |
| --- | --- |
| `gcode exec "<задача>"` | запускает агентную задачу через NeuroGate |
| `gcode route "<задача>"` | показывает предполагаемый маршрут агентов |
| `gcode skills list` | показывает встроенные skills |
| `gcode skills show <id>` | выводит JSON-описание skill |
| `gcode skills run <id> [цель]` | запускает skill через агентную систему |

Встроенные skills:

- `repo-audit` - read-only аудит структуры и рисков репозитория;
- `bug-fix` - трассировка дефекта, ограниченный патч, проверка тестами;
- `quality-check` - запуск quality gates и сводка состояния workspace.

### Сессии

| Команда | Что делает |
| --- | --- |
| `gcode sessions` | показывает сохраненные сессии |
| `gcode sessions list` | то же самое, явная форма |
| `gcode pause [session-id]` | ставит сессию на паузу |
| `gcode resume [session-id]` | продолжает paused-сессию |
| `gcode replay [session-id]` | печатает события сессии |
| `gcode export [session-id]` | экспортирует сессию в Markdown |
| `gcode checkpoint` | создает paused checkpoint-сессию |

Если `session-id` не указан, команда берет последнюю подходящую сессию.

### Подтверждения

| Команда | Что делает |
| --- | --- |
| `gcode approvals` | показывает ожидающие подтверждения |
| `gcode approve <approval-id>` | подтверждает tool call и продолжает выполнение |
| `gcode deny <approval-id>` | отклоняет tool call и продолжает выполнение |

### Workspace, diff и инструменты

| Команда | Что делает |
| --- | --- |
| `gcode files` | показывает файлы workspace |
| `gcode diff` | показывает текущий git diff |
| `gcode tools` | показывает встроенный tool registry |
| `gcode usage` | показывает локальное использование токенов сессии |
| `gcode hooks` | показывает встроенные safety hooks |
| `gcode gateway --bind 127.0.0.1:8765` | запускает локальный HTTP gateway |

### Лимиты, бюджет и защита

| Команда | Что делает |
| --- | --- |
| `gcode limits` | показывает live-лимиты NeuroGate |
| `gcode limits --json` | выводит лимиты JSON-ом |
| `gcode limits watch --once` | делает один live-снимок лимитов |
| `gcode budget set --session 100k` | задает токеновый бюджет сессии |
| `gcode budget set --agent scout 10k` | задает бюджет конкретного агента |
| `gcode protect <path> [path...]` | добавляет защищенные пути в config |

## Конфигурация

Пример фрагмента `gorsee-code.toml`:

```toml
[project]
name = "my-project"
guidance_files = ["AGENTS.md", "GORSEE.md", "README.md"]
protected_paths = ["requirements.md", "tests/**"]

[neurogate]
endpoint = "https://api.neurogate.space/v1"
auth_source = "env"

[budget]
session_tokens = 80000
session_usd = 2.0
warn_at_percent = 75
stop_at_percent = 100

[agents.coder]
model = "deepseek-v4-pro"
reasoning = "medium"
tools = ["read", "search", "propose_patch", "run_test"]
budget_tokens = 50000
temperature = 0.15
```

## Разработка

Проверки для локальной разработки:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npm test
```

Проверить npm-упаковку:

```bash
npm pack --dry-run --json
```

## Лицензия

Apache-2.0. См. `LICENSE` и `NOTICE`.
