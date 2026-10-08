### Codex Minibar application messages.
### English source; per-message fallback. Keep $parameters unchanged.

# src/provider_registry.rs
luna-reserve = Резерв Luna

# src/limits.rs
on-pace = В норме

# src/limits.rs
delta-in-deficit = Дефицит { $delta }%

# src/limits.rs
delta-in-reserve = Запас { $delta }%

# src/notifications.rs
msg-5-hour-limit-started = 5-часовой лимит активирован

# src/notifications.rs
msg-5-hour-limit-reset-and-activated = 5-часовой лимит сброшен и активирован

# src/notifications.rs
msg-5-hour-limit-reset = 5-часовой лимит сброшен

# src/notifications.rs
weekly-limit-reset = Недельный лимит сброшен

# src/notifications.rs
name-5-hour = { $name }: 5 часов

# src/notifications.rs
label-limit-is-low = Остаток лимита «{ $label }» низкий

# src/popup_window/formatting.rs
expired-at-time = истёк в { $time }

# src/popup_window/formatting.rs
expired-at-time-6c39ff = истёк в { $time } { $v0 }

# src/popup_window/state.rs
never = Никогда

# src/popup_window/formatting.rs
unlimited = Без ограничений

# src/popup_window/ui/activity.rs
unavailable = Недоступно

# src/popup_window/formatting.rs
days-d-hours-h = { $days } д { $hours } ч

# src/popup_window/formatting.rs
days-d = { $days } д

# src/popup_window/formatting.rs
hours-h-minutes-m = { $hours } ч { $minutes } мин

# src/popup_window/formatting.rs
hours-h = { $hours } ч

# src/popup_window/formatting.rs
minutes-m = { $minutes } мин

# src/popup_window/formatting.rs
waiting-for-first-update = Ожидание первого обновления

# src/popup_window/formatting.rs
just-now = только что

# src/popup_window/formatting.rs
seconds-seconds-ago = { $seconds } с назад

# src/popup_window/formatting.rs
minutes-ago = { $v0 } мин назад

# src/popup_window/formatting.rs
updated-elapsed = Обновлено { $elapsed }

# src/popup_window/state.rs
the-request-timed-out-try-refreshing-again = Время ожидания запроса истекло. Попробуйте обновить ещё раз.

# src/popup_window/state.rs
the-secure-connection-could-not-be-verified-see-log-for-details = Не удалось проверить защищённое соединение. Подробности — в журнале.

# src/popup_window/state.rs
the-provider-closed-the-connection-try-refreshing-again = Провайдер закрыл соединение. Попробуйте обновить ещё раз.

# src/popup_window/state.rs
the-provider-s-address-could-not-be-resolved-check-your-connectio = Не удалось определить адрес провайдера. Проверьте подключение.

# src/popup_window/state.rs
could-not-connect-to-the-provider-check-your-connection-and-try-a = Не удалось подключиться к провайдеру. Проверьте подключение и повторите попытку.

# src/popup_window/state.rs
the-management-key-was-rejected-update-it-in-settings = Ключ управления отклонён. Обновите его в настройках.

# src/popup_window/state.rs
authentication-failed-sign-in-again-or-update-the-provider-key = Ошибка авторизации. Войдите снова или обновите ключ провайдера.

# src/popup_window/state.rs
access-denied-by-the-provider-http-403 = Провайдер отказал в доступе (HTTP 403).

# src/popup_window/state.rs
too-many-requests-wait-a-few-minutes-before-refreshing-again = Слишком много запросов. Подождите несколько минут перед обновлением.

# src/popup_window/state.rs
the-provider-is-temporarily-unavailable-try-again-later = Провайдер временно недоступен. Повторите попытку позже.

# src/popup_window/state.rs
the-provider-rejected-the-request-see-log-for-details = Провайдер отклонил запрос. Подробности — в журнале.

# src/popup_window/state.rs
the-requested-resource-was-not-found-see-log-for-details = Запрошенный ресурс не найден. Подробности — в журнале.

# src/popup_window/state.rs
the-provider-returned-an-unexpected-response-try-refreshing-again = Провайдер вернул неожиданный ответ. Попробуйте обновить ещё раз.

# src/popup_window/state.rs
the-request-failed-see-log-for-details = Запрос не выполнен. Подробности — в журнале.

# src/popup_window/ui/activity.rs
input = Вход

# src/popup_window/ui/activity.rs
cache = Кэш

# src/popup_window/ui/usage.rs
output = Выход

# src/popup_window/ui/activity.rs
input-uncached = Вход без кэша

# src/popup_window/ui/usage.rs
cached-input = Кэшированный вход

# src/popup_window/ui/activity.rs
value-partially-priced = { $value } (неполная стоимость)

# src/popup_window/ui/activity.rs
could-not-load-model-data-error = Не удалось загрузить данные моделей: { $error }

# src/popup_window/ui/activity.rs
usage-activity = Активность использования

# src/popup_window/ui/activity.rs
waiting-for-cursor-s-usage-export-refresh-to-retry = Ожидание экспорта использования Cursor. Обновите, чтобы повторить попытку.

# src/popup_window/ui/activity.rs
model-data-unavailable = Данные моделей недоступны

# src/popup_window/ui/activity.rs
loading-models = Загрузка моделей…

# src/popup_window/ui/tooltip.rs
no-model-data = Нет данных моделей

# src/popup_window/ui/activity.rs
no-usage-data = Нет данных использования

# src/popup_window/ui/activity.rs
no-series-selected = Не выбраны ряды данных

# src/popup_window/ui/activity.rs
no-tokens-in-this-period = В этом периоде нет токенов категории «{ $v0 }»

# src/popup_window/ui/activity.rs
loading-model-breakdown = Загрузка разбивки по моделям

# src/popup_window/ui/activity.rs
group-tokens-or-cost-by-model = Группировать токены или расходы по моделям

# src/popup_window/ui/usage.rs
model = Модель

# src/popup_window/ui/usage.rs
tokens = Токены

# src/popup_window/ui/usage.rs
cost = Расход

# src/popup_window/ui/activity.rs
no-cost-data-for-this-period = Нет данных расходов за этот период

# src/popup_window/ui/activity.rs
no-token-data-for-this-period = Нет данных токенов за этот период

# src/popup_window/ui/activity.rs
daily-cost-in-usd = Расход по дням в USD

# src/popup_window/ui/activity.rs
daily-token-volume = Объём токенов по дням

# src/popup_window/ui/activity.rs
of-total-models-scroll-for-more = { $v0 }–{ $v1 } из { $total } моделей · Прокрутите, чтобы увидеть остальные

# src/popup_window/ui/activity.rs
cost-usd-by-model = Расход (USD) по моделям

# src/popup_window/ui/activity.rs
tokens-by-model = Токены по моделям

# src/settings.rs
today = Сегодня

# src/popup_window/ui/activity.rs
last-period-days = Последние { $period } дн.

# src/popup_window/ui/activity.rs
no-data = Нет данных

# src/popup_window/ui/cards.rs
loading-usage-statistics = Загрузка статистики использования…

# src/popup_window/ui/cards.rs
available-balance = Доступный баланс

# src/popup_window/ui/cards.rs
resets-in = До сброса

# src/popup_window/ui/cards.rs
session-not-started = Сессия не начата

# src/popup_window/ui/cards.rs
expires-in = До истечения

# src/popup_window/ui/cards.rs
usage = Расход:

# src/settings_window/providers/page.rs
remove-key = Удалить ключ

# src/popup_window/ui/cards.rs
msg-1-banked-reset = 1 запасной сброс

# src/popup_window/ui/cards.rs
count-banked-resets = { $count ->
    [one] { $count } запасной сброс
    [few] { $count } запасных сброса
   *[many] { $count } запасных сбросов
    }

# src/popup_window/ui/cards.rs
available-to-use = Можно использовать

# src/popup_window/ui/cards.rs
no-expiration-date = Без срока действия

# src/popup_window/ui/cards.rs
banked-reset = Запасной сброс { $v0 }

# src/popup_window/ui/cards.rs
codex-limits = Лимиты Codex

# src/popup_window/ui/cards.rs
open-announcement-source = Открыть источник объявления

# src/popup_window/ui/cards.rs
source-not-provided = Источник не указан

# src/popup_window/ui/cards.rs
tibo-reset = Сброс Tibo™

# src/settings_window/customize.rs
home = Главная

# src/popup_window/ui/usage.rs
usage-0bb186 = Использование

# src/popup_window/ui/footer.rs
settings = Настройки

# src/popup_window/ui/footer.rs
install-update = Установить обновление

# src/popup_window/ui/footer.rs
refreshing-limits-and-usage = Обновление лимитов и использования…

# src/popup_window/ui/footer.rs
updated = Обновлено{ " " }

# src/popup_window/ui/footer.rs
refresh-last-updated-relative = Обновить | Последнее обновление: { $relative }

# src/popup_window/ui/home.rs
edit-home = Изменить главную
drag-to-reorder = Перетащите, чтобы изменить порядок

# src/popup_window/ui/home.rs
drop-here = Перетащите сюда

# src/settings_window/general.rs
usage-stats = Статистика использования

# src/popup_window/ui/usage.rs
loading-usage = Загрузка использования…

# src/settings_window/window.rs
update-failed = Ошибка обновления

