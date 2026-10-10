# Правила работы над video-p2p-mvp

Этот файл действует для всего репозитория. Более конкретные инструкции во вложенных `AGENTS.md` уточняют правила своего каталога. Явные указания пользователя имеют приоритет.

## Общие правила

- Перед работой прочитай [README](README.md), [TODO](todo.md), [архитектуру](docs/ARCHITECTURE.md), [QA checklist](docs/QA_CHECKLIST.md) и руководства из `docs/`, относящиеся к изменению. Проверь `git status` и текущий diff; сохрани существующие изменения пользователя.
- Выполняй одну законченную задачу из TODO за раз: реализация, подтверждение тестами, актуальная документация. Не смешивай с ней независимые рефакторинги.
- Каждое изменение поведения подтверждай тестами: для исправления добавляй регрессионный сценарий, для новой функции — нормальный путь, ошибки и значимые границы. Обновление существующего теста допустимо, если он проверяет новое поведение. Проверяй результат, а не копируй реализацию в assertions.
- Каждое законченное изменение отражай в соответствующем документе и TODO. Для изменения только документации проверь ссылки, якоря, команды и соответствие коду; новые тесты приложения ради Markdown не создавай. Общий гейт проверок ниже всё равно обязателен.
- Реализацию, автоматическую проверку и ручную QA учитывай отдельно. Не отмечай непроверенные сценарии как выполненные и не представляй scaffold, mock/demo или RTP-счётчики как доказательство реального playback.
- Сохраняй границы компонентов: capture contracts в `capture-core`, платформенный захват в `capture-macos`/`capture-linux`, кодеки в `media-codec`, Rust WebRTC в `transport-webrtc`, orchestration в `app-core`, UI/IPC в `apps/desktop`.
- Media worker не должен зависеть от GUI polling. Desktop viewer декодирует media в WebView; сырые/encoded media bytes не передавай через Tauri IPC. Rust bridge обслуживает signaling и diagnostics.
- Зависимости и публичные API добавляй только для текущей задачи. Объясняй необходимость, обновляй соответствующий lockfile и проверяй платформенные ограничения. Сохраняй edition и принятые соглашения существующих crate.

## Стайлгайд Rust

- Используй стандартный `rustfmt`; не вводи ручное форматирование, конфликтующее с ним. Имена функций/модулей — `snake_case`, типов/traits — `UpperCamelCase`, констант — `SCREAMING_SNAKE_CASE`.
- Делай функции и модули с понятной ответственностью. Ограничивай видимость: private по умолчанию, `pub(crate)` для внутренних связей, `pub` для необходимого API.
- Выражай состояния через enum и отдельные типы, когда это предотвращает недопустимые комбинации. Используй `Option` для отсутствия значения и `Result` для ошибки; избегай неявных sentinel-значений.
- Для ожидаемых OS/network/codec/input ошибок возвращай `Result` с контекстом; используй `?`. `unwrap`/`expect` допустимы в тестах и для обоснованных внутренних инвариантов, а не для внешнего ввода или штатных отказов.
- Предпочитай borrowing, slices и стандартные traits (`From`, `TryFrom`, `AsRef`, `Iterator`). Не добавляй лишние `clone`, allocation, `Arc<Mutex<_>>` или generics без необходимости.
- Держи lock недолго; не удерживай его через `.await`, блокирующий I/O, encode или callback. Не блокируй async executor; используй подходящий worker или `spawn_blocking`.
- Для размеров/offsets/timestamps используй проверяемую арифметику и преобразования. Валидируй входные буферы и значения перед использованием; единицы времени/bitrate явно отражай в типе или имени.
- Документируй публичный API через rustdoc: поведение, ограничения и ошибки. Комментарии объясняют инварианты и причины решений.
- Не скрывай проблемы широкими `#[allow(...)]` и не подавляй Clippy ради прохождения гейта. Узкое исключение сопровождай причиной.

