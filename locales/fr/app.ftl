### Codex Minibar application messages.
### French translation; per-message English fallback. Keep $parameters unchanged.

# src/provider_registry.rs
luna-reserve = Luna Reserve

# src/limits.rs
on-pace = Au bon rythme

# src/limits.rs
delta-in-deficit = { $delta } % au-dessus du rythme prévu

# src/limits.rs
delta-in-reserve = { $delta } % de marge

# src/notifications.rs
msg-5-hour-limit-started = La limite de 5 heures a commencé

# src/notifications.rs
msg-5-hour-limit-reset-and-activated = Limite de 5 heures réinitialisée et activée

# src/notifications.rs
msg-5-hour-limit-reset = Limite de 5 heures réinitialisée

# src/notifications.rs
weekly-limit-reset = Limite hebdomadaire réinitialisée

# src/notifications.rs
name-5-hour = { $name } 5 heures

# src/notifications.rs
label-limit-is-low = Le quota restant de { $label } est faible

# src/popup_window/formatting.rs
expired-at-time = expiré à { $time }

# src/popup_window/formatting.rs
expired-at-time-6c39ff = expiré à { $time } { $v0 }

# src/popup_window/state.rs
never = Jamais

# src/popup_window/formatting.rs
unlimited = Illimité

# src/popup_window/ui/activity.rs
unavailable = Indisponible

# src/popup_window/formatting.rs
days-d-hours-h = { $days } j { $hours } h

# src/popup_window/formatting.rs
days-d = { $days } j

# src/popup_window/formatting.rs
hours-h-minutes-m = { $hours } h { $minutes } min

# src/popup_window/formatting.rs
hours-h = { $hours } h

# src/popup_window/formatting.rs
minutes-m = { $minutes } min

# src/popup_window/formatting.rs
waiting-for-first-update = En attente de la première mise à jour

# src/popup_window/formatting.rs
just-now = à l'instant

# src/popup_window/formatting.rs
seconds-seconds-ago = il y a { $seconds } secondes

# src/popup_window/formatting.rs
minutes-ago = il y a { $v0 } minutes

# src/popup_window/formatting.rs
updated-elapsed = Mis à jour : { $elapsed }

# src/popup_window/state.rs
the-request-timed-out-try-refreshing-again = Le délai de la requête a été dépassé. Essayez d'actualiser à nouveau.

# src/popup_window/state.rs
the-secure-connection-could-not-be-verified-see-log-for-details = La connexion sécurisée n'a pas pu être vérifiée. Voir Journal pour plus de détails.

# src/popup_window/state.rs
the-provider-closed-the-connection-try-refreshing-again = Le fournisseur a fermé la connexion. Essayez d'actualiser à nouveau.

# src/popup_window/state.rs
the-provider-s-address-could-not-be-resolved-check-your-connectio = L'adresse du fournisseur n'a pas pu être résolue. Vérifiez votre connexion.

# src/popup_window/state.rs
could-not-connect-to-the-provider-check-your-connection-and-try-a = Impossible de se connecter au fournisseur. Vérifiez votre connexion et réessayez.

# src/popup_window/state.rs
the-management-key-was-rejected-update-it-in-settings = La clé de gestion a été rejetée. Mettez-la à jour dans les paramètres.

# src/popup_window/state.rs
authentication-failed-sign-in-again-or-update-the-provider-key = L'authentification a échoué. Connectez-vous à nouveau ou mettez à jour la clé du fournisseur.

# src/popup_window/state.rs
access-denied-by-the-provider-http-403 = Accès refusé par le fournisseur (HTTP 403).

# src/popup_window/state.rs
too-many-requests-wait-a-few-minutes-before-refreshing-again = Trop de requêtes. Attendez quelques minutes avant d'actualiser à nouveau.

# src/popup_window/state.rs
the-provider-is-temporarily-unavailable-try-again-later = Le fournisseur est temporairement indisponible. Réessayez plus tard.

# src/popup_window/state.rs
the-provider-rejected-the-request-see-log-for-details = Le fournisseur a rejeté la demande. Voir Journal pour plus de détails.

# src/popup_window/state.rs
the-requested-resource-was-not-found-see-log-for-details = La ressource demandée n'a pas été trouvée. Voir Journal pour plus de détails.

# src/popup_window/state.rs
the-provider-returned-an-unexpected-response-try-refreshing-again = Le fournisseur a renvoyé une réponse inattendue. Essayez d'actualiser à nouveau.

# src/popup_window/state.rs
the-request-failed-see-log-for-details = La demande a échoué. Voir Journal pour plus de détails.

# src/popup_window/ui/activity.rs
input = Entrée

# src/popup_window/ui/activity.rs
cache = Cache

# src/popup_window/ui/usage.rs
output = Sortie

# src/popup_window/ui/activity.rs
input-uncached = Entrée (non mise en cache)

# src/popup_window/ui/usage.rs
cached-input = Entrée en cache

# src/popup_window/ui/activity.rs
value-partially-priced = { $value } (tarification partielle)

# src/popup_window/ui/activity.rs
could-not-load-model-data-error = Impossible de charger les données du modèle : { $error }

# src/popup_window/ui/activity.rs
usage-activity = Activité d'utilisation

# src/popup_window/ui/activity.rs
waiting-for-cursor-s-usage-export-refresh-to-retry = En attente de l'exportation de l'utilisation de Cursor. Actualisez pour réessayer.

# src/popup_window/ui/activity.rs
model-data-unavailable = Données du modèle indisponibles

# src/popup_window/ui/activity.rs
loading-models = Chargement des modèles…

# src/popup_window/ui/tooltip.rs
no-model-data = Aucune donnée de modèle

# src/popup_window/ui/activity.rs
no-usage-data = Aucune donnée d'utilisation

# src/popup_window/ui/activity.rs
no-series-selected = Aucune série sélectionnée

# src/popup_window/ui/activity.rs
no-tokens-in-this-period = Aucun jeton { $v0 } pendant cette période

# src/popup_window/ui/activity.rs
loading-model-breakdown = Chargement de la répartition par modèle

# src/popup_window/ui/activity.rs
group-tokens-or-cost-by-model = Regrouper les jetons ou coût par modèle

# src/popup_window/ui/activity.rs
split-type = Type

# src/popup_window/ui/activity.rs
split-by-token-type = Diviser les barres par type de jeton

# src/popup_window/ui/usage.rs
model = Modèle

# src/popup_window/ui/usage.rs
tokens = Jetons

# src/popup_window/ui/usage.rs
cost = Coût

# src/popup_window/ui/activity.rs
no-cost-data-for-this-period = Aucune donnée de coût pour cette période

# src/popup_window/ui/activity.rs
no-token-data-for-this-period = Aucune donnée de jeton pour cette période

# src/popup_window/ui/activity.rs
daily-cost-in-usd = Coût quotidien en USD

# src/popup_window/ui/activity.rs
daily-token-volume = Volume quotidien des jetons

# src/popup_window/ui/activity.rs
of-total-models-scroll-for-more = { $v0 }–{ $v1 } sur { $total } modèles · Faites défiler pour en voir plus

# src/popup_window/ui/activity.rs
cost-usd-by-model = Coût (USD) par modèle

# src/popup_window/ui/activity.rs
tokens-by-model = Jetons par modèle

# src/settings.rs
today = Aujourd'hui

# src/popup_window/ui/activity.rs
last-period-days = { $period } derniers jours

# src/popup_window/ui/activity.rs
no-data = Aucune donnée

# src/popup_window/ui/cards.rs
loading-usage-statistics = Chargement des statistiques d'utilisation…

# src/popup_window/ui/cards.rs
available-balance = Solde disponible

# src/popup_window/ui/cards.rs
resets-in = Réinitialisation dans

# src/popup_window/ui/cards.rs
session-not-started = Session non démarrée

# src/popup_window/ui/cards.rs
expires-in = Expiration dans

# src/popup_window/ui/cards.rs
usage = Utilisation :

# src/settings_window/providers/page.rs
remove-key = Retirer la clé

# src/popup_window/ui/cards.rs
msg-1-banked-reset = 1 réinitialisation en réserve

# src/popup_window/ui/cards.rs
count-banked-resets = { $count ->
    [one] { $count } réinitialisation
    [many] { $count } réinitialisations
   *[other] { $count } réinitialisations
    }

# src/popup_window/ui/cards.rs
available-to-use = Disponible pour utilisation

# src/popup_window/ui/cards.rs
no-expiration-date = Pas de date d'expiration

# src/popup_window/ui/cards.rs
banked-reset = Réinitialisation en réserve { $v0 }

# src/popup_window/ui/cards.rs
codex-limits = Limites Codex

# src/popup_window/ui/cards.rs
open-announcement-source = Ouvrir la source de l'annonce

# src/popup_window/ui/cards.rs
source-not-provided = Source non fournie

# src/popup_window/ui/cards.rs
tibo-reset = Tibo Reset™

# src/settings_window/customize.rs
home = Accueil

# src/popup_window/ui/usage.rs
usage-0bb186 = Utilisation

# src/popup_window/ui/footer.rs
settings = Paramètres

# src/popup_window/ui/footer.rs
install-update = Installer la mise à jour

# src/popup_window/ui/footer.rs
refreshing-limits-and-usage = Actualisation des limites et de l'utilisation…

# src/popup_window/ui/footer.rs
updated = Mise à jour{ " " }

# src/popup_window/ui/footer.rs
refresh-last-updated-relative = Actualiser · Dernière mise à jour : { $relative }

# src/popup_window/ui/home.rs
edit-home = Modifier l'accueil
home-card-layout = Disposition
home-card-layout-cards = Cartes
home-card-layout-lines = Lignes
home-card-layout-rings = Anneaux
drag-to-reorder = Faites glisser pour réorganiser

# src/popup_window/ui/home.rs
drop-here = Déposer ici

# src/settings_window/general.rs
usage-stats = Statistiques d'utilisation

# src/popup_window/ui/usage.rs
loading-usage = Chargement de l'utilisation…

# src/settings_window/window.rs
update-failed = La mise à jour a échoué

# src/popup_window/ui/root.rs
something-went-wrong = Quelque chose s'est mal passé

# src/popup_window/ui/root.rs
no-providers-enabled = Aucun fournisseur activé

