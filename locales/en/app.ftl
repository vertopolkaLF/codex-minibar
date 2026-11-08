### Codex Minibar application messages.
### English source; per-message fallback. Keep $parameters unchanged.

# src/providers/registry.rs
luna-reserve = Luna Reserve

# src/quota/limits.rs
on-pace = On pace

# src/quota/limits.rs
delta-in-deficit = { $delta }% in deficit

# src/quota/limits.rs
delta-in-reserve = { $delta }% in reserve

# src/surfaces/notifications.rs
msg-5-hour-limit-started = 5-hour limit started

# src/surfaces/notifications.rs
msg-5-hour-limit-reset-and-activated = 5-hour limit reset and activated

# src/surfaces/notifications.rs
msg-5-hour-limit-reset = 5-hour limit reset

# src/surfaces/notifications.rs
weekly-limit-reset = Weekly limit reset

# src/surfaces/notifications.rs
name-5-hour = { $name } 5-hour

# src/surfaces/notifications.rs
label-limit-is-low = { $label } limit is low

# src/ui/popup_window/formatting.rs
expired-at-time = expired at { $time }

# src/ui/popup_window/formatting.rs
expired-at-time-6c39ff = expired at { $time } { $v0 }

# src/ui/popup_window/state.rs
never = Never

# src/ui/popup_window/formatting.rs
unlimited = Unlimited

# src/ui/popup_window/ui/activity.rs
unavailable = Unavailable

# src/ui/popup_window/formatting.rs
days-d-hours-h = { $days }d { $hours }h

# src/ui/popup_window/formatting.rs
days-d = { $days }d

# src/ui/popup_window/formatting.rs
hours-h-minutes-m = { $hours }h { $minutes }m

# src/ui/popup_window/formatting.rs
hours-h = { $hours }h

# src/ui/popup_window/formatting.rs
minutes-m = { $minutes }m

# src/ui/popup_window/formatting.rs
waiting-for-first-update = Waiting for first update

# src/ui/popup_window/formatting.rs
just-now = just now

# src/ui/popup_window/formatting.rs
seconds-seconds-ago = { $seconds } seconds ago

# src/ui/popup_window/formatting.rs
minutes-ago = { $v0 } minutes ago

# src/ui/popup_window/formatting.rs
updated-elapsed = Updated { $elapsed }

# src/ui/popup_window/state.rs
the-request-timed-out-try-refreshing-again = The request timed out. Try refreshing again.

# src/ui/popup_window/state.rs
the-secure-connection-could-not-be-verified-see-log-for-details = The secure connection could not be verified. See Log for details.

# src/ui/popup_window/state.rs
the-provider-closed-the-connection-try-refreshing-again = The provider closed the connection. Try refreshing again.

# src/ui/popup_window/state.rs
the-provider-s-address-could-not-be-resolved-check-your-connectio = The provider's address could not be resolved. Check your connection.

# src/ui/popup_window/state.rs
could-not-connect-to-the-provider-check-your-connection-and-try-a = Could not connect to the provider. Check your connection and try again.

# src/ui/popup_window/state.rs
the-management-key-was-rejected-update-it-in-settings = The management key was rejected. Update it in Settings.

# src/ui/popup_window/state.rs
authentication-failed-sign-in-again-or-update-the-provider-key = Authentication failed. Sign in again or update the provider key.

# src/ui/popup_window/state.rs
access-denied-by-the-provider-http-403 = Access denied by the provider (HTTP 403).

# src/ui/popup_window/state.rs
too-many-requests-wait-a-few-minutes-before-refreshing-again = Too many requests. Wait a few minutes before refreshing again.

# src/ui/popup_window/state.rs
the-provider-is-temporarily-unavailable-try-again-later = The provider is temporarily unavailable. Try again later.

# src/ui/popup_window/state.rs
the-provider-rejected-the-request-see-log-for-details = The provider rejected the request. See Log for details.

# src/ui/popup_window/state.rs
the-requested-resource-was-not-found-see-log-for-details = The requested resource was not found. See Log for details.

# src/ui/popup_window/state.rs
the-provider-returned-an-unexpected-response-try-refreshing-again = The provider returned an unexpected response. Try refreshing again.

# src/ui/popup_window/state.rs
the-request-failed-see-log-for-details = The request failed. See Log for details.

# src/ui/popup_window/ui/activity.rs
input = Input

# src/ui/popup_window/ui/activity.rs
cache = Cache

# src/ui/popup_window/ui/usage.rs
output = Output

# src/ui/popup_window/ui/activity.rs
input-uncached = Input (uncached)

# src/ui/popup_window/ui/usage.rs
cached-input = Cached input

# src/ui/popup_window/ui/activity.rs
value-partially-priced = { $value } (partially priced)

# src/ui/popup_window/ui/activity.rs
could-not-load-model-data-error = Could not load model data: { $error }

# src/ui/popup_window/ui/activity.rs
usage-activity = Usage activity

# src/ui/popup_window/ui/activity.rs
waiting-for-cursor-s-usage-export-refresh-to-retry = Waiting for Cursor's usage export. Refresh to retry.

# src/ui/popup_window/ui/activity.rs
model-data-unavailable = Model data unavailable

# src/ui/popup_window/ui/activity.rs
loading-models = Loading models…

# src/ui/popup_window/ui/tooltip.rs
no-model-data = No model data

# src/ui/popup_window/ui/activity.rs
no-usage-data = No usage data

# src/ui/popup_window/ui/activity.rs
no-series-selected = No series selected

# src/ui/popup_window/ui/activity.rs
no-tokens-in-this-period = No { $v0 } tokens in this period

# src/ui/popup_window/ui/activity.rs
loading-model-breakdown = Loading model breakdown

# src/ui/popup_window/ui/activity.rs
group-tokens-or-cost-by-model = Group tokens or cost by model

# src/ui/popup_window/ui/activity.rs
split-type = Type

# src/ui/popup_window/ui/activity.rs
split-by-token-type = Split bars by token type

# src/ui/popup_window/ui/usage.rs
model = Model

# src/ui/popup_window/ui/usage.rs
project = Project

# src/usage/overview.rs
unknown-project = Unknown project

# src/ui/popup_window/ui/usage.rs
no-project-usage = No project data in this period

# src/ui/popup_window/ui/usage.rs
tokens = Tokens

# src/ui/popup_window/ui/usage.rs
cost = Cost

# src/ui/popup_window/ui/activity.rs
no-cost-data-for-this-period = No cost data for this period

# src/ui/popup_window/ui/activity.rs
no-token-data-for-this-period = No token data for this period

# src/ui/popup_window/ui/activity.rs
daily-cost-in-usd = Daily cost in USD

# src/ui/popup_window/ui/activity.rs
daily-token-volume = Daily token volume

# src/ui/popup_window/ui/activity.rs
of-total-models-scroll-for-more = { $v0 }–{ $v1 } of { $total } models · Scroll for more

# src/ui/popup_window/ui/activity.rs
cost-usd-by-model = Cost (USD) by model

# src/ui/popup_window/ui/activity.rs
tokens-by-model = Tokens by model

# src/settings.rs
today = Today

# src/ui/popup_window/ui/activity.rs
last-period-days = Last { $period } days

# src/ui/popup_window/ui/activity.rs
no-data = No data

# src/ui/popup_window/ui/cards.rs
loading-usage-statistics = Loading usage statistics…

# src/ui/popup_window/ui/cards.rs
available-balance = Available balance

# src/ui/popup_window/ui/cards.rs
resets-in = Resets in

# src/ui/popup_window/ui/cards.rs
session-not-started = Not started

# src/ui/popup_window/ui/cards.rs
expires-in = Expires in

# src/ui/popup_window/ui/cards.rs
usage = Usage:

# src/ui/settings_window/providers/page.rs
remove-key = Remove key

# src/ui/popup_window/ui/cards.rs
msg-1-banked-reset = 1 Banked Reset

# src/ui/popup_window/ui/cards.rs
count-banked-resets = { $count ->
    [one] { $count } Reset
   *[other] { $count } Resets
    }

# src/ui/popup_window/ui/cards.rs
available-to-use = Available to use

# src/ui/popup_window/ui/cards.rs
no-expiration-date = No expiration date

# src/ui/popup_window/ui/cards.rs
banked-reset = Banked Reset { $v0 }

# src/ui/popup_window/ui/cards.rs
codex-limits = Codex limits

# src/ui/popup_window/ui/cards.rs
open-announcement-source = Open announcement source

# src/ui/popup_window/ui/cards.rs
source-not-provided = Source not provided

# src/ui/popup_window/ui/cards.rs
tibo-reset = Tibo Reset™

# src/ui/settings_window/customize.rs
home = Home

# src/ui/popup_window/ui/usage.rs
usage-0bb186 = Usage

# src/ui/popup_window/ui/footer.rs
settings = Settings

# src/ui/popup_window/ui/footer.rs
install-update = Install update

# src/ui/popup_window/ui/footer.rs
refreshing-limits-and-usage = Refreshing limits and usage…

# src/ui/popup_window/ui/footer.rs
updated = Updated{ " " }

# src/ui/popup_window/ui/footer.rs
refresh-last-updated-relative = Refresh | Last updated { $relative }

# src/ui/popup_window/ui/footer.rs
popup-pin = Pin on top and keep open
popup-unpin = Unpin and close on focus loss

# src/ui/popup_window/ui/home.rs
edit-home = Edit Home
home-card-layout = Layout
home-card-layout-cards = Cards
home-card-layout-lines = Lines
home-card-layout-rings = Rings
home-card-layout-tiles = Tiles
home-card-layout-classic = Classic
drag-to-reorder = Drag to reorder

# src/ui/popup_window/ui/home.rs
drop-here = Drop here

# src/ui/settings_window/general.rs
usage-stats = Usage Stats

# src/ui/popup_window/ui/usage.rs
loading-usage = Loading usage…

# src/ui/settings_window/window.rs
update-failed = Update failed

# src/ui/popup_window/ui/root.rs
something-went-wrong = Something went wrong

# src/ui/popup_window/ui/root.rs
no-providers-enabled = No providers enabled

# src/ui/popup_window/ui/root.rs
turn-one-on-in-settings-providers = Turn one on in Settings > Providers.

# src/ui/popup_window/ui/root.rs
sign-in-again-to-keep-limits-updating = Sign in again to keep limits updating.

# src/ui/settings_window/providers/page.rs
sign-in-again = Sign in again

# src/ui/popup_window/ui/tooltip.rs
total = Total

