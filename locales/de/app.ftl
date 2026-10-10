### Codex Minibar application messages.
### German translation; per-message English fallback. Keep $parameters unchanged.

# src/provider_registry.rs
luna-reserve = Luna Reserve

# src/limits.rs
on-pace = Im Soll

# src/limits.rs
delta-in-deficit = { $delta }% über Soll

# src/limits.rs
delta-in-reserve = { $delta }% Reserve

# src/notifications.rs
msg-5-hour-limit-started = 5-Stunden-Limit gestartet

# src/notifications.rs
msg-5-hour-limit-reset-and-activated = 5-Stunden-Limit zurückgesetzt und aktiviert

# src/notifications.rs
msg-5-hour-limit-reset = 5-Stunden-Limit zurückgesetzt

# src/notifications.rs
weekly-limit-reset = Wochenlimit zurückgesetzt

# src/notifications.rs
name-5-hour = { $name } 5 Stunden

# src/notifications.rs
label-limit-is-low = Für { $label } bleibt wenig Kontingent

# src/popup_window/formatting.rs
expired-at-time = abgelaufen um { $time }

# src/popup_window/formatting.rs
expired-at-time-6c39ff = abgelaufen bei { $time } { $v0 }

# src/popup_window/state.rs
never = Nie

# src/popup_window/formatting.rs
unlimited = Unbegrenzt

# src/popup_window/ui/activity.rs
unavailable = Nicht verfügbar

# src/popup_window/formatting.rs
days-d-hours-h = { $days } T { $hours } Std.

# src/popup_window/formatting.rs
days-d = { $days } T

# src/popup_window/formatting.rs
hours-h-minutes-m = { $hours } Std. { $minutes } Min.

# src/popup_window/formatting.rs
hours-h = { $hours } Std.

# src/popup_window/formatting.rs
minutes-m = { $minutes } Min.

# src/popup_window/formatting.rs
waiting-for-first-update = Warten auf das erste Update

# src/popup_window/formatting.rs
just-now = gerade eben

# src/popup_window/formatting.rs
seconds-seconds-ago = vor { $seconds } Sekunden

# src/popup_window/formatting.rs
minutes-ago = vor { $v0 } Minuten

# src/popup_window/formatting.rs
updated-elapsed = Aktualisiert: { $elapsed }

# src/popup_window/state.rs
the-request-timed-out-try-refreshing-again = Die Anfrage hat das Zeitlimit überschritten. Versuchen Sie erneut zu aktualisieren.

# src/popup_window/state.rs
the-secure-connection-could-not-be-verified-see-log-for-details = Die sichere Verbindung konnte nicht überprüft werden. Weitere Informationen finden Sie unter Protokoll.

# src/popup_window/state.rs
the-provider-closed-the-connection-try-refreshing-again = Der Anbieter hat die Verbindung geschlossen. Versuchen Sie erneut zu aktualisieren.

# src/popup_window/state.rs
the-provider-s-address-could-not-be-resolved-check-your-connectio = Die Adresse des Anbieters konnte nicht aufgelöst werden. Überprüfen Sie Ihre Verbindung.

# src/popup_window/state.rs
could-not-connect-to-the-provider-check-your-connection-and-try-a = Es konnte keine Verbindung zum Anbieter hergestellt werden. Überprüfen Sie Ihre Verbindung und versuchen Sie es erneut.

# src/popup_window/state.rs
the-management-key-was-rejected-update-it-in-settings = Der Verwaltungsschlüssel wurde abgelehnt. Aktualisieren Sie ihn in den Einstellungen.

# src/popup_window/state.rs
authentication-failed-sign-in-again-or-update-the-provider-key = Die Authentifizierung ist fehlgeschlagen. Melden Sie sich erneut an oder aktualisieren Sie den Anbieterschlüssel.

# src/popup_window/state.rs
access-denied-by-the-provider-http-403 = Zugriff vom Provider verweigert (HTTP 403).

# src/popup_window/state.rs
too-many-requests-wait-a-few-minutes-before-refreshing-again = Zu viele Anfragen. Warten Sie einige Minuten, bevor Sie erneut aktualisieren.

# src/popup_window/state.rs
the-provider-is-temporarily-unavailable-try-again-later = Der Anbieter ist vorübergehend nicht erreichbar. Versuchen Sie es später noch einmal.

# src/popup_window/state.rs
the-provider-rejected-the-request-see-log-for-details = Der Anbieter lehnte die Anfrage ab. Weitere Informationen finden Sie unter Protokoll.

# src/popup_window/state.rs
the-requested-resource-was-not-found-see-log-for-details = Die angeforderte Ressource wurde nicht gefunden. Weitere Informationen finden Sie unter Protokoll.

# src/popup_window/state.rs
the-provider-returned-an-unexpected-response-try-refreshing-again = Der Anbieter hat eine unerwartete Antwort geliefert. Versuchen Sie erneut zu aktualisieren.

# src/popup_window/state.rs
the-request-failed-see-log-for-details = Die Anfrage ist fehlgeschlagen. Weitere Informationen finden Sie unter Protokoll.

# src/popup_window/ui/activity.rs
input = Eingabe

# src/popup_window/ui/activity.rs
cache = Cache

# src/popup_window/ui/usage.rs
output = Ausgabe

# src/popup_window/ui/activity.rs
input-uncached = Eingabe (nicht zwischengespeichert)

# src/popup_window/ui/usage.rs
cached-input = Zwischengespeicherte Eingabe

# src/popup_window/ui/activity.rs
value-partially-priced = { $value } (Preise teilweise bekannt)

# src/popup_window/ui/activity.rs
could-not-load-model-data-error = Modelldaten konnten nicht geladen werden: { $error }

# src/popup_window/ui/activity.rs
usage-activity = Nutzungsaktivität

# src/popup_window/ui/activity.rs
waiting-for-cursor-s-usage-export-refresh-to-retry = Warten auf den Nutzungsexport von Cursor. Aktualisieren Sie, um es erneut zu versuchen.

# src/popup_window/ui/activity.rs
model-data-unavailable = Modelldaten nicht verfügbar

# src/popup_window/ui/activity.rs
loading-models = Modelle werden geladen…

# src/popup_window/ui/tooltip.rs
no-model-data = Keine Modelldaten

# src/popup_window/ui/activity.rs
no-usage-data = Keine Nutzungsdaten

# src/popup_window/ui/activity.rs
no-series-selected = Keine Serie ausgewählt

# src/popup_window/ui/activity.rs
no-tokens-in-this-period = Keine { $v0 }-Tokens in diesem Zeitraum

# src/popup_window/ui/activity.rs
loading-model-breakdown = Modellaufschlüsselung wird geladen

# src/popup_window/ui/activity.rs
group-tokens-or-cost-by-model = Gruppieren Sie Token oder Kosten nach Modell

# src/popup_window/ui/activity.rs
split-type = Typ

# src/popup_window/ui/activity.rs
split-by-token-type = Balken nach Token-Typ aufteilen

# src/popup_window/ui/usage.rs
model = Modell

# src/popup_window/ui/usage.rs
tokens = Token

# src/popup_window/ui/usage.rs
cost = Kosten

# src/popup_window/ui/activity.rs
no-cost-data-for-this-period = Für diesen Zeitraum liegen keine Kostendaten vor

# src/popup_window/ui/activity.rs
no-token-data-for-this-period = Für diesen Zeitraum liegen keine Token-Daten vor

# src/popup_window/ui/activity.rs
daily-cost-in-usd = Tägliche Kosten in USD

# src/popup_window/ui/activity.rs
daily-token-volume = Tägliches Token-Volumen

# src/popup_window/ui/activity.rs
of-total-models-scroll-for-more = { $v0 }–{ $v1 } von { $total } Modellen · Für weitere scrollen

# src/popup_window/ui/activity.rs
cost-usd-by-model = Kosten (USD) nach Modell

# src/popup_window/ui/activity.rs
tokens-by-model = Token nach Modell

# src/settings.rs
today = Heute

# src/popup_window/ui/activity.rs
last-period-days = Letzte { $period } Tage

# src/popup_window/ui/activity.rs
no-data = Keine Daten

# src/popup_window/ui/cards.rs
loading-usage-statistics = Nutzungsstatistiken werden geladen…

# src/popup_window/ui/cards.rs
available-balance = Verfügbares Guthaben

# src/popup_window/ui/cards.rs
resets-in = Zurücksetzung in

# src/popup_window/ui/cards.rs
session-not-started = Sitzung nicht gestartet

# src/popup_window/ui/cards.rs
expires-in = Läuft ab in

# src/popup_window/ui/cards.rs
usage = Nutzung:

# src/settings_window/providers/page.rs
remove-key = Schlüssel entfernen

# src/popup_window/ui/cards.rs
msg-1-banked-reset = 1 gespeicherter Reset

# src/popup_window/ui/cards.rs
count-banked-resets = { $count ->
    [one] { $count } gespeicherter Reset
   *[other] { $count } gespeicherte Resets
    }

# src/popup_window/ui/cards.rs
available-to-use = Zur Nutzung verfügbar

# src/popup_window/ui/cards.rs
no-expiration-date = Kein Ablaufdatum

# src/popup_window/ui/cards.rs
banked-reset = Gespeicherter Reset { $v0 }

# src/popup_window/ui/cards.rs
codex-limits = Codex-Grenzwerte

# src/popup_window/ui/cards.rs
open-announcement-source = Quelle der Ankündigung öffnen

# src/popup_window/ui/cards.rs
source-not-provided = Quelle nicht angegeben

# src/popup_window/ui/cards.rs
tibo-reset = Tibo Reset™

# src/settings_window/customize.rs
home = Startseite

# src/popup_window/ui/usage.rs
usage-0bb186 = Nutzung

# src/popup_window/ui/footer.rs
settings = Einstellungen

# src/popup_window/ui/footer.rs
install-update = Update installieren

# src/popup_window/ui/footer.rs
refreshing-limits-and-usage = Limits und Nutzung werden aktualisiert…

# src/popup_window/ui/footer.rs
updated = Aktualisiert{ " " }

# src/popup_window/ui/footer.rs
refresh-last-updated-relative = Aktualisieren · Zuletzt aktualisiert: { $relative }

# src/popup_window/ui/home.rs
edit-home = Startseite bearbeiten
home-card-layout = Layout
home-card-layout-cards = Karten
home-card-layout-lines = Linien
home-card-layout-rings = Ringe
drag-to-reorder = Zum Neuanordnen ziehen

# src/popup_window/ui/home.rs
drop-here = Hier ablegen

# src/settings_window/general.rs
usage-stats = Nutzungsstatistiken

# src/popup_window/ui/usage.rs
loading-usage = Nutzung wird geladen…

# src/settings_window/window.rs
update-failed = Update fehlgeschlagen

# src/popup_window/ui/root.rs
something-went-wrong = Etwas ist schief gelaufen

# src/popup_window/ui/root.rs
no-providers-enabled = Keine Anbieter aktiviert

# src/popup_window/ui/root.rs
turn-one-on-in-settings-providers = Aktivieren Sie eine Option unter „Einstellungen“ > „Anbieter“.

