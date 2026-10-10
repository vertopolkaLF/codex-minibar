### Codex Minibar application messages.
### Japanese translation; per-message English fallback. Keep $parameters unchanged.

# src/provider_registry.rs
luna-reserve = Luna Reserve

# src/limits.rs
on-pace = 適正ペース

# src/limits.rs
delta-in-deficit = { $delta }% 超過ペース

# src/limits.rs
delta-in-reserve = { $delta }% 余裕あり

# src/notifications.rs
msg-5-hour-limit-started = 5時間制限が始まりました

# src/notifications.rs
msg-5-hour-limit-reset-and-activated = 5時間制限がリセットされ有効化されました

# src/notifications.rs
msg-5-hour-limit-reset = 5時間の利用枠がリセットされました

# src/notifications.rs
weekly-limit-reset = 週間の利用枠がリセットされました

# src/notifications.rs
name-5-hour = { $name } 5時間

# src/notifications.rs
label-limit-is-low = { $label } の利用枠の残量が少なくなっています

# src/popup_window/formatting.rs
expired-at-time = 期限切れ: { $time }

# src/popup_window/formatting.rs
expired-at-time-6c39ff = 期限切れ: { $time } { $v0 }

# src/popup_window/state.rs
never = なし

# src/popup_window/formatting.rs
unlimited = 無制限

# src/popup_window/ui/activity.rs
unavailable = 利用不可

# src/popup_window/formatting.rs
days-d-hours-h = { $days }日 { $hours }時間

# src/popup_window/formatting.rs
days-d = { $days }日

# src/popup_window/formatting.rs
hours-h-minutes-m = { $hours }時間 { $minutes }分

# src/popup_window/formatting.rs
hours-h = { $hours }時間

# src/popup_window/formatting.rs
minutes-m = { $minutes }分

# src/popup_window/formatting.rs
waiting-for-first-update = 最初の更新を待っています

# src/popup_window/formatting.rs
just-now = たった今

# src/popup_window/formatting.rs
seconds-seconds-ago = { $seconds } 秒前

# src/popup_window/formatting.rs
minutes-ago = { $v0 } 分前

# src/popup_window/formatting.rs
updated-elapsed = 最終更新: { $elapsed }

# src/popup_window/state.rs
the-request-timed-out-try-refreshing-again = リクエストがタイムアウトしました。もう一度更新してください。

# src/popup_window/state.rs
the-secure-connection-could-not-be-verified-see-log-for-details = 安全な接続を確認できませんでした。詳細については、「ログ」を参照してください。

# src/popup_window/state.rs
the-provider-closed-the-connection-try-refreshing-again = プロバイダーが接続を閉じました。もう一度更新してください。

# src/popup_window/state.rs
the-provider-s-address-could-not-be-resolved-check-your-connectio = プロバイダーのアドレスを解決できませんでした。接続を確認してください。

# src/popup_window/state.rs
could-not-connect-to-the-provider-check-your-connection-and-try-a = プロバイダーに接続できませんでした。接続を確認して、もう一度試してください。

# src/popup_window/state.rs
the-management-key-was-rejected-update-it-in-settings = 管理キーが拒否されました。設定で更新してください。

# src/popup_window/state.rs
authentication-failed-sign-in-again-or-update-the-provider-key = 認証に失敗しました。再度サインインするか、プロバイダー キーを更新します。

# src/popup_window/state.rs
access-denied-by-the-provider-http-403 = プロバイダーによってアクセスが拒否されました (HTTP 403)。

# src/popup_window/state.rs
too-many-requests-wait-a-few-minutes-before-refreshing-again = リクエストが多すぎます。数分待ってから再度更新してください。

# src/popup_window/state.rs
the-provider-is-temporarily-unavailable-try-again-later = プロバイダーが一時的に利用できなくなりました。後でもう一度試してください。

# src/popup_window/state.rs
the-provider-rejected-the-request-see-log-for-details = プロバイダーがリクエストを拒否しました。詳細については、「ログ」を参照してください。

# src/popup_window/state.rs
the-requested-resource-was-not-found-see-log-for-details = 要求されたリソースが見つかりませんでした。詳細については、「ログ」を参照してください。

# src/popup_window/state.rs
the-provider-returned-an-unexpected-response-try-refreshing-again = プロバイダーから予期しない応答が返されました。もう一度更新してください。

# src/popup_window/state.rs
the-request-failed-see-log-for-details = リクエストは失敗しました。詳細については、「ログ」を参照してください。

# src/popup_window/ui/activity.rs
input = 入力

# src/popup_window/ui/activity.rs
cache = キャッシュ

# src/popup_window/ui/usage.rs
output = 出力

# src/popup_window/ui/activity.rs
input-uncached = 入力 (キャッシュされていない)

# src/popup_window/ui/usage.rs
cached-input = キャッシュされた入力

# src/popup_window/ui/activity.rs
value-partially-priced = { $value }（一部のみ料金算出）

# src/popup_window/ui/activity.rs
could-not-load-model-data-error = モデル データをロードできませんでした: { $error }

# src/popup_window/ui/activity.rs
usage-activity = 利用活動

# src/popup_window/ui/activity.rs
waiting-for-cursor-s-usage-export-refresh-to-retry = Cursor の使用状況のエクスポートを待っています。再試行するには更新してください。

# src/popup_window/ui/activity.rs
model-data-unavailable = モデルデータが利用できません

# src/popup_window/ui/activity.rs
loading-models = モデルをロード中…

# src/popup_window/ui/tooltip.rs
no-model-data = モデルデータがありません

# src/popup_window/ui/activity.rs
no-usage-data = 使用状況データなし

# src/popup_window/ui/activity.rs
no-series-selected = シリーズが選択されていません

# src/popup_window/ui/activity.rs
no-tokens-in-this-period = この期間には { $v0 } トークンはありません

# src/popup_window/ui/activity.rs
loading-model-breakdown = モデル別の内訳を読み込み中

# src/popup_window/ui/activity.rs
group-tokens-or-cost-by-model = モデルごとにトークンまたはコストをグループ化する

# src/popup_window/ui/activity.rs
split-type = 種類

# src/popup_window/ui/activity.rs
split-by-token-type = トークンの種類ごとにバーを分割する

# src/popup_window/ui/usage.rs
model = モデル

# src/popup_window/ui/usage.rs
tokens = トークン

# src/popup_window/ui/usage.rs
cost = コスト

# src/popup_window/ui/activity.rs
no-cost-data-for-this-period = この期間のコストデータはありません

# src/popup_window/ui/activity.rs
no-token-data-for-this-period = この期間のトークン データはありません

# src/popup_window/ui/activity.rs
daily-cost-in-usd = USD の 1 日あたりの費用

# src/popup_window/ui/activity.rs
daily-token-volume = 毎日のトークン量

# src/popup_window/ui/activity.rs
of-total-models-scroll-for-more = { $total } モデル中 { $v0 }–{ $v1 } · スクロールして続きを表示

# src/popup_window/ui/activity.rs
cost-usd-by-model = モデル別のコスト (USD)

# src/popup_window/ui/activity.rs
tokens-by-model = モデルごとのトークン

# src/settings.rs
today = 今日

# src/popup_window/ui/activity.rs
last-period-days = 過去 { $period } 日間

# src/popup_window/ui/activity.rs
no-data = データなし

# src/popup_window/ui/cards.rs
loading-usage-statistics = 使用状況統計を読み込んでいます…

# src/popup_window/ui/cards.rs
available-balance = 利用可能残高

# src/popup_window/ui/cards.rs
resets-in = リセットまで

# src/popup_window/ui/cards.rs
session-not-started = セッションが開始されていません

# src/popup_window/ui/cards.rs
expires-in = 有効期限まで

# src/popup_window/ui/cards.rs
usage = 使用量:

# src/settings_window/providers/page.rs
remove-key = キーを削除

# src/popup_window/ui/cards.rs
msg-1-banked-reset = 保存済みリセット 1 回

# src/popup_window/ui/cards.rs
count-banked-resets = { $count } 回の保存済みリセット

# src/popup_window/ui/cards.rs
available-to-use = 使用可能

# src/popup_window/ui/cards.rs
no-expiration-date = 有効期限なし

# src/popup_window/ui/cards.rs
banked-reset = 保存済みリセット { $v0 }

# src/popup_window/ui/cards.rs
codex-limits = Codex 制限

# src/popup_window/ui/cards.rs
open-announcement-source = 告知の出典を開く

# src/popup_window/ui/cards.rs
source-not-provided = ソースが提供されていません

# src/popup_window/ui/cards.rs
tibo-reset = Tibo Reset™

# src/settings_window/customize.rs
home = ホーム

# src/popup_window/ui/usage.rs
usage-0bb186 = 使用量

# src/popup_window/ui/footer.rs
settings = 設定

# src/popup_window/ui/footer.rs
install-update = アップデートをインストールする

# src/popup_window/ui/footer.rs
refreshing-limits-and-usage = 利用枠と使用量を更新中…

# src/popup_window/ui/footer.rs
updated = 更新済み{ " " }

# src/popup_window/ui/footer.rs
refresh-last-updated-relative = 更新 · 最終更新: { $relative }

# src/popup_window/ui/home.rs
edit-home = ホームを編集
home-card-layout = レイアウト
home-card-layout-cards = カード
home-card-layout-lines = ライン
home-card-layout-rings = リング
drag-to-reorder = ドラッグして並べ替えます

# src/popup_window/ui/home.rs
drop-here = ここにドロップ

# src/settings_window/general.rs
usage-stats = 使用状況統計

# src/popup_window/ui/usage.rs
loading-usage = 使用量を読み込み中…

# src/settings_window/window.rs
update-failed = アップデートに失敗しました