# src/ui/popup_window/ui/usage.rs
enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = Enable a provider in Settings and include it in Usage Stats to see local API usage.

# src/ui/popup_window/ui/usage.rs
hourly-cost = Hourly cost

# src/ui/popup_window/ui/usage.rs
hourly-processed-tokens = Hourly processed tokens

# src/ui/popup_window/ui/usage.rs
no-activity-in-this-range = No activity in this range

# src/ui/popup_window/ui/usage.rs
day = Day

# src/ui/popup_window/ui/usage.rs
breakdown = Breakdown

# src/ui/popup_window/ui/usage.rs
totals = Totals

# src/ui/popup_window/ui/usage.rs
processed-tokens = Processed tokens

# src/ui/popup_window/ui/usage.rs
uncached-input = Uncached input

# src/ui/popup_window/ui/usage.rs
cache-savings = Cache savings

# src/ui/popup_window/ui/usage.rs
share = Share

# src/ui/popup_window/ui/usage.rs
hour = Hour

# src/usage/overview.rs
sat = Sat

# src/usage/overview.rs
sun = Sun

# src/providers/registry.rs
msg-5h-session = 5h session

# src/providers/registry.rs
weekly = Weekly

# src/providers/registry.rs
cursor-models = Cursor Models

# src/ui/settings_window/tray.rs
other-models = Other Models

# src/providers/registry.rs
all-models = All Models

# src/ui/settings_window/tray.rs
grok-bot = Grok Bot

# src/providers/registry.rs
monthly = Monthly

# src/providers/registry.rs
spending-limit = Spending limit

# src/providers/registry.rs
gemini = Gemini

# src/providers/registry.rs
claude-gpt = Claude + GPT

# src/providers/registry.rs
credits = Credits

# src/providers/registry.rs
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

# src/providers/registry.rs
banked-resets = Banked resets

# src/providers/registry.rs
usage-stats-7b9e1a = Usage stats

# src/providers/registry.rs
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

# src/ui/settings_window/about.rs
check-github-for-a-new-version = Check GitHub for a new version

# src/ui/settings_window/about.rs
checking-for-updates = Checking for updates…

# src/ui/settings_window/about.rs
you-re-up-to-date = You're up to date

# src/ui/settings_window/about.rs
update-available = Update { $v0 } available

# src/ui/settings_window/about.rs
installing-update = Installing update…

# src/ui/settings_window/about.rs
couldn-t-check-for-updates = Couldn't check for updates

# src/ui/settings_window/about.rs
version = Version { $v0 }

# src/ui/settings_window/about.rs
about-tagline = Free, open-source companion for AI subscription usage limits, resets and spend, right in the Windows tray.

# src/ui/settings_window/about.rs
a-new-release-is-ready-to-install = A new release is ready to install.

# src/ui/settings_window/about.rs
what-s-new = What's new

# src/ui/settings_window/about.rs
update = Update

# src/ui/settings_window/about.rs
check-for-updates = Check for updates

# src/ui/settings_window/about.rs
check-for-updates-on-startup = Check for updates on startup

# src/ui/settings_window/about.rs
notify-when-a-new-version-is-found = Notify when a new version is found

# src/ui/settings_window/about.rs
github = GitHub

# src/ui/settings_window/about.rs
source-code = Source code

# src/ui/settings_window/about.rs
releases = Releases

# src/ui/settings_window/about.rs
see-what-s-new = See what's new

# src/ui/settings_window/about.rs
report-an-issue = Report an issue

# src/ui/settings_window/about.rs
found-a-bug = Found a bug?

# src/ui/settings_window/about.rs
author = Author

# src/ui/settings_window/onboarding.rs
updates = Updates

# src/ui/settings_window/about.rs
resources = Resources

# src/usage/overview.rs
mon = Mon

# src/usage/overview.rs
tue = Tue

# src/usage/overview.rs
wed = Wed

# src/usage/overview.rs
thu = Thu

# src/usage/overview.rs
fri = Fri

# src/ui/settings_window/activation.rs
every-day = Every day

# src/ui/settings_window/activation.rs
weekdays = Weekdays

# src/ui/settings_window/activation.rs
weekends = Weekends

# src/ui/settings_window/activation.rs
no-days = No days

# src/ui/settings_window/onboarding.rs
start-5-hour-sessions-automatically = Start 5-hour sessions automatically

# src/ui/settings_window/activation.rs
starts-a-new-session-as-soon-as-a-window-is-available-instead-of = Starts a new session as soon as a window is available, instead of waiting for your first request. Each account uses its own login.

# src/ui/settings_window/activation.rs
add-codex-or-claude-in-providers-first = Add Codex or Claude in Providers first.

# src/ui/settings_window/activation.rs
off-in-providers = Off in Providers

# src/ui/settings_window/activation.rs
quiet-periods = Quiet periods

# src/ui/settings_window/activation.rs
don-t-auto-start-sessions-during-these-times = Don't auto-start sessions during these times.

# src/ui/settings_window/providers/dialog.rs
add = Add

# src/ui/settings_window/activation.rs
all-day = All day

# src/ui/settings_window/activation.rs
from = From

# src/ui/settings_window/activation.rs
until = Until

# src/ui/settings_window/activation.rs
scheduled-activations = Scheduled activations

# src/ui/settings_window/activation.rs
start-a-5-hour-session-at-a-set-time = Start a 5-hour session at a set time.

# src/ui/settings_window/activation.rs
time = Time

# src/ui/settings_window/activation.rs
turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = Turn on Codex or Claude in Providers first, with a config folder login.

# src/ui/settings_window/activation.rs
no-quiet-periods-yet = No quiet periods yet.

# src/ui/settings_window/activation.rs
no-scheduled-activations-yet = No scheduled activations yet.

# src/ui/settings_window/activation.rs
unknown-provider = Unknown provider

# src/ui/settings_window/activation.rs
remove-quiet-period = Remove quiet period

# src/ui/settings_window/activation.rs
remove-activation = Remove activation

# src/ui/settings_window/tray.rs
provider = Provider

# src/ui/settings_window/activation.rs
days = Days

# src/ui/settings_window/advanced.rs
export-settings = Export settings

# src/ui/settings_window/advanced.rs
save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = Save every setting to a .toml file. Saved keys stay in Windows user storage.

# src/ui/settings_window/advanced.rs
export = Export

# src/ui/settings_window/advanced.rs
import-settings = Import settings

# src/ui/settings_window/advanced.rs
replace-the-current-settings-with-a-previously-exported-file = Replace the current settings with a previously exported file.

# src/ui/settings_window/advanced.rs
import = Import

# src/ui/settings_window/advanced.rs
clear-usage-data = Clear Usage data

# src/ui/settings_window/advanced.rs
delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = Delete the collected Usage history. It is rebuilt from local provider logs on the next scan.

# src/ui/settings_window/advanced.rs
clear = Clear

# src/ui/settings_window/advanced.rs
usage-data-clear-failed = Usage data clear failed

# src/ui/settings_window/advanced.rs
the-background-worker-is-unavailable = The background worker is unavailable.

# src/ui/settings_window/advanced.rs
usage-data-cleared = Usage data cleared.

# src/ui/settings_window/advanced.rs
reset-all-settings = Reset all settings

# src/ui/settings_window/advanced.rs
restore-every-default-and-start-the-welcome-flow-again = Restore every default and start the welcome flow again.

# src/ui/settings_window/advanced.rs
reset = Reset

# src/ui/settings_window/advanced.rs
backup = Backup

# src/ui/settings_window/advanced.rs
data = Data

# src/ui/settings_window/advanced.rs
rendering = Rendering

# src/ui/settings_window/advanced.rs
software-rendering = Software rendering

# src/ui/settings_window/advanced.rs
software-rendering-description = Draw windows on the CPU instead of the graphics card. Can help when a game keeps the GPU busy; animations may be less smooth.

# src/ui/settings_window/advanced.rs
settings-exported = Settings exported.

# src/ui/settings_window/advanced.rs
settings-export-failed = Settings export failed

# src/ui/settings_window/advanced.rs
settings-imported = Settings imported.

# src/ui/settings_window/advanced.rs
settings-import-failed = Settings import failed

# src/ui/settings_window/advanced.rs
settings-reset-failed = Settings reset failed

# src/ui/settings_window/advanced.rs
reset-all-settings-bbfe66 = Reset all settings?

# src/ui/settings_window/advanced.rs
every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = Every setting returns to its default and the welcome flow opens again. Saved keys and Usage data are kept.

# src/ui/settings_window/troubleshoot.rs
cancel = Cancel
use-reset = Use reset
use-reset-short = Use
reset-using = Using…
reset-confirm-message = Use one banked reset for this account? This clears eligible rate-limit windows and cannot be undone.
reset-applied = Reset applied. Your eligible limits have been cleared.
reset-nothing-to-reset = Nothing to reset right now.
reset-no-credit = No usable reset credit left.
reset-already-redeemed = This reset was already used.
reset-unconfirmed = The reset could not be confirmed. Retry to check the same request.
reset-applied-refresh-failed = Reset applied, but the new limits could not be loaded. Refresh to check.
reset-in-progress = A reset is already in progress for this account.
reset-cooldown = Resets are cooling down. Try again later.
reset-rate-limited = Too many reset requests. Try again later.
reset-sign-in-again = Sign in to Claude again to use resets.
reset-cli-login-required = Use the matching Claude CLI login to redeem resets.
reset-invalid-grant = Claude returned an invalid reset credit.

# src/ui/settings_window/appearance.rs
windows = Windows

# src/ui/settings_window/appearance.rs
light = Light

# src/ui/settings_window/appearance.rs
dark = Dark

# src/ui/settings_window/appearance.rs
color-theme = Color theme

# src/ui/settings_window/appearance.rs
applies-to-settings-the-popup-and-its-tray-menu = Applies to Settings, the popup and its tray menu.

# src/ui/settings_window/appearance.rs
accent-color = Accent color

# src/ui/settings_window/appearance.rs
windows-follows-your-system-accent = Windows follows your system accent.

# src/ui/settings_window/appearance.rs

# src/ui/settings_window/appearance.rs
font = Font

# src/ui/settings_window/appearance.rs
any-font-installed-on-this-pc = Any font installed on this PC.
no-matching-fonts = No matching fonts

# src/ui/settings_window/appearance.rs
windows-default = Windows default

# src/ui/settings_window/appearance.rs

# src/ui/settings_window/appearance.rs