# src/popup_window/ui/root.rs
sign-in-again-to-keep-limits-updating = Melden Sie sich erneut an, um die Limits auf dem neuesten Stand zu halten.

# src/settings_window/providers/page.rs
sign-in-again = Erneut anmelden

# src/popup_window/ui/tooltip.rs
total = Insgesamt

# src/popup_window/ui/usage.rs
enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = Aktivieren Sie einen Anbieter in den Einstellungen und fügen Sie ihn in die Nutzungsstatistiken ein, um die lokale API-Nutzung anzuzeigen.

# src/popup_window/ui/usage.rs
hourly-cost = Stündliche Kosten

# src/popup_window/ui/usage.rs
hourly-processed-tokens = Stündlich verarbeitete Token

# src/popup_window/ui/usage.rs
no-activity-in-this-range = Keine Aktivität in diesem Bereich

# src/popup_window/ui/usage.rs
day = Tag

# src/popup_window/ui/usage.rs
breakdown = Aufschlüsselung

# src/popup_window/ui/usage.rs
totals = Summen

# src/popup_window/ui/usage.rs
processed-tokens = Verarbeitete Token

# src/popup_window/ui/usage.rs
uncached-input = Nicht zwischengespeicherte Eingabe

# src/popup_window/ui/usage.rs
cache-savings = Cache-Einsparungen

# src/popup_window/ui/usage.rs
share = Anteil

# src/popup_window/ui/usage.rs
hour = Stunde

# src/usage_overview.rs
sat = Sa

# src/usage_overview.rs
sun = So

# src/provider_registry.rs
msg-5h-session = 5-Stunden-Sitzung

# src/provider_registry.rs
weekly = Wöchentlich

# src/provider_registry.rs
cursor-models = Cursor-Modelle

# src/settings_window/tray.rs
other-models = Andere Modelle

# src/provider_registry.rs
all-models = Alle Modelle

# src/settings_window/tray.rs
grok-bot = Grok Bot

# src/provider_registry.rs
monthly = Monatlich

# src/provider_registry.rs
spending-limit = Ausgabenlimit

# src/provider_registry.rs
gemini = Gemini

# src/provider_registry.rs
claude-gpt = Claude + GPT

# src/provider_registry.rs
credits = Credits

# src/provider_registry.rs
monthly-credits = Monatliche Credits

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
banked-resets = Gespeicherte Resets

# src/provider_registry.rs
usage-stats-7b9e1a = Nutzungsstatistiken

# src/provider_registry.rs
spending = Ausgaben

# src/settings.rs
separate-tabs = Separate Registerkarten

# src/settings.rs
grouped-switcher = Gruppiert, mit Umschalter

# src/settings.rs
grouped-all-accounts = Gruppiert, alle Konten

# src/settings.rs
openrouter-account = OpenRouter-Konto

# src/settings.rs
yesterday = Gestern

# src/settings.rs
history-retention-must-be-between-1-and-365-days = Die Aufbewahrung des Verlaufs muss zwischen 1 und 365 Tagen liegen

# src/settings.rs
session-low-usage-threshold-must-be-between-1-and-99-percent = Der Schwellenwert für die geringe Sitzungsnutzung muss zwischen 1 und 99 Prozent liegen

# src/settings.rs
weekly-low-usage-threshold-must-be-between-1-and-99-percent = Der Schwellenwert für die wöchentliche niedrige Nutzung muss zwischen 1 und 99 Prozent liegen

# src/settings_window/about.rs
check-github-for-a-new-version = Überprüfen Sie GitHub auf eine neue Version

# src/settings_window/about.rs
checking-for-updates = Suche nach Updates…

# src/settings_window/about.rs
you-re-up-to-date = Bereits auf dem neuesten Stand

# src/settings_window/about.rs
update-available = Update { $v0 } verfügbar

# src/settings_window/about.rs
installing-update = Update wird installiert…

# src/settings_window/about.rs
couldn-t-check-for-updates = Es konnte nicht nach Updates gesucht werden

# src/settings_window/about.rs
version = Version { $v0 }

# src/settings_window/about.rs
about-tagline = Kostenloser Open-Source-Begleiter für Nutzungslimits, Resets und Ausgaben deiner KI-Abos – direkt im Windows-Infobereich.

# src/settings_window/about.rs
a-new-release-is-ready-to-install = Eine neue Version ist zur Installation bereit.

# src/settings_window/about.rs
what-s-new = Was ist neu?

# src/settings_window/about.rs
update = Aktualisieren

# src/settings_window/about.rs
check-for-updates = Nach Updates suchen

# src/settings_window/about.rs
check-for-updates-on-startup = Suchen Sie beim Start nach Updates

# src/settings_window/about.rs
notify-when-a-new-version-is-found = Benachrichtigen, wenn eine neue Version gefunden wird

# src/settings_window/about.rs
github = GitHub

# src/settings_window/about.rs
source-code = Quellcode

# src/settings_window/about.rs
releases = Versionen

# src/settings_window/about.rs
see-what-s-new = Sehen Sie, was es Neues gibt

# src/settings_window/about.rs
report-an-issue = Problem melden

# src/settings_window/about.rs
found-a-bug = Einen Fehler gefunden?

# src/settings_window/about.rs
author = Autor

# src/settings_window/onboarding.rs
updates = Aktualisierungen

# src/settings_window/about.rs
resources = Ressourcen

# src/usage_overview.rs
mon = Mo

# src/usage_overview.rs
tue = Di

# src/usage_overview.rs
wed = Mi

# src/usage_overview.rs
thu = Do

# src/usage_overview.rs
fri = Fr

# src/settings_window/activation.rs
every-day = Jeden Tag

# src/settings_window/activation.rs
weekdays = Wochentags

# src/settings_window/activation.rs
weekends = Wochenenden

# src/settings_window/activation.rs
no-days = Keine Tage

# src/settings_window/onboarding.rs
start-5-hour-sessions-automatically = Starten Sie automatisch 5-stündige Sitzungen

# src/settings_window/activation.rs
starts-a-new-session-as-soon-as-a-window-is-available-instead-of = Startet eine neue Sitzung, sobald ein Nutzungskontingent verfügbar ist, ohne auf die erste Anfrage zu warten. Jedes Konto verwendet seine eigene Anmeldung.

# src/settings_window/activation.rs
add-codex-or-claude-in-providers-first = Fügen Sie zuerst Codex oder Claude unter „Anbieter“ hinzu.

# src/settings_window/activation.rs
off-in-providers = Aus bei Anbietern

# src/settings_window/activation.rs
quiet-periods = Ruhezeiten

# src/settings_window/activation.rs
don-t-auto-start-sessions-during-these-times = Starten Sie in diesen Zeiten keine Sitzungen automatisch.

# src/settings_window/providers/dialog.rs
add = Hinzufügen

# src/settings_window/activation.rs
all-day = Den ganzen Tag

# src/settings_window/activation.rs
from = Von

# src/settings_window/activation.rs
until = Bis

# src/settings_window/activation.rs
scheduled-activations = Geplante Aktivierungen

# src/settings_window/activation.rs
start-a-5-hour-session-at-a-set-time = Beginnen Sie eine 5-stündige Sitzung zu einer festgelegten Zeit.

# src/settings_window/activation.rs
time = Uhrzeit

# src/settings_window/activation.rs
turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = Aktivieren Sie zunächst Codex oder Claude in den Anbietern mit einer Anmeldung im Konfigurationsordner.

# src/settings_window/activation.rs
no-quiet-periods-yet = Noch keine Ruhezeiten.

# src/settings_window/activation.rs
no-scheduled-activations-yet = Noch keine geplanten Aktivierungen.

# src/settings_window/activation.rs
unknown-provider = Unbekannter Anbieter

# src/settings_window/activation.rs
remove-quiet-period = Ruhezeit entfernen

# src/settings_window/activation.rs
remove-activation = Aktivierung entfernen

# src/settings_window/tray.rs
provider = Anbieter

# src/settings_window/activation.rs
days = Tage

# src/settings_window/advanced.rs
export-settings = Einstellungen exportieren

# src/settings_window/advanced.rs
save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = Speichern Sie jede Einstellung in einer .toml-Datei. Gespeicherte Schlüssel bleiben im Windows-Benutzerspeicher.

# src/settings_window/advanced.rs
export = Exportieren

# src/settings_window/advanced.rs
import-settings = Einstellungen importieren

# src/settings_window/advanced.rs
replace-the-current-settings-with-a-previously-exported-file = Ersetzen Sie die aktuellen Einstellungen durch eine zuvor exportierte Datei.

# src/settings_window/advanced.rs
import = Importieren

# src/settings_window/advanced.rs
clear-usage-data = Nutzungsdaten löschen

# src/settings_window/advanced.rs
delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = Löschen Sie den gesammelten Nutzungsverlauf. Beim nächsten Scan wird es aus den Protokollen des lokalen Anbieters neu erstellt.

# src/settings_window/advanced.rs
clear = Löschen

# src/settings_window/advanced.rs
usage-data-clear-failed = Das Löschen der Nutzungsdaten ist fehlgeschlagen

# src/settings_window/advanced.rs
the-background-worker-is-unavailable = Der Hintergrundprozess ist nicht verfügbar.

# src/settings_window/advanced.rs
usage-data-cleared = Nutzungsdaten gelöscht.

# src/settings_window/advanced.rs
reset-all-settings = Alle Einstellungen zurücksetzen

# src/settings_window/advanced.rs
restore-every-default-and-start-the-welcome-flow-again = Stellen Sie alle Standardeinstellungen wieder her und starten Sie den Begrüßungsablauf erneut.

# src/settings_window/advanced.rs
reset = Zurücksetzen

# src/settings_window/advanced.rs
backup = Sicherung

# src/settings_window/advanced.rs
data = Daten

# src/settings_window/advanced.rs
rendering = Darstellung

# src/settings_window/advanced.rs
software-rendering = Software-Rendering

# src/settings_window/advanced.rs
software-rendering-description = Fenster mit dem Prozessor statt der Grafikkarte zeichnen. Kann helfen, wenn ein Spiel die GPU auslastet; Animationen laufen eventuell weniger flüssig.

# src/settings_window/advanced.rs
settings-exported = Einstellungen exportiert.

# src/settings_window/advanced.rs
settings-export-failed = Der Export der Einstellungen ist fehlgeschlagen

# src/settings_window/advanced.rs
settings-imported = Einstellungen importiert.

# src/settings_window/advanced.rs
settings-import-failed = Der Import der Einstellungen ist fehlgeschlagen

# src/settings_window/advanced.rs
settings-reset-failed = Das Zurücksetzen der Einstellungen ist fehlgeschlagen

# src/settings_window/advanced.rs
reset-all-settings-bbfe66 = Alle Einstellungen zurücksetzen?

# src/settings_window/advanced.rs
every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = Alle Einstellungen werden auf die Standardeinstellungen zurückgesetzt und der Begrüßungsablauf wird erneut geöffnet. Gespeicherte Schlüssel und Nutzungsdaten bleiben erhalten.

# src/settings_window/troubleshoot.rs
cancel = Abbrechen

# src/settings_window/appearance.rs
windows = Windows

