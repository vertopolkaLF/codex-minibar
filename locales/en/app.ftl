### Codex Minibar application messages.
### English source; per-message fallback. Keep $parameters unchanged.

# src/provider_registry.rs
luna-reserve = Luna Reserve

# src/limits.rs
on-pace = On pace

# src/limits.rs
delta-in-deficit = { $delta }% in deficit

# src/limits.rs
delta-in-reserve = { $delta }% in reserve

# src/notifications.rs
msg-5-hour-limit-started = 5-hour limit started

# src/notifications.rs
msg-5-hour-limit-reset-and-activated = 5-hour limit reset and activated

# src/notifications.rs
msg-5-hour-limit-reset = 5-hour limit reset

# src/notifications.rs
weekly-limit-reset = Weekly limit reset

# src/notifications.rs
name-5-hour = { $name } 5-hour

# src/notifications.rs
label-limit-is-low = { $label } limit is low

# src/popup_window/formatting.rs
expired-at-time = expired at { $time }

# src/popup_window/formatting.rs
expired-at-time-6c39ff = expired at { $time } { $v0 }

# src/popup_window/state.rs
never = Never

# src/popup_window/formatting.rs
unlimited = Unlimited

# src/popup_window/ui/activity.rs
unavailable = Unavailable

# src/popup_window/formatting.rs
days-d-hours-h = { $days }d { $hours }h

# src/popup_window/formatting.rs
days-d = { $days }d

# src/popup_window/formatting.rs
hours-h-minutes-m = { $hours }h { $minutes }m

# src/popup_window/formatting.rs
hours-h = { $hours }h

# src/popup_window/formatting.rs
minutes-m = { $minutes }m

# src/popup_window/formatting.rs
waiting-for-first-update = Waiting for first update

# src/popup_window/formatting.rs
just-now = just now

# src/popup_window/formatting.rs
seconds-seconds-ago = { $seconds } seconds ago

# src/popup_window/formatting.rs
minutes-ago = { $v0 } minutes ago

# src/popup_window/formatting.rs
updated-elapsed = Updated { $elapsed }

# src/popup_window/state.rs
the-request-timed-out-try-refreshing-again = The request timed out. Try refreshing again.

# src/popup_window/state.rs
the-secure-connection-could-not-be-verified-see-log-for-details = The secure connection could not be verified. See Log for details.

# src/popup_window/state.rs
the-provider-closed-the-connection-try-refreshing-again = The provider closed the connection. Try refreshing again.

# src/popup_window/state.rs
the-provider-s-address-could-not-be-resolved-check-your-connectio = The provider's address could not be resolved. Check your connection.

# src/popup_window/state.rs
could-not-connect-to-the-provider-check-your-connection-and-try-a = Could not connect to the provider. Check your connection and try again.

# src/popup_window/state.rs
the-management-key-was-rejected-update-it-in-settings = The management key was rejected. Update it in Settings.

# src/popup_window/state.rs
authentication-failed-sign-in-again-or-update-the-provider-key = Authentication failed. Sign in again or update the provider key.

# src/popup_window/state.rs
access-denied-by-the-provider-http-403 = Access denied by the provider (HTTP 403).

# src/popup_window/state.rs
too-many-requests-wait-a-few-minutes-before-refreshing-again = Too many requests. Wait a few minutes before refreshing again.

# src/popup_window/state.rs
the-provider-is-temporarily-unavailable-try-again-later = The provider is temporarily unavailable. Try again later.

# src/popup_window/state.rs
the-provider-rejected-the-request-see-log-for-details = The provider rejected the request. See Log for details.

# src/popup_window/state.rs
the-requested-resource-was-not-found-see-log-for-details = The requested resource was not found. See Log for details.

# src/popup_window/state.rs
the-provider-returned-an-unexpected-response-try-refreshing-again = The provider returned an unexpected response. Try refreshing again.

# src/popup_window/state.rs
the-request-failed-see-log-for-details = The request failed. See Log for details.

# src/popup_window/ui/activity.rs
input = Input

# src/popup_window/ui/activity.rs
cache = Cache

# src/popup_window/ui/usage.rs
output = Output

# src/popup_window/ui/activity.rs
input-uncached = Input (uncached)

# src/popup_window/ui/usage.rs
cached-input = Cached input

# src/popup_window/ui/activity.rs
value-partially-priced = { $value } (partially priced)

# src/popup_window/ui/activity.rs
could-not-load-model-data-error = Could not load model data: { $error }

# src/popup_window/ui/activity.rs
usage-activity = Usage activity

# src/popup_window/ui/activity.rs
waiting-for-cursor-s-usage-export-refresh-to-retry = Waiting for Cursor's usage export. Refresh to retry.

# src/popup_window/ui/activity.rs
model-data-unavailable = Model data unavailable

# src/popup_window/ui/activity.rs
loading-models = Loading models…

# src/popup_window/ui/tooltip.rs
no-model-data = No model data

# src/popup_window/ui/activity.rs
no-usage-data = No usage data

# src/popup_window/ui/activity.rs
no-series-selected = No series selected

# src/popup_window/ui/activity.rs
no-tokens-in-this-period = No { $v0 } tokens in this period

# src/popup_window/ui/activity.rs
loading-model-breakdown = Loading model breakdown

# src/popup_window/ui/activity.rs
group-tokens-or-cost-by-model = Group tokens or cost by model

# src/popup_window/ui/activity.rs
split-type = Type

# src/popup_window/ui/activity.rs
split-by-token-type = Split bars by token type

# src/popup_window/ui/usage.rs
model = Model

# src/popup_window/ui/usage.rs
tokens = Tokens

# src/popup_window/ui/usage.rs
cost = Cost

# src/popup_window/ui/activity.rs
no-cost-data-for-this-period = No cost data for this period

# src/popup_window/ui/activity.rs
no-token-data-for-this-period = No token data for this period

# src/popup_window/ui/activity.rs
daily-cost-in-usd = Daily cost in USD

# src/popup_window/ui/activity.rs
daily-token-volume = Daily token volume

# src/popup_window/ui/activity.rs
of-total-models-scroll-for-more = { $v0 }–{ $v1 } of { $total } models · Scroll for more

# src/popup_window/ui/activity.rs
cost-usd-by-model = Cost (USD) by model

# src/popup_window/ui/activity.rs
tokens-by-model = Tokens by model

# src/settings.rs
today = Today

# src/popup_window/ui/activity.rs
last-period-days = Last { $period } days

# src/popup_window/ui/activity.rs
no-data = No data

# src/popup_window/ui/cards.rs
loading-usage-statistics = Loading usage statistics…

# src/popup_window/ui/cards.rs
available-balance = Available balance

# src/popup_window/ui/cards.rs
resets-in = Resets in

# src/popup_window/ui/cards.rs
session-not-started = Session not started

# src/popup_window/ui/cards.rs
expires-in = Expires in

# src/popup_window/ui/cards.rs
usage = Usage:

# src/settings_window/providers/page.rs
remove-key = Remove key

# src/popup_window/ui/cards.rs
msg-1-banked-reset = 1 Banked Reset

# src/popup_window/ui/cards.rs
count-banked-resets = { $count ->
    [one] { $count } Banked Reset
   *[other] { $count } Banked Resets
    }

# src/popup_window/ui/cards.rs
available-to-use = Available to use

# src/popup_window/ui/cards.rs
no-expiration-date = No expiration date

# src/popup_window/ui/cards.rs
banked-reset = Banked Reset { $v0 }

# src/popup_window/ui/cards.rs
codex-limits = Codex limits

# src/popup_window/ui/cards.rs
open-announcement-source = Open announcement source

# src/popup_window/ui/cards.rs
source-not-provided = Source not provided

# src/popup_window/ui/cards.rs
tibo-reset = Tibo Reset™

# src/settings_window/customize.rs
home = Home

# src/popup_window/ui/usage.rs
usage-0bb186 = Usage

# src/popup_window/ui/footer.rs
settings = Settings

# src/popup_window/ui/footer.rs
install-update = Install update

# src/popup_window/ui/footer.rs
refreshing-limits-and-usage = Refreshing limits and usage…

# src/popup_window/ui/footer.rs
updated = Updated{ " " }

# src/popup_window/ui/footer.rs
refresh-last-updated-relative = Refresh | Last updated { $relative }