# src/popup_window/ui/root.rs
something-went-wrong = Что-то пошло не так

# src/popup_window/ui/root.rs
no-providers-enabled = Нет включённых провайдеров

# src/popup_window/ui/root.rs
turn-one-on-in-settings-providers = Включите провайдера в разделе «Настройки > Провайдеры».

# src/popup_window/ui/root.rs
sign-in-again-to-keep-limits-updating = Войдите снова, чтобы продолжить обновление лимитов.

# src/settings_window/providers/page.rs
sign-in-again = Войти снова

# src/popup_window/ui/tooltip.rs
total = Всего

# src/popup_window/ui/usage.rs
enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = Включите провайдера в настройках и добавьте его в статистику использования, чтобы видеть локальное использование API.

# src/popup_window/ui/usage.rs
hourly-cost = Расход по часам

# src/popup_window/ui/usage.rs
hourly-processed-tokens = Обработанные токены по часам

# src/popup_window/ui/usage.rs
no-activity-in-this-range = Нет активности за этот период

# src/popup_window/ui/usage.rs
day = День

# src/popup_window/ui/usage.rs
breakdown = Разбивка

# src/popup_window/ui/usage.rs
totals = Итоги

# src/popup_window/ui/usage.rs
processed-tokens = Обработанные токены

# src/popup_window/ui/usage.rs
uncached-input = Вход без кэша

# src/popup_window/ui/usage.rs
cache-savings = Экономия кэша

# src/popup_window/ui/usage.rs
share = Доля

# src/popup_window/ui/usage.rs
hour = Час

# src/usage_overview.rs
sat = Сб

# src/usage_overview.rs
sun = Вс

# src/provider_registry.rs
msg-5h-session = Сессия 5 ч

# src/provider_registry.rs
weekly = Неделя

# src/popup_window/ui/activity.rs
split-type = Тип

# src/popup_window/ui/activity.rs
split-by-token-type = Разбить столбцы по типу токенов

# src/provider_registry.rs
cursor-models = Модели Cursor

# src/settings_window/tray.rs
other-models = Другие модели

# src/provider_registry.rs
all-models = Все модели

# src/settings_window/tray.rs
grok-bot = Бот Grok

# src/provider_registry.rs
monthly = Месяц

# src/provider_registry.rs
spending-limit = Лимит расходов

# src/provider_registry.rs
gemini = Gemini

# src/provider_registry.rs
claude-gpt = Claude + GPT

# src/provider_registry.rs
credits = Кредиты

# src/provider_registry.rs
monthly-credits = Кредиты за месяц

# src/settings.rs
codex = Codex

# src/settings.rs
claude = Claude

# src/settings.rs
cursor = Cursor

# src/settings.rs
opencode-zen = OpenCode Zen

# src/settings.rs
opencode-go = OpenCode Go

# src/settings.rs
openrouter = OpenRouter

# src/settings.rs
antigravity = Antigravity

# src/settings.rs
grok = Grok

# src/settings.rs
kiro = Kiro

# src/provider_registry.rs
banked-resets = Запасные сбросы

# src/provider_registry.rs
usage-stats-7b9e1a = Статистика использования

# src/provider_registry.rs
spending = Расходы

# src/settings.rs
separate-tabs = Отдельные вкладки

# src/settings.rs
grouped-switcher = Группы с переключателем

# src/settings.rs
grouped-all-accounts = Группы со всеми аккаунтами

# src/settings.rs
openrouter-account = Аккаунт OpenRouter

# src/settings.rs
yesterday = Вчера

# src/settings.rs
history-retention-must-be-between-1-and-365-days = Срок хранения истории должен быть от 1 до 365 дней

# src/settings.rs
session-low-usage-threshold-must-be-between-1-and-99-percent = Порог остатка сессионного лимита должен быть от 1 до 99 процентов

# src/settings.rs
weekly-low-usage-threshold-must-be-between-1-and-99-percent = Порог остатка недельного лимита должен быть от 1 до 99 процентов

# src/settings_window/about.rs
check-github-for-a-new-version = Проверить новую версию на GitHub

# src/settings_window/about.rs
checking-for-updates = Проверка обновлений…

# src/settings_window/about.rs
you-re-up-to-date = Установлена последняя версия

# src/settings_window/about.rs
update-available = Доступно обновление { $v0 }

# src/settings_window/about.rs
installing-update = Установка обновления…

# src/settings_window/about.rs
couldn-t-check-for-updates = Не удалось проверить обновления

# src/settings_window/about.rs
version = Версия { $v0 }

# src/settings_window/about.rs
usage-limits-in-the-windows-tray = Лимиты использования в трее Windows.

# src/settings_window/about.rs
a-new-release-is-ready-to-install = Новая версия готова к установке.

# src/settings_window/about.rs
what-s-new = Что нового

# src/settings_window/about.rs
update = Обновить

# src/settings_window/about.rs
check-for-updates = Проверить обновления

# src/settings_window/about.rs
check-for-updates-on-startup = Проверять обновления при запуске

# src/settings_window/about.rs
notify-when-a-new-version-is-found = Уведомлять о новой версии

# src/settings_window/about.rs
github = GitHub

# src/settings_window/about.rs
source-code = Исходный код

# src/settings_window/about.rs
releases = Версии

# src/settings_window/about.rs
see-what-s-new = Посмотреть изменения

# src/settings_window/about.rs
report-an-issue = Сообщить о проблеме

# src/settings_window/about.rs
found-a-bug = Нашли ошибку?

# src/settings_window/about.rs
author = Автор

# src/settings_window/onboarding.rs
updates = Обновления

# src/settings_window/about.rs
resources = Ресурсы

# src/usage_overview.rs
mon = Пн

# src/usage_overview.rs
tue = Вт

# src/usage_overview.rs
wed = Ср

# src/usage_overview.rs
thu = Чт

# src/usage_overview.rs
fri = Пт

# src/settings_window/activation.rs
every-day = Каждый день

# src/settings_window/activation.rs
weekdays = Будни

# src/settings_window/activation.rs
weekends = Выходные

# src/settings_window/activation.rs
no-days = Дни не выбраны

# src/settings_window/onboarding.rs
start-5-hour-sessions-automatically = Автоматически начинать 5-часовые сессии

# src/settings_window/activation.rs
starts-a-new-session-as-soon-as-a-window-is-available-instead-of = Начинает новую сессию, как только доступен лимит, не дожидаясь первого запроса. Каждый аккаунт использует собственную авторизацию.

# src/settings_window/activation.rs
add-codex-or-claude-in-providers-first = Сначала добавьте Codex или Claude в разделе «Провайдеры».

# src/settings_window/activation.rs
off-in-providers = Выключен в разделе «Провайдеры»

# src/settings_window/activation.rs
quiet-periods = Тихие периоды

# src/settings_window/activation.rs
don-t-auto-start-sessions-during-these-times = Не начинать сессии автоматически в это время.

# src/settings_window/providers/dialog.rs
add = Добавить

# src/settings_window/activation.rs
all-day = Весь день

# src/settings_window/activation.rs
from = С

# src/settings_window/activation.rs
until = До

# src/settings_window/activation.rs
scheduled-activations = Активации по расписанию

# src/settings_window/activation.rs
start-a-5-hour-session-at-a-set-time = Начинать 5-часовую сессию в заданное время.

# src/settings_window/activation.rs
time = Время

# src/settings_window/activation.rs
turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = Сначала включите Codex или Claude в разделе «Провайдеры» с авторизацией из папки конфигурации.

# src/settings_window/activation.rs
no-quiet-periods-yet = Тихих периодов пока нет.

# src/settings_window/activation.rs
no-scheduled-activations-yet = Активаций по расписанию пока нет.

# src/settings_window/activation.rs
unknown-provider = Неизвестный провайдер

# src/settings_window/activation.rs
remove-quiet-period = Удалить тихий период

# src/settings_window/activation.rs
remove-activation = Удалить активацию

# src/settings_window/tray.rs
provider = Провайдер

# src/settings_window/activation.rs
days = Дни

# src/settings_window/advanced.rs
export-settings = Экспорт настроек

# src/settings_window/advanced.rs
save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = Сохранить все настройки в файл .toml. Сохранённые ключи останутся в хранилище пользователя Windows.

# src/settings_window/advanced.rs
export = Экспортировать

# src/settings_window/advanced.rs
import-settings = Импорт настроек

# src/settings_window/advanced.rs
replace-the-current-settings-with-a-previously-exported-file = Заменить текущие настройки ранее экспортированным файлом.

# src/settings_window/advanced.rs
import = Импортировать

# src/settings_window/advanced.rs
clear-usage-data = Очистить данные использования

# src/settings_window/advanced.rs
delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = Удалить собранную историю использования. При следующем сканировании она будет восстановлена из локальных журналов провайдеров.

# src/settings_window/advanced.rs
clear = Очистить

# src/settings_window/advanced.rs
usage-data-clear-failed = Не удалось очистить данные использования

# src/settings_window/advanced.rs
the-background-worker-is-unavailable = Фоновый обработчик недоступен.

# src/settings_window/advanced.rs
usage-data-cleared = Данные использования очищены.

# src/settings_window/advanced.rs
reset-all-settings = Сбросить все настройки

# src/settings_window/advanced.rs
restore-every-default-and-start-the-welcome-flow-again = Восстановить значения по умолчанию и снова открыть первоначальную настройку.