# src/ui/settings_window/tray.rs
monochrome = Monochrome

# src/ui/settings_window/appearance.rs
time-format = Time format

# src/ui/settings_window/appearance.rs
msg-12-hour = 12-hour

# src/ui/settings_window/appearance.rs
msg-24-hour = 24-hour

# src/ui/settings_window/appearance.rs
popup-background = Popup background
popup-theme = Popup theme
popup-theme-description = Changes only the popup. Settings keep the Windows look.
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel
popup-theme-built-in = Built-in
popup-theme-vscode = VS Code theme
vscode-themes = VS Code themes
vscode-themes-description = Use Visual Studio Code color themes in the popup: browse Open VSX or import a .vsix or .json theme file.
import-theme = Import theme
importing-theme = Importing…
remove-theme = Remove theme
browse-open-vsx = Browse Open VSX
browse-open-vsx-description = Find and install color themes from the open-vsx.org registry
search-color-themes = Search color themes
open-vsx-search = Search
open-vsx-browse = Browse
open-vsx-searching = Searching…
no-themes-found = No color themes found
open-vsx-load-failed = Couldn't load themes from Open VSX
open-vsx-retry = Retry
open-vsx-preview-loading = Loading preview…
open-vsx-preview-hint = Click a theme to preview it in the popup. Install keeps the whole pack; a theme's download button keeps just that one.
open-vsx-preview-empty = This extension does not contribute any color themes
open-vsx-install = Install
open-vsx-install-one = Install only this theme
open-vsx-installed = Installed
open-vsx-installing = Installing…
theme-installed = Theme installed
theme-install-failed = Couldn't install the theme
theme-remove-failed = Couldn't remove the theme

# src/ui/settings_window/appearance.rs
acrylic = Acrylic

# src/ui/settings_window/appearance.rs
mica = Mica

# src/ui/settings_window/appearance.rs
solid = Solid

# src/ui/settings_window/appearance.rs
bottom-bar-size = Bottom bar size

# src/ui/settings_window/appearance.rs
comfortable = Comfortable

# src/ui/settings_window/appearance.rs
compact = Compact

# src/ui/settings_window/appearance.rs
popup-corner-radius = Popup corner radius
popup-borders = Borders
popup-borders-description = Choose which parts of the popup are outlined
popup-card-borders = Cards and controls
popup-window-border = Window border
popup-theme-contrast = Contrast
popup-theme-contrast-description = Make text and outlines of VS Code themes stand out more or less
popup-theme-tint = Tint
popup-theme-tint-description = How much of the theme's background covers the Mica or Acrylic glass

# src/ui/settings_window/appearance.rs
animation-effects = Animation effects

# src/ui/settings_window/appearance.rs
glide-transitions-in-the-popup-and-settings-windows-own-animation = Glide transitions in the popup and Settings. Windows' own animation setting is also respected.

# src/ui/settings_window/appearance.rs
popup = Popup

# src/ui/settings_window/appearance.rs
motion = Motion

# src/ui/settings_window/appearance.rs
blue = Blue

# src/ui/settings_window/appearance.rs
purple = Purple

# src/ui/settings_window/appearance.rs
pink = Pink

# src/ui/settings_window/appearance.rs
red = Red

# src/ui/settings_window/appearance.rs
orange = Orange

# src/ui/settings_window/appearance.rs
green = Green

# src/ui/settings_window/appearance.rs
teal = Teal

# src/ui/settings_window/customize.rs
show-edit-button = Show Edit button in bottom bar
show-edit-button-description = Edit mode reveals grips on Home to drag widgets into a new order. Start it here to keep the bottom bar compact.
start-editing = Start editing

# src/ui/settings_window/customize.rs
use-two-columns = Use two columns

# src/ui/settings_window/customize.rs
several-accounts-of-one-provider = Several accounts of one provider

# src/ui/settings_window/customize.rs
separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = Separate tabs gives every instance its own tab. Grouped shows one tab per provider, with an account switcher or every account stacked.

# src/ui/settings_window/customize.rs
use-monochrome-icons = Use monochrome icons

# src/ui/settings_window/customize.rs
draw-provider-marks-in-the-popup-without-brand-colors = Draw provider marks in the popup without brand colors.

# src/ui/settings_window/onboarding.rs
show-used-instead-of-remaining = Show used instead of remaining

# src/ui/settings_window/customize.rs
show-usage-in-values-when-possible = Show usage in values (when possible)

# src/ui/settings_window/customize.rs
adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = Adds exact used/limit amounts next to percentages when a provider reports them.

# src/ui/settings_window/onboarding.rs
show-usage-pace = Show usage pace

# src/ui/settings_window/onboarding.rs
marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = Marks whether you're burning quota faster or slower than an even pace.

# src/ui/settings_window/onboarding.rs
show-account-name = Show account name

# src/ui/settings_window/customize.rs
show-on-home-tab = Show on Home tab

# src/ui/settings_window/customize.rs
layout = Layout

# src/ui/settings_window/customize.rs
donut = Donut

# src/ui/settings_window/customize.rs
cards = Cards

# src/ui/settings_window/customize.rs
tabs = Tabs

# src/ui/settings_window/customize.rs
usage-widget = Usage Widget

# src/ui/settings_window/customize.rs
card = Card

# src/ui/settings_window/customize.rs
tab = Tab

# src/ui/settings_window/customize.rs
popup-cards = Popup cards

# src/ui/settings_window/customize.rs
choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = Choose which cards this account shows on Home and on its own tab.

# src/ui/settings_window/customize.rs
choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = Choose which cards this account shows on its own tab. Turn on Show on Home to pick Home cards.

# src/ui/settings_window/general.rs
msg-30-seconds = 30 seconds

# src/ui/settings_window/general.rs
msg-1-minute = 1 minute

# src/ui/settings_window/general.rs
msg-2-minutes = 2 minutes

# src/ui/settings_window/general.rs
msg-5-minutes = 5 minutes

# src/ui/settings_window/general.rs
msg-10-minutes = 10 minutes

# src/ui/settings_window/general.rs
msg-15-minutes = 15 minutes

# src/ui/settings_window/general.rs
msg-30-minutes = 30 minutes

# src/ui/settings_window/general.rs
msg-45-minutes = 45 minutes

# src/ui/settings_window/general.rs
msg-60-minutes = 60 minutes

# src/ui/settings_window/onboarding.rs
start-with-windows = Start with Windows

# src/ui/settings_window/general.rs
open-codex-minibar-in-the-tray-when-you-sign-in = Open Codex Minibar in the tray when you sign in.

# src/ui/settings_window/onboarding.rs
refresh-interval = Refresh interval

# src/ui/settings_window/general.rs
how-often-this-instance-s-quotas-are-read = How often this instance's quotas are read. Longer intervals avoid provider rate limits.

# src/ui/settings_window/general.rs
enable-usage-stats = Enable Usage Stats

# src/ui/settings_window/general.rs
scan-local-provider-history-for-the-usage-tab-and-cost-totals = Scan local provider history for the Usage tab and cost totals.

# src/ui/settings_window/general.rs
collection-period = Collection period

# src/ui/settings_window/general.rs
how-often-local-provider-history-is-scanned = How often local provider history is scanned.

# src/ui/settings_window/general.rs
check-for-confirmed-tibo-resets = Check for confirmed Tibo resets

# src/ui/settings_window/general.rs
reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = Reads the app's public GitHub feed and keeps the latest announcement cached.

# src/ui/settings_window/general.rs
notify-when-new-reset-info-arrives = Notify when new reset info arrives

# src/ui/settings_window/general.rs
shows-a-notification-when-the-feed-reports-a-possible-reset-never = Shows a notification when the feed reports a possible reset, never at the reset time.

# src/ui/settings_window/general.rs
check-every = Check every

# src/ui/settings_window/general.rs
the-feed-is-also-checked-immediately-when-the-app-starts-or-this = The feed is also checked immediately when the app starts or this option is enabled.

# src/ui/settings_window/general.rs
msg-1-hour = 1 hour

# src/ui/settings_window/general.rs
msg-3-hours = 3 hours

# src/ui/settings_window/general.rs
msg-2-hours = 2 hours

# src/ui/settings_window/general.rs
msg-5-hours = 5 hours

# src/ui/settings_window/general.rs
msg-6-hours = 6 hours

# src/ui/settings_window/general.rs
msg-12-hours = 12 hours

# src/ui/settings_window/general.rs
msg-24-hours = 24 hours

# src/ui/settings_window/general.rs
tibo-resets = Tibo Resets™

# src/ui/settings_window/general.rs
enable-a-provider-in-the-providers-tab-to-include-it-here = Enable a provider in the Providers tab to include it here.

# src/ui/settings_window/general.rs
add-a-management-key = Add a management key

# src/ui/settings_window/general.rs
usage-statistics-are-off-on-this-provider-s-page = Usage statistics are off on this provider's page

# src/ui/settings_window/general.rs
included-providers = Included providers

# src/ui/settings_window/general.rs
choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = Choose which accounts count toward this machine's Usage tab and Home total. Excluded accounts keep collecting and still show usage on their own page.

# src/ui/settings_window/integrations.rs
download-the-latest-stream-deck-companion-from-github-and-open-it = Download the latest Stream Deck companion from GitHub and open its installer.

# src/ui/settings_window/integrations.rs
downloading-the-latest-stream-deck-companion-from-github = Downloading the latest Stream Deck companion from GitHub...

# src/ui/settings_window/integrations.rs
opening-the-stream-deck-installer = Opening the Stream Deck installer...

# src/ui/settings_window/integrations.rs
the-installer-is-open-finish-the-installation-in-stream-deck = The installer is open. Finish the installation in Stream Deck.

# src/ui/settings_window/integrations.rs
the-plugin-could-not-be-downloaded-or-opened-try-again = The plugin could not be downloaded or opened. Try again.

# src/ui/settings_window/integrations.rs
downloading = Downloading...

# src/ui/settings_window/integrations.rs
opening = Opening...

# src/ui/settings_window/integrations.rs
install-plugin = Install plugin

# src/ui/settings_window/integrations.rs
stream-deck-companion = Stream Deck companion

# src/ui/settings_window/integrations.rs
check-your-connection-then-try-again = Check your connection, then try again.

# src/ui/settings_window/providers/page.rs
on = On

# src/ui/settings_window/providers/page.rs
off = Off

# src/ui/gui/kit.rs
more-options = More options

# src/ui/gui/kit.rs
custom-color = Custom color

