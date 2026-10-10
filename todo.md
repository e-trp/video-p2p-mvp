# TODO — актуальный статус

Статус сверен с кодом и проверками коммита `0c6273f`.

Правила разработки, обязательные проверки и порядок реализации: [AGENTS.md](AGENTS.md).

- **ПОЛНОСТЬЮ СДЕЛАНО**: конкретный результат реализован в указанном объёме и подтверждён кодом, готовыми файлами или автоматическими проверками. Непройденная ручная QA перечислена отдельно; весь продукт ещё не готов к выпуску.
- **ЧАСТИЧНО СДЕЛАНО — НЕ ЗАВЕРШЕНО**: часть результата существует, но остаются реализация или необходимые проверки.
- **НЕ СДЕЛАНО**: указанный результат не реализован либо указанная проверка ещё не выполнена. Непроверенный сценарий не считается полностью завершённым.

`[x]` используется только в разделе полностью выполненных результатов. `[ ]` обозначает оставшуюся работу. Заготовка runtime, наличие счётчиков RTP и успешное синтетическое воспроизведение не заменяют проверку реального захвата и приложения.

## Сводка исходных итераций

| Итерация | Статус | Что осталось |
| --- | --- | --- |
| 1. Реальный WebRTC transport | **ПОЛНОСТЬЮ СДЕЛАНО** в объёме локального SDP/ICE и transport API | Проверки других сетей вынесены в итерации 6/8; mock UDP сохранён как отдельный тестовый инструмент |
| 2. Захват macOS | **ЧАСТИЧНО СДЕЛАНО — НЕ ЗАВЕРШЕНО** | Код захвата/публикации готов; реальное окно/аудио, разрешения и packaged app ещё не проверены |
| 3. Захват Linux | **НЕ СДЕЛАНО** для реального захвата | Есть только X11-каталог и runtime scaffold; Portal/PipeWire/X11 media отсутствуют |
| 4. Tauri workflow | **ЧАСТИЧНО СДЕЛАНО — НЕ ЗАВЕРШЕНО** | Invite UX, Linux portal flow и проверки packaged app |
| 5. Усиление A/V pipeline | **ЧАСТИЧНО СДЕЛАНО — НЕ ЗАВЕРШЕНО** | Adaptive quality/pacing, A/V sync, Linux encoder и weak-network QA |
| 6. NAT/deployment | **ЧАСТИЧНО СДЕЛАНО — НЕ ЗАВЕРШЕНО** | STUN/TURN сервисы/credentials, защищённый signaling и реальные NAT/TURN проверки |
| 7. Preferences/packaging | **ЧАСТИЧНО СДЕЛАНО — НЕ ЗАВЕРШЕНО** | Release build/install QA, signing/notarization и Linux bundle configuration |
| 8. Release readiness | **ЧАСТИЧНО СДЕЛАНО — НЕ ЗАВЕРШЕНО** | Реальный capture-to-playback, packaged GUI, разные сети, latency/CPU/A/V измерения |

## ПОЛНОСТЬЮ СДЕЛАНО

### WebRTC и signaling

- [x] Реальный Rust `PeerConnection`: offer/answer, ICE, состояние соединения, stats и bootstrap data channel.
- [x] H.264/Opus host tracks и публикация encoded media; диагностический Rust viewer принимает/считает RTP.
- [x] TCP signaling client/server, SDP/ICE envelopes, relay и replay для позднего подключения второго участника.
- [x] Интеграционные проверки локального host/viewer SDP/ICE и late-join replay.
- [x] WebRTC session flow в CLI и Tauri; legacy mock UDP отделён от реального media path.

### Реализация захвата macOS и кодеков

- [x] Общие `capture-core` contracts для source/permission, BGRA/PCM и runtime config/status/events.
- [x] Нативный Objective-C/C ABI ScreenCaptureKit bridge: каталог windows/applications/displays по native IDs, asynchronous start, timeout, stop/drop.
- [x] Пассивный Screen Recording preflight, явный запрос при старте и отдельные permission-required/denied статусы.
- [x] Packed BGRA с timestamps, фильтрацией неполных кадров и сохранением aspect ratio; macOS 13+ PCM → interleaved float32, 48 kHz/stereo.
- [x] Нативные bounded queues: последний video frame и до 32 audio buffers; permission-free extraction/lifecycle tests.
- [x] VideoToolbox H.264 Constrained Baseline, SPS/PPS/IDR, Annex B, resize/odd-dimension handling и валидация PTS/буферов; реальные synthetic encoder tests.
- [x] Vendored Opus, 20 ms packet aggregation, timestamps/discontinuity handling и реальные encode/decode tests.
- [x] Непрерывный capture/encode/publish worker независимо от GUI refresh; source/audio validation, encode только при connected peer, остановка/join перед сменой source/session/runtime.
- [x] Synthetic capture → native H.264/Opus → локальный WebRTC/RTP с проверками stop/restart и audio opt-out.

### Desktop viewer и recovery

- [x] Receive-only WebView `PeerConnection`: decoding/playback через video element без передачи media через Tauri IPC.
- [x] Muted startup, Enable Sound/Mute Sound, playback/error states и отдельный viewer signaling pump.
- [x] Generation guards и peer/track cleanup при stop/reset/reconnect/role change/failure; stopped session не оживает после refresh или delayed callbacks.
- [x] Rust bridge и Node lifecycle tests, включая ICE ordering, autoplay, terminal failure и transient disconnect/recovery.
- [x] Synthetic WKWebView/Chromium playback probes: decoded dimensions/pixels, nonzero Opus PCM и media cleanup после stop/refresh.
- [x] RTCP PLI/FIR по video SSRC, FIR duplicate detection, coalescing и 500 ms cooldown; pending сбрасывается после успешного keyframe publish или disconnect/close.
- [x] Host keyframe request/publication/pending diagnostics, SRTCP и publication-race tests, capture stop/restart recovery.
- [x] WKWebView/Chromium recovery probe с пропуском первого keyframe и browser-originated RTCP refresh.