# src/settings_window/appearance.rs
light = Hell

# src/settings_window/appearance.rs
dark = Dunkel

# src/settings_window/appearance.rs
color-theme = Farbthema

# src/settings_window/appearance.rs
applies-to-settings-the-popup-and-its-tray-menu = Gilt für Einstellungen, das Popup und sein Taskleistenmenü.

# src/settings_window/appearance.rs
accent-color = Akzentfarbe

# src/settings_window/appearance.rs
windows-follows-your-system-accent = Windows folgt Ihrem Systemakzent.

# src/settings_window/appearance.rs
icons-style = Icons-Stil

# src/settings_window/appearance.rs
font = Schriftart

# src/settings_window/appearance.rs
any-font-installed-on-this-pc = Jede auf diesem PC installierte Schriftart.

# src/settings_window/appearance.rs
windows-default = Windows-Standard

# src/settings_window/appearance.rs
glyph-style-in-the-settings-sidebar = Glyphenstil in der Seitenleiste „Einstellungen“.

# src/settings_window/appearance.rs
colored = Farbig

# src/settings_window/tray.rs
monochrome = Monochrom

# src/settings_window/appearance.rs
time-format = Zeitformat

# src/settings_window/appearance.rs
msg-12-hour = 12 Stunden

# src/settings_window/appearance.rs
msg-24-hour = 24 Stunden

# src/settings_window/appearance.rs
popup-background = Popup-Hintergrund
popup-theme = Popup-Thema
popup-theme-description = Ändert nur das Popup. Die Einstellungen behalten das Windows-Aussehen.
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel
popup-theme-built-in = Integriert
popup-theme-vscode = VS Code-Themen
vscode-themes = VS Code-Themen
vscode-themes-description = Importieren Sie ein .vsix-Paket oder eine Farbthema-.json-Datei. Themes ändern nur die Farbe des Popups.
import-theme = Thema importieren
importing-theme = Importieren…
remove-theme = Thema entfernen
browse-open-vsx = Open VSX durchsuchen
browse-open-vsx-description = Suchen und installieren Sie Farbthemen aus der open-vsx.org-Registrierung
search-color-themes = Suchen Sie nach Farbthemen
open-vsx-search = Suchen
open-vsx-browse = Durchsuchen
open-vsx-searching = Suche...
no-themes-found = Keine Farbthemen gefunden
open-vsx-load-failed = Die Designs von Open VSX konnten nicht geladen werden
open-vsx-retry = Versuchen Sie es noch einmal
open-vsx-preview-loading = Vorschau wird geladen…
open-vsx-preview-hint = Klicken Sie auf ein Thema, um eine Vorschau im Popup anzuzeigen. Install behält das gesamte Paket; Der Download-Button eines Themes behält genau dieses.
open-vsx-preview-empty = Diese Erweiterung enthält keine Farbthemen
open-vsx-install = Installieren
open-vsx-install-one = Installieren Sie nur dieses Theme
open-vsx-installed = Installiert
open-vsx-installing = Installieren…
theme-installed = Theme installiert
theme-install-failed = Das Theme konnte nicht installiert werden
theme-remove-failed = Das Thema konnte nicht entfernt werden

# src/settings_window/appearance.rs
acrylic = Acrylic

# src/settings_window/appearance.rs
mica = Mica

# src/settings_window/appearance.rs
solid = Einfarbig

# src/settings_window/appearance.rs
bottom-bar-size = Größe der unteren Leiste

# src/settings_window/appearance.rs
comfortable = Geräumig

# src/settings_window/appearance.rs
compact = Kompakt

# src/settings_window/appearance.rs
popup-corner-radius = Popup-Eckenradius
popup-borders = Rahmen
popup-borders-description = Rahmen um Popup-Fenster, Karten und Steuerelemente anzeigen

# src/settings_window/appearance.rs
animation-effects = Animationseffekte

# src/settings_window/appearance.rs
glide-transitions-in-the-popup-and-settings-windows-own-animation = Sanfte Übergänge im Popup und in den Einstellungen. Die Windows-Einstellung für Animationen wird ebenfalls berücksichtigt.

# src/settings_window/appearance.rs
popup = Popup

# src/settings_window/appearance.rs
motion = Bewegung

# src/settings_window/appearance.rs
blue = Blau

# src/settings_window/appearance.rs
purple = Lila

# src/settings_window/appearance.rs
pink = Rosa

# src/settings_window/appearance.rs
red = Rot

# src/settings_window/appearance.rs
orange = Orange

# src/settings_window/appearance.rs
green = Grün

# src/settings_window/appearance.rs
teal = Blaugrün

# src/settings_window/customize.rs
use-two-columns = Verwenden Sie zwei Spalten

# src/settings_window/customize.rs
several-accounts-of-one-provider = Mehrere Konten eines Anbieters

# src/settings_window/customize.rs
separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = Separate Registerkarten geben jeder Instanz eine eigene Registerkarte. „Gruppiert“ zeigt eine Registerkarte pro Anbieter, mit einem Kontowechsler oder jedem gestapelten Konto.

# src/settings_window/customize.rs
use-monochrome-icons = Verwenden Sie monochrome Symbole

# src/settings_window/customize.rs
draw-provider-marks-in-the-popup-without-brand-colors = Anbietermarkierungen im Popup ohne Markenfarben einzeichnen.

# src/settings_window/onboarding.rs
show-used-instead-of-remaining = Verbrauch statt Restkontingent anzeigen

# src/settings_window/customize.rs
show-usage-in-values-when-possible = Nutzung nach Möglichkeit als Werte anzeigen

# src/settings_window/customize.rs
adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = Fügt neben den Prozentsätzen die genauen Verbrauchs-/Limitbeträge hinzu, wenn ein Anbieter diese meldet.

# src/settings_window/onboarding.rs
show-usage-pace = Nutzungstempo anzeigen

# src/settings_window/onboarding.rs
marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = Markiert, ob Sie Ihr Kontingent schneller oder langsamer als bei einem gleichmäßigen Tempo verbrennen.

# src/settings_window/customize.rs
use-legacy-usage-cards = Verwenden Sie ältere Nutzungskarten

# src/settings_window/customize.rs
show-the-older-layout-with-a-header-a-thin-bar-and-a-footer = Zeigen Sie das ältere Layout mit einer Kopfzeile, einer dünnen Leiste und einer Fußzeile an.

# src/settings_window/onboarding.rs
show-account-name = Kontonamen anzeigen

# src/settings_window/customize.rs
show-on-home-tab = Auf der Registerkarte „Startseite“ anzeigen

# src/settings_window/customize.rs
layout = Layout

# src/settings_window/customize.rs
donut = Ringdiagramm

# src/settings_window/customize.rs
cards = Karten

# src/settings_window/customize.rs
tabs = Tabs

# src/settings_window/customize.rs
usage-widget = Nutzungs-Widget

# src/settings_window/customize.rs
card = Karte

# src/settings_window/customize.rs
tab = Tab

# src/settings_window/customize.rs
popup-cards = Popup-Karten

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = Wählen Sie aus, welche Karten dieses Konto auf der Startseite und auf einer eigenen Registerkarte anzeigt.

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = Wählen Sie die Karten für den Tab dieses Kontos. Aktivieren Sie „Auf Startseite anzeigen“, um die Karten für die Startseite auszuwählen.

# src/settings_window/general.rs
msg-30-seconds = 30 Sekunden

# src/settings_window/general.rs
msg-1-minute = 1 Minute

# src/settings_window/general.rs
msg-2-minutes = 2 Minuten

# src/settings_window/general.rs
msg-5-minutes = 5 Minuten

# src/settings_window/general.rs
msg-10-minutes = 10 Minuten

# src/settings_window/general.rs
msg-15-minutes = 15 Minuten

# src/settings_window/general.rs
msg-30-minutes = 30 Minuten

# src/settings_window/general.rs
msg-45-minutes = 45 Minuten

# src/settings_window/general.rs
msg-60-minutes = 60 Minuten

# src/settings_window/onboarding.rs
start-with-windows = Mit Windows starten

# src/settings_window/general.rs
open-codex-minibar-in-the-tray-when-you-sign-in = Öffnen Sie Codex Minibar in der Taskleiste, wenn Sie sich anmelden.

# src/settings_window/onboarding.rs
refresh-interval = Aktualisierungsintervall

# src/settings_window/general.rs
how-often-this-instance-s-quotas-are-read = Wie oft die Kontingente dieser Instanz gelesen werden. Längere Intervalle helfen, Anfragelimits der Anbieter zu vermeiden.

# src/settings_window/general.rs
enable-usage-stats = Aktivieren Sie Nutzungsstatistiken

# src/settings_window/general.rs
scan-local-provider-history-for-the-usage-tab-and-cost-totals = Lokalen Anbieterverlauf für den Tab „Nutzung“ und die Gesamtkosten durchsuchen.

# src/settings_window/general.rs
collection-period = Erfassungsintervall

# src/settings_window/general.rs
how-often-local-provider-history-is-scanned = Wie oft der Verlauf des lokalen Anbieters gescannt wird.

# src/settings_window/general.rs
check-for-confirmed-tibo-resets = Suchen Sie nach bestätigten Tibo-Resets

# src/settings_window/general.rs
reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = Liest den öffentlichen GitHub-Feed der App und speichert die neueste Ankündigung im Cache.

# src/settings_window/general.rs
notify-when-new-reset-info-arrives = Benachrichtigen, wenn neue Reset-Informationen eintreffen

# src/settings_window/general.rs
shows-a-notification-when-the-feed-reports-a-possible-reset-never = Zeigt eine Benachrichtigung an, wenn der Feed einen möglichen Reset meldet, niemals zum Zeitpunkt des Resets.

# src/settings_window/general.rs
check-every = Prüfintervall

# src/settings_window/general.rs
the-feed-is-also-checked-immediately-when-the-app-starts-or-this = Der Feed wird auch sofort überprüft, wenn die App gestartet wird oder diese Option aktiviert ist.

# src/settings_window/general.rs
msg-1-hour = 1 Stunde

# src/settings_window/general.rs
msg-3-hours = 3 Stunden

# src/settings_window/general.rs
msg-2-hours = 2 Stunden

# src/settings_window/general.rs
msg-5-hours = 5 Stunden

# src/settings_window/general.rs
msg-6-hours = 6 Stunden

# src/settings_window/general.rs
msg-12-hours = 12 Stunden

# src/settings_window/general.rs
msg-24-hours = 24 Stunden

# src/settings_window/general.rs
tibo-resets = Tibo Resets™

# src/settings_window/general.rs
enable-a-provider-in-the-providers-tab-to-include-it-here = Aktivieren Sie auf der Registerkarte „Anbieter“ einen Anbieter, um ihn hier einzuschließen.

# src/settings_window/general.rs
add-a-management-key = Fügen Sie einen Verwaltungsschlüssel hinzu

# src/settings_window/general.rs
usage-statistics-are-off-on-this-provider-s-page = Auf der Seite dieses Anbieters sind Nutzungsstatistiken deaktiviert

# src/settings_window/general.rs
included-providers = Inklusive Anbieter