# src/popup_window/ui/root.rs
turn-one-on-in-settings-providers = Activez-en un dans Paramètres > Fournisseurs.

# src/popup_window/ui/root.rs
sign-in-again-to-keep-limits-updating = Connectez-vous à nouveau pour maintenir les limites à jour.

# src/settings_window/providers/page.rs
sign-in-again = Se reconnecter

# src/popup_window/ui/tooltip.rs
total = Total

# src/popup_window/ui/usage.rs
enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = Activez un fournisseur dans Paramètres et incluez-le dans les statistiques d'utilisation pour voir l'utilisation locale de API.

# src/popup_window/ui/usage.rs
hourly-cost = Coût horaire

# src/popup_window/ui/usage.rs
hourly-processed-tokens = Jetons traités toutes les heures

# src/popup_window/ui/usage.rs
no-activity-in-this-range = Aucune activité dans cette plage

# src/popup_window/ui/usage.rs
day = Jour

# src/popup_window/ui/usage.rs
breakdown = Répartition

# src/popup_window/ui/usage.rs
totals = Totaux

# src/popup_window/ui/usage.rs
processed-tokens = Jetons traités

# src/popup_window/ui/usage.rs
uncached-input = Entrée non mise en cache

# src/popup_window/ui/usage.rs
cache-savings = Économies de cache

# src/popup_window/ui/usage.rs
share = Part

# src/popup_window/ui/usage.rs
hour = Heure

# src/usage_overview.rs
sat = sam.

# src/usage_overview.rs
sun = dim.

# src/provider_registry.rs
msg-5h-session = Session de 5 h

# src/provider_registry.rs
weekly = Hebdomadaire

# src/provider_registry.rs
cursor-models = Modèles Cursor

# src/settings_window/tray.rs
other-models = Autres modèles

# src/provider_registry.rs
all-models = Tous les modèles

# src/settings_window/tray.rs
grok-bot = Grok Bot

# src/provider_registry.rs
monthly = Mensuel

# src/provider_registry.rs
spending-limit = Limite de dépenses

# src/provider_registry.rs
gemini = Gemini

# src/provider_registry.rs
claude-gpt = Claude + GPT

# src/provider_registry.rs
credits = Crédits

# src/provider_registry.rs
monthly-credits = Crédits mensuels

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
banked-resets = Réinitialisations en réserve

# src/provider_registry.rs
usage-stats-7b9e1a = Statistiques d'utilisation

# src/provider_registry.rs
spending = Dépenses

# src/settings.rs
separate-tabs = Onglets séparés

# src/settings.rs
grouped-switcher = Groupés, avec sélecteur

# src/settings.rs
grouped-all-accounts = Regroupé, tous les comptes

# src/settings.rs
openrouter-account = Compte OpenRouter

# src/settings.rs
yesterday = Hier

# src/settings.rs
history-retention-must-be-between-1-and-365-days = la conservation de l'historique doit être comprise entre 1 et 365 jours

# src/settings.rs
session-low-usage-threshold-must-be-between-1-and-99-percent = le seuil d'utilisation faible de la session doit être compris entre 1 et 99 pour cent

# src/settings.rs
weekly-low-usage-threshold-must-be-between-1-and-99-percent = le seuil de faible utilisation hebdomadaire doit être compris entre 1 et 99 pour cent

# src/settings_window/about.rs
check-github-for-a-new-version = Vérifiez GitHub pour une nouvelle version

# src/settings_window/about.rs
checking-for-updates = Vérification des mises à jour…

# src/settings_window/about.rs
you-re-up-to-date = Vous êtes à jour

# src/settings_window/about.rs
update-available = Mise à jour { $v0 } disponible

# src/settings_window/about.rs
installing-update = Installation de la mise à jour…

# src/settings_window/about.rs
couldn-t-check-for-updates = Impossible de vérifier les mises à jour

# src/settings_window/about.rs
version = Version { $v0 }

# src/settings_window/about.rs
about-tagline = Compagnon gratuit et open source pour suivre les limites d'utilisation, réinitialisations et dépenses de vos abonnements IA, depuis la zone de notification Windows.

# src/settings_window/about.rs
a-new-release-is-ready-to-install = Une nouvelle version est prête à être installée.

# src/settings_window/about.rs
what-s-new = Quoi de neuf

# src/settings_window/about.rs
update = Mise à jour

# src/settings_window/about.rs
check-for-updates = Rechercher des mises à jour

# src/settings_window/about.rs
check-for-updates-on-startup = Rechercher des mises à jour au démarrage

# src/settings_window/about.rs
notify-when-a-new-version-is-found = Avertir lorsqu'une nouvelle version est trouvée

# src/settings_window/about.rs
github = GitHub

# src/settings_window/about.rs
source-code = Code source

# src/settings_window/about.rs
releases = Versions

# src/settings_window/about.rs
see-what-s-new = Voir les nouveautés

# src/settings_window/about.rs
report-an-issue = Signaler un problème

# src/settings_window/about.rs
found-a-bug = Vous avez trouvé un bug ?

# src/settings_window/about.rs
author = Auteur

# src/settings_window/onboarding.rs
updates = Mises à jour

# src/settings_window/about.rs
resources = Ressources

# src/usage_overview.rs
mon = lun.

# src/usage_overview.rs
tue = mar.

# src/usage_overview.rs
wed = mer.

# src/usage_overview.rs
thu = jeu.

# src/usage_overview.rs
fri = ven.

# src/settings_window/activation.rs
every-day = Chaque jour

# src/settings_window/activation.rs
weekdays = En semaine

# src/settings_window/activation.rs
weekends = Week-ends

# src/settings_window/activation.rs
no-days = Aucun jour

# src/settings_window/onboarding.rs
start-5-hour-sessions-automatically = Démarrez automatiquement des séances de 5 heures

# src/settings_window/activation.rs
starts-a-new-session-as-soon-as-a-window-is-available-instead-of = Démarre une nouvelle session dès qu'un quota est disponible, sans attendre votre première requête. Chaque compte utilise sa propre connexion.

# src/settings_window/activation.rs
add-codex-or-claude-in-providers-first = Ajoutez d'abord Codex ou Claude dans « Fournisseurs ».

# src/settings_window/activation.rs
off-in-providers = Désactivé dans les fournisseurs

# src/settings_window/activation.rs
quiet-periods = Périodes de pause

# src/settings_window/activation.rs
don-t-auto-start-sessions-during-these-times = Ne démarrez pas automatiquement les sessions pendant ces périodes.

# src/settings_window/providers/dialog.rs
add = Ajouter

# src/settings_window/activation.rs
all-day = Toute la journée

# src/settings_window/activation.rs
from = De

# src/settings_window/activation.rs
until = Jusqu'à

# src/settings_window/activation.rs
scheduled-activations = Activations programmées

# src/settings_window/activation.rs
start-a-5-hour-session-at-a-set-time = Démarrez une séance de 5 heures à une heure fixe.

# src/settings_window/activation.rs
time = Heure

# src/settings_window/activation.rs
turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = Activez d'abord Codex ou Claude dans les fournisseurs, avec une connexion au dossier de configuration.

# src/settings_window/activation.rs
no-quiet-periods-yet = Aucune période de pause pour l'instant.

# src/settings_window/activation.rs
no-scheduled-activations-yet = Aucune activation programmée pour l'instant.

# src/settings_window/activation.rs
unknown-provider = Fournisseur inconnu

# src/settings_window/activation.rs
remove-quiet-period = Supprimer la période de pause

# src/settings_window/activation.rs
remove-activation = Supprimer l'activation

# src/settings_window/tray.rs
provider = Fournisseur

# src/settings_window/activation.rs
days = Jours

# src/settings_window/advanced.rs
export-settings = Exporter les paramètres

# src/settings_window/advanced.rs
save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = Enregistrez chaque paramètre dans un fichier .toml. Les clés enregistrées restent dans le stockage utilisateur Windows.

# src/settings_window/advanced.rs
export = Exporter

# src/settings_window/advanced.rs
import-settings = Importer les paramètres

# src/settings_window/advanced.rs
replace-the-current-settings-with-a-previously-exported-file = Remplacez les paramètres actuels par un fichier précédemment exporté.

# src/settings_window/advanced.rs
import = Importer

# src/settings_window/advanced.rs
clear-usage-data = Effacer les données d'utilisation

# src/settings_window/advanced.rs
delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = Supprimez l’historique d’utilisation collecté. Il est reconstruit à partir des journaux du fournisseur local lors de la prochaine analyse.

# src/settings_window/advanced.rs
clear = Effacer

# src/settings_window/advanced.rs
usage-data-clear-failed = Échec de la suppression des données d'utilisation

# src/settings_window/advanced.rs
the-background-worker-is-unavailable = Le processus en arrière-plan est indisponible.

# src/settings_window/advanced.rs
usage-data-cleared = Données d'utilisation effacées.

# src/settings_window/advanced.rs
reset-all-settings = Réinitialiser tous les paramètres

# src/settings_window/advanced.rs
restore-every-default-and-start-the-welcome-flow-again = Restaurez tous les paramètres par défaut et redémarrez le flux de bienvenue.

# src/settings_window/advanced.rs
reset = Réinitialiser

# src/settings_window/advanced.rs
backup = Sauvegarde

# src/settings_window/advanced.rs
data = Données

# src/settings_window/advanced.rs
rendering = Rendu

# src/settings_window/advanced.rs
software-rendering = Rendu logiciel

# src/settings_window/advanced.rs
software-rendering-description = Dessiner les fenêtres avec le processeur plutôt qu'avec la carte graphique. Utile quand un jeu occupe le GPU ; les animations peuvent être moins fluides.

# src/settings_window/advanced.rs
settings-exported = Paramètres exportés.

# src/settings_window/advanced.rs
settings-export-failed = Échec de l'exportation des paramètres

# src/settings_window/advanced.rs
settings-imported = Paramètres importés.

# src/settings_window/advanced.rs
settings-import-failed = L'importation des paramètres a échoué

# src/settings_window/advanced.rs
settings-reset-failed = Échec de la réinitialisation des paramètres

# src/settings_window/advanced.rs
reset-all-settings-bbfe66 = Réinitialiser tous les paramètres ?