# src/settings_window/advanced.rs
reset = Сбросить

# src/settings_window/advanced.rs
backup = Резервная копия

# src/settings_window/advanced.rs
data = Данные

# src/settings_window/advanced.rs
settings-exported = Настройки экспортированы.

# src/settings_window/advanced.rs
settings-export-failed = Не удалось экспортировать настройки

# src/settings_window/advanced.rs
settings-imported = Настройки импортированы.

# src/settings_window/advanced.rs
settings-import-failed = Не удалось импортировать настройки

# src/settings_window/advanced.rs
settings-reset-failed = Не удалось сбросить настройки

# src/settings_window/advanced.rs
reset-all-settings-bbfe66 = Сбросить все настройки?

# src/settings_window/advanced.rs
every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = Все настройки вернутся к значениям по умолчанию, и откроется первоначальная настройка. Сохранённые ключи и данные использования останутся.

# src/settings_window/troubleshoot.rs
cancel = Отмена

# src/settings_window/appearance.rs
windows = Windows

# src/settings_window/appearance.rs
light = Светлая

# src/settings_window/appearance.rs
dark = Тёмная

# src/settings_window/appearance.rs
color-theme = Тема оформления

# src/settings_window/appearance.rs
applies-to-settings-the-popup-and-its-tray-menu = Применяется к настройкам, всплывающему окну и меню трея.

# src/settings_window/appearance.rs
accent-color = Акцентный цвет

# src/settings_window/appearance.rs
windows-follows-your-system-accent = Windows использует системный акцентный цвет.

# src/settings_window/appearance.rs
icons-style = Стиль значков

# src/settings_window/appearance.rs
font = Шрифт

# src/settings_window/appearance.rs
any-font-installed-on-this-pc = Любой шрифт, установленный на этом компьютере.

# src/settings_window/appearance.rs
windows-default = По умолчанию Windows

# src/settings_window/appearance.rs
glyph-style-in-the-settings-sidebar = Стиль значков в боковой панели настроек.

# src/settings_window/appearance.rs
colored = Цветные

# src/settings_window/tray.rs
monochrome = Монохромные

# src/settings_window/appearance.rs
time-format = Формат времени

# src/settings_window/appearance.rs
msg-12-hour = 12 часов

# src/settings_window/appearance.rs
msg-24-hour = 24 часа

# src/settings_window/appearance.rs
popup-background = Фон всплывающего окна

# src/settings_window/appearance.rs
acrylic = Акрил

# src/settings_window/appearance.rs
mica = Mica

# src/settings_window/appearance.rs
solid = Сплошной

# src/settings_window/appearance.rs
bottom-bar-size = Размер нижней панели

# src/settings_window/appearance.rs
comfortable = Просторный

# src/settings_window/appearance.rs
compact = Компактный

# src/settings_window/appearance.rs
popup-corner-radius = Радиус углов всплывающего окна

# src/settings_window/appearance.rs
animation-effects = Эффекты анимации

# src/settings_window/appearance.rs
glide-transitions-in-the-popup-and-settings-windows-own-animation = Плавные переходы во всплывающем окне и настройках. Также учитывается системная настройка анимации Windows.

# src/settings_window/appearance.rs
popup = Всплывающее окно

# src/settings_window/appearance.rs
motion = Анимация

# src/settings_window/appearance.rs
blue = Синий

# src/settings_window/appearance.rs
purple = Фиолетовый

# src/settings_window/appearance.rs
pink = Розовый

# src/settings_window/appearance.rs
red = Красный

# src/settings_window/appearance.rs
orange = Оранжевый

# src/settings_window/appearance.rs
green = Зелёный

# src/settings_window/appearance.rs
teal = Бирюзовый

# src/settings_window/customize.rs
use-two-columns = Использовать два столбца

# src/settings_window/customize.rs
several-accounts-of-one-provider = Несколько аккаунтов одного провайдера

# src/settings_window/customize.rs
separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = Отдельные вкладки — по вкладке на каждый экземпляр. Группы — одна вкладка на провайдера с переключателем аккаунтов или всеми аккаунтами друг под другом.

# src/settings_window/customize.rs
use-monochrome-icons = Использовать монохромные значки

# src/settings_window/customize.rs
draw-provider-marks-in-the-popup-without-brand-colors = Показывать значки провайдеров без фирменных цветов.

# src/settings_window/onboarding.rs
show-used-instead-of-remaining = Показывать использованное вместо остатка

# src/settings_window/customize.rs
show-usage-in-values-when-possible = Показывать использование в числах, если доступно

# src/settings_window/customize.rs
adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = Добавляет точные значения использования и лимита рядом с процентами, если провайдер их сообщает.

# src/settings_window/onboarding.rs
show-usage-pace = Показывать темп использования

# src/settings_window/onboarding.rs
marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = Показывает, расходуется ли лимит быстрее или медленнее равномерного темпа.

# src/settings_window/customize.rs
use-legacy-usage-cards = Использовать старые карточки лимитов

# src/settings_window/customize.rs
show-the-older-layout-with-a-header-a-thin-bar-and-a-footer = Показывать прежний вид с заголовком, тонкой полосой и нижней строкой.

# src/settings_window/onboarding.rs
show-account-name = Показывать имя аккаунта

# src/settings_window/customize.rs
show-on-home-tab = Показывать на главной

# src/settings_window/customize.rs
layout = Расположение

# src/settings_window/customize.rs
donut = Кольцевая диаграмма

# src/settings_window/customize.rs
cards = Карточки

# src/settings_window/customize.rs
tabs = Вкладки

# src/settings_window/customize.rs
usage-widget = Виджет использования

# src/settings_window/customize.rs
card = Карточка

# src/settings_window/customize.rs
tab = Вкладка

# src/settings_window/customize.rs
popup-cards = Карточки всплывающего окна

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = Выберите карточки аккаунта для главной и его собственной вкладки.

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = Выберите карточки аккаунта для его вкладки. Включите показ на главной, чтобы выбрать карточки главной.

# src/settings_window/general.rs
msg-30-seconds = 30 секунд

# src/settings_window/general.rs
msg-1-minute = 1 минута

# src/settings_window/general.rs
msg-2-minutes = 2 минуты

# src/settings_window/general.rs
msg-3-minutes = 3 минуты

# src/settings_window/general.rs
msg-5-minutes = 5 минут

# src/settings_window/general.rs
msg-10-minutes = 10 минут

# src/settings_window/general.rs
msg-15-minutes = 15 минут

# src/settings_window/general.rs
msg-30-minutes = 30 минут

# src/settings_window/general.rs
msg-45-minutes = 45 минут

# src/settings_window/general.rs
msg-60-minutes = 60 минут

# src/settings_window/onboarding.rs
start-with-windows = Запускать вместе с Windows

# src/settings_window/general.rs
open-codex-minibar-in-the-tray-when-you-sign-in = Открывать Codex Minibar в трее при входе в систему.

# src/settings_window/onboarding.rs
refresh-limits = Обновление лимитов

# src/settings_window/general.rs
how-often-provider-quotas-are-read = Как часто считываются лимиты провайдеров.

# src/settings_window/general.rs
enable-usage-stats = Включить статистику использования

# src/settings_window/general.rs
scan-local-provider-history-for-the-usage-tab-and-cost-totals = Сканировать локальную историю провайдеров для вкладки использования и подсчёта стоимости.

# src/settings_window/general.rs
collection-period = Период сбора

# src/settings_window/general.rs
how-often-local-provider-history-is-scanned = Как часто сканируется локальная история провайдеров.

# src/settings_window/general.rs
check-for-confirmed-tibo-resets = Проверять подтверждённые сбросы Tibo

# src/settings_window/general.rs
reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = Читает публичную ленту приложения на GitHub и сохраняет последнее объявление в кэше.

# src/settings_window/general.rs
notify-when-new-reset-info-arrives = Уведомлять о новых сведениях о сбросе

# src/settings_window/general.rs
shows-a-notification-when-the-feed-reports-a-possible-reset-never = Показывает уведомление, когда в ленте появляется возможный сброс, а не в момент сброса.

# src/settings_window/general.rs
check-every = Проверять каждые

# src/settings_window/general.rs
the-feed-is-also-checked-immediately-when-the-app-starts-or-this = Лента также проверяется сразу при запуске приложения или включении этой настройки.

# src/settings_window/general.rs
msg-1-hour = 1 час

# src/settings_window/general.rs
msg-3-hours = 3 часа

# src/settings_window/general.rs
msg-6-hours = 6 часов

# src/settings_window/general.rs
msg-12-hours = 12 часов

# src/settings_window/general.rs
msg-24-hours = 24 часа

# src/settings_window/general.rs
tibo-resets = Сбросы Tibo™

# src/settings_window/general.rs
enable-a-provider-in-the-providers-tab-to-include-it-here = Включите провайдера на вкладке «Провайдеры», чтобы добавить его сюда.

# src/settings_window/general.rs
add-a-management-key = Добавить ключ управления

# src/settings_window/general.rs
usage-statistics-are-off-on-this-provider-s-page = Статистика использования выключена на странице провайдера

# src/settings_window/general.rs
included-providers = Учитываемые провайдеры

# src/settings_window/general.rs
choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = Выберите аккаунты для вкладки использования и общего итога на главной на этом компьютере. Исключённые аккаунты продолжают собирать данные и показывают их на своей странице.

