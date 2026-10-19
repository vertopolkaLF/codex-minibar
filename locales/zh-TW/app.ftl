# Codex Minibar — zh-TW
# Full Fluent catalog; parameters and technical names remain unchanged.

luna-reserve = Luna 儲備額度

on-pace = 用量進度正常

delta-in-deficit = 用量超前 { $delta }%

delta-in-reserve = 剩餘緩衝 { $delta }%

msg-5-hour-limit-started = 5 小時額度已開始

msg-5-hour-limit-reset-and-activated = 5 小時額度已重置並啟用

msg-5-hour-limit-reset = 5 小時額度已重置

weekly-limit-reset = 每週額度已重置

name-5-hour = { $name } · 5 小時

label-limit-is-low = { $label } 剩餘額度不足

expired-at-time = 已於 { $time } 到期

expired-at-time-6c39ff = 已於 { $time } { $v0 } 到期

never = 從不

unlimited = 無限制

unavailable = 無法使用

days-d-hours-h = { $days }天 { $hours }小時

days-d = { $days }天

hours-h-minutes-m = { $hours }小時 { $minutes }分鐘

hours-h = { $hours }小時

minutes-m = { $minutes }分鐘

waiting-for-first-update = 等待首次更新

just-now = 剛剛

seconds-seconds-ago = { $seconds } 秒前

minutes-ago = { $v0 } 分鐘前

updated-elapsed = 更新於 { $elapsed }

the-request-timed-out-try-refreshing-again = 請求逾時。請重新整理後再試一次。

the-secure-connection-could-not-be-verified-see-log-for-details = 無法驗證安全連線。詳情請檢視記錄。

the-provider-closed-the-connection-try-refreshing-again = 服務供應商已關閉連線。請重新整理後再試一次。

the-provider-s-address-could-not-be-resolved-check-your-connectio = 無法解析服務供應商位址。請檢查網路連線。

could-not-connect-to-the-provider-check-your-connection-and-try-a = 無法連線到服務供應商。請檢查網路連線後重試。

the-management-key-was-rejected-update-it-in-settings = 管理金鑰被拒絕。請在設定中更新。

authentication-failed-sign-in-again-or-update-the-provider-key = 身分驗證失敗。請重新登入或更新服務供應商金鑰。

access-denied-by-the-provider-http-403 = 服務供應商拒絕存取（HTTP 403）。

too-many-requests-wait-a-few-minutes-before-refreshing-again = 請求過多。請等待幾分鐘後再重新整理。

the-provider-is-temporarily-unavailable-try-again-later = 服務供應商暫時不可用。請稍後重試。

the-provider-rejected-the-request-see-log-for-details = 服務供應商拒絕了請求。詳情請檢視記錄。

the-requested-resource-was-not-found-see-log-for-details = 未找到請求的資源。詳情請檢視記錄。

the-provider-returned-an-unexpected-response-try-refreshing-again = 服務供應商傳回非預期的回應。請重新整理後再試一次。

the-request-failed-see-log-for-details = 請求失敗。詳情請檢視記錄。

input = 輸入

cache = 快取

output = 輸出

input-uncached = 輸入（未快取）

cached-input = 快取輸入

value-partially-priced = { $value }（部分已計價）

could-not-load-model-data-error = 無法載入模型資料：{ $error }

usage-activity = 用量活動

waiting-for-cursor-s-usage-export-refresh-to-retry = 等待 Cursor 匯出用量。請重新整理以重試。

model-data-unavailable = 模型資料不可用

loading-models = 正在載入模型…

no-model-data = 暫無模型資料

no-usage-data = 暫無用量資料

no-series-selected = 未選擇資料系列

no-tokens-in-this-period = 此期間沒有 { $v0 } 的 token

loading-model-breakdown = 正在載入模型明細

group-tokens-or-cost-by-model = 按模型分組 token 或費用

model = 模型

# src/popup_window/ui/usage.rs
project = 專案

# src/usage_overview.rs
unknown-project = 未知專案

# src/popup_window/ui/usage.rs
no-project-usage = 此期間沒有專案資料

tokens = Token

cost = 費用

no-cost-data-for-this-period = 此期間暫無費用資料

no-token-data-for-this-period = 此期間暫無 token 資料

daily-cost-in-usd = 每日費用（USD）

daily-token-volume = 每日 token 數量

of-total-models-scroll-for-more = 第 { $v0 }–{ $v1 } 個，共 { $total } 個模型 · 捲動檢視更多

cost-usd-by-model = 各模型費用（USD）

tokens-by-model = 各模型 token 數量

today = 今天

last-period-days = 最近 { $period } 天

no-data = 暫無資料

loading-usage-statistics = 正在載入用量統計…

available-balance = 可用餘額

resets-in = 距重置還有

session-not-started = 尚未開始

expires-in = 距到期還有

usage = 用量：

remove-key = 移除金鑰

msg-1-banked-reset = 1 次已儲備重置

count-banked-resets = { $count } 次重置

available-to-use = 可用

no-expiration-date = 無到期日期

banked-reset = 已儲備重置 { $v0 }

codex-limits = Codex 額度

open-announcement-source = 開啟公告來源

source-not-provided = 未提供來源

tibo-reset = Tibo Reset™

home = 首頁

usage-0bb186 = 用量

settings = 設定

install-update = 安裝更新

refreshing-limits-and-usage = 正在重新整理額度和用量…

updated = 已更新{ " " }

refresh-last-updated-relative = 重新整理 · 上次更新於 { $relative }

edit-home = 編輯首頁
home-card-layout = 版面配置
home-card-layout-cards = 卡片
home-card-layout-lines = 列
home-card-layout-rings = 圓環
home-card-layout-tiles = 磚塊
home-card-layout-classic = 經典
drag-to-reorder = 拖曳以重新排序

drop-here = 拖放到此處

usage-stats = 用量統計

loading-usage = 正在載入用量…

update-failed = 更新失敗

something-went-wrong = 出現了問題

no-providers-enabled = 未啟用任何服務供應商

turn-one-on-in-settings-providers = 請在設定 > 服務供應商中啟用一個。

sign-in-again-to-keep-limits-updating = 請重新登入以繼續更新額度。

sign-in-again = 重新登入

total = 總計

enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = 在設定中啟用服務供應商並將其納入用量統計，以檢視本機 API 用量。

hourly-cost = 每小時費用

hourly-processed-tokens = 每小時處理的 token

no-activity-in-this-range = 此範圍內暫無活動

day = 天

breakdown = 明細

totals = 總計

processed-tokens = 已處理 token

uncached-input = 未快取輸入

cache-savings = 快取節省

share = 佔比

hour = 小時

sat = 週六

sun = 週日

msg-5h-session = 5 小時工作階段

weekly = 每週

cursor-models = Cursor 模型

other-models = 其他模型

all-models = 所有模型

grok-bot = Grok Bot

monthly = 每月

spending-limit = 支出上限

gemini = Gemini

claude-gpt = Claude + GPT

credits = 點數

monthly-credits = 每月點數

codex = Codex

claude = Claude

cursor = Cursor

opencode-zen = OpenCode Zen

opencode-go = OpenCode Go

openrouter = OpenRouter

antigravity = Antigravity

grok = Grok

kiro = Kiro

banked-resets = 已儲備重置

usage-stats-7b9e1a = 用量統計

spending = 支出

separate-tabs = 獨立分頁

grouped-switcher = 分組，切換帳號

grouped-all-accounts = 分組，顯示所有帳號

openrouter-account = OpenRouter 帳號

yesterday = 昨天

history-retention-must-be-between-1-and-365-days = 歷史保留時間必須為 1 至 365 天

session-low-usage-threshold-must-be-between-1-and-99-percent = 工作階段低用量門檻必須為 1% 至 99%

weekly-low-usage-threshold-must-be-between-1-and-99-percent = 每週低用量門檻必須為 1% 至 99%

check-github-for-a-new-version = 在 GitHub 檢查新版本

checking-for-updates = 正在檢查更新…

you-re-up-to-date = 已是最新版本