# src/settings_window/general.rs
choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = Wählen Sie aus, welche Konten auf die Registerkarte „Nutzung“ und die Gesamtsumme dieser Maschine angerechnet werden sollen. Ausgeschlossene Konten sammeln weiterhin Daten und zeigen die Nutzung weiterhin auf ihrer eigenen Seite an.

# src/settings_window/integrations.rs
download-the-latest-stream-deck-companion-from-github-and-open-it = Laden Sie den neuesten Stream Deck-Begleiter von GitHub herunter und öffnen Sie das Installationsprogramm.

# src/settings_window/integrations.rs
downloading-the-latest-stream-deck-companion-from-github = Laden Sie den neuesten Stream Deck-Begleiter von GitHub herunter ...

# src/settings_window/integrations.rs
opening-the-stream-deck-installer = Öffnen des Stream Deck-Installationsprogramms...

# src/settings_window/integrations.rs
the-installer-is-open-finish-the-installation-in-stream-deck = Das Installationsprogramm ist geöffnet. Beenden Sie die Installation in Stream Deck.

# src/settings_window/integrations.rs
the-plugin-could-not-be-downloaded-or-opened-try-again = Das Plugin konnte nicht heruntergeladen oder geöffnet werden. Versuchen Sie es erneut.

# src/settings_window/integrations.rs
downloading = Herunterladen...

# src/settings_window/integrations.rs
opening = Wird geöffnet…

# src/settings_window/integrations.rs
install-plugin = Plugin installieren

# src/settings_window/integrations.rs
stream-deck-companion = Stream Deck-Begleitplugin

# src/settings_window/integrations.rs
check-your-connection-then-try-again = Überprüfen Sie Ihre Verbindung und versuchen Sie es dann erneut.

# src/settings_window/providers/page.rs
on = Ein

# src/settings_window/providers/page.rs
off = Aus

# src/settings_window/kit.rs
more-options = Weitere Optionen

# src/settings_window/kit.rs
custom-color = Benutzerdefinierte Farbe

# src/settings_window/troubleshoot.rs
run-troubleshoot-with-ai = Führen Sie „Fehlerbehebung mit KI“ aus

# src/settings_window/log.rs
let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = Lassen Sie eine installierte KI CLI das Protokoll lesen und ein Problem untersuchen.

# src/settings_window/log.rs
choose-tool = Werkzeug auswählen

# src/settings_window/log.rs
application-log = Anwendungsprotokoll

# src/settings_window/log.rs
log-txt-in-the-app-data-folder = log.txt im App-Datenordner.

# src/settings_window/log.rs
open-log-txt = Öffnen Sie log.txt

# src/settings_window/log.rs
could-not-open-log-txt = Log.txt konnte nicht geöffnet werden

# src/settings_window/log.rs
open-logs-folder = Öffnen Sie den Protokollordner

# src/settings_window/log.rs
could-not-open-logs-folder = Der Protokollordner konnte nicht geöffnet werden

# src/settings_window/log.rs
no-log-events-yet = Noch keine Protokollereignisse.

# src/settings_window/log.rs
live-tail = Live-Protokoll

# src/settings_window/log.rs
no-supported-ai-tool-found = Kein unterstütztes KI-Tool gefunden

# src/settings_window/log.rs
install-codex-or-claude-code-and-make-it-available-to-minibar = Installieren Sie Codex oder Claude Code und machen Sie es für Minibar verfügbar.

# src/settings_window/window.rs
general = Allgemein

# src/settings_window/window.rs
appearance = Aussehen

# src/settings_window/window.rs
providers = Anbieter

# src/settings_window/window.rs
customize = Anpassen

# src/settings_window/window.rs
limit-activation = Sitzungsaktivierung

# src/settings_window/window.rs
tray = Infobereich

# src/settings_window/window.rs
notifications = Benachrichtigungen

# src/settings_window/window.rs
advanced = Erweitert

# src/settings_window/window.rs
log = Protokoll

# src/settings_window/window.rs
integrations = Integrationen

# src/settings_window/nav.rs
about-updates = Info und Updates

# src/settings_window/onboarding.rs
successful-activations = Erfolgreiche Aktivierungen

# src/settings_window/onboarding.rs
failed-activations = Fehlgeschlagene Aktivierungen

# src/settings_window/onboarding.rs
when-limits-reset = Wenn Grenzwerte zurückgesetzt werden

# src/settings_window/onboarding.rs
when-5-hour-remaining-hits = Wenn im 5-Stunden-Kontingent noch { $v0 }% übrig sind

# src/settings_window/onboarding.rs
when-weekly-remaining-hits = Wenn im Wochenkontingent noch { $v0 }% übrig sind

# src/settings_window/onboarding.rs
low-usage = Wenig Restkontingent

# src/settings_window/notifications.rs
shows-a-notification-once-per-window-when-the-remaining-share-dro = Zeigt pro Nutzungszeitraum einmal eine Benachrichtigung, wenn das Restkontingent auf den Schwellenwert sinkt.

# src/settings_window/notifications.rs
threshold = Schwelle

# src/settings_window/onboarding.rs
found-in-opencode-auth-or-local-history = Gefunden in der OpenCode-Authentifizierung oder im lokalen Verlauf.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-set-up-elsewhere = Nicht gefunden. Schalten Sie es ein, wenn es woanders eingerichtet ist.

# src/settings_window/onboarding.rs
account-credentials-are-already-set = Die Anmeldeinformationen für das Konto sind bereits festgelegt.

# src/settings_window/onboarding.rs
optional-add-accounts-later-in-providers = Optional. Fügen Sie Konten später in „Anbieter“ hinzu.

# src/settings_window/onboarding.rs
found-an-official-agy-sign-in-on-this-pc = Auf diesem PC wurde eine offizielle Agy-Anmeldung gefunden.

# src/settings_window/onboarding.rs
not-found-sign-in-with-agy-before-enabling-it = Nicht gefunden. Melden Sie sich mit agy an, bevor Sie es aktivieren.

# src/settings_window/onboarding.rs
found-an-official-grok-cli-sign-in-on-this-pc = Auf diesem PC wurde eine offizielle Grok CLI-Anmeldung gefunden.

# src/settings_window/onboarding.rs
not-found-run-grok-login-before-enabling-it = Nicht gefunden. Führen Sie grok login aus, bevor Sie es aktivieren.

# src/settings_window/onboarding.rs
found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Gefunden: Kiro IDE, Kiro Crew oder ein angemeldeter Kiro CLI.

# src/settings_window/onboarding.rs
not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = Nicht gefunden. Installieren Sie Kiro IDE oder Kiro Crew oder melden Sie sich bei Kiro CLI an.

# src/settings_window/onboarding.rs
found-on-this-pc = Auf diesem PC gefunden.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-installed-somewhere-else = Nicht gefunden. Schalten Sie es ein, wenn es woanders installiert ist.

# src/settings_window/onboarding.rs
setup-could-not-be-saved = Setup konnte nicht gespeichert werden

# src/settings_window/onboarding.rs
detected = Erkannt

# src/settings_window/onboarding.rs
starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = Startet eine neue Codex- oder Claude-Sitzung, sobald ein Nutzungskontingent verfügbar ist, ohne auf die erste Anfrage zu warten.

# src/settings_window/onboarding.rs
collect-usage-data = Sammeln Sie Nutzungsdaten

# src/settings_window/onboarding.rs
scans-local-provider-history-for-usage-stats = Durchsucht den Verlauf des lokalen Anbieters nach Nutzungsstatistiken.

# src/settings_window/onboarding.rs
startup = Autostart

# src/settings_window/providers/page.rs
features = Funktionen

# src/settings_window/onboarding.rs
customization = Anpassung

# src/settings_window/onboarding.rs
when-a-new-version-is-found = Wenn eine neue Version gefunden wird

# src/settings_window/onboarding.rs
activity = Aktivität

# src/settings_window/onboarding.rs
choose-providers = Anbieter auswählen
choose-a-theme = Theme auswählen
theme-step-description = Wählen Sie die Farben für die Einstellungen und das Pop-up. Das Pop-up zeigt jede Auswahl sofort an.

# src/settings_window/onboarding.rs
we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = Wir haben die auf diesem PC gefundenen Anbieter aktiviert. Sie können dies später ändern.

# src/settings_window/onboarding.rs
general-settings = Allgemeine Einstellungen

# src/settings_window/onboarding.rs
you-can-change-these-later-in-settings = Sie können diese später in den Einstellungen ändern.

# src/settings_window/onboarding.rs
turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = Schalten Sie alles aus, worüber Sie nichts hören möchten. Sie können diese später in den Einstellungen ändern.

# src/settings_window/window.rs
back = Zurück

# src/settings_window/onboarding.rs
continue = Weiter

# src/settings_window/tray.rs
done = Fertig

# src/settings_window/providers/dialog.rs
details = Einzelheiten

# src/settings_window/providers/dialog.rs
added = Hinzugefügt

# src/settings_window/providers/dialog.rs
reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Liest die Sitzungs- und Wochenlimits eines Claude-Abonnements ohne Claude Code-Login.

# src/settings_window/providers/dialog.rs
msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. Öffnen Sie Claude in einem separaten Browserprofil oder privaten Fenster. Melden Sie sich bei dem Konto an, das Sie verfolgen möchten, und bestätigen Sie dessen E-Mail-Adresse in den Einstellungen von Claude.

# src/settings_window/providers/dialog.rs
open-claude-ai = claude.ai öffnen

# src/settings_window/providers/dialog.rs
msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. Drücken Sie in Chrome oder Edge F12. Öffnen Sie Anwendung > Speicher > Cookies und wählen Sie https://claude.ai.

# src/settings_window/providers/dialog.rs
msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. Suchen Sie sessionKey. Kopieren Sie seinen Wert, nicht seinen Namen oder die gesamte Cookie-Tabelle, und fügen Sie ihn in das Feld oben ein.

# src/settings_window/providers/dialog.rs
how-to-view-cookies-in-chrome = So zeigen Sie Cookies in Chrome an

# src/settings_window/providers/dialog.rs
a-cookie-header-containing-sessionkey-also-works-when-the-session = Ein Cookie-Header mit sessionKey funktioniert ebenfalls. Wenn die Sitzung abläuft, fügen Sie hier eine neue ein.

# src/settings_window/providers/dialog.rs
use-the-access-token-from-a-claude-code-subscription-login-miniba = Verwenden Sie das Zugriffstoken einer Claude Code-Abonnementanmeldung. Minibar kann ein eingefügtes Token nicht erneuern. Verwenden Sie nach Möglichkeit „Quelle: Konfigurationsordner“.

# src/settings_window/providers/dialog.rs
copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = Kopieren Sie nur claudeAiOauth.accessToken (beginnt mit sk-ant-oat) aus der .credentials.json dieser Anmeldung, ohne Anführungszeichen. Verwenden Sie nicht claude setup-token: Diesem Token kann der Zugriff auf Nutzungsdaten fehlen.

# src/settings_window/providers/dialog.rs
claude-code-log-in-with-multiple-accounts = Claude Code: Melden Sie sich mit mehreren Konten an