# src/popup_window/ui/root.rs
something-went-wrong = 何か問題が発生しました

# src/popup_window/ui/root.rs
no-providers-enabled = 有効なプロバイダーがありません

# src/popup_window/ui/root.rs
turn-one-on-in-settings-providers = [設定] > [プロバイダー] で 1 つをオンにします。

# src/popup_window/ui/root.rs
sign-in-again-to-keep-limits-updating = 制限を更新し続けるには、再度サインインしてください。

# src/settings_window/providers/page.rs
sign-in-again = 再度サインイン

# src/popup_window/ui/tooltip.rs
total = 合計

# src/popup_window/ui/usage.rs
enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = 設定でプロバイダーを有効にし、それを使用状況統計に含めると、ローカルの API の使用状況が表示されます。

# src/popup_window/ui/usage.rs
hourly-cost = 時間当たりのコスト

# src/popup_window/ui/usage.rs
hourly-processed-tokens = 時間ごとに処理されるトークン

# src/popup_window/ui/usage.rs
no-activity-in-this-range = この範囲ではアクティビティはありません

# src/popup_window/ui/usage.rs
day = 日

# src/popup_window/ui/usage.rs
breakdown = 内訳

# src/popup_window/ui/usage.rs
totals = 合計

# src/popup_window/ui/usage.rs
processed-tokens = 処理されたトークン

# src/popup_window/ui/usage.rs
uncached-input = キャッシュされていない入力

# src/popup_window/ui/usage.rs
cache-savings = キャッシュの節約

# src/popup_window/ui/usage.rs
share = 割合

# src/popup_window/ui/usage.rs
hour = 時間

# src/usage_overview.rs
sat = 土

# src/usage_overview.rs
sun = 日

# src/provider_registry.rs
msg-5h-session = 5時間セッション

# src/provider_registry.rs
weekly = 毎週

# src/provider_registry.rs
cursor-models = Cursorモデル

# src/settings_window/tray.rs
other-models = その他のモデル

# src/provider_registry.rs
all-models = 全モデル

# src/settings_window/tray.rs
grok-bot = Grok Bot

# src/provider_registry.rs
monthly = 毎月

# src/provider_registry.rs
spending-limit = 支出制限

# src/provider_registry.rs
gemini = Gemini

# src/provider_registry.rs
claude-gpt = Claude + GPT

# src/provider_registry.rs
credits = クレジット

# src/provider_registry.rs
monthly-credits = 毎月のクレジット

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
banked-resets = 保存済みリセット

# src/provider_registry.rs
usage-stats-7b9e1a = 使用状況の統計

# src/provider_registry.rs
spending = 支出

# src/settings.rs
separate-tabs = 個別のタブ

# src/settings.rs
grouped-switcher = まとめて表示・切り替え

# src/settings.rs
grouped-all-accounts = グループ化されたすべてのアカウント

# src/settings.rs
openrouter-account = OpenRouterアカウント

# src/settings.rs
yesterday = 昨日

# src/settings.rs
history-retention-must-be-between-1-and-365-days = 履歴の保持期間は 1 ～ 365 日である必要があります

# src/settings.rs
session-low-usage-threshold-must-be-between-1-and-99-percent = セッションの低使用量しきい値は 1 ～ 99 パーセントの範囲にする必要があります

# src/settings.rs
weekly-low-usage-threshold-must-be-between-1-and-99-percent = 毎週の低使用量しきい値は 1 ～ 99 パーセントの範囲にする必要があります

# src/settings_window/about.rs
check-github-for-a-new-version = 新しいバージョンについては GitHub を確認してください

# src/settings_window/about.rs
checking-for-updates = アップデートをチェック中…

# src/settings_window/about.rs
you-re-up-to-date = 最新バージョンです

# src/settings_window/about.rs
update-available = アップデート { $v0 } が利用可能

# src/settings_window/about.rs
installing-update = アップデートをインストール中…

# src/settings_window/about.rs
couldn-t-check-for-updates = アップデートを確認できませんでした

# src/settings_window/about.rs
version = バージョン { $v0 }

# src/settings_window/about.rs
usage-limits-in-the-windows-tray = Windows の通知領域で利用枠を確認。

# src/settings_window/about.rs
a-new-release-is-ready-to-install = 新しいリリースをインストールする準備ができました。

# src/settings_window/about.rs
what-s-new = 新機能

# src/settings_window/about.rs
update = アップデート

# src/settings_window/about.rs
check-for-updates = 更新を確認

# src/settings_window/about.rs
check-for-updates-on-startup = 起動時にアップデートを確認する

# src/settings_window/about.rs
notify-when-a-new-version-is-found = 新しいバージョンが見つかったら通知する

# src/settings_window/about.rs
github = GitHub

# src/settings_window/about.rs
source-code = ソースコード

# src/settings_window/about.rs
releases = リリース

# src/settings_window/about.rs
see-what-s-new = 新機能を見る

# src/settings_window/about.rs
report-an-issue = 問題を報告

# src/settings_window/about.rs
found-a-bug = バグが見つかりましたか?

# src/settings_window/about.rs
author = 著者

# src/settings_window/onboarding.rs
updates = アップデート

# src/settings_window/about.rs
resources = リソース

# src/usage_overview.rs
mon = 月

# src/usage_overview.rs
tue = 火

# src/usage_overview.rs
wed = 水

# src/usage_overview.rs
thu = 木

# src/usage_overview.rs
fri = 金

# src/settings_window/activation.rs
every-day = 毎日

# src/settings_window/activation.rs
weekdays = 平日

# src/settings_window/activation.rs
weekends = 週末

# src/settings_window/activation.rs
no-days = 日はありません

# src/settings_window/onboarding.rs
start-5-hour-sessions-automatically = 5 時間のセッションを自動的に開始する

# src/settings_window/activation.rs
starts-a-new-session-as-soon-as-a-window-is-available-instead-of = 最初のリクエストを待たず、利用枠が空き次第、新しいセッションを開始します。各アカウントは自身のログイン情報を使用します。

# src/settings_window/activation.rs
add-codex-or-claude-in-providers-first = 先に「プロバイダー」で Codex または Claude を追加してください。

# src/settings_window/activation.rs
off-in-providers = プロバイダーではオフ

# src/settings_window/activation.rs
quiet-periods = 自動開始の休止時間

# src/settings_window/activation.rs
don-t-auto-start-sessions-during-these-times = このような時間帯にはセッションを自動開始しないでください。

# src/settings_window/providers/dialog.rs
add = 追加

# src/settings_window/activation.rs
all-day = 一日中

# src/settings_window/activation.rs
from = から

# src/settings_window/activation.rs
until = まで

# src/settings_window/activation.rs
scheduled-activations = スケジュールされたアクティベーション

# src/settings_window/activation.rs
start-a-5-hour-session-at-a-set-time = 設定した時間に 5 時間のセッションを開始します。

# src/settings_window/activation.rs
time = 時刻

# src/settings_window/activation.rs
turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = まず、構成フォルダーにログインして、プロバイダーで Codex または Claude をオンにします。

# src/settings_window/activation.rs
no-quiet-periods-yet = 休止時間はまだありません。

# src/settings_window/activation.rs
no-scheduled-activations-yet = スケジュールされたアクティベーションはまだありません。

# src/settings_window/activation.rs
unknown-provider = 不明なプロバイダー

# src/settings_window/activation.rs
remove-quiet-period = 休止時間を削除

# src/settings_window/activation.rs
remove-activation = アクティベーションを削除する

# src/settings_window/tray.rs
provider = プロバイダー

# src/settings_window/activation.rs
days = 日数

# src/settings_window/advanced.rs
export-settings = 設定をエクスポート

# src/settings_window/advanced.rs
save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = すべての設定を .toml ファイルに保存します。保存されたキーは Windows ユーザー ストレージに残ります。

# src/settings_window/advanced.rs
export = エクスポート

# src/settings_window/advanced.rs
import-settings = 設定をインポート

# src/settings_window/advanced.rs
replace-the-current-settings-with-a-previously-exported-file = 現在の設定を以前にエクスポートしたファイルに置き換えます。

# src/settings_window/advanced.rs
import = インポート

# src/settings_window/advanced.rs
clear-usage-data = 使用状況データのクリア

# src/settings_window/advanced.rs
delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = 収集した利用履歴を削除します。次回のスキャン時にローカル プロバイダー ログから再構築されます。

# src/settings_window/advanced.rs
clear = 消去

# src/settings_window/advanced.rs
usage-data-clear-failed = 使用量データのクリアに失敗しました

# src/settings_window/advanced.rs
the-background-worker-is-unavailable = バックグラウンド処理が利用できません。

# src/settings_window/advanced.rs
usage-data-cleared = 使用状況データがクリアされました。

# src/settings_window/advanced.rs
reset-all-settings = すべての設定をリセットする

# src/settings_window/advanced.rs
restore-every-default-and-start-the-welcome-flow-again = すべてのデフォルトを復元し、ウェルカム フローを再度開始します。

# src/settings_window/advanced.rs
reset = リセット

# src/settings_window/advanced.rs
backup = バックアップ

# src/settings_window/advanced.rs
data = データ

# src/settings_window/advanced.rs
settings-exported = 設定がエクスポートされました。

# src/settings_window/advanced.rs
settings-export-failed = 設定のエクスポートに失敗しました

# src/settings_window/advanced.rs
settings-imported = 設定がインポートされました。

# src/settings_window/advanced.rs
settings-import-failed = 設定のインポートに失敗しました

# src/settings_window/advanced.rs
settings-reset-failed = 設定のリセットに失敗しました

# src/settings_window/advanced.rs
reset-all-settings-bbfe66 = すべての設定をリセットしますか?