# src/settings_window/advanced.rs
every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = Chaque paramètre revient à sa valeur par défaut et le flux de bienvenue s'ouvre à nouveau. Les clés enregistrées et les données d'utilisation sont conservées.

# src/settings_window/troubleshoot.rs
cancel = Annuler

# src/settings_window/appearance.rs
windows = Windows

# src/settings_window/appearance.rs
light = Clair

# src/settings_window/appearance.rs
dark = Sombre

# src/settings_window/appearance.rs
color-theme = Thème de couleur

# src/settings_window/appearance.rs
applies-to-settings-the-popup-and-its-tray-menu = S'applique aux paramètres, à la fenêtre contextuelle et à son menu de barre d'état.

# src/settings_window/appearance.rs
accent-color = Couleur d'accentuation

# src/settings_window/appearance.rs
windows-follows-your-system-accent = Windows suit l'accent de votre système.

# src/settings_window/appearance.rs
icons-style = Style d'icônes

# src/settings_window/appearance.rs
font = Police

# src/settings_window/appearance.rs
any-font-installed-on-this-pc = Toute police installée sur ce PC.

# src/settings_window/appearance.rs
windows-default = Windows par défaut

# src/settings_window/appearance.rs
glyph-style-in-the-settings-sidebar = Style de glyphe dans la barre latérale Paramètres.

# src/settings_window/appearance.rs
colored = Coloré

# src/settings_window/tray.rs
monochrome = Monochromes

# src/settings_window/appearance.rs
time-format = Format de l'heure

# src/settings_window/appearance.rs
msg-12-hour = 12 heures

# src/settings_window/appearance.rs
msg-24-hour = 24 heures

# src/settings_window/appearance.rs
popup-background = Arrière-plan contextuel
popup-theme = Thème contextuel
popup-theme-description = Modifie uniquement la fenêtre contextuelle. Les paramètres conservent le look Windows.
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel
popup-theme-built-in = Intégré
popup-theme-vscode = Thèmes VS Code
vscode-themes = Thèmes VS Code
vscode-themes-description = Utilisez des thèmes de couleur Visual Studio Code dans la fenêtre contextuelle : parcourez Open VSX ou importez un fichier de thème .vsix ou .json.
import-theme = Importer un thème
importing-theme = Importation…
remove-theme = Supprimer le thème
browse-open-vsx = Parcourir Open VSX
browse-open-vsx-description = Recherchez et installez des thèmes de couleurs à partir du registre open-vsx.org
search-color-themes = Rechercher des thèmes de couleurs
open-vsx-search = Rechercher
open-vsx-browse = Parcourir
open-vsx-searching = Recherche…
no-themes-found = Aucun thème de couleur trouvé
open-vsx-load-failed = Impossible de charger les thèmes depuis Open VSX
open-vsx-retry = Réessayer
open-vsx-preview-loading = Chargement de l'aperçu…
open-vsx-preview-hint = Cliquez sur un thème pour le prévisualiser dans la fenêtre contextuelle. L'installation conserve l'intégralité du pack ; le bouton de téléchargement d'un thème ne conserve que celui-là.
open-vsx-preview-empty = Cette extension ne contient aucun thème de couleurs
open-vsx-install = Installer
open-vsx-install-one = Installez uniquement ce thème
open-vsx-installed = Installé
open-vsx-installing = Installation…
theme-installed = Thème installé
theme-install-failed = Impossible d'installer le thème
theme-remove-failed = Impossible de supprimer le thème

# src/settings_window/appearance.rs
acrylic = Acrylic

# src/settings_window/appearance.rs
mica = Mica

# src/settings_window/appearance.rs
solid = Uni

# src/settings_window/appearance.rs
bottom-bar-size = Taille de la barre inférieure

# src/settings_window/appearance.rs
comfortable = Spacieux

# src/settings_window/appearance.rs
compact = Compacte

# src/settings_window/appearance.rs
popup-corner-radius = Rayon du coin contextuel
popup-borders = Bordures
popup-borders-description = Afficher les contours de la fenêtre popup, des cartes et des contrôles
popup-theme-contrast = Contraste
popup-theme-contrast-description = Faire ressortir plus ou moins le texte et les contours des thèmes VS Code

# src/settings_window/appearance.rs
animation-effects = Effets d'animation

# src/settings_window/appearance.rs
glide-transitions-in-the-popup-and-settings-windows-own-animation = Transitions fluides dans le popup et les paramètres. Le réglage d'animation de Windows est également respecté.

# src/settings_window/appearance.rs
popup = Fenêtre contextuelle

# src/settings_window/appearance.rs
motion = Mouvement

# src/settings_window/appearance.rs
blue = Bleu

# src/settings_window/appearance.rs
purple = Violet

# src/settings_window/appearance.rs
pink = Rose

# src/settings_window/appearance.rs
red = Rouge

# src/settings_window/appearance.rs
orange = Orange

# src/settings_window/appearance.rs
green = Vert

# src/settings_window/appearance.rs
teal = Bleu-vert

# src/settings_window/customize.rs
use-two-columns = Utiliser deux colonnes

# src/settings_window/customize.rs
several-accounts-of-one-provider = Plusieurs comptes d'un même fournisseur

# src/settings_window/customize.rs
separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = Des onglets séparés donnent à chaque instance son propre onglet. Groupé affiche un onglet par fournisseur, avec un sélecteur de compte ou chaque compte empilé.

# src/settings_window/customize.rs
use-monochrome-icons = Utiliser des icônes monochromes

# src/settings_window/customize.rs
draw-provider-marks-in-the-popup-without-brand-colors = Dessinez les marques du fournisseur dans la fenêtre contextuelle sans les couleurs de la marque.

# src/settings_window/onboarding.rs
show-used-instead-of-remaining = Afficher la part utilisée plutôt que restante

# src/settings_window/customize.rs
show-usage-in-values-when-possible = Afficher l'utilisation en valeurs si possible

# src/settings_window/customize.rs
adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = Ajoute les montants exacts utilisés/limites à côté des pourcentages lorsqu'un fournisseur les signale.

# src/settings_window/onboarding.rs
show-usage-pace = Afficher le rythme d'utilisation

# src/settings_window/onboarding.rs
marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = Indique si vous brûlez votre quota plus rapidement ou plus lentement qu'un rythme régulier.

# src/settings_window/customize.rs
use-legacy-usage-cards = Utiliser les anciennes cartes d'utilisation

# src/settings_window/customize.rs
show-the-older-layout-with-a-header-a-thin-bar-and-a-footer = Affichez l'ancienne mise en page avec un en-tête, une barre fine et un pied de page.

# src/settings_window/onboarding.rs
show-account-name = Afficher le nom du compte

# src/settings_window/customize.rs
show-on-home-tab = Afficher dans l'onglet Accueil

# src/settings_window/customize.rs
layout = Disposition

# src/settings_window/customize.rs
donut = Diagramme en anneau

# src/settings_window/customize.rs
cards = Cartes

# src/settings_window/customize.rs
tabs = Onglets

# src/settings_window/customize.rs
usage-widget = Widget d'utilisation

# src/settings_window/customize.rs
card = Carte

# src/settings_window/customize.rs
tab = Onglet

# src/settings_window/customize.rs
popup-cards = Cartes contextuelles

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = Choisissez les cartes que ce compte affiche sur l'accueil et sur son propre onglet.

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = Choisissez les cartes affichées dans l'onglet de ce compte. Activez « Afficher sur la page d'accueil » pour sélectionner les cartes d'accueil.

# src/settings_window/general.rs
msg-30-seconds = 30 secondes

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
start-with-windows = Démarrer avec Windows

# src/settings_window/general.rs
open-codex-minibar-in-the-tray-when-you-sign-in = Ouvrez Codex Minibar dans la barre d'état lorsque vous vous connectez.

# src/settings_window/onboarding.rs
refresh-interval = Intervalle d'actualisation

# src/settings_window/general.rs
how-often-this-instance-s-quotas-are-read = Fréquence de lecture des quotas de cette instance. Des intervalles plus longs permettent d'éviter les limites de fréquence des requêtes du fournisseur.

# src/settings_window/general.rs
enable-usage-stats = Activer les statistiques d'utilisation

# src/settings_window/general.rs
scan-local-provider-history-for-the-usage-tab-and-cost-totals = Analyser l'historique local des fournisseurs pour l'onglet Utilisation et le total des coûts.

# src/settings_window/general.rs
collection-period = Intervalle de collecte

# src/settings_window/general.rs
how-often-local-provider-history-is-scanned = À quelle fréquence l’historique du fournisseur local est analysé.

# src/settings_window/general.rs
check-for-confirmed-tibo-resets = Vérifiez les réinitialisations Tibo confirmées

# src/settings_window/general.rs
reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = Lit le flux public GitHub de l'application et conserve la dernière annonce en cache.

# src/settings_window/general.rs
notify-when-new-reset-info-arrives = Avertir lorsque de nouvelles informations de réinitialisation arrivent

# src/settings_window/general.rs
shows-a-notification-when-the-feed-reports-a-possible-reset-never = Affiche une notification lorsque le flux signale une éventuelle réinitialisation, jamais à l'heure de la réinitialisation.

# src/settings_window/general.rs
check-every = Intervalle de vérification

# src/settings_window/general.rs
the-feed-is-also-checked-immediately-when-the-app-starts-or-this = Le flux est également vérifié immédiatement lorsque l'application démarre ou que cette option est activée.

# src/settings_window/general.rs
msg-1-hour = 1 heure

# src/settings_window/general.rs
msg-3-hours = 3 heures

# src/settings_window/general.rs
msg-2-hours = 2 heures

# src/settings_window/general.rs
msg-5-hours = 5 heures

# src/settings_window/general.rs
msg-6-hours = 6 heures

# src/settings_window/general.rs
msg-12-hours = 12 heures

# src/settings_window/general.rs
msg-24-hours = 24 heures

# src/settings_window/general.rs
tibo-resets = Réinitialisations Tibo™

# src/settings_window/general.rs
enable-a-provider-in-the-providers-tab-to-include-it-here = Activez un fournisseur dans l'onglet Fournisseurs pour l'inclure ici.

# src/settings_window/general.rs
add-a-management-key = Ajouter une clé de gestion