# src/popup_window/ui/home.rs
edit-home = Edit Home
home-card-layout = Layout
home-card-layout-cards = Cards
home-card-layout-lines = Lines
home-card-layout-rings = Rings
drag-to-reorder = Drag to reorder

# src/popup_window/ui/home.rs
drop-here = Drop here

# src/settings_window/general.rs
usage-stats = Usage Stats

# src/popup_window/ui/usage.rs
loading-usage = Loading usage…

# src/settings_window/window.rs
update-failed = Update failed

# src/popup_window/ui/root.rs
something-went-wrong = Something went wrong

# src/popup_window/ui/root.rs
no-providers-enabled = No providers enabled

# src/popup_window/ui/root.rs
turn-one-on-in-settings-providers = Turn one on in Settings > Providers.

# src/popup_window/ui/root.rs
sign-in-again-to-keep-limits-updating = Sign in again to keep limits updating.

# src/settings_window/providers/page.rs
sign-in-again = Sign in again

# src/popup_window/ui/tooltip.rs
total = Total

# src/popup_window/ui/usage.rs
enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = Enable a provider in Settings and include it in Usage Stats to see local API usage.

# src/popup_window/ui/usage.rs
hourly-cost = Hourly cost

# src/popup_window/ui/usage.rs
hourly-processed-tokens = Hourly processed tokens

# src/popup_window/ui/usage.rs
no-activity-in-this-range = No activity in this range

# src/popup_window/ui/usage.rs
day = Day

# src/popup_window/ui/usage.rs
breakdown = Breakdown

# src/popup_window/ui/usage.rs
totals = Totals

# src/popup_window/ui/usage.rs
processed-tokens = Processed tokens

# src/popup_window/ui/usage.rs
uncached-input = Uncached input

# src/popup_window/ui/usage.rs
cache-savings = Cache savings

# src/popup_window/ui/usage.rs
share = Share

# src/popup_window/ui/usage.rs
hour = Hour

# src/usage_overview.rs
sat = Sat

# src/usage_overview.rs
sun = Sun

# src/provider_registry.rs
msg-5h-session = 5h session

# src/provider_registry.rs
weekly = Weekly

# src/provider_registry.rs
cursor-models = Cursor Models

# src/settings_window/tray.rs
other-models = Other Models

# src/provider_registry.rs
all-models = All Models

# src/settings_window/tray.rs
grok-bot = Grok Bot

# src/provider_registry.rs
monthly = Monthly

# src/provider_registry.rs
spending-limit = Spending limit

# src/provider_registry.rs
gemini = Gemini

# src/provider_registry.rs
claude-gpt = Claude + GPT

# src/provider_registry.rs
credits = Credits

# src/provider_registry.rs
monthly-credits = Monthly credits

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
banked-resets = Banked resets

# src/provider_registry.rs
usage-stats-7b9e1a = Usage stats

# src/provider_registry.rs
spending = Spending

# src/settings.rs
separate-tabs = Separate tabs

# src/settings.rs
grouped-switcher = Grouped, switcher

# src/settings.rs
grouped-all-accounts = Grouped, all accounts

# src/settings.rs
openrouter-account = OpenRouter account

# src/settings.rs
yesterday = Yesterday

# src/settings.rs
history-retention-must-be-between-1-and-365-days = history retention must be between 1 and 365 days

# src/settings.rs
session-low-usage-threshold-must-be-between-1-and-99-percent = session low usage threshold must be between 1 and 99 percent

# src/settings.rs
weekly-low-usage-threshold-must-be-between-1-and-99-percent = weekly low usage threshold must be between 1 and 99 percent

# src/settings_window/about.rs
check-github-for-a-new-version = Check GitHub for a new version

# src/settings_window/about.rs
checking-for-updates = Checking for updates…

# src/settings_window/about.rs
you-re-up-to-date = You're up to date

# src/settings_window/about.rs
update-available = Update { $v0 } available

# src/settings_window/about.rs
installing-update = Installing update…

# src/settings_window/about.rs
couldn-t-check-for-updates = Couldn't check for updates

# src/settings_window/about.rs
version = Version { $v0 }

# src/settings_window/about.rs
usage-limits-in-the-windows-tray = Usage limits in the Windows tray.

# src/settings_window/about.rs
a-new-release-is-ready-to-install = A new release is ready to install.

# src/settings_window/about.rs
what-s-new = What's new

# src/settings_window/about.rs
update = Update

# src/settings_window/about.rs
check-for-updates = Check for updates

# src/settings_window/about.rs
check-for-updates-on-startup = Check for updates on startup

# src/settings_window/about.rs
notify-when-a-new-version-is-found = Notify when a new version is found

# src/settings_window/about.rs
github = GitHub

# src/settings_window/about.rs
source-code = Source code

# src/settings_window/about.rs
releases = Releases

# src/settings_window/about.rs
see-what-s-new = See what's new

# src/settings_window/about.rs
report-an-issue = Report an issue

# src/settings_window/about.rs
found-a-bug = Found a bug?

# src/settings_window/about.rs
author = Author

# src/settings_window/onboarding.rs
updates = Updates

# src/settings_window/about.rs
resources = Resources

# src/usage_overview.rs
mon = Mon

# src/usage_overview.rs
tue = Tue

# src/usage_overview.rs
wed = Wed

# src/usage_overview.rs
thu = Thu

# src/usage_overview.rs
fri = Fri

# src/settings_window/activation.rs
every-day = Every day

# src/settings_window/activation.rs
weekdays = Weekdays

# src/settings_window/activation.rs
weekends = Weekends

# src/settings_window/activation.rs
no-days = No days

# src/settings_window/onboarding.rs
start-5-hour-sessions-automatically = Start 5-hour sessions automatically

# src/settings_window/activation.rs
starts-a-new-session-as-soon-as-a-window-is-available-instead-of = Starts a new session as soon as a window is available, instead of waiting for your first request. Each account uses its own login.

# src/settings_window/activation.rs
add-codex-or-claude-in-providers-first = Add Codex or Claude in Providers first.

# src/settings_window/activation.rs
off-in-providers = Off in Providers

# src/settings_window/activation.rs
quiet-periods = Quiet periods

# src/settings_window/activation.rs
don-t-auto-start-sessions-during-these-times = Don't auto-start sessions during these times.

# src/settings_window/providers/dialog.rs
add = Add

# src/settings_window/activation.rs
all-day = All day

# src/settings_window/activation.rs
from = From

# src/settings_window/activation.rs
until = Until

# src/settings_window/activation.rs
scheduled-activations = Scheduled activations

# src/settings_window/activation.rs
start-a-5-hour-session-at-a-set-time = Start a 5-hour session at a set time.

# src/settings_window/activation.rs
time = Time

# src/settings_window/activation.rs
turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = Turn on Codex or Claude in Providers first, with a config folder login.

# src/settings_window/activation.rs
no-quiet-periods-yet = No quiet periods yet.

# src/settings_window/activation.rs
no-scheduled-activations-yet = No scheduled activations yet.

# src/settings_window/activation.rs
unknown-provider = Unknown provider

# src/settings_window/activation.rs
remove-quiet-period = Remove quiet period

# src/settings_window/activation.rs
remove-activation = Remove activation

# src/settings_window/tray.rs
provider = Provider

# src/settings_window/activation.rs
days = Days

# src/settings_window/advanced.rs
export-settings = Export settings

# src/settings_window/advanced.rs
save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = Save every setting to a .toml file. Saved keys stay in Windows user storage.

# src/settings_window/advanced.rs
export = Export

# src/settings_window/advanced.rs
import-settings = Import settings

# src/settings_window/advanced.rs
replace-the-current-settings-with-a-previously-exported-file = Replace the current settings with a previously exported file.

# src/settings_window/advanced.rs
import = Import

# src/settings_window/advanced.rs
clear-usage-data = Clear Usage data

# src/settings_window/advanced.rs
delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = Delete the collected Usage history. It is rebuilt from local provider logs on the next scan.

# src/settings_window/advanced.rs
clear = Clear

# src/settings_window/advanced.rs
usage-data-clear-failed = Usage data clear failed

# src/settings_window/advanced.rs
the-background-worker-is-unavailable = The background worker is unavailable.

# src/settings_window/advanced.rs
usage-data-cleared = Usage data cleared.

# src/settings_window/advanced.rs
reset-all-settings = Reset all settings

# src/settings_window/advanced.rs
restore-every-default-and-start-the-welcome-flow-again = Restore every default and start the welcome flow again.