# src/settings_window/advanced.rs
every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = すべての設定がデフォルトに戻り、ようこそフローが再び開きます。保存されたキーと使用状況データは保持されます。

# src/settings_window/troubleshoot.rs
cancel = キャンセル

# src/settings_window/appearance.rs
windows = Windows

# src/settings_window/appearance.rs
light = ライト

# src/settings_window/appearance.rs
dark = ダーク

# src/settings_window/appearance.rs
color-theme = カラーテーマ

# src/settings_window/appearance.rs
applies-to-settings-the-popup-and-its-tray-menu = 設定、ポップアップ、およびそのトレイ メニューに適用されます。

# src/settings_window/appearance.rs
accent-color = アクセントカラー

# src/settings_window/appearance.rs
windows-follows-your-system-accent = Windows はシステム アクセントに従います。

# src/settings_window/appearance.rs
icons-style = アイコンのスタイル

# src/settings_window/appearance.rs
font = フォント

# src/settings_window/appearance.rs
any-font-installed-on-this-pc = この PC にインストールされているフォント。

# src/settings_window/appearance.rs
windows-default = Windowsのデフォルト

# src/settings_window/appearance.rs
glyph-style-in-the-settings-sidebar = 設定サイドバーのグリフ スタイル。

# src/settings_window/appearance.rs
colored = 色付き

# src/settings_window/tray.rs
monochrome = モノクロ

# src/settings_window/appearance.rs
time-format = 時刻形式

# src/settings_window/appearance.rs
msg-12-hour = 12時間

# src/settings_window/appearance.rs
msg-24-hour = 24時間

# src/settings_window/appearance.rs
popup-background = ポップアップの背景
popup-theme = ポップアップテーマ
popup-theme-description = ポップアップのみを変更します。設定では Windows の外観が維持されます。
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel
popup-theme-built-in = 組み込み
popup-theme-vscode = VS Code テーマ
vscode-themes = VS Code テーマ
vscode-themes-description = .vsix パッケージまたはカラーテーマの .json ファイルをインポートします。テーマはポップアップのみの色を変更します。
import-theme = テーマのインポート
importing-theme = インポート中…
remove-theme = テーマを削除する
browse-open-vsx = Open VSX を閲覧
browse-open-vsx-description = open-vsx.org レジストリからカラー テーマを検索してインストールします。
search-color-themes = カラーテーマを検索する
open-vsx-search = 検索
open-vsx-browse = 閲覧する
open-vsx-searching = 検索中…
no-themes-found = カラーテーマが見つかりません
open-vsx-load-failed = Open VSX からテーマをロードできませんでした
open-vsx-retry = 再試行
open-vsx-preview-loading = プレビューを読み込み中…
open-vsx-preview-hint = テーマをクリックしてポップアップでプレビューします。インストールするとパック全体が保持されます。テーマのダウンロード ボタンには、そのテーマだけが保持されます。
open-vsx-preview-empty = この拡張機能にはカラーテーマが含まれていません
open-vsx-install = インストール
open-vsx-install-one = このテーマのみをインストールする
open-vsx-installed = インストール済み
open-vsx-installing = インストール中…
theme-installed = テーマがインストールされました
theme-install-failed = テーマをインストールできませんでした
theme-remove-failed = テーマを削除できませんでした

# src/settings_window/appearance.rs
acrylic = Acrylic

# src/settings_window/appearance.rs
mica = Mica

# src/settings_window/appearance.rs
solid = 単色

# src/settings_window/appearance.rs
bottom-bar-size = 下部バーのサイズ

# src/settings_window/appearance.rs
comfortable = ゆったり

# src/settings_window/appearance.rs
compact = コンパクト

# src/settings_window/appearance.rs
popup-corner-radius = ポップアップコーナー半径
popup-borders = 枠線
popup-borders-description = ポップアップ、カード、コントロールに枠線を表示します

# src/settings_window/appearance.rs
animation-effects = アニメーション効果

# src/settings_window/appearance.rs
glide-transitions-in-the-popup-and-settings-windows-own-animation = ポップアップと設定画面を滑らかに切り替えます。Windows のアニメーション設定にも従います。

# src/settings_window/appearance.rs
popup = ポップアップ

# src/settings_window/appearance.rs
motion = モーション

# src/settings_window/appearance.rs
blue = ブルー

# src/settings_window/appearance.rs
purple = 紫

# src/settings_window/appearance.rs
pink = ピンク

# src/settings_window/appearance.rs
red = 赤

# src/settings_window/appearance.rs
orange = オレンジ

# src/settings_window/appearance.rs
green = 緑

# src/settings_window/appearance.rs
teal = 青緑

# src/settings_window/customize.rs
use-two-columns = 2 つの列を使用する

# src/settings_window/customize.rs
several-accounts-of-one-provider = 1 つのプロバイダーの複数のアカウント

# src/settings_window/customize.rs
separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = 個別のタブにより、すべてのインスタンスに独自のタブが与えられます。グループ化すると、プロバイダーごとに 1 つのタブが表示され、アカウント スイッチャーまたはすべてのアカウントがスタックされます。

# src/settings_window/customize.rs
use-monochrome-icons = モノクロアイコンを使用する

# src/settings_window/customize.rs
draw-provider-marks-in-the-popup-without-brand-colors = ブランドカラーを使用せずに、ポップアップにプロバイダーマークを描画します。

# src/settings_window/onboarding.rs
show-used-instead-of-remaining = 残量の代わりに使用量を表示

# src/settings_window/customize.rs
show-usage-in-values-when-possible = 可能な場合は使用量を数値で表示

# src/settings_window/customize.rs
adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = プロバイダーが報告するときに、パーセンテージの横に正確な使用量/制限量を追加します。

# src/settings_window/onboarding.rs
show-usage-pace = 使用ペースを表示する

# src/settings_window/onboarding.rs
marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = 均等なペースよりも早くクォータを消費しているか、それとも遅く消費しているかをマークします。

# src/settings_window/customize.rs
use-legacy-usage-cards = 従来の使用カードを使用する

# src/settings_window/customize.rs
show-the-older-layout-with-a-header-a-thin-bar-and-a-footer = ヘッダー、細いバー、フッターを備えた古いレイアウトを表示します。

# src/settings_window/onboarding.rs
show-account-name = アカウント名を表示

# src/settings_window/customize.rs
show-on-home-tab = 「ホーム」タブに表示

# src/settings_window/customize.rs
layout = レイアウト

# src/settings_window/customize.rs
donut = ドーナツグラフ

# src/settings_window/customize.rs
cards = カード

# src/settings_window/customize.rs
tabs = タブ

# src/settings_window/customize.rs
usage-widget = 使用状況ウィジェット

# src/settings_window/customize.rs
card = カード

# src/settings_window/customize.rs
tab = タブ

# src/settings_window/customize.rs
popup-cards = ポップアップカード

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = このアカウントがホームと独自のタブに表示するカードを選択します。

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = このアカウントのタブに表示するカードを選択します。ホームのカードを選択するには「ホームで表示」をオンにしてください。

# src/settings_window/general.rs
msg-30-seconds = 30秒

# src/settings_window/general.rs
msg-1-minute = 1分

# src/settings_window/general.rs
msg-2-minutes = 2分

# src/settings_window/general.rs
msg-5-minutes = 5分

# src/settings_window/general.rs
msg-10-minutes = 10分

# src/settings_window/general.rs
msg-15-minutes = 15分

# src/settings_window/general.rs
msg-30-minutes = 30分

# src/settings_window/general.rs
msg-45-minutes = 45分

# src/settings_window/general.rs
msg-60-minutes = 60分

# src/settings_window/onboarding.rs
start-with-windows = Windows と同時に起動

# src/settings_window/general.rs
open-codex-minibar-in-the-tray-when-you-sign-in = サインイン時にトレイで Codex Minibar を開きます。

# src/settings_window/onboarding.rs
refresh-interval = リフレッシュ間隔

# src/settings_window/general.rs
how-often-this-instance-s-quotas-are-read = このインスタンスの利用枠を読み取る間隔です。間隔を長くすると、プロバイダーのリクエスト頻度制限を回避できます。

# src/settings_window/general.rs
enable-usage-stats = 使用状況統計を有効にする

# src/settings_window/general.rs
scan-local-provider-history-for-the-usage-tab-and-cost-totals = 「使用量」タブとコストの合計表示のため、プロバイダーのローカル履歴をスキャンします。

# src/settings_window/general.rs
collection-period = 収集間隔

# src/settings_window/general.rs
how-often-local-provider-history-is-scanned = ローカルプロバイダーの履歴がスキャンされる頻度。

# src/settings_window/general.rs
check-for-confirmed-tibo-resets = 確認された Tibo リセットを確認する

# src/settings_window/general.rs
reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = アプリのパブリック GitHub フィードを読み取り、最新のお知らせをキャッシュに保持します。

# src/settings_window/general.rs
notify-when-new-reset-info-arrives = 新しいリセット情報が到着したら通知する

# src/settings_window/general.rs
shows-a-notification-when-the-feed-reports-a-possible-reset-never = リセット時ではなく、フィードがリセットの可能性を報告したときに通知を表示します。

# src/settings_window/general.rs
check-every = 確認間隔

# src/settings_window/general.rs
the-feed-is-also-checked-immediately-when-the-app-starts-or-this = フィードは、アプリが起動したとき、またはこのオプションが有効になったときにもすぐにチェックされます。

# src/settings_window/general.rs
msg-1-hour = 1時間

# src/settings_window/general.rs
msg-3-hours = 3時間

# src/settings_window/general.rs
msg-2-hours = 2時間

# src/settings_window/general.rs
msg-5-hours = 5時間