# src/settings_window/general.rs
usage-statistics-are-off-on-this-provider-s-page = Les statistiques d'utilisation sont désactivées sur la page de ce fournisseur

# src/settings_window/general.rs
included-providers = Fournisseurs inclus

# src/settings_window/general.rs
choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = Choisissez les comptes pris en compte dans l'onglet Utilisation et le total Accueil de cette machine. Les comptes exclus continuent de collecter et affichent toujours leur utilisation sur leur propre page.

# src/settings_window/integrations.rs
download-the-latest-stream-deck-companion-from-github-and-open-it = Téléchargez le dernier compagnon Stream Deck de GitHub et ouvrez son programme d'installation.

# src/settings_window/integrations.rs
downloading-the-latest-stream-deck-companion-from-github = Téléchargement du dernier compagnon Stream Deck de GitHub...

# src/settings_window/integrations.rs
opening-the-stream-deck-installer = Ouverture du programme d'installation de Stream Deck...

# src/settings_window/integrations.rs
the-installer-is-open-finish-the-installation-in-stream-deck = Le programme d'installation est ouvert. Terminez l’installation dans Stream Deck.

# src/settings_window/integrations.rs
the-plugin-could-not-be-downloaded-or-opened-try-again = Le plugin n'a pas pu être téléchargé ou ouvert. Essayer à nouveau.

# src/settings_window/integrations.rs
downloading = Téléchargement...

# src/settings_window/integrations.rs
opening = Ouverture…

# src/settings_window/integrations.rs
install-plugin = Installer le plugin

# src/settings_window/integrations.rs
stream-deck-companion = Plugin compagnon Stream Deck

# src/settings_window/integrations.rs
check-your-connection-then-try-again = Vérifiez votre connexion, puis réessayez.

# src/settings_window/providers/page.rs
on = Activé

# src/settings_window/providers/page.rs
off = Désactivé

# src/settings_window/kit.rs
more-options = Plus d'options

# src/settings_window/kit.rs
custom-color = Couleur personnalisée

# src/settings_window/troubleshoot.rs
run-troubleshoot-with-ai = Exécutez le dépannage avec l'IA

# src/settings_window/log.rs
let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = Laissez une IA CLI installée lire le journal et enquêter sur un problème.

# src/settings_window/log.rs
choose-tool = Choisir l'outil

# src/settings_window/log.rs
application-log = Journal des applications

# src/settings_window/log.rs
log-txt-in-the-app-data-folder = log.txt dans le dossier de données de l'application.

# src/settings_window/log.rs
open-log-txt = Ouvrir le fichier log.txt

# src/settings_window/log.rs
could-not-open-log-txt = Impossible d'ouvrir le fichier log.txt

# src/settings_window/log.rs
open-logs-folder = Ouvrir le dossier des journaux

# src/settings_window/log.rs
could-not-open-logs-folder = Impossible d'ouvrir le dossier des journaux

# src/settings_window/log.rs
no-log-events-yet = Aucun événement de journal pour l'instant.

# src/settings_window/log.rs
live-tail = Suivi en direct

# src/settings_window/log.rs
no-supported-ai-tool-found = Aucun outil d'IA pris en charge trouvé

# src/settings_window/log.rs
install-codex-or-claude-code-and-make-it-available-to-minibar = Installez Codex ou Claude Code et mettez-le à disposition de Minibar.

# src/settings_window/window.rs
general = Général

# src/settings_window/window.rs
appearance = Apparence

# src/settings_window/window.rs
providers = Fournisseurs

# src/settings_window/window.rs
customize = Personnaliser

# src/settings_window/window.rs
limit-activation = Activation des sessions

# src/settings_window/window.rs
tray = Zone de notification

# src/settings_window/window.rs
notifications = Notifications

# src/settings_window/window.rs
advanced = Avancé

# src/settings_window/window.rs
log = Journal

# src/settings_window/window.rs
integrations = Intégrations

# src/settings_window/nav.rs
about-updates = À propos et mises à jour

# src/settings_window/onboarding.rs
successful-activations = Activations réussies

# src/settings_window/onboarding.rs
failed-activations = Activations échouées

# src/settings_window/onboarding.rs
when-limits-reset = Quand les limites sont réinitialisées

# src/settings_window/onboarding.rs
when-5-hour-remaining-hits = Quand le quota restant sur 5 heures atteint { $v0 } %

# src/settings_window/onboarding.rs
when-weekly-remaining-hits = Quand le quota hebdomadaire restant atteint { $v0 } %

# src/settings_window/onboarding.rs
low-usage = Quota restant faible

# src/settings_window/notifications.rs
shows-a-notification-once-per-window-when-the-remaining-share-dro = Affiche une notification une fois par période lorsque le quota restant atteint le seuil.

# src/settings_window/notifications.rs
threshold = Seuil

# src/settings_window/onboarding.rs
found-in-opencode-auth-or-local-history = Trouvé dans l'authentification OpenCode ou l'histoire locale.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-set-up-elsewhere = Pas trouvé. Allumez-le s'il est configuré ailleurs.

# src/settings_window/onboarding.rs
account-credentials-are-already-set = Les informations d'identification du compte sont déjà définies.

# src/settings_window/onboarding.rs
optional-add-accounts-later-in-providers = Facultatif. Ajoutez des comptes plus tard dans les fournisseurs.

# src/settings_window/onboarding.rs
found-an-official-agy-sign-in-on-this-pc = J'ai trouvé une connexion agy officielle sur ce PC.

# src/settings_window/onboarding.rs
not-found-sign-in-with-agy-before-enabling-it = Pas trouvé. Connectez-vous avec agy avant de l’activer.

# src/settings_window/onboarding.rs
found-an-official-grok-cli-sign-in-on-this-pc = Trouvé une connexion officielle Grok CLI sur ce PC.

# src/settings_window/onboarding.rs
not-found-run-grok-login-before-enabling-it = Pas trouvé. Exécutez grok login avant de l'activer.

# src/settings_window/onboarding.rs
found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Kiro IDE, Kiro Crew ou un Kiro CLI connecté.

# src/settings_window/onboarding.rs
not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = Pas trouvé. Installez Kiro IDE ou Kiro Crew, ou connectez-vous à Kiro CLI.

# src/settings_window/onboarding.rs
found-on-this-pc = Trouvé sur ce PC.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-installed-somewhere-else = Pas trouvé. Allumez-le s'il est installé ailleurs.

# src/settings_window/onboarding.rs
setup-could-not-be-saved = La configuration n'a pas pu être enregistrée

# src/settings_window/onboarding.rs
detected = Détecté

# src/settings_window/onboarding.rs
starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = Démarre une nouvelle session Codex ou Claude dès qu'un quota est disponible, sans attendre votre première requête.

# src/settings_window/onboarding.rs
collect-usage-data = Collecter des données d'utilisation

# src/settings_window/onboarding.rs
scans-local-provider-history-for-usage-stats = Analyse l'historique du fournisseur local pour les statistiques d'utilisation.

# src/settings_window/onboarding.rs
startup = Démarrage

# src/settings_window/providers/page.rs
features = Fonctionnalités

# src/settings_window/onboarding.rs
customization = Personnalisation

# src/settings_window/onboarding.rs
when-a-new-version-is-found = Lorsqu'une nouvelle version est trouvée

# src/settings_window/onboarding.rs
activity = Activité

# src/settings_window/onboarding.rs
choose-providers = Choisir les fournisseurs
choose-a-theme = Choisir un thème
theme-step-description = Choisissez les couleurs des paramètres et de la fenêtre contextuelle. La fenêtre contextuelle affiche chaque choix immédiatement.

# src/settings_window/onboarding.rs
we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = Nous avons activé les fournisseurs trouvés sur ce PC. Vous pourrez modifier cela plus tard.

# src/settings_window/onboarding.rs
general-settings = Paramètres généraux

# src/settings_window/onboarding.rs
you-can-change-these-later-in-settings = Vous pourrez les modifier ultérieurement dans Paramètres.

# src/settings_window/onboarding.rs
turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = Éteignez tout ce dont vous ne voulez pas entendre parler. Vous pourrez les modifier ultérieurement dans Paramètres.

# src/settings_window/window.rs
back = Retour

# src/settings_window/onboarding.rs
continue = Continuer

# src/settings_window/tray.rs
done = Terminé

# src/settings_window/providers/dialog.rs
details = Détails

# src/settings_window/providers/dialog.rs
added = Ajouté

# src/settings_window/providers/dialog.rs
reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Lit la session et les limites hebdomadaires d'un abonnement Claude sans connexion Claude Code.

# src/settings_window/providers/dialog.rs
msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. Ouvrez Claude dans un profil de navigateur distinct ou une fenêtre privée. Connectez-vous au compte que vous souhaitez suivre et confirmez son email dans les paramètres de Claude.

# src/settings_window/providers/dialog.rs
open-claude-ai = Ouvrir claude.ai

# src/settings_window/providers/dialog.rs
msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. Dans Chrome ou Edge, appuyez sur F12. Ouvrez Application > Stockage > Cookies et sélectionnez https://claude.ai.

# src/settings_window/providers/dialog.rs
msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. Recherchez sessionKey. Copiez sa valeur, et non son nom ou le tableau des cookies entier, puis collez-la dans le champ ci-dessus.

# src/settings_window/providers/dialog.rs
how-to-view-cookies-in-chrome = Comment afficher les cookies dans Chrome

# src/settings_window/providers/dialog.rs
a-cookie-header-containing-sessionkey-also-works-when-the-session = Un en-tête Cookie contenant sessionKey fonctionne également. Lorsque la session expire, collez-en une nouvelle ici.

# src/settings_window/providers/dialog.rs
use-the-access-token-from-a-claude-code-subscription-login-miniba = Utilisez le jeton d'accès d'une connexion d'abonnement Claude Code. Minibar ne peut pas renouveler un jeton collé. Préférez « Source : Dossier de configuration » lorsque c'est possible.

# src/settings_window/providers/dialog.rs
copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = Copiez uniquement claudeAiOauth.accessToken (commence par sk-ant-oat) depuis le fichier .credentials.json de cette connexion, sans les guillemets. N'utilisez pas claude setup-token : ce jeton peut ne pas avoir accès aux données d'utilisation.