# src/settings_window/providers/dialog.rs
requires-a-claude-subscription-login-with-usage-access-api-keys-a = Erfordert einen Claude-Abonnement-Login mit Nutzungszugriff. Für API-Schlüssel und Admin-API-Schlüssel werden keine Abonnementlimits angezeigt.

# src/settings_window/providers/dialog.rs
this-provider = diesen Anbieter

# src/settings_window/providers/dialog.rs
track-another-account-or-a-provider-minibar-has-not-shown-yet = Ein weiteres Konto oder einen bisher nicht in Minibar angezeigten Anbieter verfolgen.

# src/settings_window/providers/dialog.rs
e-g-work = z. B. Arbeit

# src/settings_window/providers/page.rs
name = Name

# src/settings_window/providers/dialog.rs
shown-on-its-tab-home-card-tray-and-notifications = Wird im Tab, auf der Startseitenkarte, im Infobereich und in Benachrichtigungen angezeigt.

# src/settings_window/providers/page.rs
badge = Badge

# src/settings_window/providers/dialog.rs
auto = Automatisch

# src/settings_window/providers/page.rs
badge-color = Badge-Farbe

# src/settings_window/providers/dialog.rs
up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = Bis zu drei Buchstaben. Ein leeres Feld verwendet die Initialen des Namens. Badges erscheinen, wenn mehrere Instanzen eines Anbieters aktiviert sind.

# src/settings_window/window.rs
add-provider = Anbieter hinzufügen

# src/settings_window/providers/dialog.rs
next = Weiter

# src/settings_window/providers/dialog.rs
minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar liest { $name } nicht mehr und entfernt die gespeicherten Schlüssel, Zeitpläne, Anzeigen im Infobereich und die Position auf der Startseite.{ $v0 }

# src/settings_window/providers/dialog.rs
its-config-folder-stays-on-disk = { " " }Der Konfigurationsordner bleibt auf der Festplatte.

# src/settings_window/providers/dialog.rs
delete-name = { $name } löschen?

# src/settings_window/providers/dialog.rs
delete = Löschen

# src/settings_window/providers/dialog.rs
finish-signing-in-in-your-browser-cancel-stops-this-login = Schließen Sie die Anmeldung in Ihrem Browser ab. Abbrechen stoppt diese Anmeldung.

# src/settings_window/providers/dialog.rs
runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = Führt Claude Codes eigene Anmeldung für { $name } mit seinem Konfigurationsordner als CLAUDE_CONFIG_DIR aus. Die Anmeldung bleibt in diesem Ordner, wo Claude Code sie aktuell hält. Erfordert natives Windows Claude Code.

# src/settings_window/providers/dialog.rs
runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = Führt Codexs eigene Anmeldung für { $name } mit seinem Konfigurationsordner als CODEX_HOME aus. Die Anmeldung bleibt in diesem Ordner, wo Codex sie aktuell hält. Erfordert die native Codex CLI oder Desktop-App.

# src/settings_window/providers/dialog.rs
sign-in-to-name = Melden Sie sich bei { $name } an

# src/settings_window/providers/page.rs
sign-in = Anmelden

# src/settings_window/providers/dialog.rs
for = Für { $v0 }.

# src/settings_window/providers/dialog.rs
browser-session = Browsersitzung

# src/settings_window/providers/dialog.rs
oauth-token = OAuth-Token

# src/settings_window/providers/dialog.rs
session-key = Sitzungsschlüssel

# src/settings_window/providers/dialog.rs
paste-the-sessionkey-value = Fügen Sie den sessionKey-Wert ein

# src/settings_window/providers/dialog.rs
oauth-access-token = OAuth Zugriffstoken

# src/settings_window/providers/dialog.rs
the-saved-credential-is-replaced-only-after-the-new-one-passes-th = Der gespeicherte Berechtigungsnachweis wird erst ersetzt, nachdem der neue die Prüfung bestanden hat.

# src/settings_window/providers/dialog.rs
claude-credential = Claude-Anmeldeinformationen

# src/settings_window/providers/dialog.rs
check-and-save = Prüfen und speichern

# src/settings_window/providers/dialog.rs
key-name-optional = Schlüsselname (optional)

# src/settings_window/providers/dialog.rs
e-g-personal = z. B. Privat

# src/settings_window/providers/dialog.rs
leave-blank-to-use-the-name-from-openrouter = Lassen Sie das Feld leer, um den Namen von OpenRouter zu verwenden.

# src/settings_window/providers/dialog.rs
minibar-checks-the-key-with-openrouter-before-saving-it = Minibar prüft den Schlüssel vor dem Speichern bei OpenRouter.

# src/settings_window/providers/dialog.rs
replace-api-key = Ersetzen Sie den API-Schlüssel

# src/settings_window/providers/page.rs
add-api-key = Fügen Sie den Schlüssel API hinzu

# src/settings_window/providers/page.rs
management-key = Verwaltungsschlüssel

# src/settings_window/providers/dialog.rs
create-one-under-settings-management-keys-on-openrouter-ai = Erstellen Sie einen unter Einstellungen → Verwaltungsschlüssel auf openrouter.ai.

# src/settings_window/providers/dialog.rs
replace-management-key = Ersetzen Sie den Verwaltungsschlüssel

# src/settings_window/providers/page.rs
add-management-key = Verwaltungsschlüssel hinzufügen

# src/settings_window/providers/dialog.rs
key-name = Schlüsselname

# src/settings_window/providers/page.rs
rename-key = Schlüssel umbenennen

# src/settings_window/providers/dialog.rs
save = Speichern

# src/settings_window/providers/dialog.rs
this-key = diesen Schlüssel

# src/settings_window/providers/dialog.rs
minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibar hört auf, { $hint } zu verfolgen. Der Schlüssel funktioniert weiterhin auf OpenRouter.

# src/settings_window/providers/dialog.rs
remove-api-key = API-Schlüssel entfernen?

# src/settings_window/tray.rs
remove = Entfernen

# src/settings_window/providers/dialog.rs
minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibar zeigt den Guthabenstand und den Nutzungsverlauf für { $v0 } nicht mehr an. Der Schlüssel funktioniert weiterhin auf OpenRouter.

# src/settings_window/providers/dialog.rs
remove-management-key = Verwaltungsschlüssel entfernen?

# src/settings_window/providers/dialog.rs
saved-in-windows-user-storage-never-in-the-settings-file = Wird im Windows-Benutzerspeicher gespeichert, niemals in der Einstellungsdatei.

# src/settings_window/providers/dialog.rs
save-key = Schlüssel speichern

# src/settings_window/providers/dialog.rs
minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibar vergisst den gespeicherten Schlüssel von { $v0 }. Es funktioniert weiterhin mit OpenCode.

# src/settings_window/providers/dialog.rs
signing-in = Anmelden…

# src/settings_window/providers/page.rs
checking = Überprüfen…

# src/settings_window/providers/dialog.rs
choose-a-provider = Wählen Sie einen Anbieter.

# src/settings_window/providers/dialog.rs
reason-it-is-already-in-the-list = { $reason }. Es ist bereits in der Liste.

# src/settings_window/providers/dialog.rs
added-name = { $name } hinzugefügt.

# src/settings_window/providers/dialog.rs
could-not-add-the-provider-error = Der Anbieter konnte nicht hinzugefügt werden: { $error }

# src/settings_window/providers.rs
this-provider-no-longer-exists = Dieser Anbieter existiert nicht mehr.

# src/settings_window/providers/dialog.rs
could-not-delete-the-provider-error = Der Anbieter konnte nicht gelöscht werden: { $error }

# src/settings_window/providers/dialog.rs
paste-a-credential-first = Fügen Sie zuerst einen Berechtigungsnachweis ein.

# src/settings_window/providers/dialog.rs
credential-saved-in-windows-user-storage = Anmeldeinformationen werden im Windows-Benutzerspeicher gespeichert.

# src/settings_window/providers/dialog.rs
paste-a-key-first = Fügen Sie zuerst einen Schlüssel ein.

# src/settings_window/providers/dialog.rs
this-account-no-longer-exists = Dieses Konto existiert nicht mehr.

# src/settings_window/providers/dialog.rs
api-key-saved-in-windows-user-storage = API-Schlüssel im Windows-Benutzerspeicher gespeichert.

# src/settings_window/providers/dialog.rs
management-key-replaced = Verwaltungsschlüssel ersetzt.

# src/settings_window/providers/dialog.rs
management-key-added = Verwaltungsschlüssel hinzugefügt.

# src/settings_window/providers/dialog.rs
could-not-rename-the-key-error = Der Schlüssel konnte nicht umbenannt werden: { $error }

# src/settings_window/providers/dialog.rs
api-key-renamed = API-Schlüssel umbenannt.

# src/settings_window/providers/dialog.rs
could-not-remove-the-key-error = Der Schlüssel konnte nicht entfernt werden: { $error }

# src/settings_window/providers/dialog.rs
api-key-removed = API-Schlüssel entfernt.

# src/settings_window/providers/dialog.rs
management-key-removed = Verwaltungsschlüssel entfernt.

# src/settings_window/providers/dialog.rs
could-not-save-the-key-error = Der Schlüssel konnte nicht gespeichert werden: { $error }

# src/settings_window/providers/dialog.rs
api-key-saved = API-Schlüssel gespeichert.

# src/settings_window/providers/dialog.rs
switch-source-to-config-folder-to-sign-in = Stellen Sie „Quelle“ auf „Konfigurationsordner“ um, um sich anzumelden.

# src/settings_window/providers/dialog.rs
this-provider-has-no-config-folder = Dieser Anbieter hat keinen Konfigurationsordner.

# src/settings_window/providers/dialog.rs
another-instance-already-reads-this-config-folder-choose-a-differ = Eine andere Instanz liest diesen Konfigurationsordner bereits. Wählen Sie zunächst einen anderen Ordner.

# src/settings_window/providers/dialog.rs
this-provider-has-no-sign-in = Dieser Anbieter hat keine Anmeldung.

# src/settings_window/providers/dialog.rs
signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 } hat sich angemeldet. Sein CLI behält die Anmeldung in seinem Konfigurationsordner auf dem neuesten Stand.

# src/settings_window/providers/page.rs
no-keys-yet = Noch keine Schlüssel

# src/settings_window/providers/page.rs
management-key-5c8cf2 = { " " }· Verwaltungsschlüssel

# src/settings_window/providers/page.rs
using-a-saved-credential = Verwendung eines gespeicherten Berechtigungsnachweises

# src/settings_window/providers/page.rs
paste-a-credential-under-account = Fügen Sie unter „Konto“ Anmeldeinformationen ein

# src/settings_window/providers/page.rs
needs-an-api-key-or-opencode-sign-in = Erfordert einen API-Schlüssel oder eine OpenCode-Anmeldung

# src/settings_window/providers/page.rs
needs-an-api-key = Benötigt einen API-Schlüssel

# src/settings_window/providers/page.rs
not-found-set-its-folder-under-runtime = Nicht gefunden. Legen Sie den Ordner unter Runtime fest.

# src/settings_window/providers/page.rs
using-a-saved-api-key = Verwendung eines gespeicherten API-Schlüssels