# src/settings_window/general.rs
msg-6-hours = 6時間

# src/settings_window/general.rs
msg-12-hours = 12時間

# src/settings_window/general.rs
msg-24-hours = 24時間

# src/settings_window/general.rs
tibo-resets = Tibo Reset™

# src/settings_window/general.rs
enable-a-provider-in-the-providers-tab-to-include-it-here = [プロバイダー] タブでプロバイダーを有効にして、ここに含めます。

# src/settings_window/general.rs
add-a-management-key = 管理キーを追加する

# src/settings_window/general.rs
usage-statistics-are-off-on-this-provider-s-page = このプロバイダーのページでは使用状況統計がオフになっています

# src/settings_window/general.rs
included-providers = 含まれるプロバイダー

# src/settings_window/general.rs
choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = このマシンの [使用状況] タブと [ホーム] 合計にどのアカウントをカウントするかを選択します。除外されたアカウントは収集を続け、独自のページに使用状況が表示されます。

# src/settings_window/integrations.rs
download-the-latest-stream-deck-companion-from-github-and-open-it = 最新の Stream Deck コンパニオンを GitHub からダウンロードし、インストーラーを開きます。

# src/settings_window/integrations.rs
downloading-the-latest-stream-deck-companion-from-github = 最新の Stream Deck コンパニオンを GitHub からダウンロードしています...

# src/settings_window/integrations.rs
opening-the-stream-deck-installer = Stream Deck インストーラーを開いています...

# src/settings_window/integrations.rs
the-installer-is-open-finish-the-installation-in-stream-deck = インストーラーが開いています。 Stream Deck へのインストールを完了します。

# src/settings_window/integrations.rs
the-plugin-could-not-be-downloaded-or-opened-try-again = プラグインをダウンロードまたは開くことができませんでした。もう一度やり直してください。

# src/settings_window/integrations.rs
downloading = ダウンロード中...

# src/settings_window/integrations.rs
opening = 開いています…

# src/settings_window/integrations.rs
install-plugin = プラグインのインストール

# src/settings_window/integrations.rs
stream-deck-companion = Stream Deck コンパニオン

# src/settings_window/integrations.rs
check-your-connection-then-try-again = 接続を確認して、もう一度試してください。

# src/settings_window/providers/page.rs
on = オン

# src/settings_window/providers/page.rs
off = オフ

# src/settings_window/kit.rs
more-options = その他のオプション

# src/settings_window/kit.rs
custom-color = カスタムカラー

# src/settings_window/troubleshoot.rs
run-troubleshoot-with-ai = AI を使用してトラブルシューティングを実行する

# src/settings_window/log.rs
let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = インストールされた AI CLI にログを読み取らせ、問題を調査させます。

# src/settings_window/log.rs
choose-tool = ツールの選択

# src/settings_window/log.rs
application-log = アプリケーションログ

# src/settings_window/log.rs
log-txt-in-the-app-data-folder = アプリデータフォルダー内のlog.txt。

# src/settings_window/log.rs
open-log-txt = log.txtを開く

# src/settings_window/log.rs
could-not-open-log-txt = log.txtを開けませんでした

# src/settings_window/log.rs
open-logs-folder = ログフォルダーを開く

# src/settings_window/log.rs
could-not-open-logs-folder = ログフォルダーを開けませんでした

# src/settings_window/log.rs
no-log-events-yet = まだログイベントはありません。

# src/settings_window/log.rs
live-tail = リアルタイム表示

# src/settings_window/log.rs
no-supported-ai-tool-found = サポートされている AI ツールが見つかりません

# src/settings_window/log.rs
install-codex-or-claude-code-and-make-it-available-to-minibar = Codex または Claude Code をインストールし、Minibarで使用できるようにします。

# src/settings_window/window.rs
general = 一般

# src/settings_window/window.rs
appearance = 外観

# src/settings_window/window.rs
providers = プロバイダー

# src/settings_window/window.rs
customize = カスタマイズ

# src/settings_window/window.rs
limit-activation = セッションの自動開始

# src/settings_window/window.rs
tray = 通知領域

# src/settings_window/window.rs
notifications = 通知

# src/settings_window/window.rs
advanced = 詳細設定

# src/settings_window/window.rs
log = ログ

# src/settings_window/window.rs
integrations = 統合

# src/settings_window/nav.rs
about-updates = アプリ情報と更新

# src/settings_window/onboarding.rs
successful-activations = アクティベーションの成功

# src/settings_window/onboarding.rs
failed-activations = 失敗したアクティベーション

# src/settings_window/onboarding.rs
when-limits-reset = リミットがリセットされるとき

# src/settings_window/onboarding.rs
when-5-hour-remaining-hits = 5時間枠の残量が { $v0 }% になったとき

# src/settings_window/onboarding.rs
when-weekly-remaining-hits = 週間枠の残量が { $v0 }% になったとき

# src/settings_window/onboarding.rs
low-usage = 利用枠の残量低下

# src/settings_window/notifications.rs
shows-a-notification-once-per-window-when-the-remaining-share-dro = 利用枠の残量がしきい値まで減ると、各利用期間に一度通知します。

# src/settings_window/notifications.rs
threshold = しきい値

# src/settings_window/onboarding.rs
found-in-opencode-auth-or-local-history = OpenCode 認証またはローカル履歴で見つかります。

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-set-up-elsewhere = 見つかりません。他の場所に設定されている場合はオンにします。

# src/settings_window/onboarding.rs
account-credentials-are-already-set = アカウントの資格情報はすでに設定されています。

# src/settings_window/onboarding.rs
optional-add-accounts-later-in-providers = オプション。後でプロバイダーでアカウントを追加します。

# src/settings_window/onboarding.rs
found-an-official-agy-sign-in-on-this-pc = この PC で公式の agy サインインが見つかりました。

# src/settings_window/onboarding.rs
not-found-sign-in-with-agy-before-enabling-it = 見つかりません。 agy を有効にする前に、agy でサインインしてください。

# src/settings_window/onboarding.rs
found-an-official-grok-cli-sign-in-on-this-pc = この PC で公式 Grok CLI サインインが見つかりました。

# src/settings_window/onboarding.rs
not-found-run-grok-login-before-enabling-it = 見つかりません。有効にする前に grok login を実行してください。

# src/settings_window/onboarding.rs
found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Kiro IDE、Kiro Crew、またはサインインした Kiro CLI が見つかりました。

# src/settings_window/onboarding.rs
not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = 見つかりません。 Kiro IDE または Kiro Crew をインストールするか、Kiro CLI にサインインします。

# src/settings_window/onboarding.rs
found-on-this-pc = この PC で見つかりました。

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-installed-somewhere-else = 見つかりません。他の場所にインストールされている場合はオンにします。

# src/settings_window/onboarding.rs
setup-could-not-be-saved = 設定を保存できませんでした

# src/settings_window/onboarding.rs
detected = 検出されました

# src/settings_window/onboarding.rs
starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = 最初のリクエストを待たず、利用枠が空き次第、新しい Codex または Claude セッションを開始します。

# src/settings_window/onboarding.rs
collect-usage-data = 使用状況データを収集する

# src/settings_window/onboarding.rs
scans-local-provider-history-for-usage-stats = ローカルプロバイダーの履歴をスキャンして使用状況統計を取得します。

# src/settings_window/onboarding.rs
startup = 起動

# src/settings_window/providers/page.rs
features = 機能

# src/settings_window/onboarding.rs
customization = カスタマイズ

# src/settings_window/onboarding.rs
when-a-new-version-is-found = 新しいバージョンが見つかったとき

# src/settings_window/onboarding.rs
activity = アクティビティ

# src/settings_window/onboarding.rs
choose-providers = プロバイダーを選択
choose-a-theme = テーマを選択
theme-step-description = 設定とポップアップの色を選びます。選んだ内容はすぐにポップアップに反映されます。

# src/settings_window/onboarding.rs
we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = この PC で見つかったプロバイダーをオンにしました。これは後で変更できます。

# src/settings_window/onboarding.rs
general-settings = 一般設定

# src/settings_window/onboarding.rs
you-can-change-these-later-in-settings = これらは後で [設定] で変更できます。

# src/settings_window/onboarding.rs
turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = 聞きたくないことはすべてオフにしてください。これらは後で [設定] で変更できます。

# src/settings_window/window.rs
back = 戻る

# src/settings_window/onboarding.rs
continue = 続ける

# src/settings_window/tray.rs
done = 完了

# src/settings_window/providers/dialog.rs
details = 詳細

# src/settings_window/providers/dialog.rs
added = 追加されました

# src/settings_window/providers/dialog.rs
reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Claude Code ログインせずに、Claude サブスクリプションのセッションおよび週次制限を読み取ります。

# src/settings_window/providers/dialog.rs
msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. 別のブラウザ プロファイルまたはプライベート ウィンドウで Claude を開きます。追跡したいアカウントにサインインし、Claude の設定でその電子メールを確認します。

# src/settings_window/providers/dialog.rs
open-claude-ai = claude.ai を開く

# src/settings_window/providers/dialog.rs
msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. Chrome または Edge で、F12 キーを押します。 [アプリケーション] > [ストレージ] > [Cookie] を開き、https://claude.ai を選択します。

# src/settings_window/providers/dialog.rs
msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. sessionKey を探します。名前や Cookie テーブル全体ではなく、その値をコピーして上の欄に貼り付けてください。

# src/settings_window/providers/dialog.rs
how-to-view-cookies-in-chrome = Chrome で Cookie を表示する方法

# src/settings_window/providers/dialog.rs
a-cookie-header-containing-sessionkey-also-works-when-the-session = sessionKey を含む Cookie ヘッダーも機能します。セッションの有効期限が切れたら、ここに新しいセッションを貼り付けます。