# src/ui/settings_window/troubleshoot.rs
run-troubleshoot-with-ai = Run Troubleshoot with AI

# src/ui/settings_window/log.rs
let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = Let an installed AI CLI read the log and investigate a problem.

# src/ui/settings_window/log.rs
choose-tool = Choose tool

# src/ui/settings_window/log.rs
application-log = Application log

# src/ui/settings_window/log.rs
log-txt-in-the-app-data-folder = log.txt in the app data folder.

# src/ui/settings_window/log.rs
open-log-txt = Open log.txt

# src/ui/settings_window/log.rs
could-not-open-log-txt = Could not open log.txt

# src/ui/settings_window/log.rs
open-logs-folder = Open logs folder

# src/ui/settings_window/log.rs
could-not-open-logs-folder = Could not open logs folder

# src/ui/settings_window/log.rs
no-log-events-yet = No log events yet.

# src/ui/settings_window/log.rs
live-tail = Live tail

# src/ui/settings_window/log.rs
no-supported-ai-tool-found = No supported AI tool found

# src/ui/settings_window/log.rs
install-codex-or-claude-code-and-make-it-available-to-minibar = Install Codex or Claude Code and make it available to Minibar.

# src/ui/settings_window/window.rs
general = General

# src/ui/settings_window/window.rs
appearance = Appearance

# src/ui/settings_window/window.rs
providers = Providers

# src/ui/settings_window/window.rs
customize = Customize

# src/ui/settings_window/window.rs
limit-activation = Limit activation

# src/ui/settings_window/window.rs
tray = Tray

# src/ui/settings_window/window.rs
notifications = Notifications

# src/ui/settings_window/window.rs
advanced = Advanced

# src/ui/settings_window/window.rs
log = Log

# src/ui/settings_window/window.rs
integrations = Integrations

# src/ui/settings_window/nav.rs
about-updates = About & Updates

# src/ui/settings_window/onboarding.rs
successful-activations = Successful activations

# src/ui/settings_window/onboarding.rs
failed-activations = Failed activations

# src/ui/settings_window/onboarding.rs
when-limits-reset = When limits reset

# src/ui/settings_window/onboarding.rs
when-5-hour-remaining-hits = When 5-hour remaining hits { $v0 }%

# src/ui/settings_window/onboarding.rs
when-weekly-remaining-hits = When weekly remaining hits { $v0 }%

# src/ui/settings_window/onboarding.rs
low-usage = Low usage

# src/ui/settings_window/notifications.rs
shows-a-notification-once-per-window-when-the-remaining-share-dro = Shows a notification once per window when the remaining share drops to the threshold.

# src/ui/settings_window/notifications.rs
threshold = Threshold

# src/ui/settings_window/onboarding.rs
found-in-opencode-auth-or-local-history = Found in OpenCode auth or local history.

# src/ui/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-set-up-elsewhere = Not found. Turn it on if it's set up elsewhere.

# src/ui/settings_window/onboarding.rs
account-credentials-are-already-set = Account credentials are already set.

# src/ui/settings_window/onboarding.rs
optional-add-accounts-later-in-providers = Optional. Add accounts later in Providers.

# src/ui/settings_window/onboarding.rs
found-an-official-agy-sign-in-on-this-pc = Found an official agy sign-in on this PC.

# src/ui/settings_window/onboarding.rs
not-found-sign-in-with-agy-before-enabling-it = Not found. Sign in with agy before enabling it.

# src/ui/settings_window/onboarding.rs
found-an-official-grok-cli-sign-in-on-this-pc = Found an official Grok CLI sign-in on this PC.

# src/ui/settings_window/onboarding.rs
not-found-run-grok-login-before-enabling-it = Not found. Run grok login before enabling it.

# src/ui/settings_window/onboarding.rs
found-an-official-command-code-cli-sign-in-on-this-pc = Found an official Command Code CLI sign-in on this PC.

# src/ui/settings_window/onboarding.rs
not-found-run-cmdc-login-before-enabling-it = Not found. Run cmdc login before enabling it.

# src/ui/settings_window/onboarding.rs
found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Found Kiro IDE, Kiro Crew, or a signed-in Kiro CLI.

# src/ui/settings_window/onboarding.rs
not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = Not found. Install Kiro IDE or Kiro Crew, or sign in to Kiro CLI.

# src/ui/settings_window/onboarding.rs
found-on-this-pc = Found on this PC.

# src/ui/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-installed-somewhere-else = Not found. Turn it on if it's installed somewhere else.

# src/ui/settings_window/onboarding.rs
setup-could-not-be-saved = Setup could not be saved

# src/ui/settings_window/onboarding.rs
detected = Detected

# src/ui/settings_window/onboarding.rs
starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = Starts a new Codex or Claude session as soon as a window is available, instead of waiting for your first request.

# src/ui/settings_window/onboarding.rs
collect-usage-data = Collect usage data

# src/ui/settings_window/onboarding.rs
scans-local-provider-history-for-usage-stats = Scans local provider history for Usage Stats.

# src/ui/settings_window/onboarding.rs
startup = Startup

# src/ui/settings_window/providers/page.rs
features = Features

# src/ui/settings_window/onboarding.rs
customization = Customization

# src/ui/settings_window/onboarding.rs
when-a-new-version-is-found = When a new version is found

# src/ui/settings_window/onboarding.rs
activity = Activity

# src/ui/settings_window/onboarding.rs
choose-providers = Choose providers
choose-a-theme = Choose a theme
theme-step-description = Pick the colors for Settings and the popup. The popup shows each choice as you make it.

# src/ui/settings_window/onboarding.rs
we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = We turned on the providers found on this PC. You can change this later.

# src/ui/settings_window/onboarding.rs
general-settings = General settings

# src/ui/settings_window/onboarding.rs
you-can-change-these-later-in-settings = You can change these later in Settings.

# src/ui/settings_window/onboarding.rs
turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = Turn off anything you don't want to hear about. You can change these later in Settings.

# src/ui/settings_window/window.rs
back = Back

# src/ui/settings_window/onboarding.rs
continue = Continue

# src/ui/settings_window/tray.rs
done = Done

# src/ui/settings_window/providers/dialog.rs
details = Details

# src/ui/settings_window/providers/dialog.rs
added = Added

# src/ui/settings_window/providers/dialog.rs
reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Reads the session and weekly limits of a Claude subscription without a Claude Code login.

# src/ui/settings_window/providers/dialog.rs
msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. Open Claude in a separate browser profile or private window. Sign in to the account you want to track and confirm its email in Claude's settings.

# src/ui/settings_window/providers/dialog.rs
open-claude-ai = Open claude.ai

# src/ui/settings_window/providers/dialog.rs
msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. In Chrome or Edge, press F12. Open Application > Storage > Cookies and select https://claude.ai.

# src/ui/settings_window/providers/dialog.rs
msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. Find sessionKey. Copy its Value, not its name or the whole cookie table, and paste it into the field above.

# src/ui/settings_window/providers/dialog.rs
how-to-view-cookies-in-chrome = How to view cookies in Chrome

# src/ui/settings_window/providers/dialog.rs
a-cookie-header-containing-sessionkey-also-works-when-the-session = A Cookie header containing sessionKey also works. When the session expires, paste a fresh one here.

# src/ui/settings_window/providers/dialog.rs
use-the-access-token-from-a-claude-code-subscription-login-miniba = Use the access token from a Claude Code subscription login. Minibar cannot refresh a pasted token; prefer Source: Config folder when you can.

# src/ui/settings_window/providers/dialog.rs
copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = Copy only claudeAiOauth.accessToken (starts with sk-ant-oat) from that login's .credentials.json, without quotes. Do not use claude setup-token: it may lack usage access.

# src/ui/settings_window/providers/dialog.rs
claude-code-log-in-with-multiple-accounts = Claude Code: log in with multiple accounts

# src/ui/settings_window/providers/dialog.rs
requires-a-claude-subscription-login-with-usage-access-api-keys-a = Requires a Claude subscription login with usage access. API keys and Admin API keys do not show subscription limits.

# src/ui/settings_window/providers/dialog.rs
this-provider = this provider

# src/ui/settings_window/providers/dialog.rs
track-another-account-or-a-provider-minibar-has-not-shown-yet = Track another account or a provider Minibar has not shown yet.

# src/ui/settings_window/providers/dialog.rs
e-g-work = e.g. Work

# src/ui/settings_window/providers/page.rs
name = Name

# src/ui/settings_window/providers/dialog.rs
shown-on-its-tab-home-card-tray-and-notifications = Shown on its tab, Home card, tray and notifications.

# src/ui/settings_window/providers/page.rs
badge = Badge

# src/ui/settings_window/providers/dialog.rs
auto = Auto

# src/ui/settings_window/providers/page.rs
badge-color = Badge color

# src/ui/settings_window/providers/dialog.rs
up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = Up to three letters; empty uses the name's initials. Badges show while a provider has more than one instance turned on.

# src/ui/settings_window/window.rs
add-provider = Add provider

# src/ui/settings_window/providers/dialog.rs
next = Next

# src/ui/settings_window/providers/dialog.rs
minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar stops reading { $name } and forgets its saved keys, schedules, tray indicators and Home position.{ $v0 }

# src/ui/settings_window/providers/dialog.rs
its-config-folder-stays-on-disk = { " " }Its config folder stays on disk.

# src/ui/settings_window/providers/dialog.rs
delete-name = Delete { $name }?

# src/ui/settings_window/providers/dialog.rs
delete = Delete

# src/ui/settings_window/providers/dialog.rs
finish-signing-in-in-your-browser-cancel-stops-this-login = Finish signing in in your browser. Cancel stops this login.

# src/ui/settings_window/providers/dialog.rs
runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = Runs Claude Code's own login for { $name } with its config folder as CLAUDE_CONFIG_DIR. The login stays in that folder, where Claude Code keeps it fresh. Requires native Windows Claude Code.

# src/ui/settings_window/providers/dialog.rs
runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = Runs Codex's own login for { $name } with its config folder as CODEX_HOME. The login stays in that folder, where Codex keeps it fresh. Requires the native Codex CLI or desktop app.

# src/ui/settings_window/providers/dialog.rs
sign-in-to-name = Sign in to { $name }

# src/ui/settings_window/providers/page.rs
sign-in = Sign in

# src/ui/settings_window/providers/dialog.rs
for = For { $v0 }.

# src/ui/settings_window/providers/dialog.rs
browser-session = Browser session