# src/settings_window/advanced.rs
reset = Reset

# src/settings_window/advanced.rs
backup = Backup

# src/settings_window/advanced.rs
data = Data

# src/settings_window/advanced.rs
settings-exported = Settings exported.

# src/settings_window/advanced.rs
settings-export-failed = Settings export failed

# src/settings_window/advanced.rs
settings-imported = Settings imported.

# src/settings_window/advanced.rs
settings-import-failed = Settings import failed

# src/settings_window/advanced.rs
settings-reset-failed = Settings reset failed

# src/settings_window/advanced.rs
reset-all-settings-bbfe66 = Reset all settings?

# src/settings_window/advanced.rs
every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = Every setting returns to its default and the welcome flow opens again. Saved keys and Usage data are kept.

# src/settings_window/troubleshoot.rs
cancel = Cancel

# src/settings_window/appearance.rs
windows = Windows

# src/settings_window/appearance.rs
light = Light

# src/settings_window/appearance.rs
dark = Dark

# src/settings_window/appearance.rs
color-theme = Color theme

# src/settings_window/appearance.rs
applies-to-settings-the-popup-and-its-tray-menu = Applies to Settings, the popup and its tray menu.

# src/settings_window/appearance.rs
accent-color = Accent color

# src/settings_window/appearance.rs
windows-follows-your-system-accent = Windows follows your system accent.

# src/settings_window/appearance.rs
icons-style = Icons style

# src/settings_window/appearance.rs
font = Font

# src/settings_window/appearance.rs
any-font-installed-on-this-pc = Any font installed on this PC.

# src/settings_window/appearance.rs
windows-default = Windows default

# src/settings_window/appearance.rs
glyph-style-in-the-settings-sidebar = Glyph style in the Settings sidebar.

# src/settings_window/appearance.rs
colored = Colored

# src/settings_window/tray.rs
monochrome = Monochrome

# src/settings_window/appearance.rs
time-format = Time format

# src/settings_window/appearance.rs
msg-12-hour = 12-hour

# src/settings_window/appearance.rs
msg-24-hour = 24-hour

# src/settings_window/appearance.rs
popup-background = Popup background
popup-theme = Popup theme
popup-theme-description = Changes only the popup. Settings keep the Windows look.
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel

# src/settings_window/appearance.rs
acrylic = Acrylic

# src/settings_window/appearance.rs
mica = Mica

# src/settings_window/appearance.rs
solid = Solid

# src/settings_window/appearance.rs
bottom-bar-size = Bottom bar size

# src/settings_window/appearance.rs
comfortable = Comfortable

# src/settings_window/appearance.rs
compact = Compact

# src/settings_window/appearance.rs
popup-corner-radius = Popup corner radius

# src/settings_window/appearance.rs
animation-effects = Animation effects

# src/settings_window/appearance.rs
glide-transitions-in-the-popup-and-settings-windows-own-animation = Glide transitions in the popup and Settings. Windows' own animation setting is also respected.

# src/settings_window/appearance.rs
popup = Popup

# src/settings_window/appearance.rs
motion = Motion

# src/settings_window/appearance.rs
blue = Blue

# src/settings_window/appearance.rs
purple = Purple

# src/settings_window/appearance.rs
pink = Pink

# src/settings_window/appearance.rs
red = Red

# src/settings_window/appearance.rs
orange = Orange

# src/settings_window/appearance.rs
green = Green

# src/settings_window/appearance.rs
teal = Teal

# src/settings_window/customize.rs
use-two-columns = Use two columns

# src/settings_window/customize.rs
several-accounts-of-one-provider = Several accounts of one provider

# src/settings_window/customize.rs
separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = Separate tabs gives every instance its own tab. Grouped shows one tab per provider, with an account switcher or every account stacked.

# src/settings_window/customize.rs
use-monochrome-icons = Use monochrome icons

# src/settings_window/customize.rs
draw-provider-marks-in-the-popup-without-brand-colors = Draw provider marks in the popup without brand colors.

# src/settings_window/onboarding.rs
show-used-instead-of-remaining = Show used instead of remaining

# src/settings_window/customize.rs
show-usage-in-values-when-possible = Show usage in values (when possible)

# src/settings_window/customize.rs
adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = Adds exact used/limit amounts next to percentages when a provider reports them.

# src/settings_window/onboarding.rs
show-usage-pace = Show usage pace

# src/settings_window/onboarding.rs
marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = Marks whether you're burning quota faster or slower than an even pace.

# src/settings_window/customize.rs
use-legacy-usage-cards = Use legacy usage cards

# src/settings_window/customize.rs
show-the-older-layout-with-a-header-a-thin-bar-and-a-footer = Show the older layout with a header, a thin bar and a footer.

# src/settings_window/onboarding.rs
show-account-name = Show account name

# src/settings_window/customize.rs
show-on-home-tab = Show on Home tab

# src/settings_window/customize.rs
layout = Layout

# src/settings_window/customize.rs
donut = Donut

# src/settings_window/customize.rs
cards = Cards

# src/settings_window/customize.rs
tabs = Tabs

# src/settings_window/customize.rs
usage-widget = Usage Widget

# src/settings_window/customize.rs
card = Card

# src/settings_window/customize.rs
tab = Tab

# src/settings_window/customize.rs
popup-cards = Popup cards

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = Choose which cards this account shows on Home and on its own tab.

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = Choose which cards this account shows on its own tab. Turn on Show on Home to pick Home cards.

# src/settings_window/general.rs
msg-30-seconds = 30 seconds

# src/settings_window/general.rs
msg-1-minute = 1 minute

# src/settings_window/general.rs
msg-2-minutes = 2 minutes

# src/settings_window/general.rs
msg-5-minutes = 5 minutes

# src/settings_window/general.rs
msg-10-minutes = 10 minutes

# src/settings_window/general.rs
msg-15-minutes = 15 minutes

# src/settings_window/general.rs
msg-30-minutes = 30 minutes

# src/settings_window/general.rs
msg-45-minutes = 45 minutes

# src/settings_window/general.rs
msg-60-minutes = 60 minutes

# src/settings_window/onboarding.rs
start-with-windows = Start with Windows

# src/settings_window/general.rs
open-codex-minibar-in-the-tray-when-you-sign-in = Open Codex Minibar in the tray when you sign in.

# src/settings_window/onboarding.rs
refresh-interval = Refresh interval

# src/settings_window/general.rs
how-often-this-instance-s-quotas-are-read = How often this instance's quotas are read. Longer intervals avoid provider rate limits.

# src/settings_window/general.rs
enable-usage-stats = Enable Usage Stats

# src/settings_window/general.rs
scan-local-provider-history-for-the-usage-tab-and-cost-totals = Scan local provider history for the Usage tab and cost totals.

# src/settings_window/general.rs
collection-period = Collection period

# src/settings_window/general.rs
how-often-local-provider-history-is-scanned = How often local provider history is scanned.

# src/settings_window/general.rs
check-for-confirmed-tibo-resets = Check for confirmed Tibo resets

# src/settings_window/general.rs
reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = Reads the app's public GitHub feed and keeps the latest announcement cached.

# src/settings_window/general.rs
notify-when-new-reset-info-arrives = Notify when new reset info arrives

# src/settings_window/general.rs
shows-a-notification-when-the-feed-reports-a-possible-reset-never = Shows a notification when the feed reports a possible reset, never at the reset time.

# src/settings_window/general.rs
check-every = Check every

# src/settings_window/general.rs
the-feed-is-also-checked-immediately-when-the-app-starts-or-this = The feed is also checked immediately when the app starts or this option is enabled.

# src/settings_window/general.rs
msg-1-hour = 1 hour

# src/settings_window/general.rs
msg-3-hours = 3 hours

# src/settings_window/general.rs
msg-2-hours = 2 hours

# src/settings_window/general.rs
msg-5-hours = 5 hours

# src/settings_window/general.rs
msg-6-hours = 6 hours

# src/settings_window/general.rs
msg-12-hours = 12 hours

# src/settings_window/general.rs
msg-24-hours = 24 hours

# src/settings_window/general.rs
tibo-resets = Tibo Resets™

# src/settings_window/general.rs
enable-a-provider-in-the-providers-tab-to-include-it-here = Enable a provider in the Providers tab to include it here.

# src/settings_window/general.rs
add-a-management-key = Add a management key