# src/settings_window/providers/dialog.rs
use-the-access-token-from-a-claude-code-subscription-login-miniba = Claude Code のサブスクリプションでログインしたアクセス・トークンを使用します。貼り付けたトークンは Minibar で更新できません。可能であれば「ソース: 設定フォルダー」を使用してください。

# src/settings_window/providers/dialog.rs
copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = そのログインの .credentials.json から claudeAiOauth.accessToken（sk-ant-oat で始まる値）だけを引用符なしでコピーします。claude setup-token は使用しないでください。使用量へのアクセス権がない場合があります。

# src/settings_window/providers/dialog.rs
claude-code-log-in-with-multiple-accounts = Claude Code: 複数のアカウントでログインします

# src/settings_window/providers/dialog.rs
requires-a-claude-subscription-login-with-usage-access-api-keys-a = 使用アクセス権を持つ Claude サブスクリプション ログインが必要です。 API キーと管理者 API キーにはサブスクリプション制限は表示されません。

# src/settings_window/providers/dialog.rs
this-provider = このプロバイダー

# src/settings_window/providers/dialog.rs
track-another-account-or-a-provider-minibar-has-not-shown-yet = 別のアカウントや、Minibar にまだ表示されていないプロバイダーを追加します。

# src/settings_window/providers/dialog.rs
e-g-work = 例: 仕事用

# src/settings_window/providers/page.rs
name = 名前

# src/settings_window/providers/dialog.rs
shown-on-its-tab-home-card-tray-and-notifications = タブ、ホームのカード、通知領域、通知に表示されます。

# src/settings_window/providers/page.rs
badge = バッジ

# src/settings_window/providers/dialog.rs
auto = 自動

# src/settings_window/providers/page.rs
badge-color = バッジの色

# src/settings_window/providers/dialog.rs
up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = 最大3文字。空欄の場合は名前の頭文字を使用します。同じプロバイダーで複数のインスタンスが有効な場合にバッジを表示します。

# src/settings_window/window.rs
add-provider = プロバイダーの追加

# src/settings_window/providers/dialog.rs
next = 次へ

# src/settings_window/providers/dialog.rs
minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar は { $name } の読み取りを停止し、保存済みキー、スケジュール、通知領域の表示、ホームの配置を削除します。{ $v0 }

# src/settings_window/providers/dialog.rs
its-config-folder-stays-on-disk = { " " }その設定フォルダーはディスク上に残ります。

# src/settings_window/providers/dialog.rs
delete-name = { $name }を削除しますか？

# src/settings_window/providers/dialog.rs
delete = 削除

# src/settings_window/providers/dialog.rs
finish-signing-in-in-your-browser-cancel-stops-this-login = ブラウザでのサインインを完了します。キャンセルすると、このログインが停止されます。

# src/settings_window/providers/dialog.rs
runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = 構成フォルダーを CLAUDE_CONFIG_DIR として、Claude Code 自身の { $name } ログインを実行します。ログインはそのフォルダーに残り、Claude Code によって最新の状態が保たれます。ネイティブ Windows Claude Code が必要です。

# src/settings_window/providers/dialog.rs
runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = CODEX_HOME として設定フォルダーを使用して、{ $name } に対する Codex 自身のログインを実行します。ログインはそのフォルダーに残り、Codex によって最新の状態が保たれます。ネイティブ Codex CLI またはデスクトップ アプリが必要です。

# src/settings_window/providers/dialog.rs
sign-in-to-name = { $name } にサインインする

# src/settings_window/providers/page.rs
sign-in = サインイン

# src/settings_window/providers/dialog.rs
for = { $v0 }用。

# src/settings_window/providers/dialog.rs
browser-session = ブラウザセッション

# src/settings_window/providers/dialog.rs
oauth-token = OAuthトークン

# src/settings_window/providers/dialog.rs
session-key = セッションキー

# src/settings_window/providers/dialog.rs
paste-the-sessionkey-value = sessionKey 値を貼り付けます

# src/settings_window/providers/dialog.rs
oauth-access-token = OAuth アクセストークン

# src/settings_window/providers/dialog.rs
the-saved-credential-is-replaced-only-after-the-new-one-passes-th = 保存された認証情報は、新しい認証情報がチェックに合格した後にのみ置き換えられます。

# src/settings_window/providers/dialog.rs
claude-credential = Claude 資格情報

# src/settings_window/providers/dialog.rs
check-and-save = 確認して保存する

# src/settings_window/providers/dialog.rs
key-name-optional = キー名 (オプション)

# src/settings_window/providers/dialog.rs
e-g-personal = 例: 個人用

# src/settings_window/providers/dialog.rs
leave-blank-to-use-the-name-from-openrouter = OpenRouter の名前を使用するには、空白のままにします。

# src/settings_window/providers/dialog.rs
minibar-checks-the-key-with-openrouter-before-saving-it = Minibar は保存前に OpenRouter でキーを確認します。

# src/settings_window/providers/dialog.rs
replace-api-key = APIキーを置き換えます

# src/settings_window/providers/page.rs
add-api-key = APIキーを追加

# src/settings_window/providers/page.rs
management-key = 管理キー

# src/settings_window/providers/dialog.rs
create-one-under-settings-management-keys-on-openrouter-ai = openrouter.ai の [設定] → [管理キー] で作成します。

# src/settings_window/providers/dialog.rs
replace-management-key = 管理キーを交換する

# src/settings_window/providers/page.rs
add-management-key = 管理キーの追加

# src/settings_window/providers/dialog.rs
key-name = キー名

# src/settings_window/providers/page.rs
rename-key = キーの名前を変更する

# src/settings_window/providers/dialog.rs
save = 保存

# src/settings_window/providers/dialog.rs
this-key = このキー

# src/settings_window/providers/dialog.rs
minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibarは { $hint } の追跡を停止します。キーは OpenRouter で動作し続けます。

# src/settings_window/providers/dialog.rs
remove-api-key = API キーを削除しますか?

# src/settings_window/tray.rs
remove = 削除する

# src/settings_window/providers/dialog.rs
minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibarに { $v0 } のクレジット残高と使用履歴が表示されなくなります。キーは OpenRouter で動作し続けます。

# src/settings_window/providers/dialog.rs
remove-management-key = 管理キーを削除しますか?

# src/settings_window/providers/dialog.rs
saved-in-windows-user-storage-never-in-the-settings-file = Windows ユーザー ストレージに保存され、設定ファイルには保存されません。

# src/settings_window/providers/dialog.rs
save-key = キーを保存

# src/settings_window/providers/dialog.rs
minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibarは { $v0 } の保存されたキーを忘れます。 OpenCode で動作し続けます。

# src/settings_window/providers/dialog.rs
signing-in = サインイン中…

# src/settings_window/providers/page.rs
checking = 確認中…

# src/settings_window/providers/dialog.rs
choose-a-provider = プロバイダーを選択します。

# src/settings_window/providers/dialog.rs
reason-it-is-already-in-the-list = { $reason }。すでにリストに入っています。

# src/settings_window/providers/dialog.rs
added-name = { $name }を追加しました。

# src/settings_window/providers/dialog.rs
could-not-add-the-provider-error = プロバイダーを追加できませんでした: { $error }

# src/settings_window/providers.rs
this-provider-no-longer-exists = このプロバイダーはもう存在しません。

# src/settings_window/providers/dialog.rs
could-not-delete-the-provider-error = プロバイダーを削除できませんでした: { $error }

# src/settings_window/providers/dialog.rs
paste-a-credential-first = 最初に認証情報を貼り付けます。

# src/settings_window/providers/dialog.rs
credential-saved-in-windows-user-storage = 認証情報は Windows ユーザー ストレージに保存されます。

# src/settings_window/providers/dialog.rs
paste-a-key-first = 最初にキーを貼り付けます。

# src/settings_window/providers/dialog.rs
this-account-no-longer-exists = このアカウントはもう存在しません。

# src/settings_window/providers/dialog.rs
api-key-saved-in-windows-user-storage = API キーは Windows ユーザー ストレージに保存されます。

# src/settings_window/providers/dialog.rs
management-key-replaced = 管理キーを交換しました。

# src/settings_window/providers/dialog.rs
management-key-added = 管理キーが追加されました。

# src/settings_window/providers/dialog.rs
could-not-rename-the-key-error = キーの名前を変更できませんでした: { $error }

# src/settings_window/providers/dialog.rs
api-key-renamed = API キーの名前が変更されました。

# src/settings_window/providers/dialog.rs
could-not-remove-the-key-error = キーを削除できませんでした: { $error }

# src/settings_window/providers/dialog.rs
api-key-removed = API キーが削除されました。

# src/settings_window/providers/dialog.rs
management-key-removed = 管理キーが削除されました。

# src/settings_window/providers/dialog.rs
could-not-save-the-key-error = キーを保存できませんでした: { $error }

# src/settings_window/providers/dialog.rs
api-key-saved = API キーが保存されました。

# src/settings_window/providers/dialog.rs
switch-source-to-config-folder-to-sign-in = サインインするには「ソース」を「設定フォルダー」に切り替えてください。

# src/settings_window/providers/dialog.rs
this-provider-has-no-config-folder = このプロバイダーには構成フォルダーがありません。

# src/settings_window/providers/dialog.rs
another-instance-already-reads-this-config-folder-choose-a-differ = 別のインスタンスがすでにこの構成フォルダーを読み取っています。最初に別のフォルダーを選択してください。

# src/settings_window/providers/dialog.rs
this-provider-has-no-sign-in = このプロバイダーにはサインインがありません。

# src/settings_window/providers/dialog.rs
signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 } がサインインしました。CLI は、config フォルダー内でログインを最新の状態に保ちます。