# src/settings_window/integrations.rs
download-the-latest-stream-deck-companion-from-github-and-open-it = Скачать последнюю версию дополнения Stream Deck с GitHub и открыть установщик.

# src/settings_window/integrations.rs
downloading-the-latest-stream-deck-companion-from-github = Скачивание последней версии дополнения Stream Deck с GitHub...

# src/settings_window/integrations.rs
opening-the-stream-deck-installer = Открытие установщика Stream Deck...

# src/settings_window/integrations.rs
the-installer-is-open-finish-the-installation-in-stream-deck = Установщик открыт. Завершите установку в Stream Deck.

# src/settings_window/integrations.rs
the-plugin-could-not-be-downloaded-or-opened-try-again = Не удалось скачать или открыть плагин. Повторите попытку.

# src/settings_window/integrations.rs
downloading = Скачивание...

# src/settings_window/integrations.rs
opening = Открытие...

# src/settings_window/integrations.rs
install-plugin = Установить плагин

# src/settings_window/integrations.rs
stream-deck-companion = Дополнение Stream Deck

# src/settings_window/integrations.rs
check-your-connection-then-try-again = Проверьте подключение и повторите попытку.

# src/settings_window/providers/page.rs
on = Вкл.

# src/settings_window/providers/page.rs
off = Выкл.

# src/settings_window/kit.rs
more-options = Другие действия

# src/settings_window/kit.rs
custom-color = Свой цвет

# src/settings_window/troubleshoot.rs
run-troubleshoot-with-ai = Диагностика с ИИ

# src/settings_window/log.rs
let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = Разрешить установленному ИИ-инструменту прочитать журнал и разобраться в проблеме.

# src/settings_window/log.rs
choose-tool = Выбрать инструмент

# src/settings_window/log.rs
application-log = Журнал приложения

# src/settings_window/log.rs
log-txt-in-the-app-data-folder = log.txt в папке данных приложения.

# src/settings_window/log.rs
open-log-txt = Открыть log.txt

# src/settings_window/log.rs
could-not-open-log-txt = Не удалось открыть log.txt

# src/settings_window/log.rs
open-logs-folder = Открыть папку журналов

# src/settings_window/log.rs
could-not-open-logs-folder = Не удалось открыть папку журналов

# src/settings_window/log.rs
no-log-events-yet = В журнале пока нет событий.

# src/settings_window/log.rs
live-tail = В реальном времени

# src/settings_window/log.rs
no-supported-ai-tool-found = Поддерживаемый ИИ-инструмент не найден

# src/settings_window/log.rs
install-codex-or-claude-code-and-make-it-available-to-minibar = Установите Codex или Claude Code и сделайте его доступным для Minibar.

# src/settings_window/window.rs
general = Общие

# src/settings_window/window.rs
appearance = Оформление

# src/settings_window/window.rs
providers = Провайдеры

# src/settings_window/window.rs
customize = Вид окна

# src/settings_window/window.rs
limit-activation = Активация лимитов

# src/settings_window/window.rs
tray = Трей

# src/settings_window/window.rs
notifications = Уведомления

# src/settings_window/window.rs
advanced = Дополнительно

# src/settings_window/window.rs
log = Журнал

# src/settings_window/window.rs
integrations = Интеграции

# src/settings_window/nav.rs
about-updates = О приложении и обновления

# src/settings_window/onboarding.rs
successful-activations = Успешные активации

# src/settings_window/onboarding.rs
failed-activations = Неудачные активации

# src/settings_window/onboarding.rs
when-limits-reset = При сбросе лимитов

# src/settings_window/onboarding.rs
when-5-hour-remaining-hits = Когда остаток 5-часового лимита достигает { $v0 }%

# src/settings_window/onboarding.rs
when-weekly-remaining-hits = Когда остаток недельного лимита достигает { $v0 }%

# src/settings_window/onboarding.rs
low-usage = Низкий остаток

# src/settings_window/notifications.rs
shows-a-notification-once-per-window-when-the-remaining-share-dro = Уведомляет один раз за период лимита, когда остаток опускается до порога.

# src/settings_window/notifications.rs
threshold = Порог

# src/settings_window/onboarding.rs
found-in-opencode-auth-or-local-history = Найден в авторизации OpenCode или локальной истории.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-set-up-elsewhere = Не найден. Включите, если он настроен в другом месте.

# src/settings_window/onboarding.rs
account-credentials-are-already-set = Данные авторизации аккаунта уже заданы.

# src/settings_window/onboarding.rs
optional-add-accounts-later-in-providers = Необязательно. Аккаунты можно добавить позже в разделе «Провайдеры».

# src/settings_window/onboarding.rs
found-an-official-agy-sign-in-on-this-pc = На этом компьютере найдена официальная авторизация agy.

# src/settings_window/onboarding.rs
not-found-sign-in-with-agy-before-enabling-it = Не найдена. Войдите через agy перед включением.

# src/settings_window/onboarding.rs
found-an-official-grok-cli-sign-in-on-this-pc = На этом компьютере найдена официальная авторизация Grok CLI.

# src/settings_window/onboarding.rs
not-found-run-grok-login-before-enabling-it = Не найдена. Выполните grok login перед включением.

# src/settings_window/onboarding.rs
found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Найдены Kiro IDE, Kiro Crew или авторизованный Kiro CLI.

# src/settings_window/onboarding.rs
not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = Не найдены. Установите Kiro IDE или Kiro Crew либо войдите в Kiro CLI.

# src/settings_window/onboarding.rs
found-on-this-pc = Найден на этом компьютере.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-installed-somewhere-else = Не найден. Включите, если он установлен в другом месте.

# src/settings_window/onboarding.rs
setup-could-not-be-saved = Не удалось сохранить настройку

# src/settings_window/onboarding.rs
detected = Обнаружен

# src/settings_window/onboarding.rs
starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = Начинает новую сессию Codex или Claude, как только доступен лимит, не дожидаясь первого запроса.

# src/settings_window/onboarding.rs
collect-usage-data = Собирать данные использования

# src/settings_window/onboarding.rs
scans-local-provider-history-for-usage-stats = Сканирует локальную историю провайдеров для статистики использования.

# src/settings_window/onboarding.rs
startup = Запуск

# src/settings_window/providers/page.rs
features = Возможности

# src/settings_window/onboarding.rs
customization = Настройка вида

# src/settings_window/onboarding.rs
when-a-new-version-is-found = При обнаружении новой версии

# src/settings_window/onboarding.rs
activity = Активность

# src/settings_window/onboarding.rs
choose-providers = Выберите провайдеров

# src/settings_window/onboarding.rs
we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = Мы включили провайдеров, найденных на этом компьютере. Это можно изменить позже.

# src/settings_window/onboarding.rs
general-settings = Общие настройки

# src/settings_window/onboarding.rs
you-can-change-these-later-in-settings = Их можно изменить позже в настройках.

# src/settings_window/onboarding.rs
turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = Выключите ненужные уведомления. Это можно изменить позже в настройках.

# src/settings_window/window.rs
back = Назад

# src/settings_window/onboarding.rs
continue = Продолжить

# src/settings_window/tray.rs
done = Готово

# src/settings_window/providers/dialog.rs
details = Подробности

# src/settings_window/providers/dialog.rs
added = Добавлен

# src/settings_window/providers/dialog.rs
reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Читает сессионный и недельный лимиты подписки Claude без авторизации Claude Code.

# src/settings_window/providers/dialog.rs
msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. Откройте Claude в отдельном профиле браузера или приватном окне. Войдите в нужный аккаунт и проверьте его адрес почты в настройках Claude.

# src/settings_window/providers/dialog.rs
open-claude-ai = Открыть claude.ai

# src/settings_window/providers/dialog.rs
msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. В Chrome или Edge нажмите F12. Откройте Application > Storage > Cookies и выберите https://claude.ai.

# src/settings_window/providers/dialog.rs
msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. Найдите sessionKey. Скопируйте его значение Value, а не имя или всю таблицу cookie, и вставьте в поле выше.

# src/settings_window/providers/dialog.rs
how-to-view-cookies-in-chrome = Как посмотреть cookie в Chrome

# src/settings_window/providers/dialog.rs
a-cookie-header-containing-sessionkey-also-works-when-the-session = Подойдёт и заголовок Cookie с sessionKey. Когда сессия истечёт, вставьте новый.

# src/settings_window/providers/dialog.rs
use-the-access-token-from-a-claude-code-subscription-login-miniba = Используйте токен доступа авторизации подписки Claude Code. Minibar не может обновлять вставленный токен; по возможности выбирайте источник «Папка конфигурации».

# src/settings_window/providers/dialog.rs
copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = Скопируйте только claudeAiOauth.accessToken (начинается с sk-ant-oat) из файла .credentials.json этой авторизации, без кавычек. Не используйте claude setup-token: у него может не быть доступа к использованию.

# src/settings_window/providers/dialog.rs
claude-code-log-in-with-multiple-accounts = Claude Code: вход в несколько аккаунтов

# src/settings_window/providers/dialog.rs
requires-a-claude-subscription-login-with-usage-access-api-keys-a = Нужна авторизация подписки Claude с доступом к использованию. API-ключи и ключи Admin API не показывают лимиты подписки.