# src/settings_window/general.rs
usage-statistics-are-off-on-this-provider-s-page = Usage statistics are off on this provider's page

# src/settings_window/general.rs
included-providers = Included providers

# src/settings_window/general.rs
choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = Choose which accounts count toward this machine's Usage tab and Home total. Excluded accounts keep collecting and still show usage on their own page.

# src/settings_window/integrations.rs
download-the-latest-stream-deck-companion-from-github-and-open-it = Download the latest Stream Deck companion from GitHub and open its installer.

# src/settings_window/integrations.rs
downloading-the-latest-stream-deck-companion-from-github = Downloading the latest Stream Deck companion from GitHub...

# src/settings_window/integrations.rs
opening-the-stream-deck-installer = Opening the Stream Deck installer...

# src/settings_window/integrations.rs
the-installer-is-open-finish-the-installation-in-stream-deck = The installer is open. Finish the installation in Stream Deck.

# src/settings_window/integrations.rs
the-plugin-could-not-be-downloaded-or-opened-try-again = The plugin could not be downloaded or opened. Try again.

# src/settings_window/integrations.rs
downloading = Downloading...

# src/settings_window/integrations.rs
opening = Opening...

# src/settings_window/integrations.rs
install-plugin = Install plugin

# src/settings_window/integrations.rs
stream-deck-companion = Stream Deck companion

# src/settings_window/integrations.rs
check-your-connection-then-try-again = Check your connection, then try again.

# src/settings_window/providers/page.rs
on = On

# src/settings_window/providers/page.rs
off = Off

# src/settings_window/kit.rs
more-options = More options

# src/settings_window/kit.rs
custom-color = Custom color

# src/settings_window/troubleshoot.rs
run-troubleshoot-with-ai = Run Troubleshoot with AI

# src/settings_window/log.rs
let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = Let an installed AI CLI read the log and investigate a problem.

# src/settings_window/log.rs
choose-tool = Choose tool

# src/settings_window/log.rs
application-log = Application log

# src/settings_window/log.rs
log-txt-in-the-app-data-folder = log.txt in the app data folder.

# src/settings_window/log.rs
open-log-txt = Open log.txt

# src/settings_window/log.rs
could-not-open-log-txt = Could not open log.txt

# src/settings_window/log.rs
open-logs-folder = Open logs folder

# src/settings_window/log.rs
could-not-open-logs-folder = Could not open logs folder

# src/settings_window/log.rs
no-log-events-yet = No log events yet.

# src/settings_window/log.rs
live-tail = Live tail

# src/settings_window/log.rs
no-supported-ai-tool-found = No supported AI tool found

# src/settings_window/log.rs
install-codex-or-claude-code-and-make-it-available-to-minibar = Install Codex or Claude Code and make it available to Minibar.

# src/settings_window/window.rs
general = General

# src/settings_window/window.rs
appearance = Appearance

# src/settings_window/window.rs
providers = Providers

# src/settings_window/window.rs
customize = Customize

# src/settings_window/window.rs
limit-activation = Limit activation

# src/settings_window/window.rs
tray = Tray

# src/settings_window/window.rs
notifications = Notifications

# src/settings_window/window.rs
advanced = Advanced

# src/settings_window/window.rs
log = Log

# src/settings_window/window.rs
integrations = Integrations

# src/settings_window/nav.rs
about-updates = About & Updates

# src/settings_window/onboarding.rs
successful-activations = Successful activations

# src/settings_window/onboarding.rs
failed-activations = Failed activations

# src/settings_window/onboarding.rs
when-limits-reset = When limits reset

# src/settings_window/onboarding.rs
when-5-hour-remaining-hits = When 5-hour remaining hits { $v0 }%

# src/settings_window/onboarding.rs
when-weekly-remaining-hits = When weekly remaining hits { $v0 }%

# src/settings_window/onboarding.rs
low-usage = Low usage

# src/settings_window/notifications.rs
shows-a-notification-once-per-window-when-the-remaining-share-dro = Shows a notification once per window when the remaining share drops to the threshold.

# src/settings_window/notifications.rs
threshold = Threshold

# src/settings_window/onboarding.rs
found-in-opencode-auth-or-local-history = Found in OpenCode auth or local history.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-set-up-elsewhere = Not found. Turn it on if it's set up elsewhere.

# src/settings_window/onboarding.rs
account-credentials-are-already-set = Account credentials are already set.

# src/settings_window/onboarding.rs
optional-add-accounts-later-in-providers = Optional. Add accounts later in Providers.

# src/settings_window/onboarding.rs
found-an-official-agy-sign-in-on-this-pc = Found an official agy sign-in on this PC.

# src/settings_window/onboarding.rs
not-found-sign-in-with-agy-before-enabling-it = Not found. Sign in with agy before enabling it.

# src/settings_window/onboarding.rs
found-an-official-grok-cli-sign-in-on-this-pc = Found an official Grok CLI sign-in on this PC.

# src/settings_window/onboarding.rs
not-found-run-grok-login-before-enabling-it = Not found. Run grok login before enabling it.

# src/settings_window/onboarding.rs
found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Found Kiro IDE, Kiro Crew, or a signed-in Kiro CLI.

# src/settings_window/onboarding.rs
not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = Not found. Install Kiro IDE or Kiro Crew, or sign in to Kiro CLI.

# src/settings_window/onboarding.rs
found-on-this-pc = Found on this PC.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-installed-somewhere-else = Not found. Turn it on if it's installed somewhere else.

# src/settings_window/onboarding.rs
setup-could-not-be-saved = Setup could not be saved

# src/settings_window/onboarding.rs
detected = Detected

# src/settings_window/onboarding.rs
starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = Starts a new Codex or Claude session as soon as a window is available, instead of waiting for your first request.

# src/settings_window/onboarding.rs
collect-usage-data = Collect usage data

# src/settings_window/onboarding.rs
scans-local-provider-history-for-usage-stats = Scans local provider history for Usage Stats.

# src/settings_window/onboarding.rs
startup = Startup

# src/settings_window/providers/page.rs
features = Features

# src/settings_window/onboarding.rs
customization = Customization

# src/settings_window/onboarding.rs
when-a-new-version-is-found = When a new version is found

# src/settings_window/onboarding.rs
activity = Activity

# src/settings_window/onboarding.rs
choose-providers = Choose providers

# src/settings_window/onboarding.rs
we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = We turned on the providers found on this PC. You can change this later.

# src/settings_window/onboarding.rs
general-settings = General settings

# src/settings_window/onboarding.rs
you-can-change-these-later-in-settings = You can change these later in Settings.

# src/settings_window/onboarding.rs
turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = Turn off anything you don't want to hear about. You can change these later in Settings.

# src/settings_window/window.rs
back = Back

# src/settings_window/onboarding.rs
continue = Continue

# src/settings_window/tray.rs
done = Done

# src/settings_window/providers/dialog.rs
details = Details

# src/settings_window/providers/dialog.rs
added = Added

# src/settings_window/providers/dialog.rs
reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Reads the session and weekly limits of a Claude subscription without a Claude Code login.

# src/settings_window/providers/dialog.rs
msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. Open Claude in a separate browser profile or private window. Sign in to the account you want to track and confirm its email in Claude's settings.

# src/settings_window/providers/dialog.rs
open-claude-ai = Open claude.ai

# src/settings_window/providers/dialog.rs
msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. In Chrome or Edge, press F12. Open Application > Storage > Cookies and select https://claude.ai.

# src/settings_window/providers/dialog.rs
msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. Find sessionKey. Copy its Value, not its name or the whole cookie table, and paste it into the field above.

# src/settings_window/providers/dialog.rs
how-to-view-cookies-in-chrome = How to view cookies in Chrome

# src/settings_window/providers/dialog.rs
a-cookie-header-containing-sessionkey-also-works-when-the-session = A Cookie header containing sessionKey also works. When the session expires, paste a fresh one here.

# src/settings_window/providers/dialog.rs
use-the-access-token-from-a-claude-code-subscription-login-miniba = Use the access token from a Claude Code subscription login. Minibar cannot refresh a pasted token; prefer Source: Config folder when you can.

# src/settings_window/providers/dialog.rs
copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = Copy only claudeAiOauth.accessToken (starts with sk-ant-oat) from that login's .credentials.json, without quotes. Do not use claude setup-token: it may lack usage access.