update-available = 更新 { $v0 } 可用

installing-update = 正在安裝更新…

couldn-t-check-for-updates = 無法檢查更新

version = 版本 { $v0 }

about-tagline = 免費開源的小工具，在 Windows 系統匣中檢視 AI 訂閱的用量額度、重置和花費。

a-new-release-is-ready-to-install = 新版本已準備好安裝。

what-s-new = 更新內容

update = 更新

check-for-updates = 檢查更新

check-for-updates-on-startup = 啟動時檢查更新

notify-when-a-new-version-is-found = 發現新版本時通知

github = GitHub

source-code = 原始碼

releases = 版本釋出

see-what-s-new = 檢視更新內容

report-an-issue = 報告問題

found-a-bug = 發現了錯誤？

author = 作者

updates = 更新

resources = 資源

mon = 週一

tue = 週二

wed = 週三

thu = 週四

fri = 週五

every-day = 每天

weekdays = 工作日

weekends = 週末

no-days = 未選擇日期

start-5-hour-sessions-automatically = 自動開始 5 小時工作階段

starts-a-new-session-as-soon-as-a-window-is-available-instead-of = 額度視窗可用時立即開始新工作階段，無需等待首次請求。每個帳號使用各自的登入資訊。

add-codex-or-claude-in-providers-first = 請先在服務供應商中新增 Codex 或 Claude。

off-in-providers = 已在服務供應商中關閉

quiet-periods = 暫停時段

don-t-auto-start-sessions-during-these-times = 在這些時段內不自動開始工作階段。

add = 新增

all-day = 全天

from = 從

until = 至

scheduled-activations = 排程啟用

start-a-5-hour-session-at-a-set-time = 在指排程間開始 5 小時工作階段。

time = 時間

turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = 請先在服務供應商中啟用 Codex 或 Claude，並使用設定資料夾登入。

no-quiet-periods-yet = 尚未設定暫停時段。

no-scheduled-activations-yet = 尚未設定排程啟用。

unknown-provider = 未知服務供應商

remove-quiet-period = 移除暫停時段

remove-activation = 移除啟用

provider = 服務供應商

days = 日期

export-settings = 匯出設定

save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = 將所有設定儲存為 .toml 檔案。已儲存的金鑰保留在 Windows 使用者儲存空間中。

export = 匯出

import-settings = 匯入設定

replace-the-current-settings-with-a-previously-exported-file = 使用之前匯出的檔案替換目前設定。

import = 匯入

clear-usage-data = 清除用量資料

delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = 刪除已收集的用量歷史。下次掃描時會從服務供應商本機記錄中重建。

clear = 清除

usage-data-clear-failed = 清除用量資料失敗

the-background-worker-is-unavailable = 背景工作程序不可用。

usage-data-cleared = 用量資料已清除。

reset-all-settings = 重置所有設定

restore-every-default-and-start-the-welcome-flow-again = 恢復所有預設設定並重新開始歡迎流程。

reset = 重置

backup = 備份

data = 資料

rendering = 繪製

software-rendering = 軟體繪製

software-rendering-description = 使用 CPU 而不是顯示卡繪製視窗。在遊戲佔滿 GPU 時可能有幫助；動畫可能不夠流暢。

settings-exported = 設定已匯出。

settings-export-failed = 匯出設定失敗

settings-imported = 設定已匯入。

settings-import-failed = 匯入設定失敗

settings-reset-failed = 重置設定失敗

reset-all-settings-bbfe66 = 重置所有設定？

every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = 所有設定將恢復預設值並重新開啟歡迎流程。已儲存的金鑰和用量資料會保留。

cancel = 取消

windows = Windows

light = 淺色

dark = 深色

color-theme = 顏色主題

applies-to-settings-the-popup-and-its-tray-menu = 套用至設定、彈出視窗及其系統匣選單。

accent-color = 輔色

windows-follows-your-system-accent = Windows 選項依照系統輔色。


font = 字型

any-font-installed-on-this-pc = 此電腦上安裝的任意字型。
no-matching-fonts = 沒有匹配的字型

windows-default = Windows 預設




monochrome = 單色

time-format = 時間格式

msg-12-hour = 12 小時制

msg-24-hour = 24 小時制

popup-background = 彈出視窗背景
popup-theme = 彈出視窗主題
popup-theme-description = 僅更改彈出視窗。設定視窗保持 Windows 外觀。
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel
popup-theme-built-in = 內建
popup-theme-vscode = VS Code 主題
vscode-themes = VS Code 主題
vscode-themes-description = 在彈出視窗中使用 Visual Studio Code 顏色主題：瀏覽 Open VSX，或匯入 .vsix 或 .json 主題檔案。
import-theme = 匯入主題
importing-theme = 正在匯入…
remove-theme = 刪除主題
browse-open-vsx = 瀏覽 Open VSX
browse-open-vsx-description = 從 open-vsx.org 查詢並安裝顏色主題
search-color-themes = 搜尋顏色主題
open-vsx-search = 搜尋
open-vsx-browse = 瀏覽
open-vsx-searching = 正在搜尋…
no-themes-found = 未找到顏色主題
open-vsx-load-failed = 無法從 Open VSX 載入主題
open-vsx-retry = 重試
open-vsx-preview-loading = 正在載入預覽…
open-vsx-preview-hint = 點選主題即可在彈出視窗中預覽。「安裝」會保留整個主題包；主題的下載按鈕只保留該主題。
open-vsx-preview-empty = 此擴充套件不包含任何顏色主題
open-vsx-install = 安裝
open-vsx-install-one = 僅安裝此主題
open-vsx-installed = 已安裝
open-vsx-installing = 正在安裝…
theme-installed = 主題已安裝
theme-install-failed = 無法安裝主題
theme-remove-failed = 無法刪除主題

acrylic = 壓克力

mica = 雲母

solid = 純色

bottom-bar-size = 底欄大小

comfortable = 舒適

compact = 精簡

popup-corner-radius = 彈出視窗圓角半徑
popup-borders = 邊框
popup-borders-description = 選擇為彈出視窗的哪些部分繪製輪廓
popup-card-borders = 卡片和控制項
popup-window-border = 視窗邊框
popup-theme-contrast = 對比度
popup-theme-contrast-description = 增強或減弱 VS Code 主題中文字和輪廓的對比
popup-theme-tint = 著色
popup-theme-tint-description = 主題背景覆蓋 Mica 或 Acrylic 玻璃的程度

animation-effects = 動畫效果

glide-transitions-in-the-popup-and-settings-windows-own-animation = 彈出視窗和設定中的平滑過渡。也遵循 Windows 的動畫設定。

popup = 彈出視窗

motion = 動畫

blue = 藍色

purple = 紫色

pink = 粉色

red = 紅色

orange = 橙色

green = 綠色

teal = 青色

show-edit-button = 在底部欄顯示編輯按鈕
show-edit-button-description = 編輯模式會在首頁上顯示拖曳控點，以便重新排列小工具。從這裡開始編輯可讓底部欄保持簡潔。
start-editing = 開始編輯

use-two-columns = 使用雙欄

several-accounts-of-one-provider = 同一服務供應商的多個帳號

separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = 獨立分頁為每個執行個體提供一個分頁。分組模式為每個服務供應商顯示一個分頁，可切換帳號或堆疊顯示所有帳號。

use-monochrome-icons = 使用單色圖示

draw-provider-marks-in-the-popup-without-brand-colors = 在彈出視窗中顯示不帶品牌顏色的服務供應商圖示。

show-used-instead-of-remaining = 顯示已用額度而非剩餘額度

show-usage-in-values-when-possible = 顯示用量數值（如可用）

adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = 服務供應商傳回具體數值時，在百分比旁顯示已用量和總額度。

show-usage-pace = 顯示額度消耗進度

marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = 標記額度消耗速度是否快於或慢於均勻進度。



show-account-name = 顯示帳號名稱

show-on-home-tab = 在首頁分頁顯示

layout = 版面配置

donut = 環形圖

cards = 卡片

tabs = 分頁

usage-widget = 用量小工具

card = 卡片