Основа: [Rust Style Guide](https://doc.rust-lang.org/style-guide/) и [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html). Применяй их с учётом архитектуры проекта.

## Safe Rust и FFI

- Safe Rust — основной вариант. Новый `unsafe` допускается только при доказанной необходимости FFI/низкоуровневого API и отсутствии подходящего safe решения. Не обходи borrow checker ради удобства или неподтверждённой оптимизации.
- Изолируй `unsafe` в минимальном блоке внутри платформенного/codec bridge с проверяемым safe API. У каждого блока и `unsafe impl` должен быть конкретный `// SAFETY:`: валидность указателей, ownership/lifetime, bounds/alignment и синхронизация. Для `unsafe fn` документируй контракт в `# Safety` и используй явные внутренние unsafe blocks.
- На FFI-границе проверяй null/length, переполнение и формат буфера до создания Rust references/slices. Владение нативным handle оформляй через RAII; stop/drop должны освобождать ресурсы и исключать callbacks после освобождения.
- Не допускай unwinding panic через C/Objective-C ABI. Новые `unsafe impl Send/Sync` требуют объяснения модели потоков и синхронизации; один opaque handle сам по себе этого не доказывает.
- Изменения FFI подтверждай extraction/validation/lifecycle тестами, включая ошибку старта, stop/drop и callback lifetime. Сначала ищи способ уменьшить существующий unsafe участок.

См. [Unsafe Rust](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html). Внешний API и комментарий не заменяют доказательство безопасности.

## Порядок реализации по TODO

Источник статуса и зависимостей — [TODO](todo.md). Если пользователь задал конкретную задачу, выполни её и обнови статус; иначе выбирай первый доступный невыполненный шаг ниже.

1. Реальный macOS capture/playback и permission QA. Сохраняй результаты с commit/build, OS, source/audio/monitor/network условиями. Исправляй обнаруженные дефекты до расширения media pipeline.
2. A/V sync и latency monitoring; затем quality/FPS/bitrate presets; затем network-aware pacing/drop policy и adaptive bitrate с тестами потерь/нагрузки.
3. Linux Wayland: Portal session/permission/source lifecycle → PipeWire video/audio → совместимый H.264 encoder → WebView playback QA на GNOME/KDE. После основного Wayland path — X11 media fallback и его проверки.
4. Signaling/deployment: bounded room/message/history lifecycle → TLS и room authentication/invite access → STUN/TURN deployment и short-lived credentials → реальные direct/relay и weak-network проверки.
5. Release: platform bundle configuration/build → install/launch/capture/playback QA → signing/notarization и проверка распространения.

Не реализуй повторно полностью выполненные пункты. Если первый шаг требует недоступной OS, разрешения, второго устройства или сети, явно оставь его невыполненным, запиши ограничение и продолжи следующий независимый доступный шаг. Не выдавай synthetic probe за ручную QA.

## Правила работы с документацией

- После каждого законченного изменения просмотри затронутые документы, README и TODO. Исправь или удали ставшие неактуальными утверждения, инструкции и дубли. Если всё уже актуально, кратко зафиксируй проверку в итоговом отчёте.
- README содержит краткий статус и точки входа; `docs/` — подробности своего раздела; TODO — состояние и оставшуюся работу. Не превращай их в повторяющуюся историю разработки.
- В TODO `[x]` находится только в «ПОЛНОСТЬЮ СДЕЛАНО». Частичное выполнение описывай как незавершённое; отсутствующую реализацию и непройденную QA отмечай отдельно через `[ ]`.
- Для реализации обновляй подходящее руководство; для тестов/QA — validation/checklist. Указывай выполненные команды, результат и ограничения; не записывай запланированные или пропущенные проверки как успешные.
- Все примеры команд, UI labels, defaults и ограничения сверяй с кодом. Проверяй относительные ссылки/якоря и синтаксис shell-блоков.

## Обязательные проверки каждого законченного изменения

Запускай команды из корня репозитория после последней правки. `apps/desktop/src-tauri` — отдельный workspace; root-команды его не проверяют.

Форматирование обоих workspace и строгий lint:

```bash
cargo fmt --all
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo fmt --all -- --check
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Все Rust-тесты обоих workspace, включая doctests, и JS lifecycle tests:

```bash
cargo test --all --all-features
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features
node --test apps/desktop/tests/viewer-player.test.mjs
```

На macOS также запускай permission-free native extraction и все доступные browser playback/recovery probes:

```bash
sh scripts/test-macos-capture.sh
cargo build -p signaling-server
cargo build -p app-core --example playback_probe
node scripts/test-viewer-playback.mjs
node scripts/test-viewer-playback.mjs --keyframe-recovery
node scripts/test-viewer-playback.mjs --chrome
node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery
```

Browser probes требуют macOS desktop session, Node 22+, локальных sockets и codec/WebKit services; Chrome-режим требует установленного Chrome. На Linux не запускай macOS-specific commands. Если среда не позволяет выполнить проверку, укажи точную команду/причину и оставь проверку открытой; не обходи permission/OS ограничения. Ручную QA выполняй по [checklist](docs/QA_CHECKLIST.md).

Исправь найденные ошибки и повтори затронутые проверки. Перед завершением выполни `git diff --check`, проверь полный итоговый diff и не добавляй посторонние файлы в коммит. В итоговом отчёте укажи результат, тесты, документацию и оставшиеся ограничения.

## Очистка локальных артефактов

После тестов, probes и работы с release bundle сохрани результаты и останови запущенные для проверки процессы. Затем очисти **оба** workspace:

```bash
cargo clean
cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml
```

Очистка обязательна в конце каждого законченного изменения. Не запускай её между compile/test/probe шагами или параллельно сборке. Если Cargo target-dir переопределён, проверь область очистки и не удаляй общий cache другого проекта. Нужный пользователю release bundle сохрани вне очищаемого target до очистки. Registry/download cache, исходники, lockfiles и пользовательские preferences не удаляй. После clean новые тесты/probes потребуют сборки.

Команда удаляет build artifacts: [Cargo clean](https://doc.rust-lang.org/cargo/commands/cargo-clean.html).