# src/ui/settings_window/providers/dialog.rs
oauth-token = OAuth token

# src/ui/settings_window/providers/dialog.rs
session-key = Session key

# src/ui/settings_window/providers/dialog.rs
paste-the-sessionkey-value = Paste the sessionKey value

# src/ui/settings_window/providers/dialog.rs
oauth-access-token = OAuth access token

# src/ui/settings_window/providers/dialog.rs
the-saved-credential-is-replaced-only-after-the-new-one-passes-th = The saved credential is replaced only after the new one passes the check.

# src/ui/settings_window/providers/dialog.rs
claude-credential = Claude credential

# src/ui/settings_window/providers/dialog.rs
check-and-save = Check and save

# src/ui/settings_window/providers/dialog.rs
key-name-optional = Key name (optional)

# src/ui/settings_window/providers/dialog.rs
e-g-personal = e.g. Personal

# src/ui/settings_window/providers/dialog.rs
leave-blank-to-use-the-name-from-openrouter = Leave blank to use the name from OpenRouter.

# src/ui/settings_window/providers/dialog.rs
minibar-checks-the-key-with-openrouter-before-saving-it = Minibar checks the key with OpenRouter before saving it.

# src/ui/settings_window/providers/dialog.rs
replace-api-key = Replace API key

# src/ui/settings_window/providers/page.rs
add-api-key = Add API key

# src/ui/settings_window/providers/page.rs
management-key = Management key

# src/ui/settings_window/providers/dialog.rs
create-one-under-settings-management-keys-on-openrouter-ai = Create one under Settings → Management keys on openrouter.ai.

# src/ui/settings_window/providers/dialog.rs
replace-management-key = Replace management key

# src/ui/settings_window/providers/page.rs
add-management-key = Add management key

# src/ui/settings_window/providers/dialog.rs
key-name = Key name

# src/ui/settings_window/providers/page.rs
rename-key = Rename key

# src/ui/settings_window/providers/dialog.rs
save = Save

# src/ui/settings_window/providers/dialog.rs
this-key = this key

# src/ui/settings_window/providers/dialog.rs
minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibar stops tracking { $hint }. The key keeps working on OpenRouter.

# src/ui/settings_window/providers/dialog.rs
remove-api-key = Remove API key?

# src/ui/settings_window/tray.rs
remove = Remove

# src/ui/settings_window/providers/dialog.rs
minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibar stops showing credit balance and usage history for { $v0 }. The key keeps working on OpenRouter.

# src/ui/settings_window/providers/dialog.rs
remove-management-key = Remove management key?

# src/ui/settings_window/providers/dialog.rs
saved-in-windows-user-storage-never-in-the-settings-file = Saved in Windows user storage, never in the settings file.

# src/ui/settings_window/providers/dialog.rs
save-key = Save key

# src/ui/settings_window/providers/dialog.rs
minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibar forgets the saved key of { $v0 }. It keeps working with OpenCode.

# src/ui/settings_window/providers/dialog.rs
signing-in = Signing in…

# src/ui/settings_window/providers/page.rs
checking = Checking…

# src/ui/settings_window/providers/dialog.rs
choose-a-provider = Choose a provider.

# src/ui/settings_window/providers/dialog.rs
reason-it-is-already-in-the-list = { $reason }. It is already in the list.

# src/ui/settings_window/providers/dialog.rs
added-name = Added { $name }.

# src/ui/settings_window/providers/dialog.rs
could-not-add-the-provider-error = Could not add the provider: { $error }

# src/ui/settings_window/providers.rs
this-provider-no-longer-exists = This provider no longer exists.

# src/ui/settings_window/providers/dialog.rs
could-not-delete-the-provider-error = Could not delete the provider: { $error }

# src/ui/settings_window/providers/dialog.rs
paste-a-credential-first = Paste a credential first.

# src/ui/settings_window/providers/dialog.rs
credential-saved-in-windows-user-storage = Credential saved in Windows user storage.

# src/ui/settings_window/providers/dialog.rs
paste-a-key-first = Paste a key first.

# src/ui/settings_window/providers/dialog.rs
this-account-no-longer-exists = This account no longer exists.

# src/ui/settings_window/providers/dialog.rs
api-key-saved-in-windows-user-storage = API key saved in Windows user storage.

# src/ui/settings_window/providers/dialog.rs
management-key-replaced = Management key replaced.

# src/ui/settings_window/providers/dialog.rs
management-key-added = Management key added.

# src/ui/settings_window/providers/dialog.rs
could-not-rename-the-key-error = Could not rename the key: { $error }

# src/ui/settings_window/providers/dialog.rs
api-key-renamed = API key renamed.

# src/ui/settings_window/providers/dialog.rs
could-not-remove-the-key-error = Could not remove the key: { $error }

# src/ui/settings_window/providers/dialog.rs
api-key-removed = API key removed.

# src/ui/settings_window/providers/dialog.rs
management-key-removed = Management key removed.

# src/ui/settings_window/providers/dialog.rs
could-not-save-the-key-error = Could not save the key: { $error }

# src/ui/settings_window/providers/dialog.rs
api-key-saved = API key saved.

# src/ui/settings_window/providers/dialog.rs
switch-source-to-config-folder-to-sign-in = Switch Source to Config folder to sign in.

# src/ui/settings_window/providers/dialog.rs
this-provider-has-no-config-folder = This provider has no config folder.

# src/ui/settings_window/providers/dialog.rs
another-instance-already-reads-this-config-folder-choose-a-differ = Another instance already reads this config folder. Choose a different folder first.

# src/ui/settings_window/providers/dialog.rs
this-provider-has-no-sign-in = This provider has no sign-in.

# src/ui/settings_window/providers/dialog.rs
signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 } signed in. Its CLI keeps the login fresh in its config folder.

# src/ui/settings_window/providers/page.rs
no-keys-yet = No keys yet

# src/ui/settings_window/providers/page.rs
management-key-5c8cf2 = { " " }· management key

# src/ui/settings_window/providers/page.rs
using-a-saved-credential = Using a saved credential

# src/ui/settings_window/providers/page.rs
paste-a-credential-under-account = Paste a credential under Account

# src/ui/settings_window/providers/page.rs
needs-an-api-key-or-opencode-sign-in = Needs an API key or OpenCode sign-in

# src/ui/settings_window/providers/page.rs
needs-an-api-key = Needs an API key

# src/ui/settings_window/providers/page.rs
not-found-set-its-folder-under-runtime = Not found. Set its folder under Runtime.

# src/ui/settings_window/providers/page.rs
using-a-saved-api-key = Using a saved API key

# src/ui/settings_window/providers/page.rs
using-opencode-sign-in-or-local-history = Using OpenCode sign-in or local history

# src/ui/settings_window/providers/page.rs
reading-cli = Reading { $cli }

# src/ui/settings_window/providers/page.rs
reading-crew = Reading { $crew }

# src/ui/settings_window/providers/page.rs
reading-app = Reading { $app }

# src/ui/settings_window/providers/page.rs
checking-kiro-ide-kiro-crew-and-cli = Checking Kiro IDE, Kiro Crew, and CLI…

# src/ui/settings_window/providers/page.rs
checking-installed-app-and-cli = Checking installed app and CLI…

# src/ui/settings_window/providers/page.rs
checking-cli = Checking CLI…

# src/ui/settings_window/providers/page.rs
checking-installed-app = Checking installed app…

# src/ui/settings_window/providers/page.rs
no-providers-yet-add-one-to-start-reading-limits = No providers yet. Add one to start reading limits.

# src/ui/settings_window/providers/page.rs
is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } is off, so it doesn't appear in the minibar or tray. Turn it on to start reading usage.

# src/ui/settings_window/providers/page.rs
replace-chatgpt-logo-with-codex = Replace ChatGPT logo with Codex

# src/ui/settings_window/providers/page.rs
limits-only = Limits only

# src/ui/settings_window/providers/page.rs
delete-provider = Delete provider

# src/ui/settings_window/providers/page.rs
display-name = Display name

# src/ui/settings_window/providers/page.rs
shown-on-popup-tabs-home-cards-the-tray-and-notifications = Shown on popup tabs, Home cards, the tray and notifications.

# src/ui/settings_window/providers/page.rs
up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = Up to three letters. Leave empty to use the name's initials. Shown while a provider has more than one instance turned on.

# src/ui/settings_window/providers/page.rs
auto-uses-a-neutral-plate-that-follows-the-theme = Auto uses a neutral plate that follows the theme.

# src/ui/settings_window/providers/page.rs
show-on-home = Show on Home

# src/ui/settings_window/providers/page.rs
its-provider-tab-stays-available-when-hidden-from-home = Its provider tab stays available when hidden from Home.

# src/ui/settings_window/providers/page.rs
credential = Credential

# src/ui/settings_window/providers/page.rs
signed-in-as-identity = Signed in as { $identity }

# src/ui/settings_window/providers/page.rs
saved-in-windows-user-storage = Saved in Windows user storage

# src/ui/settings_window/providers/page.rs
paste-a-sessionkey-or-an-oauth-access-token = Paste a sessionKey or an OAuth access token.

# src/ui/settings_window/providers/page.rs
reads-limits-only-minibar-cannot-refresh-a-pasted-credential = Reads limits only. Minibar cannot refresh a pasted credential.

# src/ui/settings_window/providers/page.rs
replace-credential = Replace credential

# src/ui/settings_window/providers/page.rs
add-credential = Add credential

# src/ui/settings_window/providers/page.rs
signed-in-account = Signed-in account

# src/ui/settings_window/providers/page.rs
not-signed-in-yet-or-no-limits-read-so-far = Not signed in yet, or no limits read so far.

# src/ui/settings_window/providers/page.rs
account = Account

# src/ui/settings_window/providers/page.rs
runtime = Runtime

# src/ui/settings_window/providers/page.rs
minibar-finds-these-automatically = { $v0 } Minibar finds these automatically.

# src/ui/settings_window/providers/page.rs
in-use = In use

# src/ui/settings_window/providers/page.rs
found = Found

# src/ui/settings_window/providers/page.rs
copy-path = Copy path

# src/ui/settings_window/providers/page.rs
open-folder = Open folder

# src/ui/settings_window/providers/page.rs
path-copied = Path copied.

# src/ui/settings_window/providers/page.rs
not-installed-or-installed-somewhere-minibar-doesn-t-look = Not installed, or installed somewhere Minibar doesn't look.

# src/ui/settings_window/providers/page.rs
not-found = Not found