tab = 分頁

popup-cards = 彈出視窗卡片

choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = 選擇此帳號在首頁及自身分頁中顯示的卡片。

choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = 選擇此帳號在自身分頁顯示的卡片。啟用「在首頁顯示」以選擇首頁卡片。

msg-30-seconds = 30 秒

msg-1-minute = 1 分鐘

msg-2-minutes = 2 分鐘

msg-5-minutes = 5 分鐘

msg-10-minutes = 10 分鐘

msg-15-minutes = 15 分鐘

msg-30-minutes = 30 分鐘

msg-45-minutes = 45 分鐘

msg-60-minutes = 60 分鐘

start-with-windows = 隨 Windows 啟動

open-codex-minibar-in-the-tray-when-you-sign-in = 登入 Windows 時在系統匣中開啟 Codex Minibar。

refresh-interval = 重新整理間隔

how-often-this-instance-s-quotas-are-read = 讀取此執行個體額度的頻率。較長的間隔可避免觸發服務供應商的請求頻率限制。

enable-usage-stats = 啟用用量統計

scan-local-provider-history-for-the-usage-tab-and-cost-totals = 掃描服務供應商本機歷史，用於用量分頁和總費用統計。

collection-period = 收集間隔

how-often-local-provider-history-is-scanned = 掃描服務供應商本機歷史的頻率。

check-for-confirmed-tibo-resets = 檢查已確認的 Tibo 重置

reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = 讀取應用程式的 GitHub 公開資訊來源並快取最新公告。

notify-when-new-reset-info-arrives = 收到新的重置資訊時通知

shows-a-notification-when-the-feed-reports-a-possible-reset-never = 資訊來源報告可能重置時顯示通知，不在重置時間通知。

check-every = 檢查間隔

the-feed-is-also-checked-immediately-when-the-app-starts-or-this = 啟動應用程式或啟用此選項時也會立即檢查資訊來源。

msg-1-hour = 1 小時

msg-3-hours = 3 小時

# src/settings_window/general.rs
msg-2-hours = 2 小時

# src/settings_window/general.rs
msg-5-hours = 5 小時

msg-6-hours = 6 小時

msg-12-hours = 12 小時

msg-24-hours = 24 小時

tibo-resets = Tibo Resets™

enable-a-provider-in-the-providers-tab-to-include-it-here = 在服務供應商分頁中啟用服務供應商以將其納入此處。

add-a-management-key = 新增管理金鑰

usage-statistics-are-off-on-this-provider-s-page = 此服務供應商頁面中的用量統計已關閉

included-providers = 納入的服務供應商

choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = 選擇計入此電腦用量分頁和首頁總計的帳號。排除的帳號仍繼續收集資料，並在自身頁面顯示用量。

download-the-latest-stream-deck-companion-from-github-and-open-it = 從 GitHub 下載最新的 Stream Deck 配套擴充套件並開啟安裝程式。

downloading-the-latest-stream-deck-companion-from-github = 正在從 GitHub 下載最新的 Stream Deck 配套擴充套件…

opening-the-stream-deck-installer = 正在開啟 Stream Deck 安裝程式…

the-installer-is-open-finish-the-installation-in-stream-deck = 安裝程式已開啟。請在 Stream Deck 中完成安裝。

the-plugin-could-not-be-downloaded-or-opened-try-again = 無法下載或開啟擴充套件。請重試。

downloading = 正在下載…

opening = 正在開啟…

install-plugin = 安裝擴充套件

stream-deck-companion = Stream Deck 配套擴充套件

check-your-connection-then-try-again = 請檢查網路連線後重試。

on = 開啟

off = 關閉

more-options = 更多選項

custom-color = 自訂顏色

run-troubleshoot-with-ai = 使用 AI 排查問題

let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = 讓已安裝的 AI CLI 讀取記錄並排查問題。

choose-tool = 選擇工具

application-log = 應用程式記錄

log-txt-in-the-app-data-folder = 應用程式資料夾中的 log.txt。

open-log-txt = 開啟 log.txt

could-not-open-log-txt = 無法開啟 log.txt

open-logs-folder = 開啟記錄資料夾

could-not-open-logs-folder = 無法開啟記錄資料夾

no-log-events-yet = 記錄中尚無事件。

live-tail = 即時記錄

no-supported-ai-tool-found = 未找到支援的 AI 工具

install-codex-or-claude-code-and-make-it-available-to-minibar = 安裝 Codex 或 Claude Code，並使 Minibar 能夠存取。

general = 一般

appearance = 外觀

providers = 服務供應商

customize = 自訂

limit-activation = 額度啟用

tray = 系統匣

notifications = 通知

advanced = 進階

log = 記錄

integrations = 整合

about-updates = 關於與更新

successful-activations = 啟用成功

failed-activations = 啟用失敗

when-limits-reset = 額度重置時

when-5-hour-remaining-hits = 5 小時剩餘額度降至 { $v0 }% 時

when-weekly-remaining-hits = 每週剩餘額度降至 { $v0 }% 時

low-usage = 額度不足

shows-a-notification-once-per-window-when-the-remaining-share-dro = 每個額度視窗內，剩餘比例降至門檻時通知一次。

threshold = 門檻

found-in-opencode-auth-or-local-history = 已在 OpenCode 身分驗證或本機歷史中找到。

not-found-turn-it-on-if-it-s-set-up-elsewhere = 未找到。如果已在其他位置設定，請啟用。

account-credentials-are-already-set = 帳號認證資訊已設定。

optional-add-accounts-later-in-providers = 可選。稍後可在服務供應商中新增帳號。

found-an-official-agy-sign-in-on-this-pc = 已在此電腦找到官方 agy 登入資訊。

not-found-sign-in-with-agy-before-enabling-it = 未找到。請先使用 agy 登入再啟用。

found-an-official-grok-cli-sign-in-on-this-pc = 已在此電腦找到官方 Grok CLI 登入資訊。

not-found-run-grok-login-before-enabling-it = 未找到。請先執行 grok login 再啟用。

# src/settings_window/onboarding.rs
found-an-official-command-code-cli-sign-in-on-this-pc = 已在此電腦找到官方 Command Code CLI 登入資訊。

# src/settings_window/onboarding.rs
not-found-run-cmdc-login-before-enabling-it = 未找到。請先執行 cmdc login 再啟用。

found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = 已找到 Kiro IDE、Kiro Crew 或已登入的 Kiro CLI。

not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = 未找到。請安裝 Kiro IDE 或 Kiro Crew，或登入 Kiro CLI。

found-on-this-pc = 已在此電腦找到。

not-found-turn-it-on-if-it-s-installed-somewhere-else = 未找到。如果已安裝在其他位置，請啟用。

setup-could-not-be-saved = 無法儲存初始設定

detected = 已偵測到

starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = 額度視窗可用時立即開始新的 Codex 或 Claude 工作階段，無需等待首次請求。

collect-usage-data = 收集用量資料

scans-local-provider-history-for-usage-stats = 掃描服務供應商本機歷史以統計用量。

startup = 啟動

features = 功能

customization = 自訂

when-a-new-version-is-found = 發現新版本時

activity = 活動

choose-providers = 選擇服務供應商
choose-a-theme = 選擇主題
theme-step-description = 為設定和彈出視窗選擇顏色。彈出視窗會即時顯示每個選擇。

we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = 已啟用此電腦上找到的服務供應商。稍後可以更改。

general-settings = 一般設定

you-can-change-these-later-in-settings = 稍後可在設定中更改。

turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = 關閉不想收到的通知。稍後可在設定中更改。

back = 返回

continue = 繼續

done = 完成

details = 詳情

added = 已新增

reads-the-session-and-weekly-limits-of-a-claude-subscription-with = 無需 Claude Code 登入即可讀取 Claude 訂閱的工作階段和每週額度。

msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. 在獨立瀏覽器設定檔或隱私視窗中開啟 Claude。登入要追蹤的帳號，並在 Claude 設定中確認其電子郵件位址。

open-claude-ai = 開啟 claude.ai

msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. 在 Chrome 或 Edge 中按 F12。開啟 Application > Storage > Cookies，然後選擇 https://claude.ai。

msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. 找到 sessionKey。複製其 Value，而非名稱或整張 Cookie 表，然後貼上到上方欄位。

how-to-view-cookies-in-chrome = 如何在 Chrome 中檢視 Cookie

a-cookie-header-containing-sessionkey-also-works-when-the-session = 包含 sessionKey 的 Cookie 要求標頭也可使用。工作階段到期後，請在此貼上新的值。

use-the-access-token-from-a-claude-code-subscription-login-miniba = 使用 Claude Code 訂閱登入的存取權杖。Minibar 無法重新整理貼上的權杖；儘可能使用來源：設定資料夾。

copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = 僅從該登入的 .credentials.json 中複製 claudeAiOauth.accessToken（以 sk-ant-oat 開頭），不帶引號。不要使用 claude setup-token：它可能沒有用量存取權限。

claude-code-log-in-with-multiple-accounts = Claude Code：使用多個帳號登入

requires-a-claude-subscription-login-with-usage-access-api-keys-a = 需要具有用量存取權限的 Claude 訂閱登入。API 金鑰和 Admin API 金鑰不顯示訂閱額度。

this-provider = 此服務供應商

track-another-account-or-a-provider-minibar-has-not-shown-yet = 追蹤其他帳號或 Minibar 尚未顯示的服務供應商。

e-g-work = 例如：工作

name = 名稱

shown-on-its-tab-home-card-tray-and-notifications = 顯示在其分頁、首頁卡片、系統匣和通知中。

badge = 標記

auto = 自動

badge-color = 標記顏色

up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = 最多三個字母；留空則使用名稱首字母。服務供應商啟用多個執行個體時顯示標記。

add-provider = 新增服務供應商

next = 下一步

minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar 將停止讀取 { $name } 並清除其已儲存的金鑰、排程、系統匣指示器和首頁位置。{ $v0 }

its-config-folder-stays-on-disk = { " " }其設定資料夾會保留在磁碟上。

delete-name = 刪除 { $name }？

delete = 刪除

finish-signing-in-in-your-browser-cancel-stops-this-login = 請在瀏覽器中完成登入。取消將終止此次登入。

runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = 為 { $name } 執行 Claude Code 自身的登入，將其設定資料夾設為 CLAUDE_CONFIG_DIR。登入資訊保留在該資料夾，由 Claude Code 自動續期。需要原生 Windows Claude Code。

runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = 為 { $name } 執行 Codex 自身的登入，將其設定資料夾設為 CODEX_HOME。登入資訊保留在該資料夾，由 Codex 自動續期。需要原生 Codex CLI 或桌面應用程式。

sign-in-to-name = 登入 { $name }

sign-in = 登入

for = 用於 { $v0 }。

browser-session = 瀏覽器工作階段

oauth-token = OAuth 權杖

session-key = 工作階段金鑰

paste-the-sessionkey-value = 貼上 sessionKey 的值

oauth-access-token = OAuth 存取權杖

the-saved-credential-is-replaced-only-after-the-new-one-passes-th = 新認證資訊透過驗證後才會替換已儲存的認證資訊。

claude-credential = Claude 認證資訊

check-and-save = 驗證並儲存

key-name-optional = 金鑰名稱（可選）

e-g-personal = 例如：個人

leave-blank-to-use-the-name-from-openrouter = 留空以使用 OpenRouter 中的名稱。

minibar-checks-the-key-with-openrouter-before-saving-it = Minibar 會在儲存前向 OpenRouter 驗證金鑰。

replace-api-key = 替換 API 金鑰

add-api-key = 新增 API 金鑰

management-key = 管理金鑰

create-one-under-settings-management-keys-on-openrouter-ai = 在 openrouter.ai 的 Settings → Management keys 中建立。

replace-management-key = 替換管理金鑰

add-management-key = 新增管理金鑰

key-name = 金鑰名稱

rename-key = 重新命名金鑰

save = 儲存

this-key = 此金鑰

minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibar 將停止追蹤 { $hint }。金鑰仍可在 OpenRouter 中使用。

remove-api-key = 移除 API 金鑰？

remove = 移除

minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibar 將停止顯示 { $v0 } 的點數餘額和用量歷史。金鑰仍可在 OpenRouter 中使用。

remove-management-key = 移除管理金鑰？

saved-in-windows-user-storage-never-in-the-settings-file = 儲存於 Windows 使用者儲存空間，不會寫入設定檔。

save-key = 儲存金鑰

minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibar 將清除 { $v0 } 的已儲存金鑰。該金鑰仍可用於 OpenCode。

signing-in = 正在登入…

checking = 正在檢查…

choose-a-provider = 請選擇服務供應商。

reason-it-is-already-in-the-list = { $reason }。已在列表中。

added-name = 已新增 { $name }。

could-not-add-the-provider-error = 無法新增服務供應商：{ $error }

this-provider-no-longer-exists = 此服務供應商已不存在。

could-not-delete-the-provider-error = 無法刪除服務供應商：{ $error }

paste-a-credential-first = 請先貼上認證資訊。

credential-saved-in-windows-user-storage = 認證資訊已儲存於 Windows 使用者儲存空間。

paste-a-key-first = 請先貼上金鑰。

this-account-no-longer-exists = 此帳號已不存在。

api-key-saved-in-windows-user-storage = API 金鑰已儲存於 Windows 使用者儲存空間。

management-key-replaced = 管理金鑰已替換。

management-key-added = 管理金鑰已新增。

could-not-rename-the-key-error = 無法重新命名金鑰：{ $error }

api-key-renamed = API 金鑰已重新命名。

could-not-remove-the-key-error = 無法移除金鑰：{ $error }

api-key-removed = API 金鑰已移除。

management-key-removed = 管理金鑰已移除。

could-not-save-the-key-error = 無法儲存金鑰：{ $error }

api-key-saved = API 金鑰已儲存。

switch-source-to-config-folder-to-sign-in = 將來源切換為設定資料夾以登入。

this-provider-has-no-config-folder = 此服務供應商沒有設定資料夾。

another-instance-already-reads-this-config-folder-choose-a-differ = 其他執行個體已在讀取此設定資料夾。請先選擇其他資料夾。

this-provider-has-no-sign-in = 此服務供應商不支援登入。

signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 } 已登入。其 CLI 會在設定資料夾中保持登入有效。

no-keys-yet = 尚無金鑰

management-key-5c8cf2 = { " " }· 管理金鑰

using-a-saved-credential = 正在使用已儲存的認證資訊

paste-a-credential-under-account = 在帳號下貼上認證資訊

needs-an-api-key-or-opencode-sign-in = 需要 API 金鑰或 OpenCode 登入

needs-an-api-key = 需要 API 金鑰

not-found-set-its-folder-under-runtime = 未找到。請在執行環境中設定其資料夾。

using-a-saved-api-key = 正在使用已儲存的 API 金鑰

using-opencode-sign-in-or-local-history = 正在使用 OpenCode 登入或本機歷史

reading-cli = 正在讀取 { $cli }

reading-crew = 正在讀取 { $crew }

reading-app = 正在讀取 { $app }

checking-kiro-ide-kiro-crew-and-cli = 正在檢查 Kiro IDE、Kiro Crew 和 CLI…

checking-installed-app-and-cli = 正在檢查已安裝的應用程式和 CLI…

checking-cli = 正在檢查 CLI…

checking-installed-app = 正在檢查已安裝的應用程式…

no-providers-yet-add-one-to-start-reading-limits = 尚無服務供應商。新增一個以開始讀取額度。

is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } 已關閉，因此不會出現在 Minibar 或系統匣中。啟用後開始讀取用量。

replace-chatgpt-logo-with-codex = 將 ChatGPT 標誌替換為 Codex

limits-only = 僅額度

delete-provider = 刪除服務供應商

display-name = 顯示名稱

shown-on-popup-tabs-home-cards-the-tray-and-notifications = 顯示在彈出視窗分頁、首頁卡片、系統匣和通知中。