# src/settings_window/providers/page.rs
no-keys-yet = まだ鍵がありません

# src/settings_window/providers/page.rs
management-key-5c8cf2 = { " " }· 管理キー

# src/settings_window/providers/page.rs
using-a-saved-credential = 保存された認証情報の使用

# src/settings_window/providers/page.rs
paste-a-credential-under-account = 「アカウント」の下に資格情報を貼り付けます

# src/settings_window/providers/page.rs
needs-an-api-key-or-opencode-sign-in = API キーまたは OpenCode サインインが必要です

# src/settings_window/providers/page.rs
needs-an-api-key = API キーが必要です

# src/settings_window/providers/page.rs
not-found-set-its-folder-under-runtime = 見つかりませんでした。そのフォルダーを「ランタイム」の下に設定します。

# src/settings_window/providers/page.rs
using-a-saved-api-key = 保存した API キーの使用

# src/settings_window/providers/page.rs
using-opencode-sign-in-or-local-history = OpenCode サインインまたはローカル履歴の使用

# src/settings_window/providers/page.rs
reading-cli = { $cli }を読む

# src/settings_window/providers/page.rs
reading-crew = { $crew }を読む

# src/settings_window/providers/page.rs
reading-app = { $app }を読む

# src/settings_window/providers/page.rs
checking-kiro-ide-kiro-crew-and-cli = Kiro IDE、Kiro Crew、および CLI をチェックしています…

# src/settings_window/providers/page.rs
checking-installed-app-and-cli = インストールされているアプリと CLI を確認しています…

# src/settings_window/providers/page.rs
checking-cli = CLIをチェック中…

# src/settings_window/providers/page.rs
checking-installed-app = インストールされているアプリを確認しています…

# src/settings_window/providers/page.rs
no-providers-yet-add-one-to-start-reading-limits = プロバイダーがまだありません。追加すると利用枠の読み取りを開始します。

# src/settings_window/providers/page.rs
is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } は無効のため、Minibar や通知領域に表示されません。有効にすると使用量の読み取りを開始します。

# src/settings_window/providers/page.rs
replace-chatgpt-logo-with-codex = ChatGPT ロゴを Codex に置き換えます

# src/settings_window/providers/page.rs
limits-only = 利用枠のみ

# src/settings_window/providers/page.rs
delete-provider = プロバイダーの削除

# src/settings_window/providers/page.rs
display-name = 表示名

# src/settings_window/providers/page.rs
shown-on-popup-tabs-home-cards-the-tray-and-notifications = ポップアップのタブ、ホームのカード、通知領域、通知に表示されます。

# src/settings_window/providers/page.rs
up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = 文字は3文字まで。名前のイニシャルを使用する場合は、空白のままにします。プロバイダーが複数のインスタンスを有効にしている場合に表示されます。

# src/settings_window/providers/page.rs
auto-uses-a-neutral-plate-that-follows-the-theme = オートではテーマに沿ったニュートラルプレートを採用。

# src/settings_window/providers/page.rs
show-on-home = ホームで表示

# src/settings_window/providers/page.rs
its-provider-tab-stays-available-when-hidden-from-home = そのプロバイダー タブは、ホームから非表示になっている場合でも利用可能なままです。

# src/settings_window/providers/page.rs
credential = 認証情報

# src/settings_window/providers/page.rs
signed-in-as-identity = { $identity } としてサインインしています

# src/settings_window/providers/page.rs
saved-in-windows-user-storage = Windows ユーザーストレージに保存

# src/settings_window/providers/page.rs
paste-a-sessionkey-or-an-oauth-access-token = sessionKey または OAuth アクセス トークンを貼り付けます。

# src/settings_window/providers/page.rs
reads-limits-only-minibar-cannot-refresh-a-pasted-credential = 利用枠のみを読み取ります。貼り付けた認証情報は Minibar で更新できません。

# src/settings_window/providers/page.rs
replace-credential = 資格情報を置き換える

# src/settings_window/providers/page.rs
add-credential = 認証情報の追加

# src/settings_window/providers/page.rs
signed-in-account = サインインしたアカウント

# src/settings_window/providers/page.rs
not-signed-in-yet-or-no-limits-read-so-far = 未サインイン、または利用枠がまだ読み取られていません。

# src/settings_window/providers/page.rs
account = アカウント

# src/settings_window/providers/page.rs
runtime = 実行環境

# src/settings_window/providers/page.rs
minibar-finds-these-automatically = { $v0 } Minibarはこれらを自動的に見つけます。

# src/settings_window/providers/page.rs
in-use = 使用中

# src/settings_window/providers/page.rs
found = 見つかりました

# src/settings_window/providers/page.rs
copy-path = パスをコピーする

# src/settings_window/providers/page.rs
open-folder = フォルダーを開く

# src/settings_window/providers/page.rs
path-copied = パスがコピーされました。

# src/settings_window/providers/page.rs
not-installed-or-installed-somewhere-minibar-doesn-t-look = 未インストール、または Minibar の検索対象外の場所にインストールされています。

# src/settings_window/providers/page.rs
not-found = 見つかりません

# src/settings_window/providers/page.rs
choose-folder = フォルダを選択してください…

# src/settings_window/providers/page.rs
select-folder = フォルダを選択

# src/settings_window/providers/page.rs
choose-folder-1838a4 = フォルダーを選択してください

# src/settings_window/providers/page.rs
only-needed-if-automatic-detection-misses-your-install = 自動検出でインストールが失敗する場合にのみ必要です。

# src/settings_window/providers/page.rs
source = ソース

# src/settings_window/providers/page.rs
config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = Config フォルダーは、CLAUDE_CONFIG_DIR の Claude Code ログインを読み取ります。手動認証情報は、貼り付けられた認証情報からのみ制限を読み取ります。

# src/settings_window/providers/page.rs
config-folder = 設定フォルダー

# src/settings_window/providers/page.rs
manual = マニュアル

# src/settings_window/providers/page.rs
passed-to-the-cli-as-env-leave-empty-to-use = CLI に { $env } として渡されます。 { $v0 } を使用するには、空のままにしておきます。

# src/settings_window/providers/page.rs
the-standard-folder = 標準フォルダー

# src/settings_window/providers/page.rs
a-folder-minibar-creates-for-this-instance = Minibarがこのインスタンス用に作成するフォルダー

# src/settings_window/providers/page.rs
other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } がすでにこのフォルダーを読み取っています。同じログイン情報を2つのインスタンスで共有すると使用量が二重に集計されるため、共有しないでください。

# src/settings_window/providers/page.rs
automatic-activation = 自動アクティベーション

# src/settings_window/providers/page.rs
starts-this-account-s-5-hour-window-when-it-resets-using-its-own = このアカウントの利用枠がリセットされたら、自身のログイン情報で5時間セッションを開始します。スケジュールと休止時間は「セッションの自動開始」で設定します。

# src/settings_window/providers/page.rs
usage-statistics = 使用状況の統計

# src/settings_window/providers/page.rs
scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = 使用量カードの表示のため、このインスタンスのローカル履歴をスキャンします。収集を完全に停止するにはオフにしてください。

# src/settings_window/providers/page.rs
opencode-sign-in-or-local-history = OpenCode サインインまたはローカル履歴

# src/settings_window/providers/page.rs
found-in-opencode-auth-environment-a-saved-key-or-local-history = OpenCode 認証、環境、保存されたキー、またはローカル履歴で見つかります。

# src/settings_window/providers/page.rs
nothing-found-in-opencode-auth-environment-or-local-history = OpenCode 認証、環境、またはローカル履歴に何も見つかりませんでした。

# src/settings_window/providers/page.rs
replace-key = キーを交換する

# src/settings_window/providers/page.rs
optional-only-needed-without-opencode-sign-in-on-this-pc = オプション。この PC で OpenCode サインインを行わない場合にのみ必要です。

# src/settings_window/providers/page.rs
add-the-key-of-the-account-this-instance-tracks = このインスタンスが追跡するアカウントのキーを追加します。

# src/settings_window/providers/page.rs
keys = キー

# src/settings_window/providers/page.rs
a-management-key-shows-credit-balance-and-usage-history-api-keys = 管理キーにはクレジット残高や利用履歴が表示されます。 API キーには、キーごとの費用が表示されます。

# src/settings_window/providers/page.rs
could-not-read-the-saved-key-reopen-this-page-to-retry = 保存されたキーを読み取れませんでした。このページをもう一度開いて再試行してください。

# src/settings_window/providers/page.rs
credit-balance-and-account-wide-usage-history = クレジット残高とアカウント全体の使用履歴

# src/settings_window/providers/page.rs
not-added-add-one-to-see-credit-balance-and-usage-history = 追加されていません。クレジット残高と使用履歴を確認するには、1 つ追加します。

# src/settings_window/providers/page.rs
last-updated = 最終更新日 { $v0 }、{ $v1 }

# src/settings_window/providers/page.rs
no-api-keys-yet-add-one-to-track-spend-per-key = API キーはまだありません。キーごとの支出を追跡するには 1 つ追加します。

# src/settings_window/providers/page.rs
key = キー

# src/settings_window/providers/page.rs
spend = 支出

# src/settings_window/providers/page.rs
limit = 利用上限

# src/settings_window/providers/page.rs
error-reopen-this-page-to-retry = { $error }。このページをもう一度開いて再試行してください。

# src/settings_window/providers/page.rs
could-not-read-key = キーを読み取れませんでした

# src/settings_window/providers/page.rs
not-saved = 保存されていません

# src/settings_window/providers/page.rs
unnamed = 無名

# src/settings_window/providers/page.rs
of-the-limit = { $v1 } 制限の { $v0 }