# src/ui/settings_window/providers/page.rs
choose-folder = Choose folder…

# src/ui/settings_window/providers/page.rs
select-folder = Select folder

# src/ui/settings_window/providers/page.rs
choose-folder-1838a4 = Choose folder

# src/ui/settings_window/providers/page.rs
only-needed-if-automatic-detection-misses-your-install = Only needed if automatic detection misses your install.

# src/ui/settings_window/providers/page.rs
source = Source

# src/ui/settings_window/providers/page.rs
config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = Config folder reads the Claude Code login in CLAUDE_CONFIG_DIR. Manual credential reads limits only from a pasted credential.

# src/ui/settings_window/providers/page.rs
config-folder = Config folder

# src/ui/settings_window/providers/page.rs
manual = Manual

# src/ui/settings_window/providers/page.rs
passed-to-the-cli-as-env-leave-empty-to-use = Passed to the CLI as { $env }. Leave empty to use { $v0 }.

# src/ui/settings_window/providers/page.rs
the-standard-folder = the standard folder

# src/ui/settings_window/providers/page.rs
a-folder-minibar-creates-for-this-instance = a folder Minibar creates for this instance

# src/ui/settings_window/providers/page.rs
other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } already reads this folder. Two instances must not share a login, or usage is counted twice.

# src/ui/settings_window/providers/page.rs
automatic-activation = Automatic activation

# src/ui/settings_window/providers/page.rs
starts-this-account-s-5-hour-window-when-it-resets-using-its-own = Starts this account's 5-hour window when it resets, using its own login. Schedules and pauses are under Limit activation.

# src/ui/settings_window/providers/page.rs
usage-statistics = Usage statistics

# src/ui/settings_window/providers/page.rs
scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = Scans this instance's local history for its usage card. Turn off to stop collecting entirely.

# src/ui/settings_window/providers/page.rs
opencode-sign-in-or-local-history = OpenCode sign-in or local history

# src/ui/settings_window/providers/page.rs
found-in-opencode-auth-environment-a-saved-key-or-local-history = Found in OpenCode auth, environment, a saved key, or local history.

# src/ui/settings_window/providers/page.rs
nothing-found-in-opencode-auth-environment-or-local-history = Nothing found in OpenCode auth, environment, or local history.

# src/ui/settings_window/providers/page.rs
replace-key = Replace key

# src/ui/settings_window/providers/page.rs
optional-only-needed-without-opencode-sign-in-on-this-pc = Optional. Only needed without OpenCode sign-in on this PC.

# src/ui/settings_window/providers/page.rs
add-the-key-of-the-account-this-instance-tracks = Add the key of the account this instance tracks.

# src/ui/settings_window/providers/page.rs
keys = Keys

# src/ui/settings_window/providers/page.rs
a-management-key-shows-credit-balance-and-usage-history-api-keys = A management key shows credit balance and usage history. API keys show spend per key.

# src/ui/settings_window/providers/page.rs
could-not-read-the-saved-key-reopen-this-page-to-retry = Could not read the saved key. Reopen this page to retry.

# src/ui/settings_window/providers/page.rs
credit-balance-and-account-wide-usage-history = Credit balance and account-wide usage history

# src/ui/settings_window/providers/page.rs
not-added-add-one-to-see-credit-balance-and-usage-history = Not added. Add one to see credit balance and usage history.

# src/ui/settings_window/providers/page.rs
last-updated = Last updated { $v0 }, { $v1 }

# src/ui/settings_window/providers/page.rs
no-api-keys-yet-add-one-to-track-spend-per-key = No API keys yet. Add one to track spend per key.

# src/ui/settings_window/providers/page.rs
key = Key

# src/ui/settings_window/providers/page.rs
spend = Spend

# src/ui/settings_window/providers/page.rs
limit = Limit

# src/ui/settings_window/providers/page.rs
error-reopen-this-page-to-retry = { $error }. Reopen this page to retry.

# src/ui/settings_window/providers/page.rs
could-not-read-key = Could not read key

# src/ui/settings_window/providers/page.rs
not-saved = Not saved

# src/ui/settings_window/providers/page.rs
unnamed = Unnamed

# src/ui/settings_window/providers/page.rs
of-the-limit = { $v0 } of the { $v1 } limit

# src/ui/settings_window/providers/page.rs
no-spend-limit-key-spend-account-credits-purchased = No spend limit. Key spend: { $v0 }. Account credits purchased: { $v1 }.

# src/ui/settings_window/providers/page.rs
none = None

# src/ui/settings_window/providers/page.rs
add-key = Add key

# src/ui/settings_window/providers.rs
openrouter-account-credentials-are-configured = OpenRouter account credentials are configured

# src/ui/settings_window/providers.rs
saved-api-key = Saved API key

# src/ui/settings_window/providers.rs
saved-credential = Saved credential

# src/ui/settings_window/providers.rs
opencode-auth-json-or-local-database = OpenCode auth.json or local database

# src/ui/settings_window/providers.rs
that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = That doesn't look like an OpenRouter key. Keys start with sk-or-.

# src/ui/settings_window/providers.rs
codex-cli-folder = Codex CLI folder

# src/ui/settings_window/providers.rs
claude-code-cli-folder = Claude Code CLI folder

# src/ui/settings_window/providers.rs
cursor-app-folder = Cursor app folder

# src/ui/settings_window/providers.rs
grok-cli-folder = Grok CLI folder

# src/ui/settings_window/providers.rs
command-code-cli-folder = Command Code CLI folder

# src/ui/settings_window/providers.rs
kiro-ide-folder = Kiro IDE folder

# src/ui/settings_window/providers.rs
kiro-crew-app-path = Kiro Crew app path

# src/ui/settings_window/providers.rs
kiro-cli-folder = Kiro CLI folder

# src/ui/settings_window/providers.rs
reads-the-signed-in-codex-cli-or-desktop-app = Reads the signed-in Codex CLI or desktop app.

# src/ui/settings_window/providers.rs
reads-your-existing-claude-code-login = Reads your existing Claude Code login.

# src/ui/settings_window/providers.rs
reads-the-signed-in-cursor-app-for-this-billing-cycle = Reads the signed-in Cursor app for this billing cycle.

# src/ui/settings_window/providers.rs
reads-zen-auth-and-local-opencode-history = Reads Zen auth and local OpenCode history.

# src/ui/settings_window/providers.rs
reads-go-quota-windows-and-local-opencode-history = Reads Go quota windows and local OpenCode history.

# src/ui/settings_window/providers.rs
reads-api-key-usage-and-spend-limits-a-management-key-also-enable = Reads API-key usage and spend limits. A management key also enables usage history and credit balance.

# src/ui/settings_window/providers.rs
reads-subscription-quota-from-your-existing-official-agy-windows = Reads subscription quota from your existing official agy Windows sign-in.

# src/ui/settings_window/providers.rs
reads-supergrok-subscription-credits-from-your-existing-official = Reads SuperGrok subscription credits from your existing official Grok CLI sign-in.

# src/ui/settings_window/providers.rs
reads-command-code-5h-weekly-and-monthly-credits-from-your-existin = Reads Command Code 5-hour, weekly, and monthly credits from your existing official CLI sign-in.

# src/ui/settings_window/providers.rs
fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = Fetches Kiro's live monthly credits with its shared sign-in; recognizes IDE, Crew, and CLI installs.

# src/ui/settings_window/providers.rs
codex-desktop-app = Codex desktop app

# src/ui/settings_window/providers.rs
codex-cli = Codex CLI

# src/ui/settings_window/providers.rs
claude-desktop-app = Claude desktop app

# src/ui/settings_window/providers.rs
claude-code-cli = Claude Code CLI

# src/ui/settings_window/providers.rs
cursor-app = Cursor app

# src/ui/settings_window/providers.rs
antigravity-app = Antigravity app

# src/ui/settings_window/providers.rs
grok-cli = Grok CLI

# src/ui/settings_window/providers.rs
command-code-cli = Command Code CLI

# src/ui/settings_window/providers.rs
kiro-ide = Kiro IDE

# src/ui/settings_window/providers.rs
kiro-crew = Kiro Crew

# src/ui/settings_window/providers.rs
kiro-cli = Kiro CLI

# src/ui/settings_window/providers.rs
supports-one-instance = { $v0 } supports one instance

# src/ui/settings_window/tray.rs
numbers = Numbers

# src/ui/settings_window/tray.rs
progress-bars = Progress bars

# src/ui/settings_window/tray.rs
rings = Rings

# src/ui/settings_window/tray.rs
reset-time = Reset time

# src/ui/settings_window/tray.rs
countdown = Countdown

# src/ui/settings_window/tray.rs
status = Status

# src/ui/settings_window/tray.rs
fixed = Fixed

# src/ui/settings_window/tray.rs
app-accent = App accent

# src/ui/settings_window/tray.rs
percentages-as-digits = Percentages as digits

# src/ui/settings_window/tray.rs
one-bar-per-indicator = One bar per indicator

# src/ui/settings_window/tray.rs
nested-rings-one-per-indicator = Nested rings, one per indicator

# src/ui/settings_window/tray.rs
when-the-limit-resets = When the limit resets

# src/ui/settings_window/tray.rs
time-left-until-the-reset = Time left until the reset

# src/ui/settings_window/tray.rs
used = Used

# src/ui/settings_window/tray.rs
remaining = Remaining

# src/ui/settings_window/tray.rs
app-icon = App icon

# src/ui/settings_window/tray.rs
widget = Widget { $v0 }

# src/ui/settings_window/tray.rs
the-codex-minibar-icon = The Codex Minibar icon

# src/ui/settings_window/tray.rs
style-no-indicators = { $style } · No indicators

# src/ui/settings_window/tray.rs
widget-removed = Widget removed

# src/ui/settings_window/tray.rs
undo = Undo

# src/ui/settings_window/tray.rs
tray-icon = Tray icon

# src/ui/settings_window/tray.rs
shows-the-app-icon = Shows the app icon.

# src/ui/settings_window/tray.rs
add-widget = Add widget

# src/ui/settings_window/tray.rs
add-app-icon = Add app icon

# src/ui/settings_window/tray.rs
move-up = Move up

# src/ui/settings_window/tray.rs
move-down = Move down

# src/ui/settings_window/tray.rs
edit = Edit

# src/ui/settings_window/tray.rs
duplicate = Duplicate

# src/ui/settings_window/tray.rs
shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = Shows the Codex Minibar icon in the notification area. It has no indicators to set up.