# src/settings_window/providers/dialog.rs
this-provider = этот провайдер

# src/settings_window/providers/dialog.rs
track-another-account-or-a-provider-minibar-has-not-shown-yet = Отслеживайте ещё один аккаунт или провайдера, которого Minibar пока не показывал.

# src/settings_window/providers/dialog.rs
e-g-work = например, Рабочий

# src/settings_window/providers/page.rs
name = Название

# src/settings_window/providers/dialog.rs
shown-on-its-tab-home-card-tray-and-notifications = Показывается на вкладке, карточке главной, в трее и уведомлениях.

# src/settings_window/providers/page.rs
badge = Метка

# src/settings_window/providers/dialog.rs
auto = Авто

# src/settings_window/providers/page.rs
badge-color = Цвет метки

# src/settings_window/providers/dialog.rs
up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = До трёх букв; пустое поле использует инициалы названия. Метки видны, когда включено несколько экземпляров провайдера.

# src/settings_window/window.rs
add-provider = Добавить провайдера

# src/settings_window/providers/dialog.rs
next = Далее

# src/settings_window/providers/dialog.rs
minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar перестанет читать { $name } и забудет сохранённые ключи, расписания, индикаторы трея и положение на главной.{ $v0 }

# src/settings_window/providers/dialog.rs
its-config-folder-stays-on-disk = { " " }Папка конфигурации останется на диске.

# src/settings_window/providers/dialog.rs
delete-name = Удалить { $name }?

# src/settings_window/providers/dialog.rs
delete = Удалить

# src/settings_window/providers/dialog.rs
finish-signing-in-in-your-browser-cancel-stops-this-login = Завершите вход в браузере. Отмена остановит этот вход.

# src/settings_window/providers/dialog.rs
runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = Запускает собственный вход Claude Code для { $name }, передавая папку конфигурации как CLAUDE_CONFIG_DIR. Авторизация остаётся в этой папке, и Claude Code обновляет её. Нужна нативная версия Claude Code для Windows.

# src/settings_window/providers/dialog.rs
runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = Запускает собственный вход Codex для { $name }, передавая папку конфигурации как CODEX_HOME. Авторизация остаётся в этой папке, и Codex обновляет её. Нужен нативный Codex CLI или настольное приложение.

# src/settings_window/providers/dialog.rs
sign-in-to-name = Войти в { $name }

# src/settings_window/providers/page.rs
sign-in = Войти

# src/settings_window/providers/dialog.rs
for = Для { $v0 }.

# src/settings_window/providers/dialog.rs
browser-session = Сессия браузера

# src/settings_window/providers/dialog.rs
oauth-token = Токен OAuth

# src/settings_window/providers/dialog.rs
session-key = Ключ сессии

# src/settings_window/providers/dialog.rs
paste-the-sessionkey-value = Вставьте значение sessionKey

# src/settings_window/providers/dialog.rs
oauth-access-token = Токен доступа OAuth

# src/settings_window/providers/dialog.rs
the-saved-credential-is-replaced-only-after-the-new-one-passes-th = Сохранённые данные заменяются только после успешной проверки новых.

# src/settings_window/providers/dialog.rs
claude-credential = Данные авторизации Claude

# src/settings_window/providers/dialog.rs
check-and-save = Проверить и сохранить

# src/settings_window/providers/dialog.rs
key-name-optional = Название ключа (необязательно)

# src/settings_window/providers/dialog.rs
e-g-personal = например, Личный

# src/settings_window/providers/dialog.rs
leave-blank-to-use-the-name-from-openrouter = Оставьте пустым, чтобы использовать название из OpenRouter.

# src/settings_window/providers/dialog.rs
minibar-checks-the-key-with-openrouter-before-saving-it = Minibar проверяет ключ в OpenRouter перед сохранением.

# src/settings_window/providers/dialog.rs
replace-api-key = Заменить API-ключ

# src/settings_window/providers/page.rs
add-api-key = Добавить API-ключ

# src/settings_window/providers/page.rs
management-key = Ключ управления

# src/settings_window/providers/dialog.rs
create-one-under-settings-management-keys-on-openrouter-ai = Создайте его в разделе Settings → Management keys на openrouter.ai.

# src/settings_window/providers/dialog.rs
replace-management-key = Заменить ключ управления

# src/settings_window/providers/page.rs
add-management-key = Добавить ключ управления

# src/settings_window/providers/dialog.rs
key-name = Название ключа

# src/settings_window/providers/page.rs
rename-key = Переименовать ключ

# src/settings_window/providers/dialog.rs
save = Сохранить

# src/settings_window/providers/dialog.rs
this-key = этот ключ

# src/settings_window/providers/dialog.rs
minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibar перестанет отслеживать { $hint }. Ключ продолжит работать в OpenRouter.

# src/settings_window/providers/dialog.rs
remove-api-key = Удалить API-ключ?

# src/settings_window/tray.rs
remove = Удалить

# src/settings_window/providers/dialog.rs
minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibar перестанет показывать кредитный баланс и историю использования для { $v0 }. Ключ продолжит работать в OpenRouter.

# src/settings_window/providers/dialog.rs
remove-management-key = Удалить ключ управления?

# src/settings_window/providers/dialog.rs
saved-in-windows-user-storage-never-in-the-settings-file = Сохраняется в хранилище пользователя Windows, а не в файле настроек.

# src/settings_window/providers/dialog.rs
save-key = Сохранить ключ

# src/settings_window/providers/dialog.rs
minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibar забудет сохранённый ключ для { $v0 }. Ключ продолжит работать в OpenCode.

# src/settings_window/providers/dialog.rs
signing-in = Вход…

# src/settings_window/providers/page.rs
checking = Проверка…

# src/settings_window/providers/dialog.rs
choose-a-provider = Выберите провайдера.

# src/settings_window/providers/dialog.rs
reason-it-is-already-in-the-list = { $reason }. Он уже есть в списке.

# src/settings_window/providers/dialog.rs
added-name = Добавлен { $name }.

# src/settings_window/providers/dialog.rs
could-not-add-the-provider-error = Не удалось добавить провайдера: { $error }

# src/settings_window/providers.rs
this-provider-no-longer-exists = Этот провайдер больше не существует.

# src/settings_window/providers/dialog.rs
could-not-delete-the-provider-error = Не удалось удалить провайдера: { $error }

# src/settings_window/providers/dialog.rs
paste-a-credential-first = Сначала вставьте данные авторизации.

# src/settings_window/providers/dialog.rs
credential-saved-in-windows-user-storage = Данные авторизации сохранены в хранилище пользователя Windows.

# src/settings_window/providers/dialog.rs
paste-a-key-first = Сначала вставьте ключ.

# src/settings_window/providers/dialog.rs
this-account-no-longer-exists = Этот аккаунт больше не существует.

# src/settings_window/providers/dialog.rs
api-key-saved-in-windows-user-storage = API-ключ сохранён в хранилище пользователя Windows.

# src/settings_window/providers/dialog.rs
management-key-replaced = Ключ управления заменён.

# src/settings_window/providers/dialog.rs
management-key-added = Ключ управления добавлен.

# src/settings_window/providers/dialog.rs
could-not-rename-the-key-error = Не удалось переименовать ключ: { $error }

# src/settings_window/providers/dialog.rs
api-key-renamed = API-ключ переименован.

# src/settings_window/providers/dialog.rs
could-not-remove-the-key-error = Не удалось удалить ключ: { $error }

# src/settings_window/providers/dialog.rs
api-key-removed = API-ключ удалён.

# src/settings_window/providers/dialog.rs
management-key-removed = Ключ управления удалён.

# src/settings_window/providers/dialog.rs
could-not-save-the-key-error = Не удалось сохранить ключ: { $error }

# src/settings_window/providers/dialog.rs
api-key-saved = API-ключ сохранён.

# src/settings_window/providers/dialog.rs
switch-source-to-config-folder-to-sign-in = Для входа выберите источник «Папка конфигурации».

# src/settings_window/providers/dialog.rs
this-provider-has-no-config-folder = У этого провайдера нет папки конфигурации.

# src/settings_window/providers/dialog.rs
another-instance-already-reads-this-config-folder-choose-a-differ = Другой экземпляр уже читает эту папку конфигурации. Сначала выберите другую папку.

# src/settings_window/providers/dialog.rs
this-provider-has-no-sign-in = У этого провайдера нет входа.

# src/settings_window/providers/dialog.rs
signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = Вход в { $v0 } выполнен. Его CLI обновляет авторизацию в своей папке конфигурации.

# src/settings_window/providers/page.rs
no-keys-yet = Ключей пока нет

# src/settings_window/providers/page.rs
management-key-5c8cf2 = { " " }· ключ управления

# src/settings_window/providers/page.rs
using-a-saved-credential = Используются сохранённые данные авторизации

# src/settings_window/providers/page.rs
paste-a-credential-under-account = Вставьте данные авторизации в разделе «Аккаунт»

# src/settings_window/providers/page.rs
needs-an-api-key-or-opencode-sign-in = Нужен API-ключ или вход в OpenCode

# src/settings_window/providers/page.rs
needs-an-api-key = Нужен API-ключ

# src/settings_window/providers/page.rs
not-found-set-its-folder-under-runtime = Не найден. Укажите папку в разделе «Установка».

# src/settings_window/providers/page.rs
using-a-saved-api-key = Используется сохранённый API-ключ