# src/settings_window/providers/dialog.rs
claude-code-log-in-with-multiple-accounts = Claude Code : connectez-vous avec plusieurs comptes

# src/settings_window/providers/dialog.rs
requires-a-claude-subscription-login-with-usage-access-api-keys-a = Nécessite une connexion d'abonnement Claude avec accès à l'utilisation. Les clés API et les clés Admin API n'affichent pas les limites d'abonnement.

# src/settings_window/providers/dialog.rs
this-provider = ce fournisseur

# src/settings_window/providers/dialog.rs
track-another-account-or-a-provider-minibar-has-not-shown-yet = Suivre un autre compte ou un fournisseur que Minibar n'affiche pas encore.

# src/settings_window/providers/dialog.rs
e-g-work = p. ex. Travail

# src/settings_window/providers/page.rs
name = Nom

# src/settings_window/providers/dialog.rs
shown-on-its-tab-home-card-tray-and-notifications = Affiché dans son onglet, sa carte d'accueil, la zone de notification et les notifications.

# src/settings_window/providers/page.rs
badge = Badge

# src/settings_window/providers/dialog.rs
auto = Automatique

# src/settings_window/providers/page.rs
badge-color = Couleur du badge

# src/settings_window/providers/dialog.rs
up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = Jusqu'à trois lettres. Si le champ est vide, les initiales du nom sont utilisées. Les badges s'affichent lorsque plusieurs instances d'un fournisseur sont activées.

# src/settings_window/window.rs
add-provider = Ajouter un fournisseur

# src/settings_window/providers/dialog.rs
next = Suivant

# src/settings_window/providers/dialog.rs
minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar cesse de lire { $name } et oublie ses clés enregistrées, ses programmes, ses indicateurs dans la zone de notification et sa position sur l'accueil.{ $v0 }

# src/settings_window/providers/dialog.rs
its-config-folder-stays-on-disk = { " " } Son dossier de configuration reste sur le disque.

# src/settings_window/providers/dialog.rs
delete-name = Supprimer { $name } ?

# src/settings_window/providers/dialog.rs
delete = Supprimer

# src/settings_window/providers/dialog.rs
finish-signing-in-in-your-browser-cancel-stops-this-login = Terminez de vous connecter dans votre navigateur. Annuler arrête cette connexion.

# src/settings_window/providers/dialog.rs
runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = Exécute la propre connexion de Claude Code pour { $name } avec son dossier de configuration sous la forme CLAUDE_CONFIG_DIR. La connexion reste dans ce dossier, où Claude Code la conserve. Nécessite un Windows Claude Code natif.

# src/settings_window/providers/dialog.rs
runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = Exécute la propre connexion de Codex pour { $name } avec son dossier de configuration comme CODEX_HOME. La connexion reste dans ce dossier, où Codex la conserve. Nécessite le Codex CLI natif ou une application de bureau.

# src/settings_window/providers/dialog.rs
sign-in-to-name = Connectez-vous à { $name }

# src/settings_window/providers/page.rs
sign-in = Se connecter

# src/settings_window/providers/dialog.rs
for = Pour { $v0 }.

# src/settings_window/providers/dialog.rs
browser-session = Session de navigateur

# src/settings_window/providers/dialog.rs
oauth-token = Jeton OAuth

# src/settings_window/providers/dialog.rs
session-key = Clé de session

# src/settings_window/providers/dialog.rs
paste-the-sessionkey-value = Collez la valeur sessionKey

# src/settings_window/providers/dialog.rs
oauth-access-token = Jeton d'accès OAuth

# src/settings_window/providers/dialog.rs
the-saved-credential-is-replaced-only-after-the-new-one-passes-th = L'identifiant enregistré n'est remplacé qu'une fois que le nouveau a réussi le contrôle.

# src/settings_window/providers/dialog.rs
claude-credential = Identifiant Claude

# src/settings_window/providers/dialog.rs
check-and-save = Vérifiez et enregistrez

# src/settings_window/providers/dialog.rs
key-name-optional = Nom de la clé (facultatif)

# src/settings_window/providers/dialog.rs
e-g-personal = p. ex. Personnel

# src/settings_window/providers/dialog.rs
leave-blank-to-use-the-name-from-openrouter = Laissez vide pour utiliser le nom de OpenRouter.

# src/settings_window/providers/dialog.rs
minibar-checks-the-key-with-openrouter-before-saving-it = Minibar vérifie la clé auprès d'OpenRouter avant de l'enregistrer.

# src/settings_window/providers/dialog.rs
replace-api-key = Remplacer la clé API

# src/settings_window/providers/page.rs
add-api-key = Ajouter une clé API

# src/settings_window/providers/page.rs
management-key = Clé de gestion

# src/settings_window/providers/dialog.rs
create-one-under-settings-management-keys-on-openrouter-ai = Créez-en un sous Paramètres → Clés de gestion sur openrouter.ai.

# src/settings_window/providers/dialog.rs
replace-management-key = Remplacer la clé de gestion

# src/settings_window/providers/page.rs
add-management-key = Ajouter une clé de gestion

# src/settings_window/providers/dialog.rs
key-name = Nom de la clé

# src/settings_window/providers/page.rs
rename-key = Renommer la clé

# src/settings_window/providers/dialog.rs
save = Enregistrer

# src/settings_window/providers/dialog.rs
this-key = cette clé

# src/settings_window/providers/dialog.rs
minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibar arrête de suivre { $hint }. La clé continue de fonctionner sur OpenRouter.

# src/settings_window/providers/dialog.rs
remove-api-key = Supprimer la clé API ?

# src/settings_window/tray.rs
remove = Supprimer

# src/settings_window/providers/dialog.rs
minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibar cesse d’afficher le solde de crédits et l’historique d’utilisation pour { $v0 }. La clé continue de fonctionner sur OpenRouter.

# src/settings_window/providers/dialog.rs
remove-management-key = Supprimer la clé de gestion ?

# src/settings_window/providers/dialog.rs
saved-in-windows-user-storage-never-in-the-settings-file = Enregistré dans le stockage utilisateur Windows, jamais dans le fichier de paramètres.

# src/settings_window/providers/dialog.rs
save-key = Enregistrer la clé

# src/settings_window/providers/dialog.rs
minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibar oublie la clé enregistrée de { $v0 }. Il continue de fonctionner avec OpenCode.

# src/settings_window/providers/dialog.rs
signing-in = Connexion…

# src/settings_window/providers/page.rs
checking = Vérification…

# src/settings_window/providers/dialog.rs
choose-a-provider = Choisissez un fournisseur.

# src/settings_window/providers/dialog.rs
reason-it-is-already-in-the-list = { $reason }. C'est déjà dans la liste.

# src/settings_window/providers/dialog.rs
added-name = Ajout de { $name }.

# src/settings_window/providers/dialog.rs
could-not-add-the-provider-error = Impossible d'ajouter le fournisseur : { $error }

# src/settings_window/providers.rs
this-provider-no-longer-exists = Ce fournisseur n'existe plus.

# src/settings_window/providers/dialog.rs
could-not-delete-the-provider-error = Impossible de supprimer le fournisseur : { $error }

# src/settings_window/providers/dialog.rs
paste-a-credential-first = Collez d'abord un identifiant.

# src/settings_window/providers/dialog.rs
credential-saved-in-windows-user-storage = Identifiant enregistré dans le stockage utilisateur Windows.

# src/settings_window/providers/dialog.rs
paste-a-key-first = Collez d'abord une clé.

# src/settings_window/providers/dialog.rs
this-account-no-longer-exists = Ce compte n'existe plus.

# src/settings_window/providers/dialog.rs
api-key-saved-in-windows-user-storage = Clé API enregistrée dans le stockage utilisateur Windows.

# src/settings_window/providers/dialog.rs
management-key-replaced = Clé de gestion remplacée.

# src/settings_window/providers/dialog.rs
management-key-added = Clé de gestion ajoutée.

# src/settings_window/providers/dialog.rs
could-not-rename-the-key-error = Impossible de renommer la clé : { $error }

# src/settings_window/providers/dialog.rs
api-key-renamed = Clé API renommée.

# src/settings_window/providers/dialog.rs
could-not-remove-the-key-error = Impossible de retirer la clé : { $error }

# src/settings_window/providers/dialog.rs
api-key-removed = Clé API supprimée.

# src/settings_window/providers/dialog.rs
management-key-removed = Clé de gestion supprimée.

# src/settings_window/providers/dialog.rs
could-not-save-the-key-error = Impossible d'enregistrer la clé : { $error }

# src/settings_window/providers/dialog.rs
api-key-saved = Clé API enregistrée.

# src/settings_window/providers/dialog.rs
switch-source-to-config-folder-to-sign-in = Réglez « Source » sur « Dossier de configuration » pour vous connecter.

# src/settings_window/providers/dialog.rs
this-provider-has-no-config-folder = Ce fournisseur n'a pas de dossier de configuration.

# src/settings_window/providers/dialog.rs
another-instance-already-reads-this-config-folder-choose-a-differ = Une autre instance lit déjà ce dossier de configuration. Choisissez d'abord un autre dossier.

# src/settings_window/providers/dialog.rs
this-provider-has-no-sign-in = Ce fournisseur n'a aucune connexion.

# src/settings_window/providers/dialog.rs
signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 } s'est connecté. Son CLI conserve la connexion à jour dans son dossier de configuration.

# src/settings_window/providers/page.rs
no-keys-yet = Pas encore de clés

# src/settings_window/providers/page.rs
management-key-5c8cf2 = { " " }· Clé de gestion

# src/settings_window/providers/page.rs
using-a-saved-credential = Utiliser un identifiant enregistré

# src/settings_window/providers/page.rs
paste-a-credential-under-account = Collez un identifiant sous Compte

# src/settings_window/providers/page.rs
needs-an-api-key-or-opencode-sign-in = Nécessite une clé API ou une connexion OpenCode

# src/settings_window/providers/page.rs
needs-an-api-key = Nécessite une clé API

# src/settings_window/providers/page.rs
not-found-set-its-folder-under-runtime = Pas trouvé. Définissez son dossier sous Runtime.

# src/settings_window/providers/page.rs
using-a-saved-api-key = Utilisation d'une clé API enregistrée

# src/settings_window/providers/page.rs
using-opencode-sign-in-or-local-history = Utilisation de la connexion OpenCode ou de l'historique local