# src/settings_window/providers/page.rs
no-spend-limit-key-spend-account-credits-purchased = 支出上限なし。キーの支出: { $v0 }。アカウントで購入したクレジット: { $v1 }。

# src/settings_window/providers/page.rs
none = なし

# src/settings_window/providers/page.rs
add-key = キーを追加

# src/settings_window/providers.rs
openrouter-account-credentials-are-configured = OpenRouter アカウントの資格情報が構成されています

# src/settings_window/providers.rs
saved-api-key = 保存された API キー

# src/settings_window/providers.rs
saved-credential = 保存された認証情報

# src/settings_window/providers.rs
opencode-auth-json-or-local-database = OpenCode auth.json またはローカル データベース

# src/settings_window/providers.rs
that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = OpenRouter キーとは思えません。キーは sk-or- で始まります。

# src/settings_window/providers.rs
codex-cli-folder = Codex CLI フォルダー

# src/settings_window/providers.rs
claude-code-cli-folder = Claude Code CLI フォルダー

# src/settings_window/providers.rs
cursor-app-folder = Cursorアプリフォルダー

# src/settings_window/providers.rs
grok-cli-folder = Grok CLI フォルダー

# src/settings_window/providers.rs
kiro-ide-folder = Kiro IDE フォルダー

# src/settings_window/providers.rs
kiro-crew-app-path = Kiro Crew アプリのパス

# src/settings_window/providers.rs
kiro-cli-folder = Kiro CLI フォルダー

# src/settings_window/providers.rs
reads-the-signed-in-codex-cli-or-desktop-app = サインインした Codex CLI またはデスクトップ アプリを読み取ります。

# src/settings_window/providers.rs
reads-your-existing-claude-code-login = 既存の Claude Code ログインを読み取ります。

# src/settings_window/providers.rs
reads-the-signed-in-cursor-app-for-this-billing-cycle = この請求サイクルのサインイン済み Cursor アプリを読み取ります。

# src/settings_window/providers.rs
reads-zen-auth-and-local-opencode-history = Zen 認証とローカルの OpenCode 履歴を読み取ります。

# src/settings_window/providers.rs
reads-go-quota-windows-and-local-opencode-history = Go クォータ ウィンドウとローカル OpenCode 履歴を読み取ります。

# src/settings_window/providers.rs
reads-api-key-usage-and-spend-limits-a-management-key-also-enable = API キーの使用量と使用制限を読み取ります。管理キーを使用すると、使用履歴やクレジット残高も確認できます。

# src/settings_window/providers.rs
reads-subscription-quota-from-your-existing-official-agy-windows = 既存の公式 agy Windows サインインからサブスクリプション クォータを読み取ります。

# src/settings_window/providers.rs
reads-supergrok-subscription-credits-from-your-existing-official = 既存の公式 Grok CLI サインインから SuperGrok サブスクリプション クレジットを読み取ります。

# src/settings_window/providers.rs
fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = 共有サインインを使用して、Kiro のライブ月間クレジットを取得します。 IDE、Crew、および CLI インストールを認識します。

# src/settings_window/providers.rs
codex-desktop-app = Codex デスクトップ アプリ

# src/settings_window/providers.rs
codex-cli = Codex CLI

# src/settings_window/providers.rs
claude-desktop-app = Claude デスクトップ アプリ

# src/settings_window/providers.rs
claude-code-cli = Claude Code CLI

# src/settings_window/providers.rs
cursor-app = Cursorアプリ

# src/settings_window/providers.rs
antigravity-app = Antigravityアプリ

# src/settings_window/providers.rs
grok-cli = Grok CLI

# src/settings_window/providers.rs
kiro-ide = Kiro IDE

# src/settings_window/providers.rs
kiro-crew = Kiro Crew

# src/settings_window/providers.rs
kiro-cli = Kiro CLI

# src/settings_window/providers.rs
supports-one-instance = { $v0 } は 1 つのインスタンスをサポートします

# src/settings_window/tray.rs
numbers = 数字

# src/settings_window/tray.rs
progress-bars = プログレスバー

# src/settings_window/tray.rs
rings = リング

# src/settings_window/tray.rs
reset-time = リセット時刻

# src/settings_window/tray.rs
countdown = カウントダウン

# src/settings_window/tray.rs
status = ステータス

# src/settings_window/tray.rs
fixed = 固定

# src/settings_window/tray.rs
app-accent = アプリのアクセント

# src/settings_window/tray.rs
percentages-as-digits = パーセンテージを数字で表示

# src/settings_window/tray.rs
one-bar-per-indicator = インジケーターごとに 1 つのバー

# src/settings_window/tray.rs
nested-rings-one-per-indicator = ネストされたリング、インジケーターごとに 1 つ

# src/settings_window/tray.rs
when-the-limit-resets = リミットがリセットされるとき

# src/settings_window/tray.rs
time-left-until-the-reset = リセットまでの残り時間

# src/settings_window/tray.rs
used = 使用済み

# src/settings_window/tray.rs
remaining = 残り

# src/settings_window/tray.rs
app-icon = アプリアイコン

# src/settings_window/tray.rs
widget = ウィジェット { $v0 }

# src/settings_window/tray.rs
the-codex-minibar-icon = Codex Minibar アイコン

# src/settings_window/tray.rs
style-no-indicators = { $style } · インジケーターなし

# src/settings_window/tray.rs
widget-removed = ウィジェットが削除されました

# src/settings_window/tray.rs
undo = 元に戻す

# src/settings_window/tray.rs
tray-icon = トレイアイコン

# src/settings_window/tray.rs
shows-the-app-icon = アプリのアイコンを表示します。

# src/settings_window/tray.rs
add-widget = ウィジェットを追加する

# src/settings_window/tray.rs
add-app-icon = アプリアイコンを追加

# src/settings_window/tray.rs
move-up = 上へ移動

# src/settings_window/tray.rs
move-down = 下へ移動

# src/settings_window/tray.rs
edit = 編集

# src/settings_window/tray.rs
duplicate = 複製

# src/settings_window/tray.rs
shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = 通知領域に Codex Minibar アイコンを表示します。設定するインジケーターはありません。

# src/settings_window/tray.rs
style = スタイル

# src/settings_window/tray.rs
up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = 最大 { $max_indicators } 個の利用枠をこの順序で表示します。項目を展開すると変更できます。

# src/settings_window/tray.rs
indicators = 指標

# src/settings_window/tray.rs
remove-widget = ウィジェットを削除する

# src/settings_window/tray.rs
add-indicator = インジケーターを追加

# src/settings_window/tray.rs
unsupported = サポートされていない{ $v0 }

# src/settings_window/tray.rs
remove-indicator = インジケーターを削除

# src/settings_window/tray.rs
unsupported-bba2a8 = サポートされていません ({ $v0 })

# src/settings_window/tray.rs
unavailable-0c5d75 = 利用不可 ({ $v0 })

# src/settings_window/tray.rs
metric = 指標

# src/settings_window/tray.rs
show = 表示する

# src/settings_window/tray.rs
color = 色

# src/settings_window/tray.rs
fixed-color = 固定色

# src/settings_window/troubleshoot.rs
troubleshooting-could-not-start = トラブルシューティングを開始できませんでした

# src/settings_window/troubleshoot.rs
choose-which-installed-ai-tool-should-investigate-the-problem = インストールされているどの AI ツールが問題を調査するかを選択します。

# src/settings_window/troubleshoot.rs
open-terminal = ターミナルを開く

# src/settings_window/window.rs
version-is-available = { $version }は利用可能です

# src/settings_window/window.rs
update-now = 今すぐアップデートしてください

# src/settings_window/window.rs
missing-a-provider = プロバイダーが見つかりませんか?

# src/settings_window/window.rs
ask-for-it-or-build-it-yourself = 依頼するか、自分で構築してください。

# src/settings_window/window.rs
request-a-provider = プロバイダーの追加を要望

# src/settings_window/window.rs
contribute-one = プロバイダーを実装

# src/tray.rs
disabled = 無効

# src/updater.rs
update-complete = アップデート完了

# src/updater.rs
now-running-version = 現在{ $version }を実行しています。

# src/usage_overview.rs
past-24h = 過去24時間

# application
language = 言語

# application
applies-immediately-to-every-app-window-and-notification-auto-fol = すべての画面と通知に即座に適用されます。「自動」は Windows の表示言語に従います。

# application
auto-windows = 自動 (Windows)

# application
english = 英語

# application
msg-russian = Русский

# application
codex-minibar-settings = Codex Minibar の設定

# application
welcome-to-codex-minibar = Codex Minibar へようこそ

# application
exit = 終了

# application
update-available-67fd3a = 利用可能なアップデート

# application
monthly-credits-976559 = 毎月のクレジット

# application
msg-5h-session-de7ce8 = 5時間セッション

# application
name-weekly = 毎週 { $name }

# application
tokens-94e0b9 = { $v0 }: { $v1 } トークン

# application
requests-priced = { $v0 } 件のリクエスト · { $v1 } 件の料金算出済み

# application
requests = { $v0 } 件のリクエスト

# application
credits-338f52 = クレジット

# application
cloud-session-credits = クラウドセッションクレジット

# application
name-login-expires-in-days-left = { $name } のログインは { $days_left } 日後に期限切れになります

# application
name-error = { $name }エラー

# application
sessions = { $v0 } セッション

# application
api-estimate = API 料金の推定

# application
start-to-end = { $start } ～ { $end }

# application
to = { $v0 } ～ { $v1 }

# application
share-1-of-other = { $v0 } の { $share }% · { $other }

# application
excluded-other = 除外 · { $other }

# application
exclude-from-usage-stats = クリックして使用状況統計から除外します