# src/ui/settings_window/tray.rs
style = Style

# src/ui/settings_window/tray.rs
up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = Up to { $max_indicators } quotas, drawn in this order. Expand one to change it.

# src/ui/settings_window/tray.rs
indicators = Indicators

# src/ui/settings_window/tray.rs
remove-widget = Remove widget

# src/ui/settings_window/tray.rs
add-indicator = Add indicator

# src/ui/settings_window/tray.rs
unsupported = Unsupported { $v0 }

# src/ui/settings_window/tray.rs
remove-indicator = Remove indicator

# src/ui/settings_window/tray.rs
unsupported-bba2a8 = Unsupported ({ $v0 })

# src/ui/settings_window/tray.rs
unavailable-0c5d75 = Unavailable ({ $v0 })

# src/ui/settings_window/tray.rs
metric = Metric

# src/ui/settings_window/tray.rs
show = Show

# src/ui/settings_window/tray.rs
color = Color

# src/ui/settings_window/tray.rs
fixed-color = Fixed color

# src/ui/settings_window/troubleshoot.rs
troubleshooting-could-not-start = Troubleshooting could not start

# src/ui/settings_window/troubleshoot.rs
choose-which-installed-ai-tool-should-investigate-the-problem = Choose which installed AI tool should investigate the problem.

# src/ui/settings_window/troubleshoot.rs
open-terminal = Open terminal

# src/ui/settings_window/window.rs
version-is-available = { $version } is available

# src/ui/settings_window/window.rs
update-now = Update now

# src/ui/settings_window/window.rs
missing-a-provider = Missing a provider?

# src/ui/settings_window/window.rs
ask-for-it-or-build-it-yourself = Ask for it or build it yourself.

# src/ui/settings_window/window.rs
request-a-provider = Request a provider

# src/ui/settings_window/window.rs
contribute-one = Contribute one

# src/surfaces/tray.rs
disabled = Disabled

# src/update.rs
update-complete = Update complete

# src/update.rs
now-running-version = Now running { $version }.

# src/usage/overview.rs
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
name-weekly = { $name } weekly

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
excluded-other = Excluded · { $other }

# application
exclude-from-usage-stats = Click to exclude from usage stats

# application
include-in-usage-stats = Click to include in usage stats

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

# src/ui/settings_window/providers.rs
folder-with-cmdc-cmd-or-cmdc-ps1-leave-empty-to-find-it-automati = Folder with cmdc.cmd or cmdc.ps1. Leave empty to find it automatically.

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


openrouter-keys-delete-tracked = Minibar also stops tracking it.


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


openrouter-keys-untrack = Remove from app


openrouter-keys-untracking = Removing…


openrouter-keys-untrack-title = Stop tracking “{ $name }”?


openrouter-keys-untrack-message = Minibar forgets this key. It keeps working on OpenRouter.


openrouter-keys-tracked-here = Minibar tracks this key.


openrouter-keys-manage-hint = Add a management key in Settings to create, edit and delete keys.

# src/ui/settings_window/notifications.rs
notification-sounds = Notification sounds

# src/ui/settings_window/notifications.rs
plays-a-short-sound-with-each-notification = Plays a short sound with each notification

# src/update.rs
update-downloading = Downloading update

# src/update.rs
update-download-progress = { $downloaded } of { $total }

# src/update.rs
update-downloaded = { $downloaded } downloaded

# src/update.rs
update-installing = Installing update

# src/update.rs
update-unpacking = Unpacking files…

# src/update.rs
update-restarting = Restarting Codex Minibar

# src/update.rs
update-restarting-body = The app will reopen in a moment.

# src/ui/settings_window/general.rs
provider-status = Provider status

# src/ui/settings_window/general.rs
show-provider-incidents = Show provider incidents

# src/ui/settings_window/general.rs
show-provider-incidents-description = Checks the public status pages of Codex, Claude and Cursor every 5 minutes and shows ongoing incidents in the popup

# src/ui/settings_window/general.rs
notify-about-new-incidents = Notify about new incidents

# src/ui/settings_window/general.rs
notify-about-new-incidents-description = Shows a notification when a status page reports a new incident for an enabled provider

# src/ui/popup_window/ui/cards.rs
open-status-page = Open status page

# src/surfaces/notifications.rs
provider-incident-title = { $provider }: { $impact }

# src/ui/popup_window/ui/cards.rs
status-since = Since { $time }

# src/providers/status.rs
status-impact-maintenance = Maintenance

# src/providers/status.rs
status-impact-degraded = Degraded performance

# src/providers/status.rs
status-impact-partial-outage = Partial outage

# src/providers/status.rs
status-impact-major-outage = Major outage

# src/providers/status.rs
status-stage-investigating = Investigating

# src/providers/status.rs
status-stage-identified = Identified

# src/providers/status.rs
status-stage-monitoring = Monitoring

# src/providers/status.rs
status-stage-in-progress = In progress

# src/ui/settings_window/notifications.rs
news-and-status = News and status

# src/update/whats_new.rs
no-summary-for-this-version = There's no summary for this version yet. The full release notes are on GitHub.

# src/update/whats_new.rs
couldn-t-load-what-s-new = Couldn't load what's new. Check your connection, or read the release notes on GitHub.

# src/update/whats_new.rs
full-release-notes = Full release notes

# src/ui/settings_window/about.rs
show-what-s-new-after-updating = Show what's new after updating

# src/ui/settings_window/about.rs
a-short-summary-of-the-release-once-it-s-installed = A short summary of the release, once it's installed.

# src/ui/settings_window/about.rs
view = View

# src/ui/settings_window/general.rs
popup-shortcut = Popup shortcut
popup-shortcut-description = Press it anywhere in Windows to show or hide the popup.
shortcut-not-set = Not set
press-a-shortcut = Press a shortcut…
popup-shortcut-recording-hint = Esc cancels. Backspace removes the shortcut.
popup-shortcut-needs-modifier = Include Ctrl, Alt or Win, or use F13–F24.
popup-shortcut-in-use = Another app already uses this shortcut. Choose a different one.
clear-shortcut = Remove shortcut
taskbar = Taskbar
preview = Preview
taskbar-page-caption = Live usage widgets drawn inside the Windows taskbar, next to the clock. They follow the taskbar's theme; click one to open the popup on its provider.
taskbar-show-widgets = Show widgets on the taskbar
taskbar-show-widgets-description = Draws the widgets below inside the Windows taskbar.
taskbar-position = Position
taskbar-position-description = Where the widgets sit on the taskbar. The start suits centered taskbar icons best.
taskbar-position-tray = Next to the notification area
taskbar-position-start = At the start of the taskbar
taskbar-widgets = Widgets
taskbar-no-widgets = No widgets yet
taskbar-no-widgets-description = Add a widget to show a quota on the taskbar.
taskbar-show-provider-icon = Show the provider icon
taskbar-style-glance = Glance
taskbar-style-glance-hint = The provider icon beside two lines of text, like the weather button.
taskbar-style-rings-hint = A progress ring per quota with its value beside it.
taskbar-style-bars-hint = Thin progress bars stacked beside the provider icon.
taskbar-menu-open = Open Codex Minibar
taskbar-menu-settings = Taskbar widget settings
taskbar-menu-hide = Hide taskbar widgets
taskbar-card-left = { $value } left
taskbar-card-used = { $value } used
taskbar-card-resets = Resets in { $time } · { $clock }
taskbar-card-no-data = Waiting for data
short-5h = 5h
short-7d = 7d
taskbar-style-nested = Nested rings
taskbar-style-dials = Dials
taskbar-style-meters = Meters
taskbar-style-nested-hint = Concentric rings in the space of one, with values beside them if you like.
taskbar-style-dials-hint = A small ring per quota with the number inside it.
taskbar-style-meters-hint = Slim level columns, like a battery gauge.
taskbar-show-provider-icon-description = The provider's mark at the start of the widget.
taskbar-show-labels = Show labels
taskbar-show-labels-description = Short names like 5h and 7d next to the values.
taskbar-show-values = Show percentages
taskbar-show-values-description = The quota percentages on the widget.
taskbar-show-pace = Show pace
taskbar-show-pace-description = A mark where usage would be right now at an even burn.
taskbar-menu-edit = Edit this widget
taskbar-content = Content
taskbar-content-hint = What the widget writes next to its graphic.
taskbar-reset-time = Reset time
taskbar-reset-time-description = When each quota resets.
taskbar-reset-off = Off
taskbar-reset-countdown = Time left
taskbar-reset-clock = Clock time
taskbar-cycle = Cycle widgets with the scroll wheel
taskbar-cycle-description = Shows one widget at a time; scroll over it to switch to the next.
taskbar-card-tokens = { $count } tokens
taskbar-click = Click opens
taskbar-click-description = The popup page a click on a widget opens.
taskbar-click-nearest = Home, or the tab if not on Home
taskbar-click-tab = Provider tab
taskbar-click-home = Home

# src/ui/settings_window/notifications.rs
follow-do-not-disturb = Pause during Do Not Disturb

# src/ui/settings_window/notifications.rs
follow-do-not-disturb-description = While Windows Do Not Disturb or Focus is on, notifications are skipped and sounds stay off. Results of actions you start still show.

# src/ui/settings_window/advanced.rs
performance = Performance

# src/ui/settings_window/advanced.rs
game-mode = Game mode

# src/ui/settings_window/advanced.rs
game-mode-description = Checks limits and usage less often, runs at low priority and pauses notifications. Automatic turns it on while a game runs, from the moment it is in front until it closes.

# src/settings.rs
game-mode-off = Off

# src/settings.rs
game-mode-automatic = Automatic

# src/settings.rs
game-mode-always = Always on

# src/ui/settings_window/notifications.rs
notify-game-mode-changes = Game mode changes

# src/ui/settings_window/notifications.rs
notify-game-mode-changes-description = Shows when game mode turns on or off

# src/surfaces/notifications.rs
game-mode-on-title = Game mode on

# src/surfaces/notifications.rs
game-mode-on-body = Checking limits less often and pausing notifications

# src/surfaces/notifications.rs
game-mode-on-game-body = { $game } is running. Checking limits less often and pausing notifications

# src/surfaces/notifications.rs
not-a-game = Not a game

# src/ui/settings_window/advanced.rs
add-a-game = Add a game

# src/ui/settings_window/advanced.rs
add-a-game-description = Games known to Windows Game Bar and games from Steam, Epic, GOG and Xbox are detected on their own. Add any other program here.

