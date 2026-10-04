# Win-CodexBar: Codex OAuth и несколько аккаунтов

Проверен исходный код коммита `3c30537e05d69cbcada416083af7d361baf28f83` на 2026-10-04. Только чтение исходников: приложение, внешний код, авторизация и запросы с токенами не запускались. Ссылки ниже закреплены на этот коммит.

## Вход

Собственного браузерного OAuth/PKCE или device-code клиента в этом модуле нет: Win-CodexBar создаёт `managed-homes/<UUID>`, запускает установленный `codex -c cli_auth_credentials_store="file" login` с `CODEX_HOME`, указывающим на этот каталог, и читает созданный CLI файл `auth.json`. На Windows процесс скрыт; есть отмена, таймаут 180 секунд и редактирование секретов в диагностическом выводе. Браузерную авторизацию и callback обслуживает Codex CLI. `--device-auth` runner не передаёт. Реальный протокол внутри установленного CLI этим исследованием не проверялся.

Источники: [создание home и ошибки входа](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/account_manager.rs#L65-L83), [runner и команда](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/login_runner.rs#L93-L130), [таймаут и чтение identity](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/account_manager.rs#L481-L548).

## Хранение и идентичность

- Каталог: `%APPDATA%/CodexBar/codex-accounts`; `accounts.json` содержит метаданные, пути homes и tombstones удалённых аккаунтов; `snapshots.json` — обычный JSON кеш квот. Токены находятся отдельно, в `managed-homes/<UUID>/auth.json`.
- `accounts.json` защищён DPAPI: сначала user scope, при ошибке — machine scope. Но сами `auth.json`, включая `auth-backups`, записываются обычными байтами, **без DPAPI**. В credential writer режим `0600` задаётся только на Unix; Windows ACL он явно не настраивает. Нельзя переносить утверждение «DPAPI-хранилище аккаунтов» на все OAuth-секреты.
- Сохраняются `access_token`, `refresh_token`, `id_token`, `account_id`, `last_refresh`. Email, `sub`, план и ChatGPT account ID извлекаются из payload `id_token` без проверки подписи; это локальные подсказки identity, не независимая проверка пользователя.
- UUID — локальный ID записи. Дедупликация сначала отвергает конфликт пользователя (`sub`/email), затем сравнивает выбранный workspace/account ID; без workspace используются `sub`, email, и только при отсутствии identity — путь home. Явный `workspace_account_id` отделён от дефолтного account ID в auth-файле и переживает rediscovery.

Источники: [пути](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/file_locations.rs#L44-L93), [stores](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/stores.rs#L39-L112), [DPAPI и fallback](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/secure_file.rs#L107-L149), [обычный credential writer](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/credentials.rs#L121-L201), [identity/JWT](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/credentials.rs#L204-L276), [модель и matches](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/models.rs#L115-L263).

## Выбор аккаунта и связь с CLI

Это полноценное переключение глобальной CLI-identity, а не только выбор карточки мониторинга. Ambient home всегда `~/.codex` (environment `CODEX_HOME` здесь не читается). При Switch приложение сохраняет уходящий ambient аккаунт в managed home, делает резервную копию ambient auth и заменяет `~/.codex/auth.json` токенами выбранного аккаунта. При совпадающей identity операция не меняет auth. Уже существующий managed home переиспользуется; более свежая managed копия не затирается заведомо старой ambient копией.

Дополнительно меняется глобальное состояние Codex Desktop (`creator_id`) и готовится перенос Desktop session state. Для применения к Desktop есть отдельный restart handoff с проверкой, что выбранный аккаунт ещё тот же. Это отдельная интеграция с другим приложением, которая для мониторинга нескольких аккаунтов Minibar не требуется.

Источники: [ambient home](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/file_locations.rs#L84-L93), [switch и materialize](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/account_manager.rs#L193-L335), [Desktop state](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/account_manager.rs#L346-L408), [restart handoff](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/switch_runtime.rs#L30-L117), [Desktop restart](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/codex_desktop.rs#L240-L298).

## Квоты всех аккаунтов и refresh

На каждом provider refresh Tauri запускает отдельные lanes для всех ambient/managed аккаунтов, параллельно с общим ограничением конкурентности и таймаутом. Кеш привязан к локальному UUID; неудача одной lane сохраняет её последний успешный snapshot. `codex-accounts-updated` синхронизирует UI. Settings поддерживают управление аккаунтами, а меню показывает все аккаунты, когда их больше одного. Старый ambient provider продолжает отдельно владеть главным Codex snapshot: авторы явно называют это переходным coexistence в ADR.

Quota request: `GET https://chatgpt.com/backend-api/wham/usage`, Bearer token и `ChatGPT-Account-Id` выбранного workspace. Возможен пользовательский backend из `config.toml`; workspace влияет на заголовок запроса, не переписывает default в auth. Refresh выполняется заранее при отсутствии `last_refresh` или возрасте более восьми дней и повторно после usage 401: `POST https://auth.openai.com/oauth/token`, `grant_type=refresh_token`, client ID `app_EMoamEEZ73f0CkXaXp7hrann`, scope `openid profile email`. Новые access/refresh/id tokens сохраняются обратно; отсутствующие поля ответа сохраняют предыдущие значения.

Внутри процесса refresh сериализован по каноническому пути `auth.json`; switch/reauth получают глобальный exclusive lock. Если managed identity сейчас активна в ambient, fetch использует ambient home и синхронизирует его с managed копией до и после запроса, выбирая более свежий `last_refresh`. Это снижает риск одновременной ротации двух копий одного refresh token внутри самого Win-CodexBar.

Источники: [lanes и кеш](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/apps/desktop-tauri/src-tauri/src/commands/codex_accounts.rs#L94-L197), [меню и события](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/apps/desktop-tauri/src/components/CodexAccountsMenu.tsx#L17-L97), [ADR](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/docs/adr/0003-multi-account-codex.md), [refresh policy](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/api.rs#L111-L167), [запросы usage/refresh](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/api.rs#L330-L453), [координация копий](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/fetch_coordination.rs#L22-L157).

## Ограничения, которые не стоит копировать

Следующее — выводы по коду, не воспроизведённые runtime-баги:

- Ошибки записи rotated credentials игнорируются; запрос продолжает работу с токеном в памяти. При одноразовом refresh token следующий запуск может остаться со старым токеном на диске.
- Ошибка самого refresh подавляется в `fetch_locked_snapshot`; вызывающая сторона может получить исходный usage 401 вместо точного `refresh_token_reused`/`refresh_token_invalidated` сообщения, которое сформировала функция refresh.
- Mutex lanes и global RW lock действуют только внутри процесса Win-CodexBar. Они не координируют refresh с отдельным Codex CLI/Desktop, который тоже читает ambient auth.
- Изолированный login передаёт `CODEX_HOME`, но не очищает унаследованные переменные авторизации и не меняет working directory. Это слабее изоляции уже реализованного Claude login в Minibar.
- Plaintext managed auth и дополнительные backups расширяют число мест хранения OAuth-секретов. DPAPI metadata этого не исправляет.

Основания: [игнорирование save/refresh ошибок](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/api.rs#L124-L167), [process-local locks](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/fetch_coordination.rs#L13-L39), [login env](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/login_runner.rs#L106-L120), [backup writer](https://github.com/nesszer/Win-CodexBar/blob/3c30537e05d69cbcada416083af7d361baf28f83/rust/src/codex_accounts/account_manager.rs#L325-L335).

## Что применимо к Codex Minibar

Локальный Codex клиент пока читает только access token/account ID из фиксированного `.codex/auth.json`: [src/codex.rs](../src/codex.rs#L298). [src/store/codex_accounts.rs](../src/store/codex_accounts.rs#L74) хранит атрибуцию истории usage, а не OAuth-профили. Эти две сущности следует сохранить раздельными.

Рекомендуемая адаптация: использовать существующую схему Claude profiles — ambient default плюс независимые именованные профили; изолированный CLI login для захвата полного refreshable session; перенос сессии в [user-scoped DPAPI vault](../src/secrets.rs); удаление временного login home; отдельный fetch/refresh для каждого включённого профиля; устойчивый profile ID и identity с учётом пользователя/workspace. Ротацию следует сохранять с сериализацией и проверкой версии сессии, а ошибку сохранения показывать явно. Локальные опорные реализации: [ClaudeProfile](../src/settings.rs#L755), [fetch всех профилей](../src/claude.rs#L412), [profile refresh](../src/claude/profile_oauth.rs#L127), [изолированный login](../src/claude/profile_oauth.rs#L293).

Это направление для следующей реализации, а не выполненное изменение приложения. Switch с переписыванием пользовательского `.codex/auth.json` и restart Codex Desktop стоит рассматривать отдельно, только если потребуется именно смена аккаунта CLI/Desktop.

## Minibar implementation follow-up

The subsequent implementation uses `src/codex/profile_oauth.rs` for isolated native CLI login and user-scoped DPAPI sessions, and `src/codex/profiles.rs` for independent quota snapshots. It retains the existing ambient Default login and keeps OAuth profiles separate from usage attribution. The npm native resolver covers the current `vendor/<target>/bin/codex.exe` layout and the legacy `vendor/<target>/codex/codex.exe` layout; see the [official CLI launcher](https://github.com/openai/codex/blob/main/codex-cli/bin/codex.js). Live login and WinUI rendering were not tested.