# src/settings_window/providers/page.rs
reading-cli = Lecture de { $cli }

# src/settings_window/providers/page.rs
reading-crew = Lecture de { $crew }

# src/settings_window/providers/page.rs
reading-app = Lecture de { $app }

# src/settings_window/providers/page.rs
checking-kiro-ide-kiro-crew-and-cli = Vérification des Kiro IDE, Kiro Crew et CLI…

# src/settings_window/providers/page.rs
checking-installed-app-and-cli = Vérification de l'application installée et de CLI…

# src/settings_window/providers/page.rs
checking-cli = Vérification de CLI…

# src/settings_window/providers/page.rs
checking-installed-app = Vérification de l'application installée…

# src/settings_window/providers/page.rs
no-providers-yet-add-one-to-start-reading-limits = Aucun fournisseur pour l'instant. Ajoutez-en un pour commencer à lire les limites.

# src/settings_window/providers/page.rs
is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } est désactivé et n'apparaît donc ni dans Minibar ni dans la zone de notification. Activez-le pour commencer à lire l'utilisation.

# src/settings_window/providers/page.rs
replace-chatgpt-logo-with-codex = Remplacez le logo ChatGPT par Codex

# src/settings_window/providers/page.rs
limits-only = Limites uniquement

# src/settings_window/providers/page.rs
delete-provider = Supprimer le fournisseur

# src/settings_window/providers/page.rs
display-name = Nom d'affichage

# src/settings_window/providers/page.rs
shown-on-popup-tabs-home-cards-the-tray-and-notifications = Affiché dans les onglets du popup, les cartes d'accueil, la zone de notification et les notifications.

# src/settings_window/providers/page.rs
up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = Jusqu'à trois lettres. Laissez vide pour utiliser les initiales du nom. Affiché lorsqu'un fournisseur a plusieurs instances activées.

# src/settings_window/providers/page.rs
auto-uses-a-neutral-plate-that-follows-the-theme = Auto utilise une plaque neutre qui suit le thème.

# src/settings_window/providers/page.rs
show-on-home = Afficher sur la page d'accueil

# src/settings_window/providers/page.rs
its-provider-tab-stays-available-when-hidden-from-home = Son onglet fournisseur reste disponible lorsqu'il est masqué dans l'Accueil.

# src/settings_window/providers/page.rs
credential = Informations d'identification

# src/settings_window/providers/page.rs
signed-in-as-identity = Connecté sous le nom { $identity }

# src/settings_window/providers/page.rs
saved-in-windows-user-storage = Enregistré dans le stockage utilisateur Windows

# src/settings_window/providers/page.rs
paste-a-sessionkey-or-an-oauth-access-token = Collez une sessionKey ou un jeton d'accès OAuth.

# src/settings_window/providers/page.rs
reads-limits-only-minibar-cannot-refresh-a-pasted-credential = Lit uniquement les limites. Minibar ne peut pas renouveler des informations d'identification collées.

# src/settings_window/providers/page.rs
replace-credential = Remplacer l'identifiant

# src/settings_window/providers/page.rs
add-credential = Ajouter un identifiant

# src/settings_window/providers/page.rs
signed-in-account = Compte connecté

# src/settings_window/providers/page.rs
not-signed-in-yet-or-no-limits-read-so-far = Pas encore connecté ou aucune limite lue pour l'instant.

# src/settings_window/providers/page.rs
account = Compte

# src/settings_window/providers/page.rs
runtime = Environnement d'exécution

# src/settings_window/providers/page.rs
minibar-finds-these-automatically = Minibar { $v0 } les trouve automatiquement.

# src/settings_window/providers/page.rs
in-use = En cours d'utilisation

# src/settings_window/providers/page.rs
found = Trouvé

# src/settings_window/providers/page.rs
copy-path = Copier le chemin

# src/settings_window/providers/page.rs
open-folder = Ouvrir le dossier

# src/settings_window/providers/page.rs
path-copied = Chemin copié.

# src/settings_window/providers/page.rs
not-installed-or-installed-somewhere-minibar-doesn-t-look = Non installé, ou installé dans un emplacement que Minibar ne recherche pas.

# src/settings_window/providers/page.rs
not-found = Pas trouvé

# src/settings_window/providers/page.rs
choose-folder = Choisissez un dossier…

# src/settings_window/providers/page.rs
select-folder = Sélectionner un dossier

# src/settings_window/providers/page.rs
choose-folder-1838a4 = Choisir un dossier

# src/settings_window/providers/page.rs
only-needed-if-automatic-detection-misses-your-install = Nécessaire uniquement si la détection automatique manque votre installation.

# src/settings_window/providers/page.rs
source = Source

# src/settings_window/providers/page.rs
config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = Le dossier de configuration lit la connexion Claude Code dans CLAUDE_CONFIG_DIR. Les informations d'identification manuelles lisent uniquement les limites d'une information d'identification collée.

# src/settings_window/providers/page.rs
config-folder = Dossier de configuration

# src/settings_window/providers/page.rs
manual = Manuel

# src/settings_window/providers/page.rs
passed-to-the-cli-as-env-leave-empty-to-use = Transmis au CLI sous le nom de { $env }. Laissez vide pour utiliser { $v0 }.

# src/settings_window/providers/page.rs
the-standard-folder = le dossier standard

# src/settings_window/providers/page.rs
a-folder-minibar-creates-for-this-instance = un dossier que Minibar crée pour cette instance

# src/settings_window/providers/page.rs
other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } lit déjà ce dossier. Deux instances ne doivent pas partager une connexion, sinon l'utilisation est comptée deux fois.

# src/settings_window/providers/page.rs
automatic-activation = Activation automatique

# src/settings_window/providers/page.rs
starts-this-account-s-5-hour-window-when-it-resets-using-its-own = Démarre la période de 5 heures de ce compte après sa réinitialisation avec sa propre connexion. Les programmes et les pauses se règlent dans « Activation des sessions ».

# src/settings_window/providers/page.rs
usage-statistics = Statistiques d'utilisation

# src/settings_window/providers/page.rs
scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = Analyse l'historique local de cette instance pour alimenter sa carte d'utilisation. Désactivez cette option pour arrêter complètement la collecte.

# src/settings_window/providers/page.rs
opencode-sign-in-or-local-history = Connexion OpenCode ou historique local

# src/settings_window/providers/page.rs
found-in-opencode-auth-environment-a-saved-key-or-local-history = Trouvé dans l'authentification OpenCode, l'environnement, une clé enregistrée ou l'historique local.

# src/settings_window/providers/page.rs
nothing-found-in-opencode-auth-environment-or-local-history = Rien trouvé dans l'authentification OpenCode, l'environnement ou l'historique local.

# src/settings_window/providers/page.rs
replace-key = Remplacer la clé

# src/settings_window/providers/page.rs
optional-only-needed-without-opencode-sign-in-on-this-pc = Facultatif. Nécessaire uniquement sans connexion OpenCode sur ce PC.

# src/settings_window/providers/page.rs
add-the-key-of-the-account-this-instance-tracks = Ajoutez la clé du compte suivi par cette instance.

# src/settings_window/providers/page.rs
keys = Clés

# src/settings_window/providers/page.rs
a-management-key-shows-credit-balance-and-usage-history-api-keys = Une clé de gestion affiche le solde de crédits et l’historique d’utilisation. Les clés API affichent les dépenses par clé.

# src/settings_window/providers/page.rs
could-not-read-the-saved-key-reopen-this-page-to-retry = Impossible de lire la clé enregistrée. Rouvrez cette page pour réessayer.

# src/settings_window/providers/page.rs
credit-balance-and-account-wide-usage-history = Solde créditeur et historique d'utilisation à l'échelle du compte

# src/settings_window/providers/page.rs
not-added-add-one-to-see-credit-balance-and-usage-history = Non ajouté. Ajoutez-en un pour voir le solde de crédits et l’historique d’utilisation.

# src/settings_window/providers/page.rs
last-updated = Dernière mise à jour { $v0 }, { $v1 }

# src/settings_window/providers/page.rs
no-api-keys-yet-add-one-to-track-spend-per-key = Pas encore de clés API. Ajoutez-en un pour suivre les dépenses par clé.

# src/settings_window/providers/page.rs
key = Clé

# src/settings_window/providers/page.rs
spend = Dépenses

# src/settings_window/providers/page.rs
limit = Limite

# src/settings_window/providers/page.rs
error-reopen-this-page-to-retry = { $error }. Rouvrez cette page pour réessayer.

# src/settings_window/providers/page.rs
could-not-read-key = Impossible de lire la clé

# src/settings_window/providers/page.rs
not-saved = Non enregistré

# src/settings_window/providers/page.rs
unnamed = Sans nom

# src/settings_window/providers/page.rs
of-the-limit = { $v0 } de la limite { $v1 }

# src/settings_window/providers/page.rs
no-spend-limit-key-spend-account-credits-purchased = Aucune limite de dépenses. Dépenses de la clé : { $v0 }. Crédits achetés sur le compte : { $v1 }.

# src/settings_window/providers/page.rs
none = Aucun

# src/settings_window/providers/page.rs
add-key = Ajouter une clé

# src/settings_window/providers.rs
openrouter-account-credentials-are-configured = Les informations d'identification du compte OpenRouter sont configurées

# src/settings_window/providers.rs
saved-api-key = Clé API enregistrée

# src/settings_window/providers.rs
saved-credential = Identifiant enregistré

# src/settings_window/providers.rs
opencode-auth-json-or-local-database = OpenCode auth.json ou base de données locale

# src/settings_window/providers.rs
that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = Cela ne ressemble pas à une clé OpenRouter. Les clés commencent par sk-or-.

# src/settings_window/providers.rs
codex-cli-folder = Dossier Codex CLI

# src/settings_window/providers.rs
claude-code-cli-folder = Dossier Claude Code CLI

# src/settings_window/providers.rs
cursor-app-folder = Dossier d'application Cursor

# src/settings_window/providers.rs
grok-cli-folder = Dossier Grok CLI

# src/settings_window/providers.rs
kiro-ide-folder = Dossier Kiro IDE

# src/settings_window/providers.rs
kiro-crew-app-path = Chemin d'accès à l'application Kiro Crew