up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = 最多三個字母。留空則使用名稱首字母。服務供應商啟用多個執行個體時顯示。

auto-uses-a-neutral-plate-that-follows-the-theme = 自動使用依照主題的中性背景。

show-on-home = 在首頁顯示

its-provider-tab-stays-available-when-hidden-from-home = 從首頁隱藏後，其服務供應商分頁仍可使用。

credential = 認證資訊

signed-in-as-identity = 已登入為 { $identity }

saved-in-windows-user-storage = 已儲存於 Windows 使用者儲存空間

paste-a-sessionkey-or-an-oauth-access-token = 貼上 sessionKey 或 OAuth 存取權杖。

reads-limits-only-minibar-cannot-refresh-a-pasted-credential = 僅讀取額度。Minibar 無法重新整理貼上的認證資訊。

replace-credential = 替換認證資訊

add-credential = 新增認證資訊

signed-in-account = 已登入的帳號

not-signed-in-yet-or-no-limits-read-so-far = 尚未登入，或尚未讀取到額度。

account = 帳號

runtime = 執行環境

minibar-finds-these-automatically = { $v0 } Minibar 會自動尋找這些項目。

in-use = 使用中

found = 已找到

copy-path = 複製路徑

open-folder = 開啟資料夾

path-copied = 路徑已複製。

not-installed-or-installed-somewhere-minibar-doesn-t-look = 未安裝，或安裝在 Minibar 不會搜尋的位置。

not-found = 未找到

choose-folder = 選擇資料夾…

select-folder = 選擇資料夾

choose-folder-1838a4 = 選擇資料夾

only-needed-if-automatic-detection-misses-your-install = 僅在自動偵測找不到已安裝的程式時需要。

source = 來源

config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = 設定資料夾讀取 CLAUDE_CONFIG_DIR 中的 Claude Code 登入資訊。手動認證資訊僅透過貼上的認證資訊讀取額度。

config-folder = 設定資料夾

manual = 手動

passed-to-the-cli-as-env-leave-empty-to-use = 作為 { $env } 傳遞給 CLI。留空以使用 { $v0 }。

the-standard-folder = 預設資料夾

a-folder-minibar-creates-for-this-instance = Minibar 為此執行個體建立的資料夾

other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } 已在讀取此資料夾。兩個執行個體不能共用登入資訊，否則用量會被重複計算。

automatic-activation = 自動啟用

starts-this-account-s-5-hour-window-when-it-resets-using-its-own = 此帳號的 5 小時額度重置後，使用其自身登入資訊開啟額度視窗。排程和暫停時段位於額度啟用中。

usage-statistics = 用量統計

scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = 掃描此執行個體的本機歷史以生成用量卡片。關閉後完全停止收集。

opencode-sign-in-or-local-history = OpenCode 登入或本機歷史

found-in-opencode-auth-environment-a-saved-key-or-local-history = 已在 OpenCode 身分驗證、環境變數、已儲存金鑰或本機歷史中找到。

nothing-found-in-opencode-auth-environment-or-local-history = 在 OpenCode 身分驗證、環境變數或本機歷史中未找到任何資訊。

replace-key = 替換金鑰

optional-only-needed-without-opencode-sign-in-on-this-pc = 可選。僅在此電腦未登入 OpenCode 時需要。

add-the-key-of-the-account-this-instance-tracks = 新增此執行個體所追蹤帳號的金鑰。

keys = 金鑰

a-management-key-shows-credit-balance-and-usage-history-api-keys = 管理金鑰顯示點數餘額和用量歷史。API 金鑰顯示各金鑰支出。

could-not-read-the-saved-key-reopen-this-page-to-retry = 無法讀取已儲存的金鑰。請重新開啟此頁面以重試。

credit-balance-and-account-wide-usage-history = 點數餘額和整個帳號的用量歷史

not-added-add-one-to-see-credit-balance-and-usage-history = 尚未新增。新增後可檢視點數餘額和用量歷史。

last-updated = 上次更新於 { $v0 }，{ $v1 }

no-api-keys-yet-add-one-to-track-spend-per-key = 尚無 API 金鑰。新增一個以追蹤各金鑰支出。

key = 金鑰

spend = 支出

limit = 上限

error-reopen-this-page-to-retry = { $error }。請重新開啟此頁面以重試。

could-not-read-key = 無法讀取金鑰

not-saved = 未儲存

unnamed = 未命名

of-the-limit = { $v0 }，上限為 { $v1 }

no-spend-limit-key-spend-account-credits-purchased = 無支出上限。金鑰支出：{ $v0 }。帳號已購買點數：{ $v1 }。

none = 無

add-key = 新增金鑰

openrouter-account-credentials-are-configured = OpenRouter 帳號認證資訊已設定

saved-api-key = 已儲存的 API 金鑰

saved-credential = 已儲存的認證資訊

opencode-auth-json-or-local-database = OpenCode auth.json 或本機資料庫

that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = 這似乎不是 OpenRouter 金鑰。金鑰以 sk-or- 開頭。

codex-cli-folder = Codex CLI 資料夾

claude-code-cli-folder = Claude Code CLI 資料夾

cursor-app-folder = Cursor 應用程式資料夾

grok-cli-folder = Grok CLI 資料夾

# src/settings_window/providers.rs
command-code-cli-folder = Command Code CLI 資料夾

kiro-ide-folder = Kiro IDE 資料夾

kiro-crew-app-path = Kiro Crew 應用程式路徑

kiro-cli-folder = Kiro CLI 資料夾

reads-the-signed-in-codex-cli-or-desktop-app = 讀取已登入的 Codex CLI 或桌面應用程式。

reads-your-existing-claude-code-login = 讀取現有的 Claude Code 登入資訊。

reads-the-signed-in-cursor-app-for-this-billing-cycle = 讀取已登入 Cursor 應用程式在目前計費週期的資料。

reads-zen-auth-and-local-opencode-history = 讀取 Zen 身分驗證和 OpenCode 本機歷史。

reads-go-quota-windows-and-local-opencode-history = 讀取 Go 額度視窗和 OpenCode 本機歷史。

reads-api-key-usage-and-spend-limits-a-management-key-also-enable = 讀取 API 金鑰用量和支出上限。管理金鑰還可用於檢視用量歷史和點數餘額。

reads-subscription-quota-from-your-existing-official-agy-windows = 透過現有的官方 agy Windows 登入讀取訂閱額度。

reads-supergrok-subscription-credits-from-your-existing-official = 透過現有的官方 Grok CLI 登入讀取 SuperGrok 訂閱點數。

# src/settings_window/providers.rs
reads-command-code-5h-weekly-and-monthly-credits-from-your-existin = 透過現有的官方 CLI 登入讀取 Command Code 的 5 小時、每週和每月點數。

fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = 透過 Kiro 共用登入取得即時每月點數；可識別 IDE、Crew 和 CLI 安裝。

codex-desktop-app = Codex 桌面應用程式

codex-cli = Codex CLI

claude-desktop-app = Claude 桌面應用程式

claude-code-cli = Claude Code CLI

cursor-app = Cursor 應用程式

antigravity-app = Antigravity 應用程式

grok-cli = Grok CLI

# src/settings_window/providers.rs
command-code-cli = Command Code CLI

kiro-ide = Kiro IDE

kiro-crew = Kiro Crew

kiro-cli = Kiro CLI

supports-one-instance = { $v0 } 支援一個執行個體

numbers = 數字

progress-bars = 進度列

rings = 圓環

reset-time = 重置時間

countdown = 倒數計時

status = 狀態

fixed = 固定

app-accent = 應用程式輔色

percentages-as-digits = 以數字顯示百分比

one-bar-per-indicator = 每個指示器一條進度列

nested-rings-one-per-indicator = 巢狀圓環，每個指示器一個

when-the-limit-resets = 額度重置時間

time-left-until-the-reset = 距重置的剩餘時間

used = 已用

remaining = 剩餘

app-icon = 應用程式圖示

widget = 小工具 { $v0 }

the-codex-minibar-icon = Codex Minibar 圖示