# src/settings_window/providers/page.rs
using-opencode-sign-in-or-local-history = Используется вход OpenCode или локальная история

# src/settings_window/providers/page.rs
reading-cli = Чтение { $cli }

# src/settings_window/providers/page.rs
reading-crew = Чтение { $crew }

# src/settings_window/providers/page.rs
reading-app = Чтение { $app }

# src/settings_window/providers/page.rs
checking-kiro-ide-kiro-crew-and-cli = Проверка Kiro IDE, Kiro Crew и CLI…

# src/settings_window/providers/page.rs
checking-installed-app-and-cli = Проверка установленного приложения и CLI…

# src/settings_window/providers/page.rs
checking-cli = Проверка CLI…

# src/settings_window/providers/page.rs
checking-installed-app = Проверка установленного приложения…

# src/settings_window/providers/page.rs
no-providers-yet-add-one-to-start-reading-limits = Провайдеров пока нет. Добавьте провайдера, чтобы читать лимиты.

# src/settings_window/providers/page.rs
is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } выключен и не отображается в Minibar или трее. Включите его, чтобы читать использование.

# src/settings_window/providers/page.rs
replace-chatgpt-logo-with-codex = Заменить логотип ChatGPT на Codex

# src/settings_window/providers/page.rs
limits-only = Только лимиты

# src/settings_window/providers/page.rs
delete-provider = Удалить провайдера

# src/settings_window/providers/page.rs
display-name = Отображаемое название

# src/settings_window/providers/page.rs
shown-on-popup-tabs-home-cards-the-tray-and-notifications = Показывается на вкладках окна, карточках главной, в трее и уведомлениях.

# src/settings_window/providers/page.rs
up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = До трёх букв. Оставьте пустым для инициалов названия. Показывается, когда включено несколько экземпляров провайдера.

# src/settings_window/providers/page.rs
auto-uses-a-neutral-plate-that-follows-the-theme = Авто использует нейтральный фон в цвет темы.

# src/settings_window/providers/page.rs
show-on-home = Показывать на главной

# src/settings_window/providers/page.rs
its-provider-tab-stays-available-when-hidden-from-home = Вкладка провайдера доступна, даже если он скрыт с главной.

# src/settings_window/providers/page.rs
credential = Авторизация

# src/settings_window/providers/page.rs
signed-in-as-identity = Вы вошли как { $identity }

# src/settings_window/providers/page.rs
saved-in-windows-user-storage = Сохранено в хранилище пользователя Windows

# src/settings_window/providers/page.rs
paste-a-sessionkey-or-an-oauth-access-token = Вставьте sessionKey или токен доступа OAuth.

# src/settings_window/providers/page.rs
reads-limits-only-minibar-cannot-refresh-a-pasted-credential = Читает только лимиты. Minibar не может обновлять вставленные данные авторизации.

# src/settings_window/providers/page.rs
replace-credential = Заменить авторизацию

# src/settings_window/providers/page.rs
add-credential = Добавить авторизацию

# src/settings_window/providers/page.rs
signed-in-account = Авторизованный аккаунт

# src/settings_window/providers/page.rs
not-signed-in-yet-or-no-limits-read-so-far = Вход ещё не выполнен или лимиты ещё не считаны.

# src/settings_window/providers/page.rs
account = Аккаунт

# src/settings_window/providers/page.rs
runtime = Установка

# src/settings_window/providers/page.rs
minibar-finds-these-automatically = { $v0 } Minibar находит их автоматически.

# src/settings_window/providers/page.rs
in-use = Используется

# src/settings_window/providers/page.rs
found = Найден

# src/settings_window/providers/page.rs
copy-path = Копировать путь

# src/settings_window/providers/page.rs
open-folder = Открыть папку

# src/settings_window/providers/page.rs
path-copied = Путь скопирован.

# src/settings_window/providers/page.rs
not-installed-or-installed-somewhere-minibar-doesn-t-look = Не установлен или установлен в месте, где Minibar не ищет.

# src/settings_window/providers/page.rs
not-found = Не найден

# src/settings_window/providers/page.rs
choose-folder = Выбрать папку…

# src/settings_window/providers/page.rs
select-folder = Выбрать папку

# src/settings_window/providers/page.rs
choose-folder-1838a4 = Выберите папку

# src/settings_window/providers/page.rs
only-needed-if-automatic-detection-misses-your-install = Нужно только если автоматическое обнаружение не нашло установку.

# src/settings_window/providers/page.rs
source = Источник

# src/settings_window/providers/page.rs
config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = Папка конфигурации читает авторизацию Claude Code из CLAUDE_CONFIG_DIR. Ручная авторизация читает только лимиты из вставленных данных.

# src/settings_window/providers/page.rs
config-folder = Папка конфигурации

# src/settings_window/providers/page.rs
manual = Вручную

# src/settings_window/providers/page.rs
passed-to-the-cli-as-env-leave-empty-to-use = Передаётся в CLI как { $env }. Оставьте пустым, чтобы использовать { $v0 }.

# src/settings_window/providers/page.rs
the-standard-folder = стандартную папку

# src/settings_window/providers/page.rs
a-folder-minibar-creates-for-this-instance = папку, которую Minibar создаёт для этого экземпляра

# src/settings_window/providers/page.rs
other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } уже читает эту папку. Два экземпляра не должны использовать одну авторизацию, иначе использование считается дважды.

# src/settings_window/providers/page.rs
automatic-activation = Автоматическая активация

# src/settings_window/providers/page.rs
starts-this-account-s-5-hour-window-when-it-resets-using-its-own = Начинает 5-часовой период аккаунта после сброса, используя его авторизацию. Расписания и паузы — в разделе «Активация лимитов».

# src/settings_window/providers/page.rs
usage-statistics = Статистика использования

# src/settings_window/providers/page.rs
scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = Сканирует локальную историю экземпляра для его карточки использования. Выключите, чтобы полностью остановить сбор.

# src/settings_window/providers/page.rs
opencode-sign-in-or-local-history = Вход OpenCode или локальная история

# src/settings_window/providers/page.rs
found-in-opencode-auth-environment-a-saved-key-or-local-history = Найдено в авторизации OpenCode, переменных окружения, сохранённом ключе или локальной истории.

# src/settings_window/providers/page.rs
nothing-found-in-opencode-auth-environment-or-local-history = Ничего не найдено в авторизации OpenCode, переменных окружения или локальной истории.

# src/settings_window/providers/page.rs
replace-key = Заменить ключ

# src/settings_window/providers/page.rs
optional-only-needed-without-opencode-sign-in-on-this-pc = Необязательно. Нужно только без входа OpenCode на этом компьютере.

# src/settings_window/providers/page.rs
add-the-key-of-the-account-this-instance-tracks = Добавьте ключ аккаунта, который отслеживает этот экземпляр.

# src/settings_window/providers/page.rs
keys = Ключи

# src/settings_window/providers/page.rs
a-management-key-shows-credit-balance-and-usage-history-api-keys = Ключ управления показывает кредитный баланс и историю использования. API-ключи показывают расходы по каждому ключу.

# src/settings_window/providers/page.rs
could-not-read-the-saved-key-reopen-this-page-to-retry = Не удалось прочитать сохранённый ключ. Откройте страницу снова, чтобы повторить попытку.

# src/settings_window/providers/page.rs
credit-balance-and-account-wide-usage-history = Кредитный баланс и история использования всего аккаунта

# src/settings_window/providers/page.rs
not-added-add-one-to-see-credit-balance-and-usage-history = Не добавлен. Добавьте, чтобы видеть кредитный баланс и историю использования.

# src/settings_window/providers/page.rs
last-updated = Последнее обновление: { $v0 }, { $v1 }

# src/settings_window/providers/page.rs
no-api-keys-yet-add-one-to-track-spend-per-key = API-ключей пока нет. Добавьте ключ, чтобы отслеживать его расходы.

# src/settings_window/providers/page.rs
key = Ключ

# src/settings_window/providers/page.rs
spend = Расходы

# src/settings_window/providers/page.rs
limit = Лимит

# src/settings_window/providers/page.rs
error-reopen-this-page-to-retry = { $error }. Откройте страницу снова, чтобы повторить попытку.

# src/settings_window/providers/page.rs
could-not-read-key = Не удалось прочитать ключ

# src/settings_window/providers/page.rs
not-saved = Не сохранён

# src/settings_window/providers/page.rs
unnamed = Без названия

# src/settings_window/providers/page.rs
of-the-limit = { $v0 } из лимита { $v1 }

# src/settings_window/providers/page.rs
no-spend-limit-key-spend-account-credits-purchased = Без лимита расходов. Расходы ключа: { $v0 }. Куплено кредитов аккаунта: { $v1 }.

# src/settings_window/providers/page.rs
none = Нет

# src/settings_window/providers/page.rs
add-key = Добавить ключ

# src/settings_window/providers.rs
openrouter-account-credentials-are-configured = Данные авторизации аккаунта OpenRouter настроены

# src/settings_window/providers.rs
saved-api-key = Сохранённый API-ключ

# src/settings_window/providers.rs
saved-credential = Сохранённые данные авторизации

# src/settings_window/providers.rs
opencode-auth-json-or-local-database = OpenCode auth.json или локальная база данных