### UI, настройки и инструменты

- [x] Host/viewer preparation, source picker, capture start/poll/stop, session stop/reset и ручной reconnect с recovery guidance.
- [x] Transport/capture/playback diagnostics, selected ICE path и доступные RTT/bitrate/bytes/loss metrics.
- [x] Сохранение room/signaling/ICE/source/audio selection и Auto Refresh preferences; несохранённые form edits не теряются при polling.
- [x] Валидация custom STUN/TURN entries и передача в Rust/WebView peers через CLI/Tauri settings.
- [x] State-only local GUI demo и отдельные non-playable debug bursts.
- [x] Linux `wmctrl` metadata catalog, example fallback и runtime contract scaffold — только в этом объёме, без захвата media.
- [x] macOS `.app`/`.dmg` bundle configuration, icons/metadata и helper scripts для их генерации/сборки.
- [x] Root CI: Rust format/Clippy/tests и Node viewer tests на Linux/macOS; native extraction tests на macOS.
- [x] Инструкции installation/user/networking/media/capture/playback и manual QA checklist.
- [x] Проектный `AGENTS.md`: Rust style, safe FFI, порядок реализации по TODO, тесты, актуализация документации и очистка build artifacts.

## ЧАСТИЧНО СДЕЛАНО — НЕ ЗАВЕРШЕНО

| Результат | Уже есть | Не хватает для завершения |
| --- | --- | --- |
| macOS end-to-end streaming | Native capture, codecs, worker, viewer и synthetic playback | Проверка реального окна/аудио в desktop и packaged app |
| Cross-platform capture/playback | Общие contracts, macOS backend, Linux metadata/scaffold | Linux media backend/encoder и Linux WebView QA |
| Production GUI | Source/session/media controls и diagnostics | Invite UX, quality/audio-routing settings и platform QA |
| Backpressure/weak-network handling | Bounded queues, latest-only diagnostics, write timeout, keyframe recovery | Network-aware pacing/drop policy и adaptive bitrate |
| NAT traversal | Custom ICE configuration и direct/relay hints | Реальные STUN/TURN/NAT проверки, сервисы и managed credentials |
| Packaging/release | macOS bundle config, icons и build scripts | Проверенная установка/запуск, signing/notarization, Linux bundles |

## НЕ СДЕЛАНО

### Реализация

- [ ] Linux Wayland Portal ScreenCast session lifecycle: user source selection, permission, PipeWire stream consumption и cleanup.
- [ ] Linux audio capture и X11 fallback media capture; текущий `wmctrl` даёт только metadata.
- [ ] Linux video encoder, совместимый с согласованным H.264 media path.
- [ ] Bitrate presets и UI settings для quality/FPS/resolution/audio routing; текущие defaults фиксированы в backend.
- [ ] Adaptive bitrate/quality, network-aware frame pacing и congestion/drop policy.
- [ ] A/V sync monitoring и capture-to-render latency instrumentation; browser playout уже существует, измерения skew нет.
- [ ] Room/invite code/link UX и authenticated room access.
- [ ] TLS signaling; текущий TCP server не поддерживает HTTPS/WSS.
- [ ] Production room TTL, replay/message-size bounds, rate limits и deployment observability.
- [ ] Bundled/deployed STUN/TURN configuration и short-lived TURN credential issuance.
- [ ] Signing/notarization workflow и Linux bundle targets/configuration.

### Проверки, которые ещё НЕ ВЫПОЛНЕНЫ

- [ ] Реальный macOS permission deny/grant/relaunch/retry на app/CLI.
- [ ] Реальный window/application/display capture и audio filtering/routing, включая несколько мониторов и смену/закрытие source.
- [ ] Реальный native capture → desktop viewer video/audio, autoplay/user gestures и sound output devices.
- [ ] Packaged Tauri build/install/launch, playback и cleanup при stop/reset/close/startup.
- [ ] Linux capture на GNOME Wayland, KDE Wayland и X11 после реализации backend.
- [ ] Linux WebView H.264/Opus playback и unsupported-codec behavior.
- [ ] STUN direct и TURN relay между разными сетями/NAT; восстановление после смены/потери connectivity.
- [ ] Sustained packet loss, congestion и weak-network recovery; initial-keyframe-drop probe этого не заменяет.
- [ ] Измерения latency, CPU, A/V skew и packet-loss behavior на реальном media path.

## Следующие шаги

1. Пройти реальный macOS capture/playback и permission QA по [checklist](docs/QA_CHECKLIST.md) и сохранить результаты с OS/build/source/network условиями.
2. Добавить A/V sync/latency monitoring, затем quality presets и adaptive pacing/bitrate.
3. Реализовать Linux Portal/PipeWire и video encoder, затем platform playback QA.
4. Завершить защищённый signaling/STUN/TURN deployment и cross-network QA.
5. Собрать/проверить release bundles и signing/notarization.

Детальные ограничения: [capture](docs/MACOS_CAPTURE.md), [media](docs/MEDIA_PIPELINE.md), [viewer](docs/VIEWER_PLAYBACK.md), [networking](docs/NETWORKING.md). Наличие manual checklist означает готовность инструкции, а не выполнение её проверок.