# src/ui/settings_window/advanced.rs
never-a-game-path = Never a game · { $path }

# src/surfaces/notifications.rs
game-mode-off-title = Game mode off

# src/surfaces/notifications.rs
game-mode-off-body = Back to normal checks and notifications

# src/ui/settings_window/command_palette.rs
command-palette = Command Palette

# src/ui/settings_window/command_palette.rs
stream-deck = Stream Deck

# src/ui/settings_window/command_palette.rs
install = Install

# src/ui/settings_window/command_palette.rs
cmdpal-extension = Command Palette extension

# src/ui/settings_window/command_palette.rs
cmdpal-description = Adds Minibar to PowerToys Command Palette: type a provider to see its limits, or pin bands to the Dock. Installing asks for administrator approval.

# src/ui/settings_window/command_palette.rs
cmdpal-installed-hint = Installed. Type “minibar” or a provider name in Command Palette, or pin bands from Edit Dock.

# src/ui/settings_window/command_palette.rs
cmdpal-downloading = Downloading the extension…

# src/ui/settings_window/command_palette.rs
cmdpal-installing = Waiting for the installer. Approve the administrator prompt to continue.

# src/ui/settings_window/command_palette.rs
cmdpal-installing-short = Installing…

# src/ui/settings_window/command_palette.rs
cmdpal-removing = Removing the extension…

# src/ui/settings_window/command_palette.rs
cmdpal-failed = The extension could not be installed or removed.

# src/ui/settings_window/command_palette.rs
cmdpal-reinstall = Reinstall

# src/ui/settings_window/command_palette.rs
cmdpal-dock-bands = Dock bands

# src/ui/settings_window/command_palette.rs
cmdpal-dock-bands-caption = Each band appears in the Command Palette Dock settings. Pin it from Edit Dock, then choose where it sits.

# src/ui/settings_window/command_palette.rs
cmdpal-no-bands = No bands

# src/ui/settings_window/command_palette.rs
cmdpal-no-bands-description = Add a band to pin provider limits to the Dock.

# src/ui/settings_window/command_palette.rs
cmdpal-add-band = Add band

# src/ui/settings_window/command_palette.rs
cmdpal-band = Band { $v0 }

# src/ui/settings_window/command_palette.rs
cmdpal-band-removed = Band removed

# src/ui/settings_window/command_palette.rs
cmdpal-remove-band = Remove band

# src/ui/settings_window/command_palette.rs
cmdpal-content-hint = The Dock writes a band as a title over a subtitle.

# src/ui/settings_window/command_palette.rs
cmdpal-indicators-hint = Up to 2 quotas: one fills the title and subtitle, two take a line each.

# src/ui/popup_window/ui/cards.rs
tibo-banked-reset = Tibo Banked Reset™

# src/ui/popup_window/ui/cards.rs
banked-reset-credit = Banked reset credit

# src/ui/popup_window/ui/cards.rs
before-when = Before { $when }

# src/ui/popup_window/ui/cards.rs
time-ago = { $time } ago

# src/ui/popup_window/ui/cards.rs
new-codex-banked-reset-info = New Codex banked reset info

# src/ui/popup_window/ui/cards.rs
a-possible-codex-reset-is-expected-before-when-within-countdown = A possible Codex reset is expected before { $when } (within { $countdown })

# src/ui/popup_window/ui/cards.rs
label-possible-reset-before-when-within-countdown = { $label }: possible reset before { $when } (within { $countdown })

# src/ui/popup_window/ui/cards.rs
time-late = { $time } late

# Floating usage panel
popup-floating = Floating panel
linux-popup-keep-open = Keep open
linux-popup-keep-open-description = Keep the popup open when switching apps. On Wayland, right-click the panel header and choose Always on Top in your desktop's window menu, if available.
linux-popup-unpin = Unpin panel
popup-floating-description = Keep the panel open and drag its header to another position or monitor.
popup-always-on-top = Always on top
popup-always-on-top-description = Keep the panel above other applications and open when switching apps.
popup-position = Panel position
popup-position-description = The position is remembered between sessions.
popup-reset-position = Reset position
popup-placement = Popup position
popup-placement-description = Where the popup opens on the screen. Automatic opens it by the tray or taskbar.
popup-placement-auto = Automatic
popup-placement-top-left = Top left
popup-placement-top-center = Top center
popup-placement-top-right = Top right
popup-placement-bottom-left = Bottom left
popup-placement-bottom-center = Bottom center
popup-placement-bottom-right = Bottom right
linux-popup-placement-description = Where the popup opens on the screen.
linux-popup-placement-wayland = Wayland desktops choose where windows open, so this only applies in X11 sessions.
popup-drag-header = Drag to move
popup-close = Close panel

show-popup-action-bar = Show popup action bar
show-popup-action-bar-description = Show the app title and action buttons in the regular popup. Floating panels always show this bar.

# src/ui/settings_window/activation.rs
use-banked-resets-automatically = Use banked resets automatically

# src/ui/settings_window/activation.rs
spends-a-banked-reset-shortly-before-it-expires = Spends a banked reset shortly before it expires so it isn't lost. Skipped while the limits have nothing to clear.

# src/ui/settings_window/activation.rs
use-before-expiry = Use before expiry

# src/ui/settings_window/activation.rs
how-long-before-a-banked-reset-expires-it-is-applied = How long before a banked reset expires it is applied.

# src/ui/settings_window/providers/page.rs
automatic-resets = Automatic resets

# src/app/auto_reset.rs
banked-reset-applied-automatically = Banked reset applied automatically

# src/providers/instances.rs
this-provider-has-no-banked-resets = This provider has no banked resets.

# src/app/auto_reset.rs
banked-reset-applies-in-minutes = Banked reset will be applied in { $minutes } min

# src/ui/settings_window/notifications.rs
before-an-automatic-reset = Before an automatic reset

# src/ui/settings_window/notifications.rs
warns-before-a-banked-reset-is-applied-automatically = Warns 10 minutes before a banked reset is applied automatically.

# src/app/auto_reset.rs
apply-reset-now = Apply now

# src/app/auto_reset.rs
skip-automatic-reset = Cancel

# src/app/auto_reset.rs
banked-reset-applied = Banked reset applied

# src/ui/settings_window/providers/page.rs
claude-other-account-needs-cli = Extra Claude accounts need the Claude Code CLI. The Claude app's built-in copy only serves the app's own account.
# src/surfaces/tray.rs
linux-no-tray-title = No system tray found

# src/surfaces/tray.rs
linux-no-tray-body = Enable the AppIndicator extension to see Codex Minibar in the panel. Launch Codex Minibar again to open it.

# src/ui/settings_window/general.rs
start-at-login = Start at login

# src/ui/settings_window/general.rs
linux-popup-shortcut-description = Add a custom shortcut in your desktop's keyboard settings that runs this command to show or hide the popup.
linux-popup-shortcut-native-description = Saved in your desktop's keyboard settings. Changes apply immediately.
linux-popup-shortcut-choose = Set shortcut
linux-popup-shortcut-dialog-hint = Press the key combination you want to use. It is saved as a custom shortcut in your desktop's keyboard settings.
linux-popup-shortcut-needs-modifier = Include Ctrl, Alt or Super, or use F13–F24.
linux-popup-shortcut-save-failed = Your desktop did not accept the shortcut. Try again, or add the command in its keyboard settings.

# src/ui/settings_window/general.rs
copy-command = Copy command

# src/ui/settings_window/general.rs
command-copied = Command copied

# src/ui/settings_window/about.rs
linux-about-tagline = Free, open-source companion for AI subscription usage limits, resets and spend, right in the system tray.

# src/ui/settings_window/advanced.rs
linux-save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = Save every setting to a .toml file. Saved keys stay encrypted on this computer.

# src/ui/settings_window/appearance.rs
linux-windows = System

# src/ui/settings_window/appearance.rs
linux-windows-follows-your-system-accent = System follows your desktop accent.

# src/ui/settings_window/appearance.rs
linux-windows-default = System default

# src/ui/settings_window/appearance.rs
linux-popup-theme-description = Changes only the popup. Settings keep their own look.

# src/ui/settings_window/appearance.rs
linux-glide-transitions-in-the-popup-and-settings-windows-own-animation = Glide transitions in the popup and Settings. Your desktop's own animation setting is also respected.

# src/ui/settings_window/providers/dialog.rs
linux-saved-in-windows-user-storage-never-in-the-settings-file = Encrypted with a key from your system keyring, never in the settings file.

# src/ui/settings_window/providers/dialog.rs
linux-credential-saved-in-windows-user-storage = Credential saved, encrypted with your system keyring.

# src/ui/settings_window/providers/dialog.rs
linux-api-key-saved-in-windows-user-storage = API key saved, encrypted with your system keyring.

# src/ui/settings_window/providers/page.rs
linux-saved-in-windows-user-storage = Encrypted with your system keyring

# src/ui/settings_window/general.rs
linux-applies-immediately-to-every-app-window-and-notification-auto-fol = Applies immediately to every app window and notification. Auto follows the system language.

# src/ui/settings_window/providers/dialog.rs
linux-runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = Runs Claude Code's own login for { $name } with its config folder as CLAUDE_CONFIG_DIR. The login stays in that folder, where Claude Code keeps it fresh.

# src/ui/settings_window/advanced.rs
linux-add-a-game-description = Games launched by Steam and games in Steam or Heroic libraries are detected on their own. Add any other program here.
# src/ui/settings_window/providers.rs
linux-reads-subscription-quota-from-your-existing-official-agy-windows = Reads subscription quota from your existing official agy sign-in.
linux-folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = Folder with the agy executable. Leave empty to find it automatically.

linux-folder-with-grok-cli = Folder with the grok executable. Leave empty to find it automatically.
linux-folder-with-command-code-cli = Folder with command-code or cmd. Leave empty to find it automatically.

usage-currency = Usage currency
usage-currency-description = Display usage costs at the latest available rate, including historical usage.
currency-unavailable = Rate unavailable; showing USD
currency-cached = Cached rate
currency-latest = Latest rate
currency-usd = Usage currency: USD
currency-daily-cost = Daily cost in the selected currency
currency-cost-by-model = Cost by model
no-matching-currencies = No matching currencies
currency-symbol = Currency symbol
currency-symbol-description = Shown exactly as typed, spaces included. Leave empty for the default.
currency-symbol-position = Symbol position
currency-symbol-before = Before amount
currency-symbol-after = After amount
currency-source = Rates by Exchange Rate API