style-no-indicators = { $style } · 無指示器

widget-removed = 小工具已移除

undo = 復原

tray-icon = 系統匣圖示

shows-the-app-icon = 顯示應用程式圖示。

add-widget = 新增小工具

add-app-icon = 新增應用程式圖示

move-up = 上移

move-down = 下移

edit = 編輯

duplicate = 複製

shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = 在通知區域顯示 Codex Minibar 圖示。沒有需要設定的指示器。

style = 樣式

up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = 最多 { $max_indicators } 個額度，按此順序繪製。展開其中一個以修改。

indicators = 指示器

remove-widget = 移除小工具

add-indicator = 新增指示器

unsupported = 不支援 { $v0 }

remove-indicator = 移除指示器

unsupported-bba2a8 = 不支援（{ $v0 }）

unavailable-0c5d75 = 無法使用（{ $v0 }）

metric = 指標

show = 顯示

color = 顏色

fixed-color = 固定顏色

troubleshooting-could-not-start = 無法開始排查問題

choose-which-installed-ai-tool-should-investigate-the-problem = 選擇用於排查問題的已安裝 AI 工具。

open-terminal = 開啟終端機

version-is-available = { $version } 可用

update-now = 立即更新

missing-a-provider = 缺少服務供應商？

ask-for-it-or-build-it-yourself = 申請新增或自行開發。

request-a-provider = 申請新增服務供應商

contribute-one = 貢獻服務供應商

disabled = 已停用

update-complete = 更新完成

now-running-version = 目前執行版本為 { $version }。

past-24h = 過去 24 小時

language = 語言

applies-immediately-to-every-app-window-and-notification-auto-fol = 立即套用至所有應用程式視窗和通知。自動依照 Windows 顯示語言。

auto-windows = 自動（Windows）

english = English

msg-russian = Русский

codex-minibar-settings = Codex Minibar 設定

welcome-to-codex-minibar = 歡迎使用 Codex Minibar

exit = 結束

update-available-67fd3a = 有可用更新

monthly-credits-976559 = 每月點數

msg-5h-session-de7ce8 = 5 小時工作階段

name-weekly = { $name } 每週

tokens-94e0b9 = { $v0 }：{ $v1 } 個 token

requests-priced = { $v0 } 次請求 · { $v1 } 次已計價

requests = { $v0 } 次請求

credits-338f52 = 點數

cloud-session-credits = 雲端工作階段點數

name-login-expires-in-days-left = { $name } 登入將在 { $days_left } 天后到期

name-error = { $name } 錯誤

sessions = { $v0 } 個工作階段

api-estimate = API 估算

start-to-end = { $start } 至 { $end }

to = { $v0 } 至 { $v1 }

share-1-of-other = 佔 { $v0 } 的 { $share }% · { $other }

# application
excluded-other = 已排除 · { $other }

# application
exclude-from-usage-stats = 點選以從用量統計中排除

# application
include-in-usage-stats = 點選以計入用量統計

cost-885dc4 = 費用

tokens-339143 = token

sessions-0e5e29 = · { $v0 } 個工作階段

api-key = API 金鑰

api-keys = API 金鑰

name-deleted = 已刪除 { $name }。

openrouter-api-key-no-longer-exists = OpenRouter API 金鑰已不存在

openrouter-account-no-longer-exists = OpenRouter 帳號已不存在

credit = { $v0 } 點數

agy-cli-folder = agy CLI 資料夾

folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = 包含 codex.exe、codex.cmd 或 codex.ps1 的資料夾。留空以自動查詢。

folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = 包含 claude.exe、claude.cmd 或 claude.ps1 的資料夾。留空以自動查詢。

folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = 包含 Cursor.exe 的資料夾。留空以自動查詢。用量仍來自已登入的設定檔。

folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = 包含 agy.exe、agy.cmd 或 agy.ps1 的資料夾。留空以自動查詢。

folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = 包含 grok.exe、grok.cmd 或 grok.ps1 的資料夾。留空以自動查詢。

# src/settings_window/providers.rs
folder-with-cmdc-cmd-or-cmdc-ps1-leave-empty-to-find-it-automati = 包含 cmdc.cmd 或 cmdc.ps1 的資料夾。留空以自動查詢。

folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = 包含 Kiro.exe 的資料夾或可執行檔案本身。留空以自動查詢。

folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = 包含 KiroCrew.exe 的資料夾或可執行檔案本身。留空以自動偵測目前使用者和所有使用者的安裝。

folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = 包含 kiro-cli.exe 的資料夾或可執行檔案本身。留空以自動查詢。

off-45080e = { $v0 }（已關閉）

each-widget-is-one-icon-in-the-notification-area-indicators-show = 每個小工具是通知區域中的一個圖示。指示器以數字、進度列、圓環或重置時鐘顯示服務供應商額度。

a-reset-clock-follows-one-quota = 重置時鐘追蹤一個額度。

ai-tool = AI 工具

msg-7-days = 7 天

msg-30-days = 30 天

msg-90-days = 90 天

yellow = 黃色

custom = 自訂

not-available-for-manual-credentials-switch-source-to-config-fold = 手動認證資訊不可用。請將來源切換為設定資料夾。

opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = OpenCode 本機歷史由第一個 OpenCode 執行個體追蹤。

this-provider-has-no-local-usage-history = 此服務供應商沒有本機用量歷史。

this-provider-has-no-session-window-to-start = 此服務供應商沒有可開啟的工作階段視窗。

this-provider-does-not-use-a-config-folder = 此服務供應商不使用設定資料夾。

could-not-save-provider-settings-error-restoring-its-previous-cre = 無法儲存服務供應商設定（{ $error }）；恢復先前認證資訊也失敗（{ $rollback_error }）。

msg-5h-7d = 5 小時  ·  { $v0 }  ·  { $v1 }
    7 天  ·  { $v2 }  ·  { $v3 }

api-key-count = { $v0 } 個 API 金鑰

widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = 加寬首頁和用量頁面。可在欄之間拖曳首頁區塊；服務供應商分頁保持精簡。

a-possible-codex-reset-is-scheduled-for-when-in-countdown = Codex 可能於 { $when } 重置（還有 { $countdown }）

label-possible-reset-on-when-in-countdown = { $label }：可能於 { $when } 重置（還有 { $countdown }）

new-codex-reset-info = 新的 Codex 重置資訊

login-expires-soon = { $v0 } 登入即將到期

it-stops-renewing-on-open-minibar-and-choose-sign-in-again = 將在 { $v0 } 停止續期。開啟 Minibar 並選擇重新登入。

could-not-clear-usage-data-error = 無法清除用量資料：{ $error }

succeeded-at = { $v0 } 於 { $v1 } 成功

failed-at-error = { $v0 } 於 { $v1 } 失敗：{ $error }

key-39df89 = 金鑰 { $v0 }

expired = 已到期

jan = 1月

feb = 2月

mar = 3月

apr = 4月

may = 5月

jun = 6月

jul = 7月

aug = 8月

sep = 9月

oct = 10月

nov = 11月

dec = 12月

quota-percent-used = 已用 { $value }%

quota-percent-left = 剩餘 { $value }%

cloud-amount-used = 已用 { $amount }，共 { $limit }

cloud-amount-left = 剩餘 { $amount }，共 { $limit }

range-24h = 24小時

range-7d = 7天

range-30d = 30天

range-90d = 90天

home-usage-title = 用量

# Activity chart grouping
split-type = 類型

split-by-token-type = 按 token 類型拆分長條圖