# src/settings_window/providers/page.rs
using-opencode-sign-in-or-local-history = Verwenden der OpenCode-Anmeldung oder des lokalen Verlaufs

# src/settings_window/providers/page.rs
reading-cli = Lesen von { $cli }

# src/settings_window/providers/page.rs
reading-crew = Lesen von { $crew }

# src/settings_window/providers/page.rs
reading-app = Lesen von { $app }

# src/settings_window/providers/page.rs
checking-kiro-ide-kiro-crew-and-cli = Überprüfen von Kiro IDE, Kiro Crew und CLI…

# src/settings_window/providers/page.rs
checking-installed-app-and-cli = Überprüfen der installierten App und CLI…

# src/settings_window/providers/page.rs
checking-cli = Überprüfung CLI…

# src/settings_window/providers/page.rs
checking-installed-app = Installierte App wird überprüft…

# src/settings_window/providers/page.rs
no-providers-yet-add-one-to-start-reading-limits = Noch keine Anbieter. Fügen Sie einen hinzu, um die Limits zu lesen.

# src/settings_window/providers/page.rs
is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } ist deaktiviert und erscheint daher weder in Minibar noch im Infobereich. Aktivieren Sie den Anbieter, um die Nutzung zu lesen.

# src/settings_window/providers/page.rs
replace-chatgpt-logo-with-codex = Ersetzen Sie das ChatGPT-Logo durch Codex

# src/settings_window/providers/page.rs
limits-only = Nur Limits

# src/settings_window/providers/page.rs
delete-provider = Anbieter löschen

# src/settings_window/providers/page.rs
display-name = Anzeigename

# src/settings_window/providers/page.rs
shown-on-popup-tabs-home-cards-the-tray-and-notifications = Wird in Popup-Tabs, auf Startseitenkarten, im Infobereich und in Benachrichtigungen angezeigt.

# src/settings_window/providers/page.rs
up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = Bis zu drei Buchstaben. Lassen Sie das Feld leer, um die Initialen des Namens zu verwenden. Wird angezeigt, wenn bei einem Anbieter mehr als eine Instanz aktiviert ist.

# src/settings_window/providers/page.rs
auto-uses-a-neutral-plate-that-follows-the-theme = Auto verwendet eine neutrale Platte, die dem Thema folgt.

# src/settings_window/providers/page.rs
show-on-home = Auf Startseite anzeigen

# src/settings_window/providers/page.rs
its-provider-tab-stays-available-when-hidden-from-home = Die Registerkarte „Anbieter“ bleibt verfügbar, wenn sie auf der Startseite ausgeblendet ist.

# src/settings_window/providers/page.rs
credential = Anmeldedaten

# src/settings_window/providers/page.rs
signed-in-as-identity = Angemeldet als { $identity }

# src/settings_window/providers/page.rs
saved-in-windows-user-storage = Im Windows-Benutzerspeicher gespeichert

# src/settings_window/providers/page.rs
paste-a-sessionkey-or-an-oauth-access-token = Fügen Sie einen SessionKey oder ein OAuth-Zugriffstoken ein.

# src/settings_window/providers/page.rs
reads-limits-only-minibar-cannot-refresh-a-pasted-credential = Liest nur die Limits. Minibar kann eingefügte Anmeldedaten nicht erneuern.

# src/settings_window/providers/page.rs
replace-credential = Anmeldedaten ersetzen

# src/settings_window/providers/page.rs
add-credential = Anmeldeinformationen hinzufügen

# src/settings_window/providers/page.rs
signed-in-account = Angemeldetes Konto

# src/settings_window/providers/page.rs
not-signed-in-yet-or-no-limits-read-so-far = Noch nicht angemeldet oder bisher keine Limits gelesen.

# src/settings_window/providers/page.rs
account = Konto

# src/settings_window/providers/page.rs
runtime = Laufzeitumgebung

# src/settings_window/providers/page.rs
minibar-finds-these-automatically = { $v0 } Minibar findet diese automatisch.

# src/settings_window/providers/page.rs
in-use = Im Einsatz

# src/settings_window/providers/page.rs
found = Gefunden

# src/settings_window/providers/page.rs
copy-path = Pfad kopieren

# src/settings_window/providers/page.rs
open-folder = Ordner öffnen

# src/settings_window/providers/page.rs
path-copied = Pfad kopiert.

# src/settings_window/providers/page.rs
not-installed-or-installed-somewhere-minibar-doesn-t-look = Nicht installiert oder an einem Ort installiert, den Minibar nicht durchsucht.

# src/settings_window/providers/page.rs
not-found = Nicht gefunden

# src/settings_window/providers/page.rs
choose-folder = Ordner auswählen…

# src/settings_window/providers/page.rs
select-folder = Ordner auswählen

# src/settings_window/providers/page.rs
choose-folder-1838a4 = Ordner auswählen

# src/settings_window/providers/page.rs
only-needed-if-automatic-detection-misses-your-install = Wird nur benötigt, wenn die automatische Erkennung Ihre Installation übersieht.

# src/settings_window/providers/page.rs
source = Quelle

# src/settings_window/providers/page.rs
config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = Der Konfigurationsordner liest die Claude Code-Anmeldung in CLAUDE_CONFIG_DIR. Bei der manuellen Berechtigung werden nur begrenzt die eingefügten Berechtigungsnachweise gelesen.

# src/settings_window/providers/page.rs
config-folder = Konfigurationsordner

# src/settings_window/providers/page.rs
manual = Handbuch

# src/settings_window/providers/page.rs
passed-to-the-cli-as-env-leave-empty-to-use = Als { $env } an CLI übergeben. Leer lassen, um { $v0 } zu verwenden.

# src/settings_window/providers/page.rs
the-standard-folder = der Standardordner

# src/settings_window/providers/page.rs
a-folder-minibar-creates-for-this-instance = Für diese Instanz erstellt Minibar einen Ordner

# src/settings_window/providers/page.rs
other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } liest diesen Ordner bereits. Zwei Instanzen dürfen keine Anmeldung teilen, sonst wird die Nutzung doppelt gezählt.

# src/settings_window/providers/page.rs
automatic-activation = Automatische Aktivierung

# src/settings_window/providers/page.rs
starts-this-account-s-5-hour-window-when-it-resets-using-its-own = Startet nach dem Reset das 5-Stunden-Kontingent dieses Kontos mit seiner eigenen Anmeldung. Zeitpläne und Pausen finden Sie unter „Sitzungsaktivierung“.

# src/settings_window/providers/page.rs
usage-statistics = Nutzungsstatistiken

# src/settings_window/providers/page.rs
scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = Durchsucht den lokalen Verlauf dieser Instanz für ihre Nutzungskarte. Ausschalten, um die Erfassung vollständig zu beenden.

# src/settings_window/providers/page.rs
opencode-sign-in-or-local-history = OpenCode Anmeldung oder lokaler Verlauf

# src/settings_window/providers/page.rs
found-in-opencode-auth-environment-a-saved-key-or-local-history = Gefunden in der OpenCode-Authentifizierung, der Umgebung, einem gespeicherten Schlüssel oder dem lokalen Verlauf.

# src/settings_window/providers/page.rs
nothing-found-in-opencode-auth-environment-or-local-history = In der OpenCode-Authentifizierung, der Umgebung oder dem lokalen Verlauf wurde nichts gefunden.

# src/settings_window/providers/page.rs
replace-key = Schlüssel ersetzen

# src/settings_window/providers/page.rs
optional-only-needed-without-opencode-sign-in-on-this-pc = Optional. Wird nur ohne OpenCode-Anmeldung auf diesem PC benötigt.

# src/settings_window/providers/page.rs
add-the-key-of-the-account-this-instance-tracks = Fügen Sie den Schlüssel des Kontos hinzu, das diese Instanz verfolgt.

# src/settings_window/providers/page.rs
keys = Schlüssel

# src/settings_window/providers/page.rs
a-management-key-shows-credit-balance-and-usage-history-api-keys = Ein Verwaltungsschlüssel zeigt den Guthabenstand und den Nutzungsverlauf an. API-Schlüssel zeigen die Ausgaben pro Schlüssel an.

# src/settings_window/providers/page.rs
could-not-read-the-saved-key-reopen-this-page-to-retry = Der gespeicherte Schlüssel konnte nicht gelesen werden. Öffnen Sie diese Seite erneut, um es erneut zu versuchen.

# src/settings_window/providers/page.rs
credit-balance-and-account-wide-usage-history = Guthabenstand und kontoweiter Nutzungsverlauf

# src/settings_window/providers/page.rs
not-added-add-one-to-see-credit-balance-and-usage-history = Nicht hinzugefügt. Fügen Sie eines hinzu, um den Guthabenstand und den Nutzungsverlauf anzuzeigen.

# src/settings_window/providers/page.rs
last-updated = Zuletzt aktualisiert { $v0 }, { $v1 }

# src/settings_window/providers/page.rs
no-api-keys-yet-add-one-to-track-spend-per-key = Noch keine API-Schlüssel. Fügen Sie einen hinzu, um die Ausgaben pro Schlüssel zu verfolgen.

# src/settings_window/providers/page.rs
key = Schlüssel

# src/settings_window/providers/page.rs
spend = Ausgaben

# src/settings_window/providers/page.rs
limit = Limit

# src/settings_window/providers/page.rs
error-reopen-this-page-to-retry = { $error }. Öffnen Sie diese Seite erneut, um es erneut zu versuchen.

# src/settings_window/providers/page.rs
could-not-read-key = Schlüssel konnte nicht gelesen werden

# src/settings_window/providers/page.rs
not-saved = Nicht gespeichert

# src/settings_window/providers/page.rs
unnamed = Unbenannt

# src/settings_window/providers/page.rs
of-the-limit = { $v0 } des { $v1 }-Limits

# src/settings_window/providers/page.rs
no-spend-limit-key-spend-account-credits-purchased = Kein Ausgabenlimit. Ausgaben dieses Schlüssels: { $v0 }. Gekaufte Kontocredits: { $v1 }.

# src/settings_window/providers/page.rs
none = Keine

# src/settings_window/providers/page.rs
add-key = Schlüssel hinzufügen

# src/settings_window/providers.rs
openrouter-account-credentials-are-configured = Die Anmeldeinformationen des OpenRouter-Kontos sind konfiguriert

# src/settings_window/providers.rs
saved-api-key = API-Schlüssel gespeichert

# src/settings_window/providers.rs
saved-credential = Anmeldedaten gespeichert

# src/settings_window/providers.rs
opencode-auth-json-or-local-database = OpenCode auth.json oder lokale Datenbank

# src/settings_window/providers.rs
that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = Das sieht nicht wie ein OpenRouter-Schlüssel aus. Schlüssel beginnen mit sk-or-.

# src/settings_window/providers.rs
codex-cli-folder = Ordner Codex CLI

# src/settings_window/providers.rs
claude-code-cli-folder = Ordner Claude Code CLI

# src/settings_window/providers.rs
cursor-app-folder = Cursor App-Ordner

# src/settings_window/providers.rs
grok-cli-folder = Ordner Grok CLI

# src/settings_window/providers.rs
kiro-ide-folder = Ordner Kiro IDE

# src/settings_window/providers.rs
kiro-crew-app-path = Kiro Crew App-Pfad