# src/settings_window/providers/dialog.rs
claude-code-log-in-with-multiple-accounts = Claude Code: log in with multiple accounts

# src/settings_window/providers/dialog.rs
requires-a-claude-subscription-login-with-usage-access-api-keys-a = Requires a Claude subscription login with usage access. API keys and Admin API keys do not show subscription limits.

# src/settings_window/providers/dialog.rs
this-provider = this provider

# src/settings_window/providers/dialog.rs
track-another-account-or-a-provider-minibar-has-not-shown-yet = Track another account or a provider Minibar has not shown yet.

# src/settings_window/providers/dialog.rs
e-g-work = e.g. Work

# src/settings_window/providers/page.rs
name = Name

# src/settings_window/providers/dialog.rs
shown-on-its-tab-home-card-tray-and-notifications = Shown on its tab, Home card, tray and notifications.

# src/settings_window/providers/page.rs
badge = Badge

# src/settings_window/providers/dialog.rs
auto = Auto

# src/settings_window/providers/page.rs
badge-color = Badge color

# src/settings_window/providers/dialog.rs
up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = Up to three letters; empty uses the name's initials. Badges show while a provider has more than one instance turned on.

# src/settings_window/window.rs
add-provider = Add provider

# src/settings_window/providers/dialog.rs
next = Next

# src/settings_window/providers/dialog.rs
minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar stops reading { $name } and forgets its saved keys, schedules, tray indicators and Home position.{ $v0 }

# src/settings_window/providers/dialog.rs
its-config-folder-stays-on-disk = { " " }Its config folder stays on disk.

# src/settings_window/providers/dialog.rs
delete-name = Delete { $name }?

# src/settings_window/providers/dialog.rs
delete = Delete

# src/settings_window/providers/dialog.rs
finish-signing-in-in-your-browser-cancel-stops-this-login = Finish signing in in your browser. Cancel stops this login.

# src/settings_window/providers/dialog.rs
runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = Runs Claude Code's own login for { $name } with its config folder as CLAUDE_CONFIG_DIR. The login stays in that folder, where Claude Code keeps it fresh. Requires native Windows Claude Code.

# src/settings_window/providers/dialog.rs
runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = Runs Codex's own login for { $name } with its config folder as CODEX_HOME. The login stays in that folder, where Codex keeps it fresh. Requires the native Codex CLI or desktop app.

# src/settings_window/providers/dialog.rs
sign-in-to-name = Sign in to { $name }

# src/settings_window/providers/page.rs
sign-in = Sign in

# src/settings_window/providers/dialog.rs
for = For { $v0 }.

# src/settings_window/providers/dialog.rs
browser-session = Browser session

# src/settings_window/providers/dialog.rs
oauth-token = OAuth token

# src/settings_window/providers/dialog.rs
session-key = Session key

# src/settings_window/providers/dialog.rs
paste-the-sessionkey-value = Paste the sessionKey value

# src/settings_window/providers/dialog.rs
oauth-access-token = OAuth access token

# src/settings_window/providers/dialog.rs
the-saved-credential-is-replaced-only-after-the-new-one-passes-th = The saved credential is replaced only after the new one passes the check.

# src/settings_window/providers/dialog.rs
claude-credential = Claude credential

# src/settings_window/providers/dialog.rs
check-and-save = Check and save

# src/settings_window/providers/dialog.rs
key-name-optional = Key name (optional)

# src/settings_window/providers/dialog.rs
e-g-personal = e.g. Personal

# src/settings_window/providers/dialog.rs
leave-blank-to-use-the-name-from-openrouter = Leave blank to use the name from OpenRouter.

# src/settings_window/providers/dialog.rs
minibar-checks-the-key-with-openrouter-before-saving-it = Minibar checks the key with OpenRouter before saving it.

# src/settings_window/providers/dialog.rs
replace-api-key = Replace API key

# src/settings_window/providers/page.rs
add-api-key = Add API key

# src/settings_window/providers/page.rs
management-key = Management key

# src/settings_window/providers/dialog.rs
create-one-under-settings-management-keys-on-openrouter-ai = Create one under Settings → Management keys on openrouter.ai.

# src/settings_window/providers/dialog.rs
replace-management-key = Replace management key

# src/settings_window/providers/page.rs
add-management-key = Add management key

# src/settings_window/providers/dialog.rs
key-name = Key name

# src/settings_window/providers/page.rs
rename-key = Rename key

# src/settings_window/providers/dialog.rs
save = Save

# src/settings_window/providers/dialog.rs
this-key = this key

# src/settings_window/providers/dialog.rs
minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibar stops tracking { $hint }. The key keeps working on OpenRouter.

# src/settings_window/providers/dialog.rs
remove-api-key = Remove API key?

# src/settings_window/tray.rs
remove = Remove

# src/settings_window/providers/dialog.rs
minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibar stops showing credit balance and usage history for { $v0 }. The key keeps working on OpenRouter.

# src/settings_window/providers/dialog.rs
remove-management-key = Remove management key?

# src/settings_window/providers/dialog.rs
saved-in-windows-user-storage-never-in-the-settings-file = Saved in Windows user storage, never in the settings file.

# src/settings_window/providers/dialog.rs
save-key = Save key

# src/settings_window/providers/dialog.rs
minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibar forgets the saved key of { $v0 }. It keeps working with OpenCode.

# src/settings_window/providers/dialog.rs
signing-in = Signing in…

# src/settings_window/providers/page.rs
checking = Checking…

# src/settings_window/providers/dialog.rs
choose-a-provider = Choose a provider.

# src/settings_window/providers/dialog.rs
reason-it-is-already-in-the-list = { $reason }. It is already in the list.

# src/settings_window/providers/dialog.rs
added-name = Added { $name }.

# src/settings_window/providers/dialog.rs
could-not-add-the-provider-error = Could not add the provider: { $error }

# src/settings_window/providers.rs
this-provider-no-longer-exists = This provider no longer exists.

# src/settings_window/providers/dialog.rs
could-not-delete-the-provider-error = Could not delete the provider: { $error }

# src/settings_window/providers/dialog.rs
paste-a-credential-first = Paste a credential first.

# src/settings_window/providers/dialog.rs
credential-saved-in-windows-user-storage = Credential saved in Windows user storage.

# src/settings_window/providers/dialog.rs
paste-a-key-first = Paste a key first.

# src/settings_window/providers/dialog.rs
this-account-no-longer-exists = This account no longer exists.

# src/settings_window/providers/dialog.rs
api-key-saved-in-windows-user-storage = API key saved in Windows user storage.

# src/settings_window/providers/dialog.rs
management-key-replaced = Management key replaced.

# src/settings_window/providers/dialog.rs
management-key-added = Management key added.

# src/settings_window/providers/dialog.rs
could-not-rename-the-key-error = Could not rename the key: { $error }

# src/settings_window/providers/dialog.rs
api-key-renamed = API key renamed.

# src/settings_window/providers/dialog.rs
could-not-remove-the-key-error = Could not remove the key: { $error }

# src/settings_window/providers/dialog.rs
api-key-removed = API key removed.

# src/settings_window/providers/dialog.rs
management-key-removed = Management key removed.

# src/settings_window/providers/dialog.rs
could-not-save-the-key-error = Could not save the key: { $error }

# src/settings_window/providers/dialog.rs
api-key-saved = API key saved.

# src/settings_window/providers/dialog.rs
switch-source-to-config-folder-to-sign-in = Switch Source to Config folder to sign in.

# src/settings_window/providers/dialog.rs
this-provider-has-no-config-folder = This provider has no config folder.

# src/settings_window/providers/dialog.rs
another-instance-already-reads-this-config-folder-choose-a-differ = Another instance already reads this config folder. Choose a different folder first.

# src/settings_window/providers/dialog.rs
this-provider-has-no-sign-in = This provider has no sign-in.

# src/settings_window/providers/dialog.rs
signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 } signed in. Its CLI keeps the login fresh in its config folder.

# src/settings_window/providers/page.rs
no-keys-yet = No keys yet

# src/settings_window/providers/page.rs
management-key-5c8cf2 = { " " }· management key

# src/settings_window/providers/page.rs
using-a-saved-credential = Using a saved credential

# src/settings_window/providers/page.rs
paste-a-credential-under-account = Paste a credential under Account

# src/settings_window/providers/page.rs
needs-an-api-key-or-opencode-sign-in = Needs an API key or OpenCode sign-in