# OpenRouter key administration
could-not-reach-openrouter = 無法連線到 OpenRouter
openrouter-keys-title = 金鑰
openrouter-keys-new = 新增金鑰
openrouter-keys-new-title = 新增金鑰
openrouter-keys-back = 返回
openrouter-keys-updating = 正在更新…
openrouter-keys-loading = 正在載入金鑰…
openrouter-keys-load-failed = 無法載入金鑰
openrouter-keys-retry = 重試
openrouter-keys-empty = 此帳號還沒有金鑰。
openrouter-keys-this-app = 此應用程式
openrouter-keys-no-limit = 無限額
openrouter-keys-of-limit = { $amount } / { $limit }
openrouter-keys-resets-daily = 每日重置
openrouter-keys-resets-weekly = 每週重置
openrouter-keys-resets-monthly = 每月重置
openrouter-keys-expires-on = { $date } 過期
openrouter-keys-expired-on = 已於 { $date } 過期
openrouter-keys-today = 今日 { $amount }
openrouter-keys-usage-breakdown = 今日 { $today } · 本週 { $week } · 本月 { $month } · 總計 { $total }
openrouter-keys-show-all = 顯示全部 { $count } 個金鑰
openrouter-keys-show-fewer = 收起
openrouter-keys-spending-limit = 支出限額
openrouter-keys-limit-hint = 留空表示不設限額。
openrouter-keys-resets = 重置週期
openrouter-keys-reset-never = 從不
openrouter-keys-reset-daily = 每日
openrouter-keys-reset-weekly = 每週
openrouter-keys-reset-monthly = 每月
openrouter-keys-byok = 將 BYOK 用量計入限額
openrouter-keys-enabled = 已啟用
openrouter-keys-delete = 刪除
openrouter-keys-delete-tracked = Minibar 也將停止追蹤它。
openrouter-keys-saving = 正在儲存…
openrouter-keys-delete-title = 刪除「{ $name }」？
openrouter-keys-delete-message = 所有仍在使用此金鑰的地方將立即停止工作。此操作無法復原；停用金鑰則可以恢復。
openrouter-keys-keep = 保留金鑰
openrouter-keys-delete-confirm = 刪除金鑰
openrouter-keys-deleting = 正在刪除…
openrouter-keys-name = 名稱
openrouter-keys-name-placeholder = 例如 laptop-cursor
openrouter-keys-name-required = 請輸入金鑰名稱。
openrouter-keys-invalid-amount = 請輸入美元金額，例如 25 或 12.50。
openrouter-keys-expires = 過期時間
openrouter-keys-expires-1-hour = 1 小時
openrouter-keys-expires-1-day = 1 天
openrouter-keys-expires-7-days = 7 天
openrouter-keys-expires-30-days = 30 天
openrouter-keys-expires-90-days = 90 天
openrouter-keys-expires-180-days = 180 天
openrouter-keys-expires-1-year = 1 年
openrouter-keys-expires-never = 永不過期
openrouter-keys-track = 在 Minibar 中追蹤此金鑰
openrouter-keys-create = 建立金鑰
openrouter-keys-creating = 正在建立…
openrouter-keys-created = 金鑰已建立
openrouter-keys-copy = 複製金鑰
openrouter-keys-copied = 已複製
openrouter-keys-done = 完成
openrouter-keys-once-title = 此金鑰不會再次顯示
openrouter-keys-once-message = OpenRouter 只會顯示一次新金鑰。請立即複製。
openrouter-keys-pinned = 彈出視窗會保持開啟，直到你點選「完成」。
openrouter-keys-tracking = 正在將金鑰新增到 Minibar…
openrouter-keys-tracked = Minibar 現在正在追蹤此金鑰。
openrouter-keys-track-failed = 無法將金鑰新增到 Minibar：{ $error }
openrouter-keys-no-management-key = 此帳號沒有管理金鑰。
openrouter-keys-management-key-rejected = OpenRouter 拒絕了管理金鑰。請在設定中替換它。
openrouter-keys-not-a-management-key = 此金鑰無法管理其他金鑰。請使用管理金鑰。
openrouter-keys-key-not-found = 此金鑰已不存在。
openrouter-keys-untrack = 從應用程式中移除
openrouter-keys-untracking = 正在移除…
openrouter-keys-untrack-title = 停止追蹤「{ $name }」？
openrouter-keys-untrack-message = Minibar 將忘記此金鑰。它在 OpenRouter 上仍可繼續使用。
openrouter-keys-tracked-here = Minibar 正在追蹤此金鑰。
openrouter-keys-manage-hint = 在設定中新增管理金鑰，即可建立、編輯和刪除金鑰。

# src/settings_window/notifications.rs
notification-sounds = 通知聲音

# src/settings_window/notifications.rs
plays-a-short-sound-with-each-notification = 每條通知播放一段簡短提示音

use-reset = 使用重置
use-reset-short = 使用
reset-using = 正在套用…
reset-confirm-message = 為此帳號使用一次累積重置？這將清除符合條件的速率限制，且無法復原。
reset-applied = 重置已套用。符合條件的限制已清除。
reset-nothing-to-reset = 目前沒有需要重置的限制。
reset-no-credit = 沒有可用的重置次數。
reset-already-redeemed = 此重置已使用。
reset-unconfirmed = 無法確認重置。請重試以檢查同一請求。
reset-applied-refresh-failed = 重置已套用，但無法載入新限制。請重新整理以檢查。
reset-in-progress = 此帳號已有重置正在進行。
reset-cooldown = 重置處於冷卻期。請稍後重試。
reset-rate-limited = 重置請求過多。請稍後重試。
reset-sign-in-again = 請重新登入 Claude 以使用重置。
reset-cli-login-required = 請使用同一帳號的 Claude CLI 登入來重置。
reset-invalid-grant = Claude 傳回了無效的重置額度。

# src/updater.rs
update-downloading = 正在下載更新

# src/updater.rs
update-download-progress = { $downloaded } / { $total }

# src/updater.rs
update-downloaded = 已下載 { $downloaded }

# src/updater.rs
update-installing = 正在安裝更新

# src/updater.rs
update-unpacking = 正在解壓檔案…

# src/updater.rs
update-restarting = 正在重新啟動 Codex Minibar

# src/updater.rs
update-restarting-body = 應用程式即將重新開啟。

# src/settings_window/general.rs
provider-status = 服務供應商狀態

# src/settings_window/general.rs
show-provider-incidents = 顯示服務供應商事件

# src/settings_window/general.rs
show-provider-incidents-description = 每 5 分鐘檢查 Codex、Claude 和 Cursor 的公開狀態頁，並在彈出視窗中顯示正在進行的事件

# src/settings_window/general.rs
notify-about-new-incidents = 新事件通知

# src/settings_window/general.rs
notify-about-new-incidents-description = 當狀態頁報告已啟用服務供應商的新事件時顯示通知

# src/popup_window/ui/cards.rs
open-status-page = 開啟狀態頁

# src/notifications.rs
provider-incident-title = { $provider }：{ $impact }

# src/popup_window/ui/cards.rs
status-since = 始於 { $time }

# src/provider_status.rs
status-impact-maintenance = 維護

# src/provider_status.rs
status-impact-degraded = 效能下降

# src/provider_status.rs
status-impact-partial-outage = 部分中斷

# src/provider_status.rs
status-impact-major-outage = 嚴重中斷

# src/provider_status.rs
status-stage-investigating = 調查中

# src/provider_status.rs
status-stage-identified = 已確定原因

# src/provider_status.rs
status-stage-monitoring = 監控中

# src/provider_status.rs
status-stage-in-progress = 進行中

# src/settings_window/notifications.rs
news-and-status = 動態與狀態

# src/whats_new.rs
no-summary-for-this-version = 此版本暫無摘要。完整發行說明見 GitHub。

# src/whats_new.rs
couldn-t-load-what-s-new = 無法載入新功能。請檢查網路連線，或在 GitHub 上檢視發行說明。

# src/whats_new.rs
full-release-notes = 完整發行說明

# src/settings_window/about.rs
show-what-s-new-after-updating = 更新後顯示新功能

# src/settings_window/about.rs
a-short-summary-of-the-release-once-it-s-installed = 安裝後顯示該版本的簡短摘要。

# src/settings_window/about.rs
view = 檢視