# src/settings_window/providers.rs
that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = Это не похоже на ключ OpenRouter. Ключи начинаются с sk-or-.

# src/settings_window/providers.rs
codex-cli-folder = Папка Codex CLI

# src/settings_window/providers.rs
claude-code-cli-folder = Папка Claude Code CLI

# src/settings_window/providers.rs
cursor-app-folder = Папка приложения Cursor

# src/settings_window/providers.rs
grok-cli-folder = Папка Grok CLI

# src/settings_window/providers.rs
kiro-ide-folder = Папка Kiro IDE

# src/settings_window/providers.rs
kiro-crew-app-path = Путь к приложению Kiro Crew

# src/settings_window/providers.rs
kiro-cli-folder = Папка Kiro CLI

# src/settings_window/providers.rs
reads-the-signed-in-codex-cli-or-desktop-app = Читает авторизованный Codex CLI или настольное приложение.

# src/settings_window/providers.rs
reads-your-existing-claude-code-login = Читает существующую авторизацию Claude Code.

# src/settings_window/providers.rs
reads-the-signed-in-cursor-app-for-this-billing-cycle = Читает авторизованное приложение Cursor за текущий расчётный период.

# src/settings_window/providers.rs
reads-zen-auth-and-local-opencode-history = Читает авторизацию Zen и локальную историю OpenCode.

# src/settings_window/providers.rs
reads-go-quota-windows-and-local-opencode-history = Читает периоды лимитов Go и локальную историю OpenCode.

# src/settings_window/providers.rs
reads-api-key-usage-and-spend-limits-a-management-key-also-enable = Читает использование API-ключей и лимиты расходов. Ключ управления также включает историю использования и кредитный баланс.

# src/settings_window/providers.rs
reads-subscription-quota-from-your-existing-official-agy-windows = Читает лимиты подписки из существующей официальной авторизации agy для Windows.

# src/settings_window/providers.rs
reads-supergrok-subscription-credits-from-your-existing-official = Читает кредиты подписки SuperGrok из существующей официальной авторизации Grok CLI.

# src/settings_window/providers.rs
fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = Получает актуальные месячные кредиты Kiro через общую авторизацию; распознаёт установки IDE, Crew и CLI.

# src/settings_window/providers.rs
codex-desktop-app = Настольное приложение Codex

# src/settings_window/providers.rs
codex-cli = Codex CLI

# src/settings_window/providers.rs
claude-desktop-app = Настольное приложение Claude

# src/settings_window/providers.rs
claude-code-cli = Claude Code CLI

# src/settings_window/providers.rs
cursor-app = Приложение Cursor

# src/settings_window/providers.rs
antigravity-app = Приложение Antigravity

# src/settings_window/providers.rs
grok-cli = Grok CLI

# src/settings_window/providers.rs
kiro-ide = Kiro IDE

# src/settings_window/providers.rs
kiro-crew = Kiro Crew

# src/settings_window/providers.rs
kiro-cli = Kiro CLI

# src/settings_window/providers.rs
supports-one-instance = { $v0 } поддерживает один экземпляр

# src/settings_window/tray.rs
numbers = Числа

# src/settings_window/tray.rs
progress-bars = Полосы прогресса

# src/settings_window/tray.rs
rings = Кольца

# src/settings_window/tray.rs
reset-time = Время сброса

# src/settings_window/tray.rs
countdown = Обратный отсчёт

# src/settings_window/tray.rs
status = Статус

# src/settings_window/tray.rs
fixed = Фиксированный

# src/settings_window/tray.rs
app-accent = Акцент приложения

# src/settings_window/tray.rs
percentages-as-digits = Проценты цифрами

# src/settings_window/tray.rs
one-bar-per-indicator = По полосе на индикатор

# src/settings_window/tray.rs
nested-rings-one-per-indicator = Вложенные кольца, по одному на индикатор

# src/settings_window/tray.rs
when-the-limit-resets = Когда сбрасывается лимит

# src/settings_window/tray.rs
time-left-until-the-reset = Время до сброса

# src/settings_window/tray.rs
used = Использовано

# src/settings_window/tray.rs
remaining = Осталось

# src/settings_window/tray.rs
app-icon = Значок приложения

# src/settings_window/tray.rs
widget = Виджет { $v0 }

# src/settings_window/tray.rs
the-codex-minibar-icon = Значок Codex Minibar

# src/settings_window/tray.rs
style-no-indicators = { $style } · Нет индикаторов

# src/settings_window/tray.rs
widget-removed = Виджет удалён

# src/settings_window/tray.rs
undo = Отменить

# src/settings_window/tray.rs
tray-icon = Значок трея

# src/settings_window/tray.rs
shows-the-app-icon = Показывает значок приложения.

# src/settings_window/tray.rs
add-widget = Добавить виджет

# src/settings_window/tray.rs
add-app-icon = Добавить значок приложения

# src/settings_window/tray.rs
move-up = Переместить вверх

# src/settings_window/tray.rs
move-down = Переместить вниз

# src/settings_window/tray.rs
edit = Изменить

# src/settings_window/tray.rs
duplicate = Дублировать

# src/settings_window/tray.rs
shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = Показывает значок Codex Minibar в области уведомлений. Настраиваемых индикаторов нет.

# src/settings_window/tray.rs
style = Стиль

# src/settings_window/tray.rs
up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = До { $max_indicators } лимитов в этом порядке. Разверните индикатор, чтобы изменить его.

# src/settings_window/tray.rs
indicators = Индикаторы

# src/settings_window/tray.rs
remove-widget = Удалить виджет

# src/settings_window/tray.rs
add-indicator = Добавить индикатор

# src/settings_window/tray.rs
unsupported = Не поддерживается: { $v0 }

# src/settings_window/tray.rs
remove-indicator = Удалить индикатор

# src/settings_window/tray.rs
unsupported-bba2a8 = Не поддерживается ({ $v0 })

# src/settings_window/tray.rs
unavailable-0c5d75 = Недоступно ({ $v0 })

# src/settings_window/tray.rs
metric = Метрика

# src/settings_window/tray.rs
show = Показывать

# src/settings_window/tray.rs
color = Цвет

# src/settings_window/tray.rs
fixed-color = Фиксированный цвет

# src/settings_window/troubleshoot.rs
troubleshooting-could-not-start = Не удалось начать диагностику

# src/settings_window/troubleshoot.rs
choose-which-installed-ai-tool-should-investigate-the-problem = Выберите установленный ИИ-инструмент для исследования проблемы.

# src/settings_window/troubleshoot.rs
open-terminal = Открыть терминал

# src/settings_window/window.rs
version-is-available = Доступна версия { $version }

# src/settings_window/window.rs
update-now = Обновить сейчас

# src/settings_window/window.rs
missing-a-provider = Не хватает провайдера?

# src/settings_window/window.rs
ask-for-it-or-build-it-yourself = Запросите его или добавьте самостоятельно.

# src/settings_window/window.rs
request-a-provider = Запросить провайдера

# src/settings_window/window.rs
contribute-one = Добавить провайдера

# src/tray.rs
disabled = Выключено

# src/updater.rs
update-complete = Обновление завершено

# src/updater.rs
now-running-version = Установлена версия { $version }.

# src/usage_overview.rs
past-24h = За 24 ч

# application
language = Язык

# application
applies-immediately-to-every-app-window-and-notification-auto-fol = Сразу применяется ко всем окнам и уведомлениям. Авто следует языку интерфейса Windows.

# application
auto-windows = Авто (Windows)

# application
english = English

# application
msg-russian = Русский

# application
codex-minibar-settings = Настройки Codex Minibar

# application
welcome-to-codex-minibar = Добро пожаловать в Codex Minibar

# application
exit = Выйти

# application
update-available-67fd3a = Доступно обновление

# application
monthly-credits-976559 = Кредиты за месяц

# application
msg-5h-session-de7ce8 = Сессия 5 ч

# application
name-weekly = { $name }: неделя

# application
remaining-remaining = Осталось { $remaining }%

# application
tokens-94e0b9 = { $v0 }: { $v1 } токенов

# application
requests-priced = { $v0 ->
    [one] { $v0 } запрос · учтена стоимость { $v1 }
    [few] { $v0 } запроса · учтена стоимость { $v1 }
   *[many] { $v0 } запросов · учтена стоимость { $v1 }
    }

# application
requests = { $v0 ->
    [one] { $v0 } запрос
    [few] { $v0 } запроса
   *[many] { $v0 } запросов
    }

# application
credits-338f52 = КРЕДИТЫ

# application
cloud-session-credits = КРЕДИТЫ ОБЛАЧНОЙ СЕССИИ

# application
name-login-expires-in-days-left = { $days_left ->
    [one] Авторизация { $name } истекает через { $days_left } день
    [few] Авторизация { $name } истекает через { $days_left } дня
   *[many] Авторизация { $name } истекает через { $days_left } дней
    }

# application
name-error = Ошибка { $name }

# application
sessions = { $v0 ->
    [one] { $v0 } сессия
    [few] { $v0 } сессии
   *[many] { $v0 } сессий
    }

# application
api-estimate = Оценка API

# application
start-to-end = { $start } — { $end }

# application
to = { $v0 } — { $v1 }

# application
share-1-of-other = { $share }% от { $v0 } · { $other }

# application
cost-885dc4 = расхода

# application
tokens-339143 = токенов