# src/settings_window/providers/page.rs
needs-an-api-key = Needs an API key

# src/settings_window/providers/page.rs
not-found-set-its-folder-under-runtime = Not found. Set its folder under Runtime.

# src/settings_window/providers/page.rs
using-a-saved-api-key = Using a saved API key

# src/settings_window/providers/page.rs
using-opencode-sign-in-or-local-history = Using OpenCode sign-in or local history

# src/settings_window/providers/page.rs
reading-cli = Reading { $cli }

# src/settings_window/providers/page.rs
reading-crew = Reading { $crew }

# src/settings_window/providers/page.rs
reading-app = Reading { $app }

# src/settings_window/providers/page.rs
checking-kiro-ide-kiro-crew-and-cli = Checking Kiro IDE, Kiro Crew, and CLI…

# src/settings_window/providers/page.rs
checking-installed-app-and-cli = Checking installed app and CLI…

# src/settings_window/providers/page.rs
checking-cli = Checking CLI…

# src/settings_window/providers/page.rs
checking-installed-app = Checking installed app…

# src/settings_window/providers/page.rs
no-providers-yet-add-one-to-start-reading-limits = No providers yet. Add one to start reading limits.

# src/settings_window/providers/page.rs
is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } is off, so it doesn't appear in the minibar or tray. Turn it on to start reading usage.

# src/settings_window/providers/page.rs
replace-chatgpt-logo-with-codex = Replace ChatGPT logo with Codex

# src/settings_window/providers/page.rs
limits-only = Limits only

# src/settings_window/providers/page.rs
delete-provider = Delete provider

# src/settings_window/providers/page.rs
display-name = Display name

# src/settings_window/providers/page.rs
shown-on-popup-tabs-home-cards-the-tray-and-notifications = Shown on popup tabs, Home cards, the tray and notifications.

# src/settings_window/providers/page.rs
up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = Up to three letters. Leave empty to use the name's initials. Shown while a provider has more than one instance turned on.

# src/settings_window/providers/page.rs
auto-uses-a-neutral-plate-that-follows-the-theme = Auto uses a neutral plate that follows the theme.

# src/settings_window/providers/page.rs
show-on-home = Show on Home

# src/settings_window/providers/page.rs
its-provider-tab-stays-available-when-hidden-from-home = Its provider tab stays available when hidden from Home.

# src/settings_window/providers/page.rs
credential = Credential

# src/settings_window/providers/page.rs
signed-in-as-identity = Signed in as { $identity }

# src/settings_window/providers/page.rs
saved-in-windows-user-storage = Saved in Windows user storage

# src/settings_window/providers/page.rs
paste-a-sessionkey-or-an-oauth-access-token = Paste a sessionKey or an OAuth access token.

# src/settings_window/providers/page.rs
reads-limits-only-minibar-cannot-refresh-a-pasted-credential = Reads limits only. Minibar cannot refresh a pasted credential.

# src/settings_window/providers/page.rs
replace-credential = Replace credential

# src/settings_window/providers/page.rs
add-credential = Add credential

# src/settings_window/providers/page.rs
signed-in-account = Signed-in account

# src/settings_window/providers/page.rs
not-signed-in-yet-or-no-limits-read-so-far = Not signed in yet, or no limits read so far.

# src/settings_window/providers/page.rs
account = Account

# src/settings_window/providers/page.rs
runtime = Runtime

# src/settings_window/providers/page.rs
minibar-finds-these-automatically = { $v0 } Minibar finds these automatically.

# src/settings_window/providers/page.rs
in-use = In use

# src/settings_window/providers/page.rs
found = Found

# src/settings_window/providers/page.rs
copy-path = Copy path

# src/settings_window/providers/page.rs
open-folder = Open folder

# src/settings_window/providers/page.rs
path-copied = Path copied.

# src/settings_window/providers/page.rs
not-installed-or-installed-somewhere-minibar-doesn-t-look = Not installed, or installed somewhere Minibar doesn't look.

# src/settings_window/providers/page.rs
not-found = Not found

# src/settings_window/providers/page.rs
choose-folder = Choose folder…

# src/settings_window/providers/page.rs
select-folder = Select folder

# src/settings_window/providers/page.rs
choose-folder-1838a4 = Choose folder

# src/settings_window/providers/page.rs
only-needed-if-automatic-detection-misses-your-install = Only needed if automatic detection misses your install.

# src/settings_window/providers/page.rs
source = Source

# src/settings_window/providers/page.rs
config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = Config folder reads the Claude Code login in CLAUDE_CONFIG_DIR. Manual credential reads limits only from a pasted credential.

# src/settings_window/providers/page.rs
config-folder = Config folder

# src/settings_window/providers/page.rs
manual = Manual

# src/settings_window/providers/page.rs
passed-to-the-cli-as-env-leave-empty-to-use = Passed to the CLI as { $env }. Leave empty to use { $v0 }.

# src/settings_window/providers/page.rs
the-standard-folder = the standard folder

# src/settings_window/providers/page.rs
a-folder-minibar-creates-for-this-instance = a folder Minibar creates for this instance

# src/settings_window/providers/page.rs
other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } already reads this folder. Two instances must not share a login, or usage is counted twice.

# src/settings_window/providers/page.rs
automatic-activation = Automatic activation

# src/settings_window/providers/page.rs
starts-this-account-s-5-hour-window-when-it-resets-using-its-own = Starts this account's 5-hour window when it resets, using its own login. Schedules and pauses are under Limit activation.

# src/settings_window/providers/page.rs
usage-statistics = Usage statistics

# src/settings_window/providers/page.rs
scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = Scans this instance's local history for its usage card. Turn off to stop collecting entirely.

# src/settings_window/providers/page.rs
opencode-sign-in-or-local-history = OpenCode sign-in or local history

# src/settings_window/providers/page.rs
found-in-opencode-auth-environment-a-saved-key-or-local-history = Found in OpenCode auth, environment, a saved key, or local history.

# src/settings_window/providers/page.rs
nothing-found-in-opencode-auth-environment-or-local-history = Nothing found in OpenCode auth, environment, or local history.

# src/settings_window/providers/page.rs
replace-key = Replace key

# src/settings_window/providers/page.rs
optional-only-needed-without-opencode-sign-in-on-this-pc = Optional. Only needed without OpenCode sign-in on this PC.

# src/settings_window/providers/page.rs
add-the-key-of-the-account-this-instance-tracks = Add the key of the account this instance tracks.

# src/settings_window/providers/page.rs
keys = Keys

# src/settings_window/providers/page.rs
a-management-key-shows-credit-balance-and-usage-history-api-keys = A management key shows credit balance and usage history. API keys show spend per key.

# src/settings_window/providers/page.rs
could-not-read-the-saved-key-reopen-this-page-to-retry = Could not read the saved key. Reopen this page to retry.

# src/settings_window/providers/page.rs
credit-balance-and-account-wide-usage-history = Credit balance and account-wide usage history

# src/settings_window/providers/page.rs
not-added-add-one-to-see-credit-balance-and-usage-history = Not added. Add one to see credit balance and usage history.

# src/settings_window/providers/page.rs
last-updated = Last updated { $v0 }, { $v1 }

# src/settings_window/providers/page.rs
no-api-keys-yet-add-one-to-track-spend-per-key = No API keys yet. Add one to track spend per key.

# src/settings_window/providers/page.rs
key = Key

# src/settings_window/providers/page.rs
spend = Spend

# src/settings_window/providers/page.rs
limit = Limit

# src/settings_window/providers/page.rs
error-reopen-this-page-to-retry = { $error }. Reopen this page to retry.

# src/settings_window/providers/page.rs
could-not-read-key = Could not read key

# src/settings_window/providers/page.rs
not-saved = Not saved

# src/settings_window/providers/page.rs
unnamed = Unnamed

# src/settings_window/providers/page.rs
of-the-limit = { $v0 } of the { $v1 } limit

# src/settings_window/providers/page.rs
no-spend-limit-key-spend-account-credits-purchased = No spend limit. Key spend: { $v0 }. Account credits purchased: { $v1 }.

# src/settings_window/providers/page.rs
none = None

# src/settings_window/providers/page.rs
add-key = Add key

# src/settings_window/providers.rs
openrouter-account-credentials-are-configured = OpenRouter account credentials are configured

# src/settings_window/providers.rs
saved-api-key = Saved API key

# src/settings_window/providers.rs
saved-credential = Saved credential