# src/settings_window/general.rs
popup-shortcut = 彈出視窗快速鍵
popup-shortcut-description = 在 Windows 任意位置按下即可顯示或隱藏彈出視窗。
shortcut-not-set = 未設定
press-a-shortcut = 請按下快速鍵…
popup-shortcut-recording-hint = Esc 取消，Backspace 刪除快速鍵。
popup-shortcut-needs-modifier = 請包含 Ctrl、Alt 或 Win，或使用 F13–F24。
popup-shortcut-in-use = 此快速鍵已被其他應用程式佔用，請選擇其他組合。
clear-shortcut = 刪除快速鍵
taskbar = 工作列
preview = 預覽
taskbar-page-caption = 直接繪製在 Windows 工作列中、時鐘旁邊的即時用量小工具。它們依照工作列主題；點選小工具即可開啟其服務供應商的彈出視窗。
taskbar-show-widgets = 在工作列上顯示小工具
taskbar-show-widgets-description = 在 Windows 工作列中繪製下面的小工具。
taskbar-position = 位置
taskbar-position-description = 小工具在工作列上的位置。開頭最適合置中的工作列圖示。
taskbar-position-tray = 通知區域旁邊
taskbar-position-start = 工作列開頭
taskbar-position-in-tray = 通知區域內（實驗性）
taskbar-widgets = 小工具
taskbar-no-widgets = 暫無小工具
taskbar-no-widgets-description = 新增小工具以在工作列上顯示額度。
taskbar-show-provider-icon = 顯示服務供應商圖示
taskbar-style-glance = 速覽
taskbar-style-glance-hint = 服務供應商圖示旁邊顯示兩行文字，就像天氣按鈕。
taskbar-style-rings-hint = 每個額度一個進度圓環，旁邊顯示數值。
taskbar-style-bars-hint = 在服務供應商圖示旁邊堆疊細進度列。
taskbar-menu-open = 開啟 Codex Minibar
taskbar-menu-settings = 工作列小工具設定
taskbar-menu-hide = 隱藏工作列小工具
taskbar-card-left = 剩餘 { $value }
taskbar-card-used = 已用 { $value }
taskbar-card-resets = { $time } 後重置 · { $clock }
taskbar-card-no-data = 正在等待資料
short-5h = 5小時
short-7d = 7天
taskbar-style-nested = 巢狀圓環
taskbar-style-dials = 錶盤
taskbar-style-meters = 電量列
taskbar-style-nested-hint = 在一個圓環的空間裡放下同心圓環，可在旁邊顯示數值。
taskbar-style-dials-hint = 每個額度一個小圓環，數字顯示在環內。
taskbar-style-meters-hint = 纖細的電量柱，像電池指示器。
taskbar-show-provider-icon-description = 在小工具開頭顯示服務供應商標誌。
taskbar-show-labels = 顯示標籤
taskbar-show-labels-description = 在數值旁顯示 5小時、7天 等簡稱。
taskbar-show-values = 顯示百分比
taskbar-show-values-description = 小工具上的額度百分比。
taskbar-show-pace = 顯示額度消耗進度
taskbar-show-pace-description = 標示按平均速度消耗時，目前應達到的用量位置。
taskbar-menu-edit = 編輯此小工具
taskbar-content = 內容
taskbar-content-hint = 小工具在圖形旁顯示的內容。
taskbar-reset-time = 重置時間
taskbar-reset-time-description = 每個額度何時重置。
taskbar-reset-off = 不顯示
taskbar-reset-countdown = 剩餘時間
taskbar-reset-clock = 時鐘時間
taskbar-cycle = 用滑鼠滾輪切換小工具
taskbar-cycle-description = 一次只顯示一個小工具；在上方捲動滑鼠滾輪即可切換。
taskbar-card-tokens = { $count } 個 token
taskbar-click = 點選開啟
taskbar-click-description = 點選小工具時開啟的彈出視窗頁面。
taskbar-click-nearest = 首頁；未顯示於首頁時開啟其分頁
taskbar-click-tab = 服務供應商分頁
taskbar-click-home = 首頁

# src/settings_window/notifications.rs
follow-do-not-disturb = 勿擾模式時暫停

# src/settings_window/notifications.rs
follow-do-not-disturb-description = Windows 勿擾或專注模式開啟期間，跳過通知並關閉提示音。你主動發起的操作結果仍會顯示。

# src/settings_window/advanced.rs
performance = 效能

# src/settings_window/advanced.rs
game-mode = 遊戲模式

# src/settings_window/advanced.rs
game-mode-description = 降低額度和用量的檢查頻率，以低優先順序執行並暫停通知。「自動」會在遊戲切到前景時開啟，直到遊戲關閉。

# src/settings.rs
game-mode-off = 關閉

# src/settings.rs
game-mode-automatic = 自動

# src/settings.rs
game-mode-always = 始終開啟

# src/settings_window/notifications.rs
notify-game-mode-changes = 遊戲模式切換

# src/settings_window/notifications.rs
notify-game-mode-changes-description = 遊戲模式開啟或關閉時通知

# src/notifications.rs
game-mode-on-title = 遊戲模式已開啟

# src/notifications.rs
game-mode-on-body = 降低額度檢查頻率並暫停通知

# src/notifications.rs
game-mode-on-game-body = { $game } 正在執行。降低額度檢查頻率並暫停通知

# src/notifications.rs
not-a-game = 不是遊戲

# src/settings_window/advanced.rs
add-a-game = 新增遊戲

# src/settings_window/advanced.rs
add-a-game-description = Windows Game Bar 已識別的遊戲以及 Steam、Epic、GOG 和 Xbox 中的遊戲會被自動偵測。可在此新增其他任何程式。

# src/settings_window/advanced.rs
never-a-game-path = 從不視為遊戲 · { $path }

# src/notifications.rs
game-mode-off-title = 遊戲模式已關閉

# src/notifications.rs
game-mode-off-body = 已恢復正常檢查和通知

# src/settings_window/command_palette.rs
command-palette = Command Palette

# src/settings_window/command_palette.rs
stream-deck = Stream Deck

# src/settings_window/command_palette.rs
install = 安裝

# src/settings_window/command_palette.rs
cmdpal-extension = Command Palette 擴充套件

# src/settings_window/command_palette.rs
cmdpal-description = 將 Minibar 新增到 PowerToys Command Palette：輸入服務供應商名稱即可檢視額度，或將條帶固定到 Dock。安裝需要管理員批准。

# src/settings_window/command_palette.rs
cmdpal-installed-hint = 已安裝。在 Command Palette 中輸入「minibar」或服務供應商名稱，或透過「編輯 Dock」固定條帶。

# src/settings_window/command_palette.rs
cmdpal-downloading = 正在下載擴充套件…

# src/settings_window/command_palette.rs
cmdpal-installing = 正在等待安裝程式。請批准管理員提示以繼續。

# src/settings_window/command_palette.rs
cmdpal-installing-short = 正在安裝…

# src/settings_window/command_palette.rs
cmdpal-removing = 正在移除擴充套件…

# src/settings_window/command_palette.rs
cmdpal-failed = 無法安裝或移除擴充套件。

# src/settings_window/command_palette.rs
cmdpal-reinstall = 重新安裝

# src/settings_window/command_palette.rs
cmdpal-dock-bands = Dock 條帶

# src/settings_window/command_palette.rs
cmdpal-dock-bands-caption = 每個條帶都會出現在 Command Palette 的 Dock 設定中。透過「編輯 Dock」固定它，然後選擇位置。

# src/settings_window/command_palette.rs
cmdpal-no-bands = 沒有條帶

# src/settings_window/command_palette.rs
cmdpal-no-bands-description = 新增條帶，將服務供應商額度固定到 Dock。

# src/settings_window/command_palette.rs
cmdpal-add-band = 新增條帶

# src/settings_window/command_palette.rs
cmdpal-band = 條帶 { $v0 }

# src/settings_window/command_palette.rs
cmdpal-band-removed = 條帶已移除

# src/settings_window/command_palette.rs
cmdpal-remove-band = 移除條帶

# src/settings_window/command_palette.rs
cmdpal-content-hint = Dock 將條帶顯示為標題和下方的副標題。

# src/settings_window/command_palette.rs
cmdpal-indicators-hint = 最多 2 個額度：一個佔用標題和副標題，兩個各佔一行。