# src/settings_window/providers.rs
kiro-cli-folder = Ordner Kiro CLI

# src/settings_window/providers.rs
reads-the-signed-in-codex-cli-or-desktop-app = Liest die angemeldete Codex CLI oder Desktop-App.

# src/settings_window/providers.rs
reads-your-existing-claude-code-login = Liest Ihr bestehendes Claude Code-Login.

# src/settings_window/providers.rs
reads-the-signed-in-cursor-app-for-this-billing-cycle = Liest die angemeldete Cursor-App für diesen Abrechnungszyklus.

# src/settings_window/providers.rs
reads-zen-auth-and-local-opencode-history = Liest die Zen-Authentifizierung und den lokalen OpenCode-Verlauf.

# src/settings_window/providers.rs
reads-go-quota-windows-and-local-opencode-history = Liest Go-Kontingentfenster und den lokalen OpenCode-Verlauf.

# src/settings_window/providers.rs
reads-api-key-usage-and-spend-limits-a-management-key-also-enable = Liest API-Schlüsselnutzungs- und Ausgabenlimits. Ein Verwaltungsschlüssel ermöglicht außerdem den Nutzungsverlauf und den Guthabenstand.

# src/settings_window/providers.rs
reads-subscription-quota-from-your-existing-official-agy-windows = Liest das Abonnementkontingent aus Ihrer bestehenden offiziellen agy Windows-Anmeldung.

# src/settings_window/providers.rs
reads-supergrok-subscription-credits-from-your-existing-official = Liest SuperGrok-Abonnementguthaben aus Ihrer bestehenden offiziellen Grok CLI-Anmeldung.

# src/settings_window/providers.rs
fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = Ruft die monatlichen Live-Credits von Kiro mit der gemeinsamen Anmeldung ab; erkennt IDE-, Crew- und CLI-Installationen.

# src/settings_window/providers.rs
codex-desktop-app = Codex Desktop-App

# src/settings_window/providers.rs
codex-cli = Codex CLI

# src/settings_window/providers.rs
claude-desktop-app = Claude Desktop-App

# src/settings_window/providers.rs
claude-code-cli = Claude Code CLI

# src/settings_window/providers.rs
cursor-app = Cursor-App

# src/settings_window/providers.rs
antigravity-app = Antigravity-App

# src/settings_window/providers.rs
grok-cli = Grok CLI

# src/settings_window/providers.rs
kiro-ide = Kiro IDE

# src/settings_window/providers.rs
kiro-crew = Kiro Crew

# src/settings_window/providers.rs
kiro-cli = Kiro CLI

# src/settings_window/providers.rs
supports-one-instance = { $v0 } unterstützt eine Instanz

# src/settings_window/tray.rs
numbers = Zahlen

# src/settings_window/tray.rs
progress-bars = Fortschrittsbalken

# src/settings_window/tray.rs
rings = Ringe

# src/settings_window/tray.rs
reset-time = Reset-Zeitpunkt

# src/settings_window/tray.rs
countdown = Countdown

# src/settings_window/tray.rs
status = Status

# src/settings_window/tray.rs
fixed = Fest

# src/settings_window/tray.rs
app-accent = App-Akzent

# src/settings_window/tray.rs
percentages-as-digits = Prozentangaben als Ziffern

# src/settings_window/tray.rs
one-bar-per-indicator = Ein Balken pro Indikator

# src/settings_window/tray.rs
nested-rings-one-per-indicator = Ineinander verschachtelte Ringe, einer pro Anzeige

# src/settings_window/tray.rs
when-the-limit-resets = Wenn das Limit zurückgesetzt wird

# src/settings_window/tray.rs
time-left-until-the-reset = Verbleibende Zeit bis zum Zurücksetzen

# src/settings_window/tray.rs
used = Verbraucht

# src/settings_window/tray.rs
remaining = Verbleibend

# src/settings_window/tray.rs
app-icon = App-Symbol

# src/settings_window/tray.rs
widget = Widget { $v0 }

# src/settings_window/tray.rs
the-codex-minibar-icon = Das Codex Minibar-Symbol

# src/settings_window/tray.rs
style-no-indicators = { $style } · Keine Indikatoren

# src/settings_window/tray.rs
widget-removed = Widget entfernt

# src/settings_window/tray.rs
undo = Rückgängig machen

# src/settings_window/tray.rs
tray-icon = Tray-Symbol

# src/settings_window/tray.rs
shows-the-app-icon = Zeigt das App-Symbol an.

# src/settings_window/tray.rs
add-widget = Widget hinzufügen

# src/settings_window/tray.rs
add-app-icon = App-Symbol hinzufügen

# src/settings_window/tray.rs
move-up = Nach oben

# src/settings_window/tray.rs
move-down = Nach unten

# src/settings_window/tray.rs
edit = Bearbeiten

# src/settings_window/tray.rs
duplicate = Duplizieren

# src/settings_window/tray.rs
shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = Zeigt das Codex Minibar-Symbol im Benachrichtigungsbereich an. Es müssen keine Indikatoren eingerichtet werden.

# src/settings_window/tray.rs
style = Stil

# src/settings_window/tray.rs
up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = Bis zu { $max_indicators } Kontingente, in dieser Reihenfolge angezeigt. Klappen Sie eines auf, um es zu ändern.

# src/settings_window/tray.rs
indicators = Indikatoren

# src/settings_window/tray.rs
remove-widget = Widget entfernen

# src/settings_window/tray.rs
add-indicator = Indikator hinzufügen

# src/settings_window/tray.rs
unsupported = Nicht unterstützt { $v0 }

# src/settings_window/tray.rs
remove-indicator = Indikator entfernen

# src/settings_window/tray.rs
unsupported-bba2a8 = Nicht unterstützt ({ $v0 })

# src/settings_window/tray.rs
unavailable-0c5d75 = Nicht verfügbar ({ $v0 })

# src/settings_window/tray.rs
metric = Kennzahl

# src/settings_window/tray.rs
show = Zeigen

# src/settings_window/tray.rs
color = Farbe

# src/settings_window/tray.rs
fixed-color = Feste Farbe

# src/settings_window/troubleshoot.rs
troubleshooting-could-not-start = Die Fehlerbehebung konnte nicht gestartet werden

# src/settings_window/troubleshoot.rs
choose-which-installed-ai-tool-should-investigate-the-problem = Wählen Sie aus, welches installierte KI-Tool das Problem untersuchen soll.

# src/settings_window/troubleshoot.rs
open-terminal = Terminal öffnen

# src/settings_window/window.rs
version-is-available = { $version } ist verfügbar

# src/settings_window/window.rs
update-now = Jetzt aktualisieren

# src/settings_window/window.rs
missing-a-provider = Fehlt Ihnen ein Anbieter?

# src/settings_window/window.rs
ask-for-it-or-build-it-yourself = Fordern Sie es an oder bauen Sie es selbst.

# src/settings_window/window.rs
request-a-provider = Anbieter vorschlagen

# src/settings_window/window.rs
contribute-one = Anbieter beisteuern

# src/tray.rs
disabled = Deaktiviert

# src/updater.rs
update-complete = Update abgeschlossen

# src/updater.rs
now-running-version = Läuft jetzt { $version }.

# src/usage_overview.rs
past-24h = Letzte 24 Stunden

# application
language = Sprache

# application
applies-immediately-to-every-app-window-and-notification-auto-fol = Gilt sofort für alle App-Fenster und Benachrichtigungen. „Automatisch“ folgt der Windows-Anzeigesprache.

# application
auto-windows = Automatisch (Windows)

# application
english = Englisch

# application
msg-russian = Russisch

# application
codex-minibar-settings = Codex Minibar – Einstellungen

# application
welcome-to-codex-minibar = Willkommen bei Codex Minibar

# application
exit = Beenden

# application
update-available-67fd3a = Update verfügbar

# application
monthly-credits-976559 = Monatliche Credits

# application
msg-5h-session-de7ce8 = 5-Stunden-Sitzung

# application
name-weekly = { $name } wöchentlich

# application
tokens-94e0b9 = { $v0 }: { $v1 } Token

# application
requests-priced = { $v0 ->
    [one] { $v0 } Anfrage · { $v1 } mit Preis
   *[other] { $v0 } Anfragen · { $v1 } mit Preis
    }

# application
requests = { $v0 ->
    [one] { $v0 } Anfrage
   *[other] { $v0 } Anfragen
    }

# application
credits-338f52 = CREDITS

# application
cloud-session-credits = CREDITS FÜR CLOUD-SITZUNGEN

# application
name-login-expires-in-days-left = { $days_left ->
    [one] Die Anmeldung für { $name } läuft in { $days_left } Tag ab
   *[other] Die Anmeldung für { $name } läuft in { $days_left } Tagen ab
    }

# application
name-error = { $name }-Fehler

# application
sessions = { $v0 ->
    [one] { $v0 } Sitzung
   *[other] { $v0 } Sitzungen
    }

# application
api-estimate = Geschätzte API-Kosten

# application
start-to-end = { $start } bis { $end }

# application
to = { $v0 } bis { $v1 }

# application
share-1-of-other = { $share }% von { $v0 } · { $other }

# application
excluded-other = Ausgeschlossen · { $other }

# application
exclude-from-usage-stats = Klicken Sie hier, um aus den Nutzungsstatistiken auszuschließen

# application
include-in-usage-stats = Klicken Sie hier, um in die Nutzungsstatistiken einzubeziehen

# application
cost-885dc4 = Kosten

# application
tokens-339143 = Token

# application
sessions-0e5e29 = { $v0 ->
    [one] · { $v0 } Sitzung
   *[other] · { $v0 } Sitzungen
    }

# application
api-key = API-Schlüssel

# application
api-keys = API-Schlüssel

# application
name-deleted = { $name } gelöscht.

# application
openrouter-api-key-no-longer-exists = Der Schlüssel OpenRouter API existiert nicht mehr

# application
openrouter-account-no-longer-exists = Das OpenRouter-Konto existiert nicht mehr

# application
credit = { $v0 } Credits

# application
agy-cli-folder = agy CLI-Ordner

# application
folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = Ordner mit codex.exe, codex.cmd oder codex.ps1. Lassen Sie das Feld leer, damit es automatisch gefunden wird.

# application
folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = Ordner mit claude.exe, claude.cmd oder claude.ps1. Lassen Sie das Feld leer, damit es automatisch gefunden wird.

# application
folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = Ordner mit Cursor.exe. Lassen Sie das Feld leer, damit es automatisch gefunden wird. Die Nutzung erfolgt weiterhin über das angemeldete Profil.

# application
folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = Ordner mit agy.exe, agy.cmd oder agy.ps1. Lassen Sie das Feld leer, damit es automatisch gefunden wird.

# application
folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = Ordner mit grok.exe, grok.cmd oder grok.ps1. Lassen Sie das Feld leer, damit es automatisch gefunden wird.

# application
folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = Ordner, der Kiro.exe oder die ausführbare Datei selbst enthält. Lassen Sie das Feld leer, damit es automatisch gefunden wird.