# src/settings_window/providers.rs
opencode-auth-json-or-local-database = OpenCode auth.json or local database

# src/settings_window/providers.rs
that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = That doesn't look like an OpenRouter key. Keys start with sk-or-.

# src/settings_window/providers.rs
codex-cli-folder = Codex CLI folder

# src/settings_window/providers.rs
claude-code-cli-folder = Claude Code CLI folder

# src/settings_window/providers.rs
cursor-app-folder = Cursor app folder

# src/settings_window/providers.rs
grok-cli-folder = Grok CLI folder

# src/settings_window/providers.rs
kiro-ide-folder = Kiro IDE folder

# src/settings_window/providers.rs
kiro-crew-app-path = Kiro Crew app path

# src/settings_window/providers.rs
kiro-cli-folder = Kiro CLI folder

# src/settings_window/providers.rs
reads-the-signed-in-codex-cli-or-desktop-app = Reads the signed-in Codex CLI or desktop app.

# src/settings_window/providers.rs
reads-your-existing-claude-code-login = Reads your existing Claude Code login.

# src/settings_window/providers.rs
reads-the-signed-in-cursor-app-for-this-billing-cycle = Reads the signed-in Cursor app for this billing cycle.

# src/settings_window/providers.rs
reads-zen-auth-and-local-opencode-history = Reads Zen auth and local OpenCode history.

# src/settings_window/providers.rs
reads-go-quota-windows-and-local-opencode-history = Reads Go quota windows and local OpenCode history.

# src/settings_window/providers.rs
reads-api-key-usage-and-spend-limits-a-management-key-also-enable = Reads API-key usage and spend limits. A management key also enables usage history and credit balance.

# src/settings_window/providers.rs
reads-subscription-quota-from-your-existing-official-agy-windows = Reads subscription quota from your existing official agy Windows sign-in.

# src/settings_window/providers.rs
reads-supergrok-subscription-credits-from-your-existing-official = Reads SuperGrok subscription credits from your existing official Grok CLI sign-in.

# src/settings_window/providers.rs
fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = Fetches Kiro's live monthly credits with its shared sign-in; recognizes IDE, Crew, and CLI installs.

# src/settings_window/providers.rs
codex-desktop-app = Codex desktop app

# src/settings_window/providers.rs
codex-cli = Codex CLI

# src/settings_window/providers.rs
claude-desktop-app = Claude desktop app

# src/settings_window/providers.rs
claude-code-cli = Claude Code CLI

# src/settings_window/providers.rs
cursor-app = Cursor app

# src/settings_window/providers.rs
antigravity-app = Antigravity app

# src/settings_window/providers.rs
grok-cli = Grok CLI

# src/settings_window/providers.rs
kiro-ide = Kiro IDE

# src/settings_window/providers.rs
kiro-crew = Kiro Crew

# src/settings_window/providers.rs
kiro-cli = Kiro CLI

# src/settings_window/providers.rs
supports-one-instance = { $v0 } supports one instance

# src/settings_window/tray.rs
numbers = Numbers

# src/settings_window/tray.rs
progress-bars = Progress bars

# src/settings_window/tray.rs
rings = Rings

# src/settings_window/tray.rs
reset-time = Reset time

# src/settings_window/tray.rs
countdown = Countdown

# src/settings_window/tray.rs
status = Status

# src/settings_window/tray.rs
fixed = Fixed

# src/settings_window/tray.rs
app-accent = App accent

# src/settings_window/tray.rs
percentages-as-digits = Percentages as digits

# src/settings_window/tray.rs
one-bar-per-indicator = One bar per indicator

# src/settings_window/tray.rs
nested-rings-one-per-indicator = Nested rings, one per indicator

# src/settings_window/tray.rs
when-the-limit-resets = When the limit resets

# src/settings_window/tray.rs
time-left-until-the-reset = Time left until the reset

# src/settings_window/tray.rs
used = Used

# src/settings_window/tray.rs
remaining = Remaining

# src/settings_window/tray.rs
app-icon = App icon

# src/settings_window/tray.rs
widget = Widget { $v0 }

# src/settings_window/tray.rs
the-codex-minibar-icon = The Codex Minibar icon

# src/settings_window/tray.rs
style-no-indicators = { $style } · No indicators

# src/settings_window/tray.rs
widget-removed = Widget removed

# src/settings_window/tray.rs
undo = Undo

# src/settings_window/tray.rs
tray-icon = Tray icon

# src/settings_window/tray.rs
shows-the-app-icon = Shows the app icon.

# src/settings_window/tray.rs
add-widget = Add widget

# src/settings_window/tray.rs
add-app-icon = Add app icon

# src/settings_window/tray.rs
move-up = Move up

# src/settings_window/tray.rs
move-down = Move down

# src/settings_window/tray.rs
edit = Edit

# src/settings_window/tray.rs
duplicate = Duplicate

# src/settings_window/tray.rs
shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = Shows the Codex Minibar icon in the notification area. It has no indicators to set up.

# src/settings_window/tray.rs
style = Style

# src/settings_window/tray.rs
up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = Up to { $max_indicators } quotas, drawn in this order. Expand one to change it.

# src/settings_window/tray.rs
indicators = Indicators

# src/settings_window/tray.rs
remove-widget = Remove widget

# src/settings_window/tray.rs
add-indicator = Add indicator

# src/settings_window/tray.rs
unsupported = Unsupported { $v0 }

# src/settings_window/tray.rs
remove-indicator = Remove indicator

# src/settings_window/tray.rs
unsupported-bba2a8 = Unsupported ({ $v0 })

# src/settings_window/tray.rs
unavailable-0c5d75 = Unavailable ({ $v0 })

# src/settings_window/tray.rs
metric = Metric

# src/settings_window/tray.rs
show = Show

# src/settings_window/tray.rs
color = Color

# src/settings_window/tray.rs
fixed-color = Fixed color

# src/settings_window/troubleshoot.rs
troubleshooting-could-not-start = Troubleshooting could not start

# src/settings_window/troubleshoot.rs
choose-which-installed-ai-tool-should-investigate-the-problem = Choose which installed AI tool should investigate the problem.

# src/settings_window/troubleshoot.rs
open-terminal = Open terminal

# src/settings_window/window.rs
version-is-available = { $version } is available

# src/settings_window/window.rs
update-now = Update now

# src/settings_window/window.rs
missing-a-provider = Missing a provider?

# src/settings_window/window.rs
ask-for-it-or-build-it-yourself = Ask for it or build it yourself.

# src/settings_window/window.rs
request-a-provider = Request a provider

# src/settings_window/window.rs
contribute-one = Contribute one

# src/tray.rs
disabled = Disabled

# src/updater.rs
update-complete = Update complete

# src/updater.rs
now-running-version = Now running { $version }.

# src/usage_overview.rs
past-24h = Past 24h

# application
language = Language

# application
applies-immediately-to-every-app-window-and-notification-auto-fol = Applies immediately to every app window and notification. Auto follows the Windows display language.

# application
auto-windows = Auto (Windows)

# application
english = English

# application
msg-russian = Русский

# application
codex-minibar-settings = Codex Minibar Settings

# application
welcome-to-codex-minibar = Welcome to Codex Minibar

# application
exit = Exit

# application
update-available-67fd3a = Update Available

# application
monthly-credits-976559 = Monthly Credits

# application
msg-5h-session-de7ce8 = 5h Session

# application
name-weekly = { $name } weekly

# application
remaining-remaining = { $remaining }% remaining

# application
tokens-94e0b9 = { $v0 }: { $v1 } tokens

# application
requests-priced = { $v0 ->
    [one] { $v0 } request · { $v1 } priced
   *[other] { $v0 } requests · { $v1 } priced
    }

# application
requests = { $v0 ->
    [one] { $v0 } requests
   *[other] { $v0 } requests
    }

# application
credits-338f52 = CREDITS

# application
cloud-session-credits = CLOUD SESSION CREDITS

# application
name-login-expires-in-days-left = { $days_left ->
    [one] { $name } login expires in { $days_left } day
   *[other] { $name } login expires in { $days_left } days
    }

# application
name-error = { $name } error

# application
sessions = { $v0 ->
    [one] { $v0 } session
   *[other] { $v0 } sessions
    }

# application
api-estimate = API estimate

# application
start-to-end = { $start } to { $end }

# application
to = { $v0 } to { $v1 }