# application
include-in-usage-stats = クリックして使用状況統計に含めます

# application
cost-885dc4 = コスト

# application
tokens-339143 = トークン

# application
sessions-0e5e29 = · { $v0 } セッション

# application
api-key = APIキー

# application
api-keys = API キー

# application
name-deleted = { $name } を削除しました。

# application
openrouter-api-key-no-longer-exists = OpenRouter API キーは存在しません

# application
openrouter-account-no-longer-exists = OpenRouter アカウントはもう存在しません

# application
credit = { $v0 } クレジット

# application
agy-cli-folder = agy CLI フォルダー

# application
folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = codex.exe、codex.cmd、または codex.ps1 のフォルダー。自動的に検索されるようにするには、空のままにします。

# application
folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = claude.exe、claude.cmd、または claude.ps1 のフォルダー。自動的に検索されるようにするには、空のままにします。

# application
folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = Cursor.exeのあるフォルダー。自動的に検索されるようにするには、空のままにします。使用量は引き続きサインインしたプロファイルから取得されます。

# application
folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = agy.exe、agy.cmd、または agy.ps1 のフォルダー。自動的に検索されるようにするには、空のままにします。

# application
folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = grok.exe、grok.cmd、または grok.ps1 のフォルダー。自動的に検索されるようにするには、空のままにします。

# application
folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = Kiro.exe を含むフォルダー、または実行可能ファイル自体。自動的に検索されるようにするには、空のままにします。

# application
folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = KiroCrew.exe を含むフォルダー、または実行可能ファイル自体。ユーザーごとおよびすべてのユーザーのインストールを自動的に検出するには、空のままにします。

# application
folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = kiro-cli.exe を含むフォルダー、または実行可能ファイル自体。自動的に検索されるようにするには、空のままにします。

# application
off-45080e = { $v0 } (オフ)

# application
each-widget-is-one-icon-in-the-notification-area-indicators-show = 各ウィジェットは通知領域の1つのアイコンです。インジケーターはプロバイダーの利用枠を数字、バー、リング、リセット時刻で表示します。

# application
a-reset-clock-follows-one-quota = リセット時計は1つの利用枠を表示します。

# application
ai-tool = AIツール

# application
msg-7-days = 7日間

# application
msg-30-days = 30日

# application
msg-90-days = 90日

# application
yellow = 黄色

# application
custom = カスタム

# application
not-available-for-manual-credentials-switch-source-to-config-fold = 手動で入力した認証情報では利用できません。「ソース」を「設定フォルダー」に切り替えてください。

# application
opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = OpenCode のローカル履歴は、最初の OpenCode インスタンスによって追跡されます。

# application
this-provider-has-no-local-usage-history = このプロバイダーにはローカルでの使用履歴がありません。

# application
this-provider-has-no-session-window-to-start = このプロバイダーには、開始するセッション ウィンドウがありません。

# application
this-provider-does-not-use-a-config-folder = このプロバイダーは構成フォルダーを使用しません。

# application
could-not-save-provider-settings-error-restoring-its-previous-cre = プロバイダー設定を保存できませんでした ({ $error })。以前の資格情報の復元も失敗しました ({ $rollback_error })。

# application
msg-5h-7d = 5時間 |  { $v0 } |  { $v1 }
    7d |  { $v2 } |  { $v3 }

# API key count in the provider header.
api-key-count = { $v0 } 個の API キー

# Application copy
widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = ホームと使用量の表示を広げます。ホームのブロックを列の間でドラッグできます。プロバイダーのタブはコンパクトなままです。

# Application copy
a-possible-codex-reset-is-scheduled-for-when-in-countdown = Codex のリセットが { $when } に予定されている可能性があります（あと { $countdown }）

# Application copy
label-possible-reset-on-when-in-countdown = { $label }: { $when } にリセットの可能性（あと { $countdown }）

# Application copy
new-codex-reset-info = 新しいCodexリセット情報

# Application copy
login-expires-soon = { $v0 } ログインの有効期限がまもなく切れます

# Application copy
it-stops-renewing-on-open-minibar-and-choose-sign-in-again = { $v0 } に自動更新が停止します。Minibar を開き、「再度サインイン」を選択してください。

# Application copy
could-not-clear-usage-data-error = 使用状況データをクリアできませんでした: { $error }

# Application copy
succeeded-at = { $v0 } は { $v1 } で成功しました

# Application copy
failed-at-error = { $v0 } が { $v1 } で失敗しました: { $error }

# Application copy
key-39df89 = キー { $v0 }

# Application copy
expired = 期限切れ

# Application copy
jan = 1月

# Application copy
feb = 2月

# Application copy
mar = 3月

# Application copy
apr = 4月

# Application copy
may = 5月

# Application copy
jun = 6月

# Application copy
jul = 7月

# Application copy
aug = 8月

# Application copy
sep = 9月

# Application copy
oct = 10月

# Application copy
nov = 11月

# Application copy
dec = 12月

# { $value }% used
quota-percent-used = { $value }% 使用済み

# { $value }% left
quota-percent-left = 残り { $value }%

# { $amount } of { $limit } used
cloud-amount-used = { $limit } 中 { $amount } 使用済み

# { $amount } of { $limit } left
cloud-amount-left = { $limit } 中、残り { $amount }

# Short usage range
range-24h = 24時間

# Short usage range
range-7d = 7日

# Short usage range
range-30d = 30日

# Short usage range
range-90d = 90日

# Home usage card title
home-usage-title = 使用量

# OpenRouter key administration

could-not-reach-openrouter = OpenRouter に到達できませんでした


openrouter-keys-title = キー


openrouter-keys-new = 新しいキー


openrouter-keys-new-title = 新しいキー


openrouter-keys-back = 戻る


openrouter-keys-updating = 更新中…


openrouter-keys-loading = キーをロード中…


openrouter-keys-load-failed = キーをロードできませんでした


openrouter-keys-retry = 再試行


openrouter-keys-empty = このアカウントにはまだ鍵がありません。


openrouter-keys-this-app = このアプリ


openrouter-keys-no-limit = 制限なし


openrouter-keys-of-limit = { $amount } / { $limit }


openrouter-keys-resets-daily = 毎日リセットされる


openrouter-keys-resets-weekly = 毎週リセット


openrouter-keys-resets-monthly = 毎月リセット


openrouter-keys-expires-on = 有効期限は{ $date }です


openrouter-keys-expired-on = 期限切れ { $date }


openrouter-keys-today = 今日 { $amount }


openrouter-keys-usage-breakdown = 今日 { $today } · 週 { $week } · 月 { $month } · 合計 { $total }


openrouter-keys-show-all = すべての { $count } 個のキーを表示


openrouter-keys-show-fewer = 表示数を減らす


openrouter-keys-spending-limit = 支出制限


openrouter-keys-limit-hint = 制限を設けない場合は空のままにします。


openrouter-keys-resets = リセット


openrouter-keys-reset-never = なし


openrouter-keys-reset-daily = 毎日


openrouter-keys-reset-weekly = 毎週


openrouter-keys-reset-monthly = 毎月


openrouter-keys-byok = 制限に向けて BYOK の使用量をカウントします


openrouter-keys-enabled = 有効


openrouter-keys-delete = 削除


openrouter-keys-delete-tracked = このアプリが使用中のキーです。先に設定で削除してください。


openrouter-keys-saving = 保存中…


openrouter-keys-delete-title = 「{ $name }」を削除しますか?


openrouter-keys-delete-message = このキーを使用中のアプリやサービスは直ちに動作しなくなります。削除は元に戻せません。無効化なら後から元に戻せます。


openrouter-keys-keep = キーを保持


openrouter-keys-delete-confirm = キーを削除


openrouter-keys-deleting = 削除中…


openrouter-keys-name = 名前


openrouter-keys-name-placeholder = 例: laptop-cursor


openrouter-keys-name-required = キーの名前を入力します。


openrouter-keys-invalid-amount = 25 または 12.50 などのドル金額を入力します。


openrouter-keys-expires = 有効期限


openrouter-keys-expires-1-hour = 1時間


openrouter-keys-expires-1-day = 1日


openrouter-keys-expires-7-days = 7日間


openrouter-keys-expires-30-days = 30日


openrouter-keys-expires-90-days = 90日


openrouter-keys-expires-180-days = 180日


openrouter-keys-expires-1-year = 1年


openrouter-keys-expires-never = 有効期限なし


openrouter-keys-track = Minibarでこのキーを追跡する


openrouter-keys-create = キーの作成


openrouter-keys-creating = 作成中…


openrouter-keys-created = キーが作成されました


openrouter-keys-copy = キーをコピーする


openrouter-keys-copied = コピーされました


openrouter-keys-done = 完了


openrouter-keys-once-title = このキーは二度と表示されません


openrouter-keys-once-message = OpenRouter では新しいキーは一度しか表示されません。今すぐコピーしてください。


openrouter-keys-pinned = 「完了」を押すまで、ポップアップは開いたままになります。


openrouter-keys-tracking = Minibar にキーを追加中…


openrouter-keys-tracked = Minibarはこのキーを追跡するようになりました。


openrouter-keys-track-failed = Minibar にキーを追加できませんでした: { $error }


openrouter-keys-no-management-key = このアカウントには管理キーがありません。


openrouter-keys-management-key-rejected = OpenRouter が管理キーを拒否しました。設定で置き換えてください。


openrouter-keys-not-a-management-key = このキーは他のキーを管理できません。管理キーを使用します。


openrouter-keys-key-not-found = このキーはもう存在しません。

# src/settings_window/notifications.rs
notification-sounds = 通知音

# src/settings_window/notifications.rs
plays-a-short-sound-with-each-notification = 通知ごとに短い音を再生します