# application
folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = Ordner, der KiroCrew.exe oder die ausführbare Datei selbst enthält. Lassen Sie das Feld leer, um Installationen pro Benutzer und für alle Benutzer automatisch zu erkennen.

# application
folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = Ordner, der kiro-cli.exe oder die ausführbare Datei selbst enthält. Lassen Sie das Feld leer, damit es automatisch gefunden wird.

# application
off-45080e = { $v0 } (aus)

# application
each-widget-is-one-icon-in-the-notification-area-indicators-show = Jedes Widget ist ein Symbol im Infobereich. Indikatoren zeigen das Kontingent eines Anbieters als Zahlen, Balken, Ringe oder Reset-Uhr an.

# application
a-reset-clock-follows-one-quota = Eine Reset-Uhr zeigt ein einzelnes Kontingent an.

# application
ai-tool = KI-Tool

# application
msg-7-days = 7 Tage

# application
msg-30-days = 30 Tage

# application
msg-90-days = 90 Tage

# application
yellow = Gelb

# application
custom = Benutzerdefiniert

# application
not-available-for-manual-credentials-switch-source-to-config-fold = Bei manuellen Anmeldedaten nicht verfügbar. Stellen Sie „Quelle“ auf „Konfigurationsordner“ um.

# application
opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = Der lokale Verlauf von OpenCode wird von der ersten OpenCode-Instanz verfolgt.

# application
this-provider-has-no-local-usage-history = Dieser Anbieter verfügt über keine lokale Nutzungshistorie.

# application
this-provider-has-no-session-window-to-start = Dieser Anbieter hat kein Sitzungsfenster zum Starten.

# application
this-provider-does-not-use-a-config-folder = Dieser Anbieter verwendet keinen Konfigurationsordner.

# application
could-not-save-provider-settings-error-restoring-its-previous-cre = Anbietereinstellungen konnten nicht gespeichert werden ({ $error }); Das Wiederherstellen der vorherigen Anmeldeinformationen ist ebenfalls fehlgeschlagen ({ $rollback_error }).

# application
msg-5h-7d = 5h |  { $v0 } |  { $v1 }
    7d |  { $v2 } |  { $v3 }

# API key count in the provider header.
api-key-count = { $v0 ->
    [one] { $v0 } API-Schlüssel
   *[other] { $v0 } API-Schlüssel
    }

# Application copy
widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = Startseite und Nutzung verbreitern. Ziehen Sie Startseitenblöcke zwischen den Spalten; Anbieter-Tabs bleiben kompakt.

# Application copy
a-possible-codex-reset-is-scheduled-for-when-in-countdown = Ein möglicher Codex-Reset ist für { $when } geplant (in { $countdown })

# Application copy
label-possible-reset-on-when-in-countdown = { $label }: möglicher Reset am { $when } (in { $countdown })

# Application copy
new-codex-reset-info = Neue Codex-Reset-Informationen

# Application copy
login-expires-soon = { $v0 }-Login läuft bald ab

# Application copy
it-stops-renewing-on-open-minibar-and-choose-sign-in-again = Die automatische Erneuerung endet am { $v0 }. Öffnen Sie Minibar und wählen Sie „Erneut anmelden“.

# Application copy
could-not-clear-usage-data-error = Nutzungsdaten konnten nicht gelöscht werden: { $error }

# Application copy
succeeded-at = { $v0 } war bei { $v1 } erfolgreich

# Application copy
failed-at-error = { $v0 } ist bei { $v1 } fehlgeschlagen: { $error }

# Application copy
key-39df89 = Schlüssel { $v0 }

# Application copy
expired = Abgelaufen

# Application copy
jan = Jan.

# Application copy
feb = Feb.

# Application copy
mar = März

# Application copy
apr = Apr.

# Application copy
may = Mai

# Application copy
jun = Juni

# Application copy
jul = Juli

# Application copy
aug = Aug.

# Application copy
sep = Sept.

# Application copy
oct = Okt.

# Application copy
nov = Nov.

# Application copy
dec = Dez.

# { $value }% used
quota-percent-used = { $value }% verbraucht

# { $value }% left
quota-percent-left = { $value }% verbleibend

# { $amount } of { $limit } used
cloud-amount-used = { $amount } von { $limit } verbraucht

# { $amount } of { $limit } left
cloud-amount-left = { $amount } von { $limit } verbleibend

# Short usage range
range-24h = 24 Std.

# Short usage range
range-7d = 7 T

# Short usage range
range-30d = 30 T

# Short usage range
range-90d = 90 T

# Home usage card title
home-usage-title = Nutzung

# OpenRouter key administration

could-not-reach-openrouter = OpenRouter konnte nicht erreicht werden


openrouter-keys-title = Schlüssel


openrouter-keys-new = Neuer Schlüssel


openrouter-keys-new-title = Neuer Schlüssel


openrouter-keys-back = Zurück


openrouter-keys-updating = Aktualisierung…


openrouter-keys-loading = Schlüssel werden geladen…


openrouter-keys-load-failed = Schlüssel konnten nicht geladen werden


openrouter-keys-retry = Versuchen Sie es noch einmal


openrouter-keys-empty = Dieses Konto hat noch keine Schlüssel.


openrouter-keys-this-app = Diese App


openrouter-keys-no-limit = Keine Begrenzung


openrouter-keys-of-limit = { $amount } von { $limit }


openrouter-keys-resets-daily = wird täglich zurückgesetzt


openrouter-keys-resets-weekly = Wird wöchentlich zurückgesetzt


openrouter-keys-resets-monthly = Wird monatlich zurückgesetzt


openrouter-keys-expires-on = läuft ab { $date }


openrouter-keys-expired-on = abgelaufen { $date }


openrouter-keys-today = heute { $amount }


openrouter-keys-usage-breakdown = Heute { $today } · Woche { $week } · Monat { $month } · Gesamt { $total }


openrouter-keys-show-all = Alle { $count } Schlüssel anzeigen


openrouter-keys-show-fewer = Weniger anzeigen


openrouter-keys-spending-limit = Ausgabenlimit


openrouter-keys-limit-hint = Für keine Begrenzung leer lassen.


openrouter-keys-resets = Zurücksetzen


openrouter-keys-reset-never = Nie


openrouter-keys-reset-daily = Täglich


openrouter-keys-reset-weekly = Wöchentlich


openrouter-keys-reset-monthly = Monatlich


openrouter-keys-byok = Zählen Sie die BYOK-Nutzung zum Limit


openrouter-keys-enabled = Aktiviert


openrouter-keys-delete = Löschen


openrouter-keys-delete-tracked = Diese App verwendet diesen Schlüssel. Entfernen Sie ihn zuerst in den Einstellungen.


openrouter-keys-saving = Wird gespeichert…


openrouter-keys-delete-title = „{ $name }“ löschen?


openrouter-keys-delete-message = Alles, was diesen Schlüssel verwendet, funktioniert sofort nicht mehr. Das Löschen kann nicht rückgängig gemacht werden; das Deaktivieren des Schlüssels dagegen schon.


openrouter-keys-keep = Schlüssel behalten


openrouter-keys-delete-confirm = Schlüssel löschen


openrouter-keys-deleting = Löschen…


openrouter-keys-name = Name


openrouter-keys-name-placeholder = z. B. laptop-cursor


openrouter-keys-name-required = Geben Sie einen Namen für den Schlüssel ein.


openrouter-keys-invalid-amount = Geben Sie einen Dollarbetrag wie 25 oder 12,50 ein.


openrouter-keys-expires = Ablaufdatum


openrouter-keys-expires-1-hour = 1 Stunde


openrouter-keys-expires-1-day = 1 Tag


openrouter-keys-expires-7-days = 7 Tage


openrouter-keys-expires-30-days = 30 Tage


openrouter-keys-expires-90-days = 90 Tage


openrouter-keys-expires-180-days = 180 Tage


openrouter-keys-expires-1-year = 1 Jahr


openrouter-keys-expires-never = Kein Ablauf


openrouter-keys-track = Verfolgen Sie diesen Schlüssel in Minibar


openrouter-keys-create = Schlüssel erstellen


openrouter-keys-creating = Erstellen…


openrouter-keys-created = Schlüssel erstellt


openrouter-keys-copy = Schlüssel kopieren


openrouter-keys-copied = Kopiert


openrouter-keys-done = Fertig


openrouter-keys-once-title = Sie werden diesen Schlüssel nicht mehr sehen


openrouter-keys-once-message = OpenRouter zeigt einen neuen Schlüssel nur einmal an. Kopieren Sie ihn jetzt.


openrouter-keys-pinned = Das Popup bleibt geöffnet, bis Sie auf „Fertig“ klicken.


openrouter-keys-tracking = Schlüssel wird zu Minibar hinzugefügt…


openrouter-keys-tracked = Minibar verfolgt jetzt diesen Schlüssel.


openrouter-keys-track-failed = Schlüssel konnte nicht zu Minibar hinzugefügt werden: { $error }


openrouter-keys-no-management-key = Dieses Konto hat keinen Verwaltungsschlüssel.


openrouter-keys-management-key-rejected = OpenRouter hat den Verwaltungsschlüssel abgelehnt. Ersetzen Sie ihn in den Einstellungen.


openrouter-keys-not-a-management-key = Dieser Schlüssel kann keine anderen Schlüssel verwalten. Verwenden Sie einen Verwaltungsschlüssel.


openrouter-keys-key-not-found = Dieser Schlüssel existiert nicht mehr.

# src/settings_window/notifications.rs
notification-sounds = Benachrichtigungstöne

# src/settings_window/notifications.rs
plays-a-short-sound-with-each-notification = Spielt bei jeder Benachrichtigung einen kurzen Ton ab

use-reset = Zurücksetzung nutzen
reset-using = Wird angewendet…
reset-confirm-message = Eine angesparte Zurücksetzung für dieses Konto nutzen? Berechtigte Limits werden zurückgesetzt. Dies kann nicht rückgängig gemacht werden.
reset-applied = Zurücksetzung angewendet. Berechtigte Limits wurden zurückgesetzt.
reset-nothing-to-reset = Derzeit gibt es nichts zurückzusetzen.
reset-no-credit = Keine nutzbaren Zurücksetzungen mehr verfügbar.
reset-already-redeemed = Diese Zurücksetzung wurde bereits genutzt.
reset-unconfirmed = Die Zurücksetzung konnte nicht bestätigt werden. Erneut versuchen, um dieselbe Anfrage zu prüfen.
reset-applied-refresh-failed = Zurücksetzung angewendet, aber die neuen Limits konnten nicht geladen werden. Zum Prüfen aktualisieren.
reset-in-progress = Für dieses Konto läuft bereits eine Zurücksetzung.
reset-cooldown = Zurücksetzungen sind vorübergehend gesperrt. Später erneut versuchen.
reset-rate-limited = Zu viele Anfragen zur Zurücksetzung. Später erneut versuchen.
reset-sign-in-again = Erneut bei Claude anmelden, um Zurücksetzungen zu nutzen.
reset-cli-login-required = Zum Zurücksetzen die Claude-CLI-Anmeldung desselben Kontos verwenden.
reset-invalid-grant = Claude hat eine ungültige Zurücksetzung zurückgegeben.