# application
share-1-of-other = { $share }% of { $v0 } · { $other }

# application
cost-885dc4 = cost

# application
tokens-339143 = tokens

# application
sessions-0e5e29 = { $v0 ->
    [one] · { $v0 } session
   *[other] · { $v0 } sessions
    }

# application
api-key = API key

# application
api-keys = API keys

# application
name-deleted = { $name } deleted.

# application
openrouter-api-key-no-longer-exists = OpenRouter API key no longer exists

# application
openrouter-account-no-longer-exists = OpenRouter account no longer exists

# application
credit = { $v0 } credit

# application
agy-cli-folder = agy CLI folder

# application
folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = Folder with codex.exe, codex.cmd, or codex.ps1. Leave empty to find it automatically.

# application
folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = Folder with claude.exe, claude.cmd, or claude.ps1. Leave empty to find it automatically.

# application
folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = Folder with Cursor.exe. Leave empty to find it automatically. Usage still comes from the signed-in profile.

# application
folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = Folder with agy.exe, agy.cmd, or agy.ps1. Leave empty to find it automatically.

# application
folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = Folder with grok.exe, grok.cmd, or grok.ps1. Leave empty to find it automatically.

# application
folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = Folder containing Kiro.exe, or the executable itself. Leave empty to find it automatically.

# application
folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = Folder containing KiroCrew.exe, or the executable itself. Leave empty to detect per-user and all-users installs automatically.

# application
folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = Folder containing kiro-cli.exe, or the executable itself. Leave empty to find it automatically.

# application
off-45080e = { $v0 } (off)

# application
each-widget-is-one-icon-in-the-notification-area-indicators-show = Each widget is one icon in the notification area. Indicators show a provider's quota as numbers, bars, rings or a reset clock.

# application
a-reset-clock-follows-one-quota = A reset clock follows one quota.

# application
ai-tool = AI tool

# application
msg-7-days = 7 days

# application
msg-30-days = 30 days

# application
msg-90-days = 90 days

# application
yellow = Yellow

# application
custom = Custom

# application
not-available-for-manual-credentials-switch-source-to-config-fold = Not available for manual credentials. Switch Source to Config folder.

# application
opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = OpenCode's local history is tracked by the first OpenCode instance.

# application
this-provider-has-no-local-usage-history = This provider has no local usage history.

# application
this-provider-has-no-session-window-to-start = This provider has no session window to start.

# application
this-provider-does-not-use-a-config-folder = This provider does not use a config folder.

# application
could-not-save-provider-settings-error-restoring-its-previous-cre = Could not save provider settings ({ $error }); restoring its previous credential also failed ({ $rollback_error }).

# application
msg-5h-7d = 5h  |  { $v0 }  |  { $v1 }
    7d  |  { $v2 }  |  { $v3 }

# API key count in the provider header.
api-key-count = { $v0 ->
    [one] { $v0 } API key
   *[other] { $v0 } API keys
    }

# Application copy
widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = Widen Home and Usage. Drag Home blocks between columns; provider tabs stay compact.

# Application copy
a-possible-codex-reset-is-scheduled-for-when-in-countdown = A possible Codex reset is scheduled for { $when } (in { $countdown })

# Application copy
label-possible-reset-on-when-in-countdown = { $label }: possible reset on { $when } (in { $countdown })

# Application copy
new-codex-reset-info = New Codex reset info

# Application copy
login-expires-soon = { $v0 } login expires soon

# Application copy
it-stops-renewing-on-open-minibar-and-choose-sign-in-again = It stops renewing on { $v0 }. Open Minibar and choose Sign in again.

# Application copy
could-not-clear-usage-data-error = Could not clear usage data: { $error }

# Application copy
succeeded-at = { $v0 } succeeded at { $v1 }

# Application copy
failed-at-error = { $v0 } failed at { $v1 }: { $error }

# Application copy
key-39df89 = Key { $v0 }

# Application copy
expired = Expired

# Application copy
jan = Jan

# Application copy
feb = Feb

# Application copy
mar = Mar

# Application copy
apr = Apr

# Application copy
may = May

# Application copy
jun = Jun

# Application copy
jul = Jul

# Application copy
aug = Aug

# Application copy
sep = Sep

# Application copy
oct = Oct

# Application copy
nov = Nov

# Application copy
dec = Dec

# { $value }% used
quota-percent-used = { $value }% used

# { $value }% left
quota-percent-left = { $value }% left

# { $amount } of { $limit } used
cloud-amount-used = { $amount } of { $limit } used

# { $amount } of { $limit } left
cloud-amount-left = { $amount } of { $limit } left

# Short usage range
range-24h = 24h

# Short usage range
range-7d = 7d

# Short usage range
range-30d = 30d

# Short usage range
range-90d = 90d

# Home usage card title
home-usage-title = Usage

# OpenRouter key administration

could-not-reach-openrouter = Could not reach OpenRouter


openrouter-keys-title = Keys


openrouter-keys-new = New key


openrouter-keys-new-title = New key


openrouter-keys-back = Back


openrouter-keys-updating = Updating…


openrouter-keys-loading = Loading keys…


openrouter-keys-load-failed = Could not load keys


openrouter-keys-retry = Retry


openrouter-keys-empty = This account has no keys yet.


openrouter-keys-this-app = This app


openrouter-keys-no-limit = No limit


openrouter-keys-of-limit = { $amount } of { $limit }


openrouter-keys-resets-daily = resets daily


openrouter-keys-resets-weekly = resets weekly


openrouter-keys-resets-monthly = resets monthly


openrouter-keys-expires-on = expires { $date }


openrouter-keys-expired-on = expired { $date }


openrouter-keys-today = today { $amount }


openrouter-keys-usage-breakdown = Today { $today } · week { $week } · month { $month } · total { $total }


openrouter-keys-show-all = Show all { $count } keys


openrouter-keys-show-fewer = Show fewer


openrouter-keys-spending-limit = Spending limit


openrouter-keys-limit-hint = Leave empty for no limit.


openrouter-keys-resets = Resets


openrouter-keys-reset-never = Never


openrouter-keys-reset-daily = Daily


openrouter-keys-reset-weekly = Weekly


openrouter-keys-reset-monthly = Monthly


openrouter-keys-byok = Count BYOK usage toward limit


openrouter-keys-enabled = Enabled


openrouter-keys-delete = Delete


openrouter-keys-delete-tracked = This app uses this key. Remove it in Settings first.


openrouter-keys-saving = Saving…


openrouter-keys-delete-title = Delete “{ $name }”?


openrouter-keys-delete-message = Anything still using this key stops working right away. This can't be undone; disabling the key is reversible.


openrouter-keys-keep = Keep key


openrouter-keys-delete-confirm = Delete key


openrouter-keys-deleting = Deleting…


openrouter-keys-name = Name


openrouter-keys-name-placeholder = e.g. laptop-cursor


openrouter-keys-name-required = Enter a name for the key.


openrouter-keys-invalid-amount = Enter a dollar amount like 25 or 12.50.


openrouter-keys-expires = Expires


openrouter-keys-expires-1-hour = 1 hour


openrouter-keys-expires-1-day = 1 day


openrouter-keys-expires-7-days = 7 days


openrouter-keys-expires-30-days = 30 days


openrouter-keys-expires-90-days = 90 days


openrouter-keys-expires-180-days = 180 days


openrouter-keys-expires-1-year = 1 year


openrouter-keys-expires-never = No expiration


openrouter-keys-track = Track this key in Minibar


openrouter-keys-create = Create key


openrouter-keys-creating = Creating…


openrouter-keys-created = Key created


openrouter-keys-copy = Copy key


openrouter-keys-copied = Copied


openrouter-keys-done = Done


openrouter-keys-once-title = You won't see this key again


openrouter-keys-once-message = OpenRouter shows a new key only once. Copy it now.


openrouter-keys-pinned = The popup stays open until you press Done.


openrouter-keys-tracking = Adding the key to Minibar…


openrouter-keys-tracked = Minibar now tracks this key.


openrouter-keys-track-failed = Could not add the key to Minibar: { $error }


openrouter-keys-no-management-key = This account has no management key.


openrouter-keys-management-key-rejected = OpenRouter rejected the management key. Replace it in Settings.


openrouter-keys-not-a-management-key = This key can't manage other keys. Use a management key.


openrouter-keys-key-not-found = This key no longer exists.