# src/settings_window/providers.rs
kiro-cli-folder = Dossier Kiro CLI

# src/settings_window/providers.rs
reads-the-signed-in-codex-cli-or-desktop-app = Lit le Codex CLI connecté ou l'application de bureau.

# src/settings_window/providers.rs
reads-your-existing-claude-code-login = Lit votre connexion Claude Code existante.

# src/settings_window/providers.rs
reads-the-signed-in-cursor-app-for-this-billing-cycle = Lit l'application Cursor connectée pour ce cycle de facturation.

# src/settings_window/providers.rs
reads-zen-auth-and-local-opencode-history = Lit l'authentification Zen et l'historique local OpenCode.

# src/settings_window/providers.rs
reads-go-quota-windows-and-local-opencode-history = Lit les fenêtres de quota Go et l’historique local OpenCode.

# src/settings_window/providers.rs
reads-api-key-usage-and-spend-limits-a-management-key-also-enable = Lit les limites d'utilisation et de dépenses de la clé API. Une clé de gestion permet également l’historique d’utilisation et le solde de crédits.

# src/settings_window/providers.rs
reads-subscription-quota-from-your-existing-official-agy-windows = Lit le quota d'abonnement à partir de votre connexion officielle agy Windows existante.

# src/settings_window/providers.rs
reads-supergrok-subscription-credits-from-your-existing-official = Lit les crédits d'abonnement SuperGrok à partir de votre connexion officielle Grok CLI existante.

# src/settings_window/providers.rs
fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = Récupère les crédits mensuels en direct de Kiro avec sa connexion partagée ; reconnaît les installations IDE, Crew et CLI.

# src/settings_window/providers.rs
codex-desktop-app = Application de bureau Codex

# src/settings_window/providers.rs
codex-cli = Codex CLI

# src/settings_window/providers.rs
claude-desktop-app = Application de bureau Claude

# src/settings_window/providers.rs
claude-code-cli = Claude Code CLI

# src/settings_window/providers.rs
cursor-app = Application Cursor

# src/settings_window/providers.rs
antigravity-app = Application Antigravity

# src/settings_window/providers.rs
grok-cli = Grok CLI

# src/settings_window/providers.rs
kiro-ide = Kiro IDE

# src/settings_window/providers.rs
kiro-crew = Kiro Crew

# src/settings_window/providers.rs
kiro-cli = Kiro CLI

# src/settings_window/providers.rs
supports-one-instance = { $v0 } prend en charge une instance

# src/settings_window/tray.rs
numbers = Chiffres

# src/settings_window/tray.rs
progress-bars = Barres de progression

# src/settings_window/tray.rs
rings = Anneaux

# src/settings_window/tray.rs
reset-time = Heure de réinitialisation

# src/settings_window/tray.rs
countdown = Compte à rebours

# src/settings_window/tray.rs
status = Statut

# src/settings_window/tray.rs
fixed = Fixe

# src/settings_window/tray.rs
app-accent = Accent d'application

# src/settings_window/tray.rs
percentages-as-digits = Pourcentages sous forme de chiffres

# src/settings_window/tray.rs
one-bar-per-indicator = Une barre par indicateur

# src/settings_window/tray.rs
nested-rings-one-per-indicator = Anneaux imbriqués, un par indicateur

# src/settings_window/tray.rs
when-the-limit-resets = Quand la limite est réinitialisée

# src/settings_window/tray.rs
time-left-until-the-reset = Temps restant avant la réinitialisation

# src/settings_window/tray.rs
used = Utilisé

# src/settings_window/tray.rs
remaining = Restant

# src/settings_window/tray.rs
app-icon = Icône de l'application

# src/settings_window/tray.rs
widget = Widget { $v0 }

# src/settings_window/tray.rs
the-codex-minibar-icon = L'icône Codex Minibar

# src/settings_window/tray.rs
style-no-indicators = { $style } · Aucun indicateur

# src/settings_window/tray.rs
widget-removed = Widget supprimé

# src/settings_window/tray.rs
undo = Annuler

# src/settings_window/tray.rs
tray-icon = Icône de la barre d'état

# src/settings_window/tray.rs
shows-the-app-icon = Affiche l'icône de l'application.

# src/settings_window/tray.rs
add-widget = Ajouter un widget

# src/settings_window/tray.rs
add-app-icon = Ajouter une icône d'application

# src/settings_window/tray.rs
move-up = Monter

# src/settings_window/tray.rs
move-down = Descendre

# src/settings_window/tray.rs
edit = Modifier

# src/settings_window/tray.rs
duplicate = Dupliquer

# src/settings_window/tray.rs
shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = Affiche l'icône Codex Minibar dans la zone de notification. Il n’a aucun indicateur à mettre en place.

# src/settings_window/tray.rs
style = Style

# src/settings_window/tray.rs
up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = Jusqu'à { $max_indicators } quotas, affichés dans cet ordre. Développez-en un pour le modifier.

# src/settings_window/tray.rs
indicators = Indicateurs

# src/settings_window/tray.rs
remove-widget = Supprimer le widget

# src/settings_window/tray.rs
add-indicator = Ajouter un indicateur

# src/settings_window/tray.rs
unsupported = { $v0 } non pris en charge

# src/settings_window/tray.rs
remove-indicator = Supprimer l'indicateur

# src/settings_window/tray.rs
unsupported-bba2a8 = Non pris en charge ({ $v0 })

# src/settings_window/tray.rs
unavailable-0c5d75 = Indisponible ({ $v0 })

# src/settings_window/tray.rs
metric = Mesure

# src/settings_window/tray.rs
show = Afficher

# src/settings_window/tray.rs
color = Couleur

# src/settings_window/tray.rs
fixed-color = Couleur fixe

# src/settings_window/troubleshoot.rs
troubleshooting-could-not-start = Le dépannage n'a pas pu démarrer

# src/settings_window/troubleshoot.rs
choose-which-installed-ai-tool-should-investigate-the-problem = Choisissez quel outil d'IA installé doit enquêter sur le problème.

# src/settings_window/troubleshoot.rs
open-terminal = Ouvrir le terminal

# src/settings_window/window.rs
version-is-available = { $version } est disponible

# src/settings_window/window.rs
update-now = Mettre à jour maintenant

# src/settings_window/window.rs
missing-a-provider = Il vous manque un fournisseur ?

# src/settings_window/window.rs
ask-for-it-or-build-it-yourself = Demandez-le ou construisez-le vous-même.

# src/settings_window/window.rs
request-a-provider = Demander un fournisseur

# src/settings_window/window.rs
contribute-one = Contribuer un fournisseur

# src/tray.rs
disabled = Désactivé

# src/updater.rs
update-complete = Mise à jour terminée

# src/updater.rs
now-running-version = Exécute maintenant { $version }.

# src/usage_overview.rs
past-24h = Dernières 24 h

# application
language = Langue

# application
applies-immediately-to-every-app-window-and-notification-auto-fol = S'applique immédiatement à toutes les fenêtres et notifications. « Automatique » suit la langue d'affichage de Windows.

# application
auto-windows = Automatique (Windows)

# application
english = Anglais

# application
msg-russian = russe

# application
codex-minibar-settings = Paramètres de Codex Minibar

# application
welcome-to-codex-minibar = Bienvenue dans Codex Minibar

# application
exit = Quitter

# application
update-available-67fd3a = Mise à jour disponible

# application
monthly-credits-976559 = Crédits mensuels

# application
msg-5h-session-de7ce8 = Session de 5 h

# application
name-weekly = { $name } hebdomadaire

# application
tokens-94e0b9 = { $v0 } : { $v1 } jetons

# application
requests-priced = { $v0 ->
    [one] { $v0 } requête · { $v1 } avec tarif
    [many] { $v0 } requêtes · { $v1 } avec tarif
   *[other] { $v0 } requêtes · { $v1 } avec tarif
    }

# application
requests = { $v0 ->
    [one] { $v0 } requête
    [many] { $v0 } requêtes
   *[other] { $v0 } requêtes
    }

# application
credits-338f52 = CRÉDITS

# application
cloud-session-credits = CRÉDITS DE SÉANCE CLOUD

# application
name-login-expires-in-days-left = { $days_left ->
    [one] La connexion de { $name } expire dans { $days_left } jour
    [many] La connexion de { $name } expire dans { $days_left } jours
   *[other] La connexion de { $name } expire dans { $days_left } jours
    }

# application
name-error = Erreur { $name }

# application
sessions = { $v0 ->
    [one] { $v0 } session
    [many] { $v0 } sessions
   *[other] { $v0 } sessions
    }

# application
api-estimate = Coût API estimé

# application
start-to-end = { $start } à { $end }

# application
to = { $v0 } à { $v1 }

# application
share-1-of-other = { $share } % de { $v0 } · { $other }

# application
excluded-other = Exclus · { $other }

# application
exclude-from-usage-stats = Cliquez pour exclure des statistiques d'utilisation

# application
include-in-usage-stats = Cliquez pour inclure dans les statistiques d'utilisation

# application
cost-885dc4 = coût

# application
tokens-339143 = jetons

# application
sessions-0e5e29 = { $v0 ->
    [one] · { $v0 } session
    [many] · { $v0 } sessions
   *[other] · { $v0 } sessions
    }

# application
api-key = Clé API

# application
api-keys = Clés API

# application
name-deleted = { $name } supprimé.

# application
openrouter-api-key-no-longer-exists = La clé OpenRouter API n'existe plus

# application
openrouter-account-no-longer-exists = Le compte OpenRouter n'existe plus

# application
credit = { $v0 } crédit

# application
agy-cli-folder = dossier agy CLI

# application
folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = Dossier avec codex.exe, codex.cmd ou codex.ps1. Laissez vide pour le retrouver automatiquement.

# application
folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = Dossier avec claude.exe, claude.cmd ou claude.ps1. Laissez vide pour le retrouver automatiquement.

# application
folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = Dossier avec Cursor.exe. Laissez vide pour le retrouver automatiquement. L'utilisation provient toujours du profil connecté.

# application
folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = Dossier avec agy.exe, agy.cmd ou agy.ps1. Laissez vide pour le retrouver automatiquement.

# application
folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = Dossier avec grok.exe, grok.cmd ou grok.ps1. Laissez vide pour le retrouver automatiquement.