# application
sessions-0e5e29 = { $v0 ->
    [one] · { $v0 } сессия
    [few] · { $v0 } сессии
   *[many] · { $v0 } сессий
    }

# application
api-key = API-ключ

# application
api-keys = API-ключи

# application
name-deleted = { $name } удалён.

# application
openrouter-api-key-no-longer-exists = API-ключ OpenRouter больше не существует

# application
openrouter-account-no-longer-exists = Аккаунт OpenRouter больше не существует

# application
credit = { $v0 } кредитов

# application
agy-cli-folder = Папка agy CLI

# application
folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = Папка с codex.exe, codex.cmd или codex.ps1. Оставьте пустым для автоматического поиска.

# application
folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = Папка с claude.exe, claude.cmd или claude.ps1. Оставьте пустым для автоматического поиска.

# application
folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = Папка с Cursor.exe. Оставьте пустым для автоматического поиска. Использование по-прежнему читается из авторизованного профиля.

# application
folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = Папка с agy.exe, agy.cmd или agy.ps1. Оставьте пустым для автоматического поиска.

# application
folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = Папка с grok.exe, grok.cmd или grok.ps1. Оставьте пустым для автоматического поиска.

# application
folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = Папка с Kiro.exe или сам исполняемый файл. Оставьте пустым для автоматического поиска.

# application
folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = Папка с KiroCrew.exe или сам исполняемый файл. Оставьте пустым для автоматического поиска личных и общих установок.

# application
folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = Папка с kiro-cli.exe или сам исполняемый файл. Оставьте пустым для автоматического поиска.

# application
off-45080e = { $v0 } (выкл.)

# application
each-widget-is-one-icon-in-the-notification-area-indicators-show = Каждый виджет — отдельный значок в области уведомлений. Индикаторы показывают лимит провайдера числами, полосами, кольцами или временем сброса.

# application
a-reset-clock-follows-one-quota = Часы сброса отслеживают один лимит.

# application
ai-tool = ИИ-инструмент

# application
msg-7-days = 7 дней

# application
msg-30-days = 30 дней

# application
msg-90-days = 90 дней

# application
yellow = Жёлтый

# application
custom = Свой

# application
not-available-for-manual-credentials-switch-source-to-config-fold = Недоступно для ручной авторизации. Выберите источник «Папка конфигурации».

# application
opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = Локальную историю OpenCode отслеживает первый экземпляр OpenCode.

# application
this-provider-has-no-local-usage-history = У этого провайдера нет локальной истории использования.

# application
this-provider-has-no-session-window-to-start = У этого провайдера нет сессионного лимита для активации.

# application
this-provider-does-not-use-a-config-folder = Этот провайдер не использует папку конфигурации.

# application
could-not-save-provider-settings-error-restoring-its-previous-cre = Не удалось сохранить настройки провайдера ({ $error }) и восстановить прежнюю авторизацию ({ $rollback_error }).

# application
msg-5h-7d = 5 ч  |  { $v0 }  |  { $v1 }
    7 д  |  { $v2 }  |  { $v3 }

# API key count in the provider header.
api-key-count = { $v0 ->
    [one] { $v0 } API-ключ
    [few] { $v0 } API-ключа
   *[many] { $v0 } API-ключей
    }

# Application copy
widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = Расширяет главную и использование. Перетаскивайте блоки главной между столбцами; вкладки провайдеров остаются компактными.

# Application copy
a-possible-codex-reset-is-scheduled-for-when-in-countdown = Возможный сброс Codex запланирован на { $when } (через { $countdown })

# Application copy
label-possible-reset-on-when-in-countdown = { $label }: возможный сброс { $when } (через { $countdown })

# Application copy
new-codex-reset-info = Новые сведения о сбросе Codex

# Application copy
login-expires-soon = Авторизация { $v0 } скоро истечёт

# Application copy
it-stops-renewing-on-open-minibar-and-choose-sign-in-again = Авторизация перестанет обновляться { $v0 }. Откройте Minibar и выберите «Войти снова».

# Application copy
could-not-clear-usage-data-error = Не удалось очистить данные использования: { $error }

# Application copy
succeeded-at = { $v0 }: успешно в { $v1 }

# Application copy
failed-at-error = { $v0 }: ошибка в { $v1 }: { $error }

# Application copy
key-39df89 = Ключ { $v0 }

# Application copy
expired = Истёк

# Application copy
jan = янв.

# Application copy
feb = февр.

# Application copy
mar = мар.

# Application copy
apr = апр.

# Application copy
may = мая

# Application copy
jun = июн.

# Application copy
jul = июл.

# Application copy
aug = авг.

# Application copy
sep = сент.

# Application copy
oct = окт.

# Application copy
nov = нояб.

# Application copy
dec = дек.

# { $value }% used
quota-percent-used = Использовано { $value }%

# { $value }% left
quota-percent-left = Осталось { $value }%

# { $amount } of { $limit } used
cloud-amount-used = Использовано { $amount } из { $limit }

# { $amount } of { $limit } left
cloud-amount-left = Осталось { $amount } из { $limit }

# Short usage range
range-24h = 24 ч

# Short usage range
range-7d = 7 дн

# Short usage range
range-30d = 30 дн

# Short usage range
range-90d = 90 дн

# Home usage card title
home-usage-title = Расход

# OpenRouter key administration

could-not-reach-openrouter = Не удалось связаться с OpenRouter


openrouter-keys-title = Ключи


openrouter-keys-new = Новый ключ


openrouter-keys-new-title = Новый ключ


openrouter-keys-back = Назад


openrouter-keys-updating = Обновление…


openrouter-keys-loading = Загрузка ключей…


openrouter-keys-load-failed = Не удалось загрузить ключи


openrouter-keys-retry = Повторить


openrouter-keys-empty = У этого аккаунта пока нет ключей.


openrouter-keys-this-app = Это приложение


openrouter-keys-no-limit = Без лимита


openrouter-keys-of-limit = { $amount } из { $limit }


openrouter-keys-resets-daily = сброс ежедневно


openrouter-keys-resets-weekly = сброс еженедельно


openrouter-keys-resets-monthly = сброс ежемесячно


openrouter-keys-expires-on = истекает { $date }


openrouter-keys-expired-on = истёк { $date }


openrouter-keys-today = сегодня { $amount }


openrouter-keys-usage-breakdown = Сегодня { $today } · неделя { $week } · месяц { $month } · всего { $total }


openrouter-keys-show-all = Показать все ключи ({ $count })


openrouter-keys-show-fewer = Свернуть


openrouter-keys-spending-limit = Лимит расходов


openrouter-keys-limit-hint = Оставьте пустым, чтобы не ограничивать.


openrouter-keys-resets = Сброс


openrouter-keys-reset-never = Никогда


openrouter-keys-reset-daily = День


openrouter-keys-reset-weekly = Неделя


openrouter-keys-reset-monthly = Месяц


openrouter-keys-byok = Учитывать BYOK в лимите


openrouter-keys-enabled = Включён


openrouter-keys-delete = Удалить


openrouter-keys-delete-tracked = Этот ключ использует приложение. Сначала удалите его в настройках.


openrouter-keys-saving = Сохранение…


openrouter-keys-delete-title = Удалить «{ $name }»?


openrouter-keys-delete-message = Всё, что использует этот ключ, сразу перестанет работать. Это нельзя отменить, а отключение ключа — можно.


openrouter-keys-keep = Оставить


openrouter-keys-delete-confirm = Удалить ключ


openrouter-keys-deleting = Удаление…


openrouter-keys-name = Название


openrouter-keys-name-placeholder = например, laptop-cursor


openrouter-keys-name-required = Введите название ключа.


openrouter-keys-invalid-amount = Введите сумму в долларах, например 25 или 12.50.


openrouter-keys-expires = Срок действия


openrouter-keys-expires-1-hour = 1 час


openrouter-keys-expires-1-day = 1 день


openrouter-keys-expires-7-days = 7 дней


openrouter-keys-expires-30-days = 30 дней


openrouter-keys-expires-90-days = 90 дней


openrouter-keys-expires-180-days = 180 дней


openrouter-keys-expires-1-year = 1 год


openrouter-keys-expires-never = Бессрочно


openrouter-keys-track = Отслеживать ключ в Minibar


openrouter-keys-create = Создать ключ


openrouter-keys-creating = Создание…


openrouter-keys-created = Ключ создан


openrouter-keys-copy = Скопировать ключ


openrouter-keys-copied = Скопировано


openrouter-keys-done = Готово


openrouter-keys-once-title = Ключ больше не будет показан


openrouter-keys-once-message = OpenRouter показывает новый ключ только один раз. Скопируйте его сейчас.


openrouter-keys-pinned = Окно не закроется, пока вы не нажмёте «Готово».


openrouter-keys-tracking = Добавление ключа в Minibar…


openrouter-keys-tracked = Minibar теперь отслеживает этот ключ.


openrouter-keys-track-failed = Не удалось добавить ключ в Minibar: { $error }


openrouter-keys-no-management-key = У этого аккаунта нет ключа управления.


openrouter-keys-management-key-rejected = OpenRouter отклонил ключ управления. Замените его в настройках.


openrouter-keys-not-a-management-key = Этот ключ не может управлять другими ключами. Используйте ключ управления.


openrouter-keys-key-not-found = Этот ключ больше не существует.