# application
folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = Dossier contenant Kiro.exe, ou l'exécutable lui-même. Laissez vide pour le retrouver automatiquement.

# application
folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = Dossier contenant KiroCrew.exe, ou l'exécutable lui-même. Laissez vide pour détecter automatiquement les installations par utilisateur et pour tous les utilisateurs.

# application
folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = Dossier contenant kiro-cli.exe, ou l'exécutable lui-même. Laissez vide pour le retrouver automatiquement.

# application
off-45080e = { $v0 } (éteint)

# application
each-widget-is-one-icon-in-the-notification-area-indicators-show = Chaque widget est une icône dans la zone de notification. Les indicateurs affichent le quota d'un fournisseur sous forme de chiffres, de barres, d'anneaux ou d'une horloge de réinitialisation.

# application
a-reset-clock-follows-one-quota = Une horloge de réinitialisation suit un seul quota.

# application
ai-tool = outil d'IA

# application
msg-7-days = 7 jours

# application
msg-30-days = 30 jours

# application
msg-90-days = 90 jours

# application
yellow = Jaune

# application
custom = Personnalisé

# application
not-available-for-manual-credentials-switch-source-to-config-fold = Indisponible avec des informations d'identification manuelles. Réglez « Source » sur « Dossier de configuration ».

# application
opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = L'historique local de OpenCode est suivi par la première instance de OpenCode.

# application
this-provider-has-no-local-usage-history = Ce fournisseur n'a pas d'historique d'utilisation local.

# application
this-provider-has-no-session-window-to-start = Ce fournisseur n'a pas de fenêtre de session pour démarrer.

# application
this-provider-does-not-use-a-config-folder = Ce fournisseur n'utilise pas de dossier de configuration.

# application
could-not-save-provider-settings-error-restoring-its-previous-cre = Impossible d'enregistrer les paramètres du fournisseur ({ $error }) ; la restauration de ses informations d'identification précédentes a également échoué ({ $rollback_error }).

# application
msg-5h-7d = 5h |  { $v0 } |  { $v1 }
    7j |  { $v2 } |  { $v3 }

# API key count in the provider header.
api-key-count = { $v0 ->
    [one] { $v0 } clé API
    [many] { $v0 } clés API
   *[other] { $v0 } clés API
    }

# Application copy
widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = Élargir l'accueil et l'utilisation. Faites glisser les blocs d'accueil entre les colonnes ; les onglets des fournisseurs restent compacts.

# Application copy
a-possible-codex-reset-is-scheduled-for-when-in-countdown = Une éventuelle réinitialisation de Codex est prévue pour { $when } (dans { $countdown })

# Application copy
label-possible-reset-on-when-in-countdown = { $label } : réinitialisation possible le { $when } (dans { $countdown })

# Application copy
new-codex-reset-info = Nouvelles informations de réinitialisation du Codex

# Application copy
login-expires-soon = La connexion { $v0 } expire bientôt

# Application copy
it-stops-renewing-on-open-minibar-and-choose-sign-in-again = Le renouvellement automatique s'arrête le { $v0 }. Ouvrez Minibar et choisissez « Se reconnecter ».

# Application copy
could-not-clear-usage-data-error = Impossible d'effacer les données d'utilisation : { $error }

# Application copy
succeeded-at = { $v0 } a réussi à { $v1 }

# Application copy
failed-at-error = Échec de { $v0 } sur { $v1 } : { $error }

# Application copy
key-39df89 = Clé { $v0 }

# Application copy
expired = Expiré

# Application copy
jan = janv.

# Application copy
feb = févr.

# Application copy
mar = mars

# Application copy
apr = avr.

# Application copy
may = mai

# Application copy
jun = juin

# Application copy
jul = juil.

# Application copy
aug = août

# Application copy
sep = sept.

# Application copy
oct = oct.

# Application copy
nov = nov.

# Application copy
dec = déc.

# { $value }% used
quota-percent-used = { $value } % utilisés

# { $value }% left
quota-percent-left = { $value } % restants

# { $amount } of { $limit } used
cloud-amount-used = { $amount } sur { $limit } utilisés

# { $amount } of { $limit } left
cloud-amount-left = { $amount } sur { $limit } restants

# Short usage range
range-24h = 24 h

# Short usage range
range-7d = 7 j

# Short usage range
range-30d = 30 j

# Short usage range
range-90d = 90 j

# Home usage card title
home-usage-title = Utilisation

# OpenRouter key administration

could-not-reach-openrouter = Impossible d'atteindre OpenRouter


openrouter-keys-title = Clés


openrouter-keys-new = Nouvelle clé


openrouter-keys-new-title = Nouvelle clé


openrouter-keys-back = Retour


openrouter-keys-updating = Mise à jour…


openrouter-keys-loading = Chargement des clés…


openrouter-keys-load-failed = Impossible de charger les clés


openrouter-keys-retry = Réessayer


openrouter-keys-empty = Ce compte n'a pas encore de clés.


openrouter-keys-this-app = Cette application


openrouter-keys-no-limit = Aucune limite


openrouter-keys-of-limit = { $amount } sur { $limit }


openrouter-keys-resets-daily = se réinitialise quotidiennement


openrouter-keys-resets-weekly = se réinitialise chaque semaine


openrouter-keys-resets-monthly = se réinitialise mensuellement


openrouter-keys-expires-on = expire { $date }


openrouter-keys-expired-on = expiré { $date }


openrouter-keys-today = aujourd'hui { $amount }


openrouter-keys-usage-breakdown = Aujourd'hui { $today } · semaine { $week } · mois { $month } · total { $total }


openrouter-keys-show-all = Afficher les { $count } clés


openrouter-keys-show-fewer = Afficher moins


openrouter-keys-spending-limit = Limite de dépenses


openrouter-keys-limit-hint = Laissez vide sans limite.


openrouter-keys-resets = Réinitialisation


openrouter-keys-reset-never = Jamais


openrouter-keys-reset-daily = Quotidiennement


openrouter-keys-reset-weekly = Hebdomadaire


openrouter-keys-reset-monthly = Mensuel


openrouter-keys-byok = Compter l'utilisation de BYOK dans la limite


openrouter-keys-enabled = Activé


openrouter-keys-delete = Supprimer


openrouter-keys-delete-tracked = Cette application utilise cette clé. Retirez-la d'abord dans les paramètres.


openrouter-keys-saving = Enregistrement…


openrouter-keys-delete-title = Supprimer « { $name } » ?


openrouter-keys-delete-message = Tout ce qui utilise cette clé cesse immédiatement de fonctionner. La suppression est irréversible ; la désactivation de la clé peut être annulée.


openrouter-keys-keep = Garder la clé


openrouter-keys-delete-confirm = Supprimer la clé


openrouter-keys-deleting = Suppression…


openrouter-keys-name = Nom


openrouter-keys-name-placeholder = p. ex. laptop-cursor


openrouter-keys-name-required = Entrez un nom pour la clé.


openrouter-keys-invalid-amount = Entrez un montant en dollars comme 25 ou 12,50.


openrouter-keys-expires = Expiration


openrouter-keys-expires-1-hour = 1 heure


openrouter-keys-expires-1-day = 1 jour


openrouter-keys-expires-7-days = 7 jours


openrouter-keys-expires-30-days = 30 jours


openrouter-keys-expires-90-days = 90 jours


openrouter-keys-expires-180-days = 180 jours


openrouter-keys-expires-1-year = 1 an


openrouter-keys-expires-never = Pas d'expiration


openrouter-keys-track = Suivez cette clé dans Minibar


openrouter-keys-create = Créer une clé


openrouter-keys-creating = Création…


openrouter-keys-created = Clé créée


openrouter-keys-copy = Copier la clé


openrouter-keys-copied = Copié


openrouter-keys-done = Terminé


openrouter-keys-once-title = Vous ne reverrez plus cette clé


openrouter-keys-once-message = OpenRouter n'affiche une nouvelle clé qu'une seule fois. Copiez-la maintenant.


openrouter-keys-pinned = La fenêtre contextuelle reste ouverte jusqu'à ce que vous appuyiez sur Terminé.


openrouter-keys-tracking = Ajout de la clé à Minibar…


openrouter-keys-tracked = Minibar suit désormais cette clé.


openrouter-keys-track-failed = Impossible d'ajouter la clé à Minibar : { $error }


openrouter-keys-no-management-key = Ce compte n'a pas de clé de gestion.


openrouter-keys-management-key-rejected = OpenRouter a rejeté la clé de gestion. Remplacez-la dans les paramètres.


openrouter-keys-not-a-management-key = Cette clé ne peut pas gérer d'autres clés. Utilisez une clé de gestion.


openrouter-keys-key-not-found = Cette clé n'existe plus.

# src/settings_window/notifications.rs
notification-sounds = Sons des notifications

# src/settings_window/notifications.rs
plays-a-short-sound-with-each-notification = Joue un son bref à chaque notification

use-reset = Utiliser une réinitialisation
use-reset-short = Utiliser
reset-using = Application…
reset-confirm-message = Utiliser une réinitialisation cumulée pour ce compte ? Les limites éligibles seront effacées. Cette action est irréversible.
reset-applied = Réinitialisation appliquée. Les limites éligibles ont été effacées.
reset-nothing-to-reset = Rien à réinitialiser pour le moment.
reset-no-credit = Aucune réinitialisation utilisable restante.
reset-already-redeemed = Cette réinitialisation a déjà été utilisée.
reset-unconfirmed = La réinitialisation n’a pas pu être confirmée. Réessayez pour vérifier la même demande.
reset-applied-refresh-failed = Réinitialisation appliquée, mais les nouvelles limites n’ont pas pu être chargées. Actualisez pour vérifier.
reset-in-progress = Une réinitialisation est déjà en cours pour ce compte.
reset-cooldown = Les réinitialisations sont temporairement bloquées. Réessayez plus tard.
reset-rate-limited = Trop de demandes de réinitialisation. Réessayez plus tard.
reset-sign-in-again = Reconnectez-vous à Claude pour utiliser les réinitialisations.
reset-cli-login-required = Utilisez la connexion Claude CLI du même compte pour réinitialiser les limites.
reset-invalid-grant = Claude a renvoyé une réinitialisation invalide.
