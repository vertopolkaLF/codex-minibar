### Codex Minibar application messages.
### Korean translation; English per-message fallback. Keep $parameters unchanged.

# src/provider_registry.rs
luna-reserve = Luna Reserve

# src/limits.rs
on-pace = 적정 사용 속도

# src/limits.rs
delta-in-deficit = { $delta }% 적자

# src/limits.rs
delta-in-reserve = { $delta }% 여유

# src/notifications.rs
msg-5-hour-limit-started = 5시간 한도 시작

# src/notifications.rs
msg-5-hour-limit-reset-and-activated = 5시간 한도 재설정 및 활성화

# src/notifications.rs
msg-5-hour-limit-reset = 5시간 한도 재설정

# src/notifications.rs
weekly-limit-reset = 주간 한도 재설정

# src/notifications.rs
name-5-hour = { $name } 5시간

# src/notifications.rs
label-limit-is-low = { $label } 한도가 낮습니다.

# src/popup_window/formatting.rs
expired-at-time = { $time }에 만료됨

# src/popup_window/formatting.rs
expired-at-time-6c39ff = { $time } { $v0 }에 만료됨

# src/popup_window/state.rs
never = 없음

# src/popup_window/formatting.rs
unlimited = 무제한

# src/popup_window/ui/activity.rs
unavailable = 사용할 수 없음

# src/popup_window/formatting.rs
days-d-hours-h = { $days }일 { $hours }시간

# src/popup_window/formatting.rs
days-d = { $days }일

# src/popup_window/formatting.rs
hours-h-minutes-m = { $hours }시간 { $minutes }분

# src/popup_window/formatting.rs
hours-h = { $hours }시간

# src/popup_window/formatting.rs
minutes-m = { $minutes }분

# src/popup_window/formatting.rs
waiting-for-first-update = 첫 번째 업데이트를 기다리는 중

# src/popup_window/formatting.rs
just-now = 방금

# src/popup_window/formatting.rs
seconds-seconds-ago = { $seconds }초 전

# src/popup_window/formatting.rs
minutes-ago = { $v0 }분 전

# src/popup_window/formatting.rs
updated-elapsed = { $elapsed } 업데이트

# src/popup_window/state.rs
the-request-timed-out-try-refreshing-again = 요청 시간이 초과되었습니다. 다시 새로고침해 보세요.

# src/popup_window/state.rs
the-secure-connection-could-not-be-verified-see-log-for-details = 보안 연결을 확인할 수 없습니다. 자세한 내용은 로그를 참조하세요.

# src/popup_window/state.rs
the-provider-closed-the-connection-try-refreshing-again = 공급자가 연결을 종료했습니다. 다시 새로고침해 보세요.

# src/popup_window/state.rs
the-provider-s-address-could-not-be-resolved-check-your-connectio = 공급자의 주소를 확인할 수 없습니다. 연결을 확인하세요.

# src/popup_window/state.rs
could-not-connect-to-the-provider-check-your-connection-and-try-a = 공급자에 연결할 수 없습니다. 연결을 확인하고 다시 시도하세요.

# src/popup_window/state.rs
the-management-key-was-rejected-update-it-in-settings = 관리 키가 거부되었습니다. 설정에서 업데이트하세요.

# src/popup_window/state.rs
authentication-failed-sign-in-again-or-update-the-provider-key = 인증에 실패했습니다. 다시 로그인하거나 공급자 키를 업데이트하세요.

# src/popup_window/state.rs
access-denied-by-the-provider-http-403 = 공급자에 의해 액세스가 거부되었습니다(HTTP 403).

# src/popup_window/state.rs
too-many-requests-wait-a-few-minutes-before-refreshing-again = 요청이 너무 많습니다. 다시 새로 고치기 전에 몇 분 정도 기다리십시오.

# src/popup_window/state.rs
the-provider-is-temporarily-unavailable-try-again-later = 공급자를 일시적으로 사용할 수 없습니다. 나중에 다시 시도하세요.

# src/popup_window/state.rs
the-provider-rejected-the-request-see-log-for-details = 공급자가 요청을 거부했습니다. 자세한 내용은 로그를 참조하세요.

# src/popup_window/state.rs
the-requested-resource-was-not-found-see-log-for-details = 요청한 리소스를 찾을 수 없습니다. 자세한 내용은 로그를 참조하세요.

# src/popup_window/state.rs
the-provider-returned-an-unexpected-response-try-refreshing-again = 공급자가 예상치 못한 응답을 반환했습니다. 다시 새로고침해 보세요.

# src/popup_window/state.rs
the-request-failed-see-log-for-details = 요청이 실패했습니다. 자세한 내용은 로그를 참조하세요.

# src/popup_window/ui/activity.rs
input = 입력

# src/popup_window/ui/activity.rs
cache = 캐시

# src/popup_window/ui/usage.rs
output = 출력

# src/popup_window/ui/activity.rs
input-uncached = 입력(캐시되지 않음)

# src/popup_window/ui/usage.rs
cached-input = 캐시된 입력

# src/popup_window/ui/activity.rs
value-partially-priced = { $value }(일부 가격 책정)

# src/popup_window/ui/activity.rs
could-not-load-model-data-error = 모델 데이터를 로드할 수 없습니다: { $error }

# src/popup_window/ui/activity.rs
usage-activity = 사용 활동

# src/popup_window/ui/activity.rs
waiting-for-cursor-s-usage-export-refresh-to-retry = Cursor의 사용량 내보내기를 기다리는 중입니다. 다시 시도하려면 새로고침하세요.

# src/popup_window/ui/activity.rs
model-data-unavailable = 모델 데이터를 사용할 수 없습니다.

# src/popup_window/ui/activity.rs
loading-models = 모델 로드 중…

# src/popup_window/ui/tooltip.rs
no-model-data = 모델 데이터 없음

# src/popup_window/ui/activity.rs
no-usage-data = 사용 데이터 없음

# src/popup_window/ui/activity.rs
no-series-selected = 선택된 시리즈가 없습니다

# src/popup_window/ui/activity.rs
no-tokens-in-this-period = 이 기간에는 { $v0 } 토큰이 없습니다.

# src/popup_window/ui/activity.rs
loading-model-breakdown = 모델별 내역 불러오는 중

# src/popup_window/ui/activity.rs
group-tokens-or-cost-by-model = 모델별 토큰 또는 비용 그룹화

# src/popup_window/ui/activity.rs
split-type = 유형

# src/popup_window/ui/activity.rs
split-by-token-type = 토큰 유형별로 막대 분할

# src/popup_window/ui/usage.rs
model = 모델

# src/popup_window/ui/usage.rs
project = 프로젝트

# src/usage_overview.rs
unknown-project = 알 수 없는 프로젝트

# src/popup_window/ui/usage.rs
no-project-usage = 이 기간에는 프로젝트 데이터가 없습니다.

# src/popup_window/ui/usage.rs
tokens = 토큰

# src/popup_window/ui/usage.rs
cost = 비용

# src/popup_window/ui/activity.rs
no-cost-data-for-this-period = 이 기간 동안의 비용 데이터가 없습니다.

# src/popup_window/ui/activity.rs
no-token-data-for-this-period = 이 기간 동안 토큰 데이터가 없습니다.

# src/popup_window/ui/activity.rs
daily-cost-in-usd = 일일 비용(USD)

# src/popup_window/ui/activity.rs
daily-token-volume = 일일 토큰 볼륨

# src/popup_window/ui/activity.rs
of-total-models-scroll-for-more = { $total } 모델의 { $v0 }–{ $v1 } · 자세한 내용을 보려면 스크롤하세요.

# src/popup_window/ui/activity.rs
cost-usd-by-model = 모델별 비용(USD)

# src/popup_window/ui/activity.rs
tokens-by-model = 모델별 토큰

# src/settings.rs
today = 오늘

# src/popup_window/ui/activity.rs
last-period-days = 지난 { $period }일

# src/popup_window/ui/activity.rs
no-data = 데이터 없음

# src/popup_window/ui/cards.rs
loading-usage-statistics = 사용 통계 로드 중…

# src/popup_window/ui/cards.rs
available-balance = 사용 가능한 잔액

# src/popup_window/ui/cards.rs
resets-in = 재설정

# src/popup_window/ui/cards.rs
session-not-started = 시작되지 않음

# src/popup_window/ui/cards.rs
expires-in = 만료일

# src/popup_window/ui/cards.rs
usage = 사용량:

# src/settings_window/providers/page.rs
remove-key = 키 제거

# src/popup_window/ui/cards.rs
msg-1-banked-reset = 적립된 재설정 1회

# src/popup_window/ui/cards.rs
count-banked-resets = 적립된 재설정 { $count }회

# src/popup_window/ui/cards.rs
available-to-use = 사용 가능

# src/popup_window/ui/cards.rs
no-expiration-date = 유효기간 없음

# src/popup_window/ui/cards.rs
banked-reset = 적립된 재설정 { $v0 }

# src/popup_window/ui/cards.rs
codex-limits = Codex 한도

# src/popup_window/ui/cards.rs
open-announcement-source = 공지 출처 열기

# src/popup_window/ui/cards.rs
source-not-provided = 소스가 제공되지 않음

# src/popup_window/ui/cards.rs
tibo-reset = Tibo Resets™

# src/settings_window/customize.rs
home = 홈

# src/popup_window/ui/usage.rs
usage-0bb186 = 사용량

# src/popup_window/ui/footer.rs
settings = 설정

# src/popup_window/ui/footer.rs
install-update = 업데이트 설치

# src/popup_window/ui/footer.rs
refreshing-limits-and-usage = 한도와 사용량을 새로고침…

# src/popup_window/ui/footer.rs
updated = 업데이트됨{ " " }

# src/popup_window/ui/footer.rs
refresh-last-updated-relative = 새로고침 | 마지막 업데이트: { $relative }

# src/popup_window/ui/footer.rs
popup-pin = 맨 위에 고정하고 열린 상태 유지
popup-unpin = 고정 해제하고 포커스를 잃으면 닫기

# src/popup_window/ui/home.rs
edit-home = 홈 편집
home-card-layout = 레이아웃
home-card-layout-cards = 카드
home-card-layout-lines = 라인
home-card-layout-rings = 링
home-card-layout-tiles = 타일
home-card-layout-classic = 클래식
drag-to-reorder = 드래그하여 재정렬

# src/popup_window/ui/home.rs
drop-here = 여기에 드롭

# src/settings_window/general.rs
usage-stats = 사용 통계

# src/popup_window/ui/usage.rs
loading-usage = 사용량 로드 중…

# src/settings_window/window.rs
update-failed = 업데이트 실패

# src/popup_window/ui/root.rs
something-went-wrong = 뭔가 잘못됐어요

# src/popup_window/ui/root.rs
no-providers-enabled = 활성화된 공급자가 없습니다.

# src/popup_window/ui/root.rs
turn-one-on-in-settings-providers = 설정 > 공급자에서 하나를 켜세요.

# src/popup_window/ui/root.rs
sign-in-again-to-keep-limits-updating = 한도를 계속 업데이트하려면 다시 로그인하세요.

# src/settings_window/providers/page.rs
sign-in-again = 다시 로그인

# src/popup_window/ui/tooltip.rs
total = 합계

# src/popup_window/ui/usage.rs
enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = 설정에서 공급자를 활성화하고 이를 사용 통계에 포함하여 로컬 API 사용량을 확인합니다.

# src/popup_window/ui/usage.rs
hourly-cost = 시간당 비용

# src/popup_window/ui/usage.rs
hourly-processed-tokens = 시간당 처리 토큰

# src/popup_window/ui/usage.rs
no-activity-in-this-range = 이 기간에는 활동이 없습니다

# src/popup_window/ui/usage.rs
day = 일

# src/popup_window/ui/usage.rs
breakdown = 상세 내역

# src/popup_window/ui/usage.rs
totals = 합계

# src/popup_window/ui/usage.rs
processed-tokens = 처리된 토큰

# src/popup_window/ui/usage.rs
uncached-input = 캐시되지 않은 입력

# src/popup_window/ui/usage.rs
cache-savings = 캐시 절감량

# src/popup_window/ui/usage.rs
share = 비중

# src/popup_window/ui/usage.rs
hour = 시간

# src/usage_overview.rs
sat = 토

# src/usage_overview.rs
sun = 일

# src/provider_registry.rs
msg-5h-session = 5시간 세션

# src/provider_registry.rs
weekly = 주간

# src/provider_registry.rs
cursor-models = Cursor 모델

# src/settings_window/tray.rs
other-models = 기타 모델

# src/provider_registry.rs
all-models = 모든 모델

# src/settings_window/tray.rs
grok-bot = Grok Bot

# src/provider_registry.rs
monthly = 월간

# src/provider_registry.rs
spending-limit = 지출 한도

# src/provider_registry.rs
gemini = Gemini

# src/provider_registry.rs
claude-gpt = Claude + GPT

# src/provider_registry.rs
credits = 크레딧

# src/provider_registry.rs
monthly-credits = 월간 크레딧

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
banked-resets = 적립된 재설정

# src/provider_registry.rs
usage-stats-7b9e1a = 사용 통계

# src/provider_registry.rs
spending = 지출

# src/settings.rs
separate-tabs = 개별 탭

# src/settings.rs
grouped-switcher = 그룹화, 계정 전환

# src/settings.rs
grouped-all-accounts = 그룹화, 모든 계정

# src/settings.rs
openrouter-account = OpenRouter 계정

# src/settings.rs
yesterday = 어제

# src/settings.rs
history-retention-must-be-between-1-and-365-days = 기록 보관 기간은 1~365일이어야 합니다

# src/settings.rs
session-low-usage-threshold-must-be-between-1-and-99-percent = 세션 잔여 한도 알림 기준은 1~99%여야 합니다

# src/settings.rs
weekly-low-usage-threshold-must-be-between-1-and-99-percent = 주간 잔여 한도 알림 기준은 1~99%여야 합니다

# src/settings_window/about.rs
check-github-for-a-new-version = GitHub에서 새 버전 확인

# src/settings_window/about.rs
checking-for-updates = 업데이트 확인 중…

# src/settings_window/about.rs
you-re-up-to-date = 최신 버전입니다

# src/settings_window/about.rs
update-available = 업데이트 { $v0 } 사용 가능

# src/settings_window/about.rs
installing-update = 업데이트 설치 중…

# src/settings_window/about.rs
couldn-t-check-for-updates = 업데이트를 확인할 수 없습니다

# src/settings_window/about.rs
version = 버전 { $v0 }

# src/settings_window/about.rs
about-tagline = Windows 트레이에서 AI 구독의 사용 한도, 재설정, 지출을 확인하는 무료 오픈 소스 도우미입니다.

# src/settings_window/about.rs
a-new-release-is-ready-to-install = 새 버전을 설치할 준비가 되었습니다.

# src/settings_window/about.rs
what-s-new = 새로운 기능

# src/settings_window/about.rs
update = 업데이트

# src/settings_window/about.rs
check-for-updates = 업데이트 확인

# src/settings_window/about.rs
check-for-updates-on-startup = 시작 시 업데이트 확인

# src/settings_window/about.rs
notify-when-a-new-version-is-found = 새 버전 발견 시 알림

# src/settings_window/about.rs
github = GitHub

# src/settings_window/about.rs
source-code = 소스 코드

# src/settings_window/about.rs
releases = 릴리스

# src/settings_window/about.rs
see-what-s-new = 새로운 기능 보기

# src/settings_window/about.rs
report-an-issue = 문제 신고

# src/settings_window/about.rs
found-a-bug = 버그를 발견하셨나요?

# src/settings_window/about.rs
author = 제작자

# src/settings_window/onboarding.rs
updates = 업데이트

# src/settings_window/about.rs
resources = 리소스

# src/usage_overview.rs
mon = 월

# src/usage_overview.rs
tue = 화

# src/usage_overview.rs
wed = 수

# src/usage_overview.rs
thu = 목

# src/usage_overview.rs
fri = 금

# src/settings_window/activation.rs
every-day = 매일

# src/settings_window/activation.rs
weekdays = 평일

# src/settings_window/activation.rs
weekends = 주말

# src/settings_window/activation.rs
no-days = 선택된 요일 없음

# src/settings_window/onboarding.rs
start-5-hour-sessions-automatically = 5시간 세션 자동 시작

# src/settings_window/activation.rs
starts-a-new-session-as-soon-as-a-window-is-available-instead-of = 첫 요청을 기다리지 않고 한도 기간이 열리면 바로 새 세션을 시작합니다. 각 계정은 자체 로그인을 사용합니다.

# src/settings_window/activation.rs
add-codex-or-claude-in-providers-first = 먼저 공급자에서 Codex 또는 Claude를 추가하세요.

# src/settings_window/activation.rs
off-in-providers = 공급자에서 꺼짐

# src/settings_window/activation.rs
quiet-periods = 자동 시작 중지 시간

# src/settings_window/activation.rs
don-t-auto-start-sessions-during-these-times = 이 시간에는 세션을 자동으로 시작하지 않습니다.

# src/settings_window/providers/dialog.rs
add = 추가

# src/settings_window/activation.rs
all-day = 하루 종일

# src/settings_window/activation.rs
from = 시작

# src/settings_window/activation.rs
until = 종료

# src/settings_window/activation.rs
scheduled-activations = 예약된 활성화

# src/settings_window/activation.rs
start-a-5-hour-session-at-a-set-time = 지정한 시각에 5시간 세션을 시작합니다.

# src/settings_window/activation.rs
time = 시간

# src/settings_window/activation.rs
turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = 먼저 구성 폴더 로그인을 통해 Providers에서 Codex 또는 Claude를 켜세요.

# src/settings_window/activation.rs
no-quiet-periods-yet = 아직 자동 시작 중지 시간이 없습니다.

# src/settings_window/activation.rs
no-scheduled-activations-yet = 아직 예약된 활성화가 없습니다.

# src/settings_window/activation.rs
unknown-provider = 알 수 없는 공급자

# src/settings_window/activation.rs
remove-quiet-period = 자동 시작 중지 시간 제거

# src/settings_window/activation.rs
remove-activation = 활성화 제거

# src/settings_window/tray.rs
provider = 공급자

# src/settings_window/activation.rs
days = 일

# src/settings_window/advanced.rs
export-settings = 설정 내보내기

# src/settings_window/advanced.rs
save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = 모든 설정을 .toml 파일에 저장합니다. 저장된 키는 Windows 사용자 저장소에 유지됩니다.

# src/settings_window/advanced.rs
export = 내보내기

# src/settings_window/advanced.rs
import-settings = 설정 가져오기

# src/settings_window/advanced.rs
replace-the-current-settings-with-a-previously-exported-file = 현재 설정을 이전에 내보낸 파일로 바꿉니다.

# src/settings_window/advanced.rs
import = 가져오기

# src/settings_window/advanced.rs
clear-usage-data = 사용 데이터 지우기

# src/settings_window/advanced.rs
delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = 수집된 사용 내역을 삭제합니다. 다음 검색 시 로컬 공급자 로그에서 다시 작성됩니다.

# src/settings_window/advanced.rs
clear = 지우기

# src/settings_window/advanced.rs
usage-data-clear-failed = 사용 데이터 삭제에 실패했습니다.

# src/settings_window/advanced.rs
the-background-worker-is-unavailable = 백그라운드 작업자를 사용할 수 없습니다.

# src/settings_window/advanced.rs
usage-data-cleared = 사용 데이터가 삭제되었습니다.

# src/settings_window/advanced.rs
reset-all-settings = 모든 설정을 재설정

# src/settings_window/advanced.rs
restore-every-default-and-start-the-welcome-flow-again = 모든 기본값을 복원하고 시작 흐름을 다시 시작합니다.

# src/settings_window/advanced.rs
reset = 재설정

# src/settings_window/advanced.rs
backup = 백업

# src/settings_window/advanced.rs
data = 데이터

# src/settings_window/advanced.rs
rendering = 렌더링

# src/settings_window/advanced.rs
software-rendering = 소프트웨어 렌더링

# src/settings_window/advanced.rs
software-rendering-description = 그래픽 카드 대신 CPU에 창을 그립니다. 게임이 GPU를 계속 사용하게 할 때 도움이 될 수 있습니다. 애니메이션이 덜 부드러워질 수 있습니다.

# src/settings_window/advanced.rs
settings-exported = 설정을 내보냈습니다.

# src/settings_window/advanced.rs
settings-export-failed = 설정 내보내기 실패

# src/settings_window/advanced.rs
settings-imported = 설정을 가져왔습니다.

# src/settings_window/advanced.rs
settings-import-failed = 설정 가져오기 실패

# src/settings_window/advanced.rs
settings-reset-failed = 설정 재설정 실패

# src/settings_window/advanced.rs
reset-all-settings-bbfe66 = 모든 설정을 재설정하시겠습니까?

# src/settings_window/advanced.rs
every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = 모든 설정이 기본값으로 돌아가고 환영 흐름이 다시 열립니다. 저장된 키와 사용 데이터는 유지됩니다.

# src/settings_window/troubleshoot.rs
cancel = 취소
use-reset = 재설정 사용
use-reset-short = 사용
reset-using = 사용 중…
reset-confirm-message = 이 계정에 적립된 재설정 1회를 사용하시겠습니까? 적용 가능한 사용 한도가 초기화되며 취소할 수 없습니다.
reset-applied = 재설정이 적용되었습니다. 적용 가능한 한도가 초기화되었습니다.
reset-nothing-to-reset = 지금은 재설정할 항목이 없습니다.
reset-no-credit = 사용할 수 있는 재설정 크레딧이 남아 있지 않습니다.
reset-already-redeemed = 이 재설정은 이미 사용되었습니다.
reset-unconfirmed = 재설정을 확인할 수 없습니다. 동일한 요청을 다시 확인해 보세요.
reset-applied-refresh-failed = 재설정이 적용되었지만 새 제한을 로드할 수 없습니다. 확인하려면 새로고침하세요.
reset-in-progress = 이 계정에 대해 재설정이 이미 진행 중입니다.
reset-cooldown = 재설정 대기 시간 중입니다. 나중에 다시 시도하세요.
reset-rate-limited = 재설정 요청이 너무 많습니다. 나중에 다시 시도하세요.
reset-sign-in-again = 재설정을 사용하려면 Claude에 다시 로그인하세요.
reset-cli-login-required = 재설정을 사용하려면 일치하는 Claude CLI 로그인을 사용하세요.
reset-invalid-grant = Claude가 잘못된 재설정 크레딧을 반환했습니다.

# src/settings_window/appearance.rs
windows = Windows

# src/settings_window/appearance.rs
light = 밝게

# src/settings_window/appearance.rs
dark = 어둡게

# src/settings_window/appearance.rs
color-theme = 색 테마

# src/settings_window/appearance.rs
applies-to-settings-the-popup-and-its-tray-menu = 설정, 팝업 및 해당 트레이 메뉴에 적용됩니다.

# src/settings_window/appearance.rs
accent-color = 강조 색상

# src/settings_window/appearance.rs
windows-follows-your-system-accent = Windows를 선택하면 시스템 강조 색상을 따릅니다.

# src/settings_window/appearance.rs

# src/settings_window/appearance.rs
font = 글꼴

# src/settings_window/appearance.rs
any-font-installed-on-this-pc = 이 PC에 설치된 모든 글꼴입니다.
no-matching-fonts = 일치하는 글꼴이 없습니다

# src/settings_window/appearance.rs
windows-default = Windows 기본값

# src/settings_window/appearance.rs

# src/settings_window/appearance.rs

# src/settings_window/tray.rs
monochrome = 모노크롬

# src/settings_window/appearance.rs
time-format = 시간 형식

# src/settings_window/appearance.rs
msg-12-hour = 12시간

# src/settings_window/appearance.rs
msg-24-hour = 24시간

# src/settings_window/appearance.rs
popup-background = 팝업 배경
popup-theme = 팝업 테마
popup-theme-description = 팝업만 변경합니다. 설정은 Windows 모양을 유지합니다.
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel
popup-theme-built-in = 내장
popup-theme-vscode = VS Code 테마
vscode-themes = VS Code 테마
vscode-themes-description = 팝업에서 Visual Studio Code 색상 테마 사용: Open VSX를 찾아보거나 .vsix 또는 .json 테마 파일을 가져옵니다.
import-theme = 테마 가져오기
importing-theme = 가져오는 중…
remove-theme = 테마 제거
browse-open-vsx = Open VSX 찾아보기
browse-open-vsx-description = open-vsx.org 레지스트리에서 색상 테마를 찾아 설치하세요.
search-color-themes = 색상 테마 검색
open-vsx-search = 검색
open-vsx-browse = 찾아보기
open-vsx-searching = 검색 중…
no-themes-found = 색상 테마를 찾을 수 없습니다.
open-vsx-load-failed = Open VSX에서 테마를 로드할 수 없습니다.
open-vsx-retry = 재시도
open-vsx-preview-loading = 미리보기 로드 중...
open-vsx-preview-hint = 팝업에서 미리 보려면 테마를 클릭하세요. 설치하면 전체 팩이 유지됩니다. 테마의 다운로드 버튼은 해당 버튼만 유지합니다.
open-vsx-preview-empty = 이 확장은 색상 테마를 제공하지 않습니다.
open-vsx-install = 설치
open-vsx-install-one = 이 테마만 설치
open-vsx-installed = 설치됨
open-vsx-installing = 설치 중…
theme-installed = 테마가 설치되었습니다
theme-install-failed = 테마를 설치할 수 없습니다.
theme-remove-failed = 테마를 삭제할 수 없습니다

# src/settings_window/appearance.rs
acrylic = 아크릴

# src/settings_window/appearance.rs
mica = Mica

# src/settings_window/appearance.rs
solid = 솔리드

# src/settings_window/appearance.rs
bottom-bar-size = 하단바 사이즈

# src/settings_window/appearance.rs
comfortable = 넉넉하게

# src/settings_window/appearance.rs
compact = 좁게

# src/settings_window/appearance.rs
popup-corner-radius = 팝업 모서리 반경
popup-borders = 테두리
popup-borders-description = 팝업의 어느 부분에 윤곽이 그려질지 선택하세요
popup-card-borders = 카드 및 컨트롤
popup-window-border = 창 테두리
popup-theme-contrast = 대비
popup-theme-contrast-description = VS Code 테마의 텍스트와 테두리가 얼마나 두드러질지 조정합니다
popup-theme-tint = 틴트
popup-theme-tint-description = 테마 배경이 Mica 또는 Acrylic 효과를 덮는 정도입니다

# src/settings_window/appearance.rs
animation-effects = 애니메이션 효과

# src/settings_window/appearance.rs
glide-transitions-in-the-popup-and-settings-windows-own-animation = 팝업 및 설정에서 글라이드 전환이 이루어집니다. Windows 자체 애니메이션 설정도 존중됩니다.

# src/settings_window/appearance.rs
popup = 팝업

# src/settings_window/appearance.rs
motion = 모션

# src/settings_window/appearance.rs
blue = 블루

# src/settings_window/appearance.rs
purple = 퍼플

# src/settings_window/appearance.rs
pink = 핑크

# src/settings_window/appearance.rs
red = 레드

# src/settings_window/appearance.rs
orange = 오렌지

# src/settings_window/appearance.rs
green = 그린

# src/settings_window/appearance.rs
teal = 틸

# src/settings_window/customize.rs
show-edit-button = 하단 바에 편집 버튼 표시
show-edit-button-description = 편집 모드에서는 홈에 그립이 표시되어 위젯을 새로운 순서로 드래그합니다. 하단 막대를 컴팩트하게 유지하려면 여기에서 시작하세요.
show-edit-button-locked-description = 작업 표시줄에는 항상 편집 버튼이 표시됩니다. 이 스위치는 작업 표시줄을 숨겼을 때 적용됩니다.
start-editing = 편집 시작

# src/settings_window/customize.rs
use-two-columns = 두 열 사용

# src/settings_window/customize.rs
several-accounts-of-one-provider = 한 공급자의 여러 계정

# src/settings_window/customize.rs
separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = 별도의 탭은 모든 인스턴스에 자체 탭을 제공합니다. 그룹화됨에는 공급자당 하나의 탭이 표시되며 계정 전환기 또는 모든 계정이 누적됩니다.

# src/settings_window/customize.rs
use-monochrome-icons = 단색 아이콘 사용

# src/settings_window/customize.rs
draw-provider-marks-in-the-popup-without-brand-colors = 브랜드 색상 없이 팝업에 공급자 표시를 그립니다.

# src/settings_window/onboarding.rs
show-used-instead-of-remaining = 잔여량 대신 사용량 표시

# src/settings_window/customize.rs
show-usage-in-values-when-possible = 값으로 사용량 표시(가능한 경우)

# src/settings_window/customize.rs
adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = 공급자가 보고할 때 백분율 옆에 정확한 사용/한도 금액을 추가합니다.

# src/settings_window/onboarding.rs
show-usage-pace = 사용 속도 표시

# src/settings_window/onboarding.rs
marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = 할당량을 균등한 속도보다 빠르게 또는 느리게 소모하는지 여부를 표시합니다.

# src/settings_window/onboarding.rs
show-account-name = 계정 이름 표시

# src/settings_window/customize.rs
show-on-home-tab = 홈 탭에 표시

# src/settings_window/customize.rs
layout = 레이아웃

# src/settings_window/customize.rs
donut = 도넛

# src/settings_window/customize.rs
cards = 카드

# src/settings_window/customize.rs
tabs = 탭

# src/settings_window/customize.rs
usage-widget = 사용 위젯

# src/settings_window/customize.rs
card = 카드

# src/settings_window/customize.rs
tab = 탭

# src/settings_window/customize.rs
popup-cards = 팝업 카드

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = 이 계정이 홈과 자체 탭에 표시할 카드를 선택하세요.

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = 이 계정이 자체 탭에 표시할 카드를 선택하세요. 홈 카드를 선택하려면 홈에 표시를 켜세요.

# src/settings_window/general.rs
msg-30-seconds = 30초

# src/settings_window/general.rs
msg-1-minute = 1분

# src/settings_window/general.rs
msg-2-minutes = 2분

# src/settings_window/general.rs
msg-5-minutes = 5분

# src/settings_window/general.rs
msg-10-minutes = 10분

# src/settings_window/general.rs
msg-15-minutes = 15분

# src/settings_window/general.rs
msg-30-minutes = 30분

# src/settings_window/general.rs
msg-45-minutes = 45분

# src/settings_window/general.rs
msg-60-minutes = 60분

# src/settings_window/onboarding.rs
start-with-windows = Windows 시작 시 실행

# src/settings_window/general.rs
open-codex-minibar-in-the-tray-when-you-sign-in = 로그인 시 트레이에서 Codex Minibar를 엽니다.

# src/settings_window/onboarding.rs
refresh-interval = 새로 고침 간격

# src/settings_window/general.rs
how-often-this-instance-s-quotas-are-read = 이 인스턴스의 할당량을 읽는 빈도입니다. 간격이 길수록 공급자 비율 제한이 발생하지 않습니다.

# src/settings_window/general.rs
enable-usage-stats = 사용 통계 활성화

# src/settings_window/general.rs
scan-local-provider-history-for-the-usage-tab-and-cost-totals = 사용량 탭 및 총 비용에 대한 로컬 공급자 기록을 스캔합니다.

# src/settings_window/general.rs
collection-period = 수집 주기

# src/settings_window/general.rs
how-often-local-provider-history-is-scanned = 로컬 공급자 기록을 스캔하는 빈도입니다.

# src/settings_window/general.rs
check-for-confirmed-tibo-resets = 확인된 Tibo 재설정을 확인합니다.

# src/settings_window/general.rs
reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = 앱의 공개 GitHub 피드를 읽고 최신 공지 사항을 캐시된 상태로 유지합니다.

# src/settings_window/general.rs
notify-when-new-reset-info-arrives = 새로운 재설정 정보가 도착하면 알림

# src/settings_window/general.rs
shows-a-notification-when-the-feed-reports-a-possible-reset-never = 피드가 재설정 가능성을 보고할 때 알림을 표시하지만 재설정 시간에는 표시하지 않습니다.

# src/settings_window/general.rs
check-every = 매번 확인하세요

# src/settings_window/general.rs
the-feed-is-also-checked-immediately-when-the-app-starts-or-this = 앱이 시작되거나 이 옵션이 활성화되면 피드도 즉시 확인됩니다.

# src/settings_window/general.rs
msg-1-hour = 1시간

# src/settings_window/general.rs
msg-3-hours = 3시간

# src/settings_window/general.rs
msg-2-hours = 2시간

# src/settings_window/general.rs
msg-5-hours = 5시간

# src/settings_window/general.rs
msg-6-hours = 6시간

# src/settings_window/general.rs
msg-12-hours = 12시간

# src/settings_window/general.rs
msg-24-hours = 24시간

# src/settings_window/general.rs
tibo-resets = Tibo Resets™

# src/settings_window/general.rs
enable-a-provider-in-the-providers-tab-to-include-it-here = 여기에 포함하려면 공급자 탭에서 공급자를 활성화하세요.

# src/settings_window/general.rs
add-a-management-key = 관리 키 추가

# src/settings_window/general.rs
usage-statistics-are-off-on-this-provider-s-page = 이 제공업체 페이지의 사용 통계가 꺼져 있습니다.

# src/settings_window/general.rs
included-providers = 포함된 공급자

# src/settings_window/general.rs
choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = 이 컴퓨터의 사용량 탭과 홈 총계에 포함되는 계정을 선택하세요. 제외된 계정은 계속해서 수집하고 자체 페이지에 사용량을 표시합니다.

# src/settings_window/integrations.rs
download-the-latest-stream-deck-companion-from-github-and-open-it = GitHub에서 최신 Stream Deck 컴패니언을 다운로드하고 설치 프로그램을 엽니다.

# src/settings_window/integrations.rs
downloading-the-latest-stream-deck-companion-from-github = GitHub에서 최신 Stream Deck 컴패니언을 다운로드하는 중...

# src/settings_window/integrations.rs
opening-the-stream-deck-installer = Stream Deck 설치 프로그램을 여는 중...

# src/settings_window/integrations.rs
the-installer-is-open-finish-the-installation-in-stream-deck = 설치 프로그램이 열려 있습니다. Stream Deck에서 설치를 완료합니다.

# src/settings_window/integrations.rs
the-plugin-could-not-be-downloaded-or-opened-try-again = 플러그인을 다운로드하거나 열 수 없습니다. 다시 시도해 보세요.

# src/settings_window/integrations.rs
downloading = 다운로드 중...

# src/settings_window/integrations.rs
opening = 여는 중...

# src/settings_window/integrations.rs
install-plugin = 플러그인 설치

# src/settings_window/integrations.rs
stream-deck-companion = Stream Deck 컴패니언

# src/settings_window/integrations.rs
check-your-connection-then-try-again = 연결을 확인한 후 다시 시도하세요.

# src/settings_window/providers/page.rs
on = 켜기

# src/settings_window/providers/page.rs
off = 끄기

# src/settings_window/kit.rs
more-options = 추가 옵션

# src/settings_window/kit.rs
custom-color = 사용자 지정 색상

# src/settings_window/troubleshoot.rs
run-troubleshoot-with-ai = AI로 문제 해결 실행

# src/settings_window/log.rs
let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = 설치된 AI CLI가 로그를 읽고 문제를 조사하도록 합니다.

# src/settings_window/log.rs
choose-tool = 도구 선택

# src/settings_window/log.rs
application-log = 애플리케이션 로그

# src/settings_window/log.rs
log-txt-in-the-app-data-folder = 앱 데이터 폴더의 log.txt.

# src/settings_window/log.rs
open-log-txt = log.txt 열기

# src/settings_window/log.rs
could-not-open-log-txt = log.txt를 열 수 없습니다.

# src/settings_window/log.rs
open-logs-folder = 로그 폴더 열기

# src/settings_window/log.rs
could-not-open-logs-folder = 로그 폴더를 열 수 없습니다.

# src/settings_window/log.rs
no-log-events-yet = 아직 로그 이벤트가 없습니다.

# src/settings_window/log.rs
live-tail = 라이브 테일

# src/settings_window/log.rs
no-supported-ai-tool-found = 지원되는 AI 도구가 없습니다.

# src/settings_window/log.rs
install-codex-or-claude-code-and-make-it-available-to-minibar = Codex 또는 Claude Code를 설치하고 Minibar에서 사용할 수 있도록 합니다.

# src/settings_window/window.rs
general = 일반

# src/settings_window/window.rs
appearance = 모양

# src/settings_window/window.rs
providers = 공급자

# src/settings_window/window.rs
customize = 사용자 지정

# src/settings_window/window.rs
limit-activation = 한도 활성화

# src/settings_window/window.rs
tray = 트레이

# src/settings_window/window.rs
notifications = 알림

# src/settings_window/window.rs
advanced = 고급

# src/settings_window/window.rs
log = 로그

# src/settings_window/window.rs
integrations = 통합

# src/settings_window/nav.rs
about-updates = 정보 및 업데이트

# src/settings_window/onboarding.rs
successful-activations = 활성화 성공

# src/settings_window/onboarding.rs
failed-activations = 활성화 실패

# src/settings_window/onboarding.rs
when-limits-reset = 한도가 재설정될 때

# src/settings_window/onboarding.rs
when-5-hour-remaining-hits = 5시간 잔여 한도가 { $v0 }%에 도달할 때

# src/settings_window/onboarding.rs
when-weekly-remaining-hits = 주간 잔여 한도가 { $v0 }%에 도달할 때

# src/settings_window/onboarding.rs
low-usage = 사용량이 적음

# src/settings_window/notifications.rs
shows-a-notification-once-per-window-when-the-remaining-share-dro = 잔여 비율이 기준값까지 떨어지면 한도 기간마다 한 번 알림을 표시합니다.

# src/settings_window/notifications.rs
threshold = 임계값

# src/settings_window/onboarding.rs
found-in-opencode-auth-or-local-history = OpenCode 인증 또는 로컬 기록에서 발견되었습니다.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-set-up-elsewhere = 찾을 수 없습니다. 다른 곳에 설정되어 있으면 켜세요.

# src/settings_window/onboarding.rs
account-credentials-are-already-set = 계정 자격 증명이 이미 설정되었습니다.

# src/settings_window/onboarding.rs
optional-add-accounts-later-in-providers = 선택사항. 나중에 공급자에서 계정을 추가하세요.

# src/settings_window/onboarding.rs
found-an-official-agy-sign-in-on-this-pc = 이 PC에서 공식 agy 로그인을 찾았습니다.

# src/settings_window/onboarding.rs
not-found-sign-in-with-agy-before-enabling-it = 찾을 수 없습니다. 활성화하기 전에 agy로 로그인하세요.

# src/settings_window/onboarding.rs
found-an-official-grok-cli-sign-in-on-this-pc = 이 PC에서 공식 Grok CLI 로그인을 찾았습니다.

# src/settings_window/onboarding.rs
not-found-run-grok-login-before-enabling-it = 찾을 수 없습니다. 활성화하기 전에 grok login을 실행하십시오.

# src/settings_window/onboarding.rs
found-an-official-command-code-cli-sign-in-on-this-pc = 이 PC에서 공식 Command Code CLI 로그인을 찾았습니다.

# src/settings_window/onboarding.rs
not-found-run-cmdc-login-before-enabling-it = 찾을 수 없습니다. 활성화하기 전에 cmdc login을 실행하세요.

# src/settings_window/onboarding.rs
found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Kiro IDE, Kiro Crew 또는 로그인된 Kiro CLI를 찾았습니다.

# src/settings_window/onboarding.rs
not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = 찾을 수 없습니다. Kiro IDE 또는 Kiro Crew를 설치하거나 Kiro CLI에 로그인하세요.

# src/settings_window/onboarding.rs
found-on-this-pc = 이 PC에서 발견되었습니다.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-installed-somewhere-else = 찾을 수 없습니다. 다른 곳에 설치되어 있으면 켜십시오.

# src/settings_window/onboarding.rs
setup-could-not-be-saved = 설정을 저장할 수 없습니다.

# src/settings_window/onboarding.rs
detected = 감지됨

# src/settings_window/onboarding.rs
starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = 첫 요청을 기다리지 않고 한도 기간이 열리면 바로 새 Codex 또는 Claude 세션을 시작합니다.

# src/settings_window/onboarding.rs
collect-usage-data = 사용 데이터 수집

# src/settings_window/onboarding.rs
scans-local-provider-history-for-usage-stats = 사용 통계에 대한 로컬 공급자 기록을 검색합니다.

# src/settings_window/onboarding.rs
startup = 시작

# src/settings_window/providers/page.rs
features = 기능

# src/settings_window/onboarding.rs
customization = 사용자 지정

# src/settings_window/onboarding.rs
when-a-new-version-is-found = 새로운 버전이 발견되었을 때

# src/settings_window/onboarding.rs
activity = 활동

# src/settings_window/onboarding.rs
choose-providers = 공급자 선택
choose-a-theme = 테마를 선택하세요
theme-step-description = 설정 및 팝업 색상을 선택하세요. 팝업에는 각 선택 사항이 표시됩니다.

# src/settings_window/onboarding.rs
we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = 이 PC에 있는 공급자를 켰습니다. 나중에 변경할 수 있습니다.

# src/settings_window/onboarding.rs
general-settings = 일반 설정

# src/settings_window/onboarding.rs
you-can-change-these-later-in-settings = 나중에 설정에서 변경할 수 있습니다.

# src/settings_window/onboarding.rs
turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = 받고 싶지 않은 알림을 끄세요. 나중에 설정에서 변경할 수 있습니다.

# src/settings_window/window.rs
back = 뒤로

# src/settings_window/onboarding.rs
continue = 계속

# src/settings_window/tray.rs
done = 완료

# src/settings_window/providers/dialog.rs
details = 세부사항

# src/settings_window/providers/dialog.rs
added = 추가됨

# src/settings_window/providers/dialog.rs
reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Claude Code 로그인 없이 Claude 구독의 세션 및 주간 한도를 읽습니다.

# src/settings_window/providers/dialog.rs
msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. 별도의 브라우저 프로필 또는 개인 창에서 Claude를 엽니다. 추적하려는 계정에 로그인하고 Claude의 설정에서 이메일을 확인하세요.

# src/settings_window/providers/dialog.rs
open-claude-ai = claude.ai 열기

# src/settings_window/providers/dialog.rs
msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. Chrome 또는 Edge에서 F12를 누릅니다. 애플리케이션 > 저장소 > 쿠키를 열고 https://claude.ai를 선택합니다.

# src/settings_window/providers/dialog.rs
msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. sessionKey를 찾습니다. 이름이나 전체 쿠키 테이블이 아닌 해당 값을 복사하여 위 필드에 붙여넣습니다.

# src/settings_window/providers/dialog.rs
how-to-view-cookies-in-chrome = Chrome에서 쿠키를 보는 방법

# src/settings_window/providers/dialog.rs
a-cookie-header-containing-sessionkey-also-works-when-the-session = sessionKey가 포함된 쿠키 헤더도 작동합니다. 세션이 만료되면 여기에 새로운 세션을 붙여넣으세요.

# src/settings_window/providers/dialog.rs
use-the-access-token-from-a-claude-code-subscription-login-miniba = Claude Code 구독 로그인의 액세스 토큰을 사용합니다. Minibar는 붙여넣은 토큰을 새로 고칠 수 없습니다. 가능하다면 Source: Config 폴더를 선호하세요.

# src/settings_window/providers/dialog.rs
copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = 해당 로그인의 .credentials.json에서 claudeAiOauth.accessToken(sk-ant-oat로 시작)만 따옴표 없이 복사하세요. claude setup-token을 사용하지 마십시오. 사용 액세스 권한이 부족할 수 있습니다.

# src/settings_window/providers/dialog.rs
claude-code-log-in-with-multiple-accounts = Claude Code: 여러 계정으로 로그인

# src/settings_window/providers/dialog.rs
requires-a-claude-subscription-login-with-usage-access-api-keys-a = 사용 액세스 권한이 있는 Claude 구독 로그인이 필요합니다. API 키와 Admin API 키에는 구독 한도가 표시되지 않습니다.

# src/settings_window/providers/dialog.rs
this-provider = 이 공급자

# src/settings_window/providers/dialog.rs
track-another-account-or-a-provider-minibar-has-not-shown-yet = 다른 계정이나 Minibar에 아직 표시되지 않은 공급자를 추적합니다.

# src/settings_window/providers/dialog.rs
e-g-work = 예: 일

# src/settings_window/providers/page.rs
name = 이름

# src/settings_window/providers/dialog.rs
shown-on-its-tab-home-card-tray-and-notifications = 탭, 홈 카드, 트레이 및 알림에 표시됩니다.

# src/settings_window/providers/page.rs
badge = 뱃지

# src/settings_window/providers/dialog.rs
auto = 자동

# src/settings_window/providers/page.rs
badge-color = 배지 색상

# src/settings_window/providers/dialog.rs
up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = 최대 3글자; 비어 있음은 이름의 이니셜을 사용합니다. 공급자가 두 개 이상의 인스턴스를 활성화한 경우 배지가 표시됩니다.

# src/settings_window/window.rs
add-provider = 공급자 추가

# src/settings_window/providers/dialog.rs
next = 다음

# src/settings_window/providers/dialog.rs
minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar가 { $name } 읽기를 중지하고 저장된 키, 일정, 트레이 표시기 및 홈 위치를 잊어버립니다.{ $v0 }

# src/settings_window/providers/dialog.rs
its-config-folder-stays-on-disk = { " " }구성 폴더가 디스크에 남아 있습니다.

# src/settings_window/providers/dialog.rs
delete-name = { $name }을 삭제하시겠습니까?

# src/settings_window/providers/dialog.rs
delete = 삭제

# src/settings_window/providers/dialog.rs
finish-signing-in-in-your-browser-cancel-stops-this-login = 브라우저에서 로그인을 완료하세요. 취소하면 이 로그인이 중지됩니다.

# src/settings_window/providers/dialog.rs
runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = 구성 폴더를 CLAUDE_CONFIG_DIR로 지정해 { $name }의 Claude Code 로그인을 실행합니다. 로그인 정보는 해당 폴더에 보관되며 Claude Code가 갱신합니다. Windows용 네이티브 Claude Code가 필요합니다.

# src/settings_window/providers/dialog.rs
runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = 구성 폴더를 CODEX_HOME으로 지정해 { $name }의 Codex 로그인을 실행합니다. 로그인 정보는 해당 폴더에 보관되며 Codex가 갱신합니다. 네이티브 Codex CLI 또는 데스크톱 앱이 필요합니다.

# src/settings_window/providers/dialog.rs
sign-in-to-name = { $name }에 로그인합니다.

# src/settings_window/providers/page.rs
sign-in = 로그인

# src/settings_window/providers/dialog.rs
for = { $v0 }의 경우.

# src/settings_window/providers/dialog.rs
browser-session = 브라우저 세션

# src/settings_window/providers/dialog.rs
oauth-token = OAuth 토큰

# src/settings_window/providers/dialog.rs
session-key = 세션 키

# src/settings_window/providers/dialog.rs
paste-the-sessionkey-value = sessionKey 값을 붙여넣으세요.

# src/settings_window/providers/dialog.rs
oauth-access-token = OAuth 액세스 토큰

# src/settings_window/providers/dialog.rs
the-saved-credential-is-replaced-only-after-the-new-one-passes-th = 저장된 자격 증명은 새 자격 증명이 검사를 통과한 후에만 교체됩니다.

# src/settings_window/providers/dialog.rs
claude-credential = Claude 자격 증명

# src/settings_window/providers/dialog.rs
check-and-save = 확인 후 저장

# src/settings_window/providers/dialog.rs
key-name-optional = 키 이름(선택 사항)

# src/settings_window/providers/dialog.rs
e-g-personal = 예: 개인

# src/settings_window/providers/dialog.rs
leave-blank-to-use-the-name-from-openrouter = OpenRouter의 이름을 사용하려면 비워 두세요.

# src/settings_window/providers/dialog.rs
minibar-checks-the-key-with-openrouter-before-saving-it = Minibar는 키를 저장하기 전에 OpenRouter로 키를 확인합니다.

# src/settings_window/providers/dialog.rs
replace-api-key = API 키 교체

# src/settings_window/providers/page.rs
add-api-key = API 키 추가

# src/settings_window/providers/page.rs
management-key = 관리 키

# src/settings_window/providers/dialog.rs
create-one-under-settings-management-keys-on-openrouter-ai = openrouter.ai의 설정 → 관리 키에서 하나를 만듭니다.

# src/settings_window/providers/dialog.rs
replace-management-key = 관리 키 교체

# src/settings_window/providers/page.rs
add-management-key = 관리 키 추가

# src/settings_window/providers/dialog.rs
key-name = 키 이름

# src/settings_window/providers/page.rs
rename-key = 키 이름 바꾸기

# src/settings_window/providers/dialog.rs
save = 저장

# src/settings_window/providers/dialog.rs
this-key = 이 키

# src/settings_window/providers/dialog.rs
minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibar가 { $hint } 추적을 중지합니다. 키는 OpenRouter에서 계속 작동합니다.

# src/settings_window/providers/dialog.rs
remove-api-key = API 키를 제거하시겠습니까?

# src/settings_window/tray.rs
remove = 제거

# src/settings_window/providers/dialog.rs
minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibar에 { $v0 }에 대한 크레딧 잔액 및 사용 내역 표시가 중지되었습니다. 키는 OpenRouter에서 계속 작동합니다.

# src/settings_window/providers/dialog.rs
remove-management-key = 관리 키를 제거하시겠습니까?

# src/settings_window/providers/dialog.rs
saved-in-windows-user-storage-never-in-the-settings-file = Windows 사용자 저장소에 저장되었으며 설정 파일에는 저장되지 않았습니다.

# src/settings_window/providers/dialog.rs
save-key = 키 저장

# src/settings_window/providers/dialog.rs
minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibar가 { $v0 }의 저장된 키를 잊어버렸습니다. OpenCode와 계속 작동합니다.

# src/settings_window/providers/dialog.rs
signing-in = 로그인 중…

# src/settings_window/providers/page.rs
checking = 확인 중…

# src/settings_window/providers/dialog.rs
choose-a-provider = 공급자를 선택하세요.

# src/settings_window/providers/dialog.rs
reason-it-is-already-in-the-list = { $reason }. 이미 목록에 있습니다.

# src/settings_window/providers/dialog.rs
added-name = { $name }을(를) 추가했습니다.

# src/settings_window/providers/dialog.rs
could-not-add-the-provider-error = 공급자를 추가할 수 없습니다: { $error }

# src/settings_window/providers.rs
this-provider-no-longer-exists = 이 공급자는 더 이상 존재하지 않습니다.

# src/settings_window/providers/dialog.rs
could-not-delete-the-provider-error = 공급자를 삭제할 수 없습니다: { $error }

# src/settings_window/providers/dialog.rs
paste-a-credential-first = 먼저 자격 증명을 붙여넣으세요.

# src/settings_window/providers/dialog.rs
credential-saved-in-windows-user-storage = Windows 사용자 저장소에 자격 증명이 저장되었습니다.

# src/settings_window/providers/dialog.rs
paste-a-key-first = 먼저 키를 붙여넣으세요.

# src/settings_window/providers/dialog.rs
this-account-no-longer-exists = 이 계정은 더 이상 존재하지 않습니다.

# src/settings_window/providers/dialog.rs
api-key-saved-in-windows-user-storage = Windows 사용자 저장소에 API 키가 저장되었습니다.

# src/settings_window/providers/dialog.rs
management-key-replaced = 관리 키가 교체되었습니다.

# src/settings_window/providers/dialog.rs
management-key-added = 관리 키가 추가되었습니다.

# src/settings_window/providers/dialog.rs
could-not-rename-the-key-error = 키 이름을 바꿀 수 없습니다: { $error }

# src/settings_window/providers/dialog.rs
api-key-renamed = API 키 이름이 변경되었습니다.

# src/settings_window/providers/dialog.rs
could-not-remove-the-key-error = 키를 제거할 수 없습니다: { $error }

# src/settings_window/providers/dialog.rs
api-key-removed = API 키가 제거되었습니다.

# src/settings_window/providers/dialog.rs
management-key-removed = 관리 키가 제거되었습니다.

# src/settings_window/providers/dialog.rs
could-not-save-the-key-error = 키를 저장할 수 없습니다: { $error }

# src/settings_window/providers/dialog.rs
api-key-saved = API 키가 저장되었습니다.

# src/settings_window/providers/dialog.rs
switch-source-to-config-folder-to-sign-in = 로그인하려면 소스를 구성 폴더로 전환하세요.

# src/settings_window/providers/dialog.rs
this-provider-has-no-config-folder = 이 공급자에는 구성 폴더가 없습니다.

# src/settings_window/providers/dialog.rs
another-instance-already-reads-this-config-folder-choose-a-differ = 다른 인스턴스가 이미 이 구성 폴더를 읽고 있습니다. 먼저 다른 폴더를 선택하세요.

# src/settings_window/providers/dialog.rs
this-provider-has-no-sign-in = 이 제공업체에는 로그인이 없습니다.

# src/settings_window/providers/dialog.rs
signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 }이(가) 로그인되었습니다. CLI는 구성 폴더에 로그인을 최신 상태로 유지합니다.

# src/settings_window/providers/page.rs
no-keys-yet = 아직 키가 없습니다

# src/settings_window/providers/page.rs
management-key-5c8cf2 = { " " }· 관리 키

# src/settings_window/providers/page.rs
using-a-saved-credential = 저장된 자격 증명을 사용하여

# src/settings_window/providers/page.rs
paste-a-credential-under-account = 계정 아래에 자격 증명을 붙여넣습니다.

# src/settings_window/providers/page.rs
needs-an-api-key-or-opencode-sign-in = API 키 또는 OpenCode 로그인이 필요합니다.

# src/settings_window/providers/page.rs
needs-an-api-key = API 키가 필요합니다

# src/settings_window/providers/page.rs
not-found-set-its-folder-under-runtime = 찾을 수 없습니다. 런타임 아래에 해당 폴더를 설정합니다.

# src/settings_window/providers/page.rs
using-a-saved-api-key = 저장된 API 키 사용

# src/settings_window/providers/page.rs
using-opencode-sign-in-or-local-history = OpenCode 로그인 또는 로컬 기록 사용

# src/settings_window/providers/page.rs
reading-cli = { $cli } 읽는 중

# src/settings_window/providers/page.rs
reading-crew = { $crew } 읽는 중

# src/settings_window/providers/page.rs
reading-app = { $app } 읽는 중

# src/settings_window/providers/page.rs
checking-kiro-ide-kiro-crew-and-cli = Kiro IDE, Kiro Crew, CLI를 확인 중…

# src/settings_window/providers/page.rs
checking-installed-app-and-cli = 설치된 앱과 CLI를 확인하는 중…

# src/settings_window/providers/page.rs
checking-cli = CLI를 확인하는 중…

# src/settings_window/providers/page.rs
checking-installed-app = 설치된 앱을 확인하는 중…

# src/settings_window/providers/page.rs
no-providers-yet-add-one-to-start-reading-limits = 아직 공급자가 없습니다. 한도를 읽으려면 공급자를 추가하세요.

# src/settings_window/providers/page.rs
is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 }이 꺼져 있으므로 Minibar나 트레이에 표시되지 않습니다. 사용량을 읽으려면 이 기능을 켜세요.

# src/settings_window/providers/page.rs
replace-chatgpt-logo-with-codex = ChatGPT 로고를 Codex로 교체

# src/settings_window/providers/page.rs
limits-only = 한도만

# src/settings_window/providers/page.rs
delete-provider = 공급자 삭제

# src/settings_window/providers/page.rs
display-name = 표시 이름

# src/settings_window/providers/page.rs
shown-on-popup-tabs-home-cards-the-tray-and-notifications = 팝업 탭, 홈 카드, 트레이 및 알림에 표시됩니다.

# src/settings_window/providers/page.rs
up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = 최대 3글자까지 가능합니다. 이름의 이니셜을 사용하려면 비워 두세요. 공급자에 둘 이상의 인스턴스가 켜져 있는 동안 표시됩니다.

# src/settings_window/providers/page.rs
auto-uses-a-neutral-plate-that-follows-the-theme = Auto는 테마를 따르는 중립 플레이트를 사용합니다.

# src/settings_window/providers/page.rs
show-on-home = 홈에 표시

# src/settings_window/providers/page.rs
its-provider-tab-stays-available-when-hidden-from-home = 홈에서 숨겨도 공급자 탭을 계속 사용할 수 있습니다.

# src/settings_window/providers/page.rs
credential = 자격증명

# src/settings_window/providers/page.rs
signed-in-as-identity = { $identity }(으)로 로그인됨

# src/settings_window/providers/page.rs
saved-in-windows-user-storage = Windows 사용자 저장소에 저장되었습니다.

# src/settings_window/providers/page.rs
paste-a-sessionkey-or-an-oauth-access-token = sessionKey 또는 OAuth 액세스 토큰을 붙여넣습니다.

# src/settings_window/providers/page.rs
reads-limits-only-minibar-cannot-refresh-a-pasted-credential = 한도만 읽습니다. Minibar는 붙여넣은 자격 증명을 갱신할 수 없습니다.

# src/settings_window/providers/page.rs
replace-credential = 자격 증명 교체

# src/settings_window/providers/page.rs
add-credential = 자격 증명 추가

# src/settings_window/providers/page.rs
signed-in-account = 로그인된 계정

# src/settings_window/providers/page.rs
not-signed-in-yet-or-no-limits-read-so-far = 아직 로그인하지 않았거나 지금까지 읽은 제한이 없습니다.

# src/settings_window/providers/page.rs
account = 계정

# src/settings_window/providers/page.rs
runtime = 런타임

# src/settings_window/providers/page.rs
minibar-finds-these-automatically = { $v0 } Minibar는 이를 자동으로 찾습니다.

# src/settings_window/providers/page.rs
in-use = 사용 중

# src/settings_window/providers/page.rs
found = 발견됨

# src/settings_window/providers/page.rs
copy-path = 경로 복사

# src/settings_window/providers/page.rs
open-folder = 폴더 열기

# src/settings_window/providers/page.rs
path-copied = 경로가 복사되었습니다.

# src/settings_window/providers/page.rs
not-installed-or-installed-somewhere-minibar-doesn-t-look = 설치되지 않았거나 Minibar가 검색하지 않는 위치에 설치되었습니다.

# src/settings_window/providers/page.rs
not-found = 찾을 수 없음

# src/settings_window/providers/page.rs
choose-folder = 폴더 선택...

# src/settings_window/providers/page.rs
select-folder = 폴더 선택

# src/settings_window/providers/page.rs
choose-folder-1838a4 = 폴더 선택

# src/settings_window/providers/page.rs
only-needed-if-automatic-detection-misses-your-install = 자동 감지가 설치를 놓친 경우에만 필요합니다.

# src/settings_window/providers/page.rs
source = 소스

# src/settings_window/providers/page.rs
config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = 구성 폴더는 CLAUDE_CONFIG_DIR에서 Claude Code 로그인을 읽습니다. 수동 자격 증명은 붙여넣은 자격 증명에서만 제한을 읽습니다.

# src/settings_window/providers/page.rs
config-folder = 구성 폴더

# src/settings_window/providers/page.rs
manual = 수동

# src/settings_window/providers/page.rs
passed-to-the-cli-as-env-leave-empty-to-use = { $env }으로 CLI에 전달되었습니다. { $v0 }을 사용하려면 비워 두세요.

# src/settings_window/providers/page.rs
the-standard-folder = 표준 폴더

# src/settings_window/providers/page.rs
a-folder-minibar-creates-for-this-instance = 이 인스턴스에 대해 Minibar가 생성하는 폴더

# src/settings_window/providers/page.rs
other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other }이(가) 이미 이 폴더를 읽었습니다. 두 인스턴스가 로그인을 공유하면 안 됩니다. 그렇지 않으면 사용량이 두 번 계산됩니다.

# src/settings_window/providers/page.rs
automatic-activation = 자동 활성화

# src/settings_window/providers/page.rs
starts-this-account-s-5-hour-window-when-it-resets-using-its-own = 재설정 시 이 계정의 로그인으로 5시간 한도 기간을 시작합니다. 일정과 중지 시간은 한도 활성화에서 설정합니다.

# src/settings_window/providers/page.rs
usage-statistics = 사용 통계

# src/settings_window/providers/page.rs
scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = 이 인스턴스의 로컬 기록에서 사용 카드를 검색합니다. 수집을 완전히 중지하려면 끄세요.

# src/settings_window/providers/page.rs
opencode-sign-in-or-local-history = OpenCode 로그인 또는 로컬 기록

# src/settings_window/providers/page.rs
found-in-opencode-auth-environment-a-saved-key-or-local-history = OpenCode 인증, 환경, 저장된 키 또는 로컬 기록에서 발견되었습니다.

# src/settings_window/providers/page.rs
nothing-found-in-opencode-auth-environment-or-local-history = OpenCode 인증, 환경 또는 로컬 기록에서 아무것도 찾을 수 없습니다.

# src/settings_window/providers/page.rs
replace-key = 키 교체

# src/settings_window/providers/page.rs
optional-only-needed-without-opencode-sign-in-on-this-pc = 선택 사항입니다. 이 PC에서 OpenCode 로그인 없이만 필요합니다.

# src/settings_window/providers/page.rs
add-the-key-of-the-account-this-instance-tracks = 이 인스턴스가 추적하는 계정의 키를 추가하세요.

# src/settings_window/providers/page.rs
keys = 키

# src/settings_window/providers/page.rs
a-management-key-shows-credit-balance-and-usage-history-api-keys = 관리 키는 크레딧 잔액과 사용 내역을 표시합니다. API 키는 키별 지출을 표시합니다.

# src/settings_window/providers/page.rs
could-not-read-the-saved-key-reopen-this-page-to-retry = 저장된 키를 읽을 수 없습니다. 다시 시도하려면 이 페이지를 다시 여세요.

# src/settings_window/providers/page.rs
credit-balance-and-account-wide-usage-history = 크레딧 잔액 및 계정 전체 사용 내역

# src/settings_window/providers/page.rs
not-added-add-one-to-see-credit-balance-and-usage-history = 추가되지 않았습니다. 크레딧 잔액 및 사용 내역을 보려면 하나를 추가하세요.

# src/settings_window/providers/page.rs
last-updated = 마지막 업데이트 { $v0 }, { $v1 }

# src/settings_window/providers/page.rs
no-api-keys-yet-add-one-to-track-spend-per-key = 아직 API 키가 없습니다. 키당 지출을 추적하려면 하나를 추가하세요.

# src/settings_window/providers/page.rs
key = 키

# src/settings_window/providers/page.rs
spend = 지출

# src/settings_window/providers/page.rs
limit = 한도

# src/settings_window/providers/page.rs
error-reopen-this-page-to-retry = { $error }. 다시 시도하려면 이 페이지를 다시 여세요.

# src/settings_window/providers/page.rs
could-not-read-key = 키를 읽을 수 없습니다

# src/settings_window/providers/page.rs
not-saved = 저장되지 않았습니다

# src/settings_window/providers/page.rs
unnamed = 이름 없음

# src/settings_window/providers/page.rs
of-the-limit = { $v1 } 제한의 { $v0 }

# src/settings_window/providers/page.rs
no-spend-limit-key-spend-account-credits-purchased = 지출 한도 없음. 키 지출: { $v0 }. 구매한 계정 크레딧: { $v1 }.

# src/settings_window/providers/page.rs
none = 없음

# src/settings_window/providers/page.rs
add-key = 키 추가

# src/settings_window/providers.rs
openrouter-account-credentials-are-configured = OpenRouter 계정 자격 증명이 구성되었습니다.

# src/settings_window/providers.rs
saved-api-key = 저장된 API 키

# src/settings_window/providers.rs
saved-credential = 저장된 자격 증명

# src/settings_window/providers.rs
opencode-auth-json-or-local-database = OpenCode auth.json 또는 로컬 데이터베이스

# src/settings_window/providers.rs
that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = OpenRouter 키가 아닌 것 같습니다. 키는 sk-or-로 시작합니다.

# src/settings_window/providers.rs
codex-cli-folder = Codex CLI 폴더

# src/settings_window/providers.rs
claude-code-cli-folder = Claude Code CLI 폴더

# src/settings_window/providers.rs
cursor-app-folder = Cursor 앱 폴더

# src/settings_window/providers.rs
grok-cli-folder = Grok CLI 폴더

# src/settings_window/providers.rs
command-code-cli-folder = Command Code CLI 폴더

# src/settings_window/providers.rs
kiro-ide-folder = Kiro IDE 폴더

# src/settings_window/providers.rs
kiro-crew-app-path = Kiro Crew 앱 경로

# src/settings_window/providers.rs
kiro-cli-folder = Kiro CLI 폴더

# src/settings_window/providers.rs
reads-the-signed-in-codex-cli-or-desktop-app = 로그인된 Codex CLI 또는 데스크톱 앱을 읽습니다.

# src/settings_window/providers.rs
reads-your-existing-claude-code-login = 기존 Claude Code 로그인을 읽습니다.

# src/settings_window/providers.rs
reads-the-signed-in-cursor-app-for-this-billing-cycle = 이 청구 주기에 대해 로그인된 Cursor 앱을 읽습니다.

# src/settings_window/providers.rs
reads-zen-auth-and-local-opencode-history = Zen 인증 및 로컬 OpenCode 기록을 읽습니다.

# src/settings_window/providers.rs
reads-go-quota-windows-and-local-opencode-history = Go의 기간별 한도와 로컬 OpenCode 기록을 읽습니다.

# src/settings_window/providers.rs
reads-api-key-usage-and-spend-limits-a-management-key-also-enable = API 키 사용량 및 지출 한도를 읽습니다. 관리 키를 사용하면 사용 내역 및 크레딧 잔액도 확인할 수 있습니다.

# src/settings_window/providers.rs
reads-subscription-quota-from-your-existing-official-agy-windows = 기존 공식 agy Windows 로그인에서 구독 할당량을 읽습니다.

# src/settings_window/providers.rs
reads-supergrok-subscription-credits-from-your-existing-official = 기존 공식 Grok CLI 로그인에서 SuperGrok 구독 크레딧을 읽습니다.

# src/settings_window/providers.rs
reads-command-code-5h-weekly-and-monthly-credits-from-your-existin = 기존 공식 CLI 로그인에서 Command Code 5시간, 주별, 월별 크레딧을 읽습니다.

# src/settings_window/providers.rs
fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = 공유 로그인을 통해 Kiro의 실시간 월간 크레딧을 가져옵니다. IDE, Crew 및 CLI 설치를 인식합니다.

# src/settings_window/providers.rs
codex-desktop-app = Codex 데스크톱 앱

# src/settings_window/providers.rs
codex-cli = Codex CLI

# src/settings_window/providers.rs
claude-desktop-app = Claude 데스크톱 앱

# src/settings_window/providers.rs
claude-code-cli = Claude Code CLI

# src/settings_window/providers.rs
cursor-app = Cursor 앱

# src/settings_window/providers.rs
antigravity-app = Antigravity 앱

# src/settings_window/providers.rs
grok-cli = Grok CLI

# src/settings_window/providers.rs
command-code-cli = Command Code CLI

# src/settings_window/providers.rs
kiro-ide = Kiro IDE

# src/settings_window/providers.rs
kiro-crew = Kiro Crew

# src/settings_window/providers.rs
kiro-cli = Kiro CLI

# src/settings_window/providers.rs
supports-one-instance = { $v0 }은 하나의 인스턴스를 지원합니다.

# src/settings_window/tray.rs
numbers = 숫자

# src/settings_window/tray.rs
progress-bars = 진행률 표시줄

# src/settings_window/tray.rs
rings = 링

# src/settings_window/tray.rs
reset-time = 재설정 시간

# src/settings_window/tray.rs
countdown = 카운트다운

# src/settings_window/tray.rs
status = 상태

# src/settings_window/tray.rs
fixed = 고정됨

# src/settings_window/tray.rs
app-accent = 앱 강조 색상

# src/settings_window/tray.rs
percentages-as-digits = 백분율을 숫자로 표현

# src/settings_window/tray.rs
one-bar-per-indicator = 표시기당 막대 1개

# src/settings_window/tray.rs
nested-rings-one-per-indicator = 중첩된 링, 표시기당 하나씩

# src/settings_window/tray.rs
when-the-limit-resets = 한도가 재설정될 때

# src/settings_window/tray.rs
time-left-until-the-reset = 초기화까지 남은 시간

# src/settings_window/tray.rs
used = 사용량

# src/settings_window/tray.rs
remaining = 잔여량

# src/settings_window/tray.rs
app-icon = 앱 아이콘

# src/settings_window/tray.rs
widget = 위젯 { $v0 }

# src/settings_window/tray.rs
the-codex-minibar-icon = Codex Minibar 아이콘

# src/settings_window/tray.rs
style-no-indicators = { $style } · 표시 없음

# src/settings_window/tray.rs
widget-removed = 위젯이 제거되었습니다.

# src/settings_window/tray.rs
undo = 실행 취소

# src/settings_window/tray.rs
tray-icon = 트레이 아이콘

# src/settings_window/tray.rs
shows-the-app-icon = 앱 아이콘을 표시합니다.

# src/settings_window/tray.rs
add-widget = 위젯 추가

# src/settings_window/tray.rs
add-app-icon = 앱 아이콘 추가

# src/settings_window/tray.rs
move-up = 위로 이동

# src/settings_window/tray.rs
move-down = 아래로 이동

# src/settings_window/tray.rs
edit = 편집

# src/settings_window/tray.rs
duplicate = 복제

# src/settings_window/tray.rs
shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = 알림 영역에 Codex Minibar 아이콘이 표시됩니다. 설정할 표시가 없습니다.

# src/settings_window/tray.rs
style = 스타일

# src/settings_window/tray.rs
up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = 최대 { $max_indicators } 할당량이 이 순서대로 그려집니다. 변경하려면 하나를 확장하세요.

# src/settings_window/tray.rs
indicators = 인디케이터

# src/settings_window/tray.rs
remove-widget = 위젯 제거

# src/settings_window/tray.rs
add-indicator = 표시 추가

# src/settings_window/tray.rs
unsupported = 지원되지 않음 { $v0 }

# src/settings_window/tray.rs
remove-indicator = 표시 제거

# src/settings_window/tray.rs
unsupported-bba2a8 = 지원되지 않음({ $v0 })

# src/settings_window/tray.rs
unavailable-0c5d75 = 사용할 수 없음({ $v0 })

# src/settings_window/tray.rs
metric = 지표

# src/settings_window/tray.rs
show = 표시

# src/settings_window/tray.rs
color = 색상

# src/settings_window/tray.rs
fixed-color = 고정 색상

# src/settings_window/troubleshoot.rs
troubleshooting-could-not-start = 문제 해결을 시작할 수 없습니다.

# src/settings_window/troubleshoot.rs
choose-which-installed-ai-tool-should-investigate-the-problem = 문제를 조사해야 하는 설치된 AI 도구를 선택합니다.

# src/settings_window/troubleshoot.rs
open-terminal = 터미널 열기

# src/settings_window/window.rs
version-is-available = { $version }을(를) 사용할 수 있습니다.

# src/settings_window/window.rs
update-now = 지금 업데이트

# src/settings_window/window.rs
missing-a-provider = 공급자가 없나요?

# src/settings_window/window.rs
ask-for-it-or-build-it-yourself = 요청하거나 직접 만들어 보세요.

# src/settings_window/window.rs
request-a-provider = 공급자 요청

# src/settings_window/window.rs
contribute-one = 공급자 구현 기여

# src/tray.rs
disabled = 비활성화됨

# src/updater.rs
update-complete = 업데이트 완료

# src/updater.rs
now-running-version = 현재 { $version }을(를) 실행 중입니다.

# src/usage_overview.rs
past-24h = 지난 24시간

# application
language = 언어

# application
applies-immediately-to-every-app-window-and-notification-auto-fol = 모든 앱 창과 알림에 즉시 적용됩니다. 자동은 Windows 표시 언어를 따릅니다.

# application
auto-windows = 자동 (Windows)

# application
english = 영어

# application
msg-russian = Русский

# application
codex-minibar-settings = Codex Minibar 설정

# application
welcome-to-codex-minibar = Codex Minibar에 오신 것을 환영합니다

# application
exit = 종료

# application
update-available-67fd3a = 업데이트 가능

# application
monthly-credits-976559 = 월간 크레딧

# application
msg-5h-session-de7ce8 = 5시간 세션

# application
name-weekly = { $name } 주간

# application
tokens-94e0b9 = { $v0 }: { $v1 } 토큰

# application
requests-priced = 요청 { $v0 }건 · 가격 확인 { $v1 }건

# application
requests = 요청 { $v0 }건

# application
credits-338f52 = 크레딧

# application
cloud-session-credits = 클라우드 세션 크레딧

# application
name-login-expires-in-days-left = { $name } 로그인은 { $days_left }일 후 만료됩니다

# application
name-error = { $name } 오류

# application
sessions = 세션 { $v0 }개

# application
api-estimate = API 추정

# application
start-to-end = { $start } ~ { $end }

# application
to = { $v0 } ~ { $v1 }

# application
share-1-of-other = { $v0 }의 { $share }% · { $other }

# application
excluded-other = 제외됨 · { $other }

# application
exclude-from-usage-stats = 사용 통계에서 제외하려면 클릭하세요.

# application
include-in-usage-stats = 사용 통계에 포함하려면 클릭하세요.

# application
cost-885dc4 = 비용

# application
tokens-339143 = 토큰

# application
sessions-0e5e29 = · 세션 { $v0 }개

# application
api-key = API 키

# application
api-keys = API 키

# application
name-deleted = { $name } 삭제됨.

# application
openrouter-api-key-no-longer-exists = OpenRouter API 키가 더 이상 존재하지 않습니다.

# application
openrouter-account-no-longer-exists = OpenRouter 계정이 더 이상 존재하지 않습니다.

# application
credit = { $v0 } 크레딧

# application
agy-cli-folder = agy CLI 폴더

# application
folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = codex.exe, codex.cmd 또는 codex.ps1이 포함된 폴더입니다. 자동으로 찾으려면 비워두세요.

# application
folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = claude.exe, claude.cmd 또는 claude.ps1이 포함된 폴더입니다. 자동으로 찾으려면 비워두세요.

# application
folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = Cursor.exe가 포함된 폴더입니다. 자동으로 찾으려면 비워두세요. 여전히 로그인된 프로필을 통해 사용량이 발생합니다.

# application
folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = agy.exe, agy.cmd 또는 agy.ps1이 포함된 폴더입니다. 자동으로 찾으려면 비워두세요.

# application
folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = grok.exe, grok.cmd 또는 grok.ps1이 포함된 폴더입니다. 자동으로 찾으려면 비워두세요.

# src/settings_window/providers.rs
folder-with-cmdc-cmd-or-cmdc-ps1-leave-empty-to-find-it-automati = cmdc.cmd 또는 cmdc.ps1이 포함된 폴더입니다. 자동으로 찾으려면 비워두세요.

# application
folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = Kiro.exe 또는 실행 파일 자체가 포함된 폴더입니다. 자동으로 찾으려면 비워두세요.

# application
folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = KiroCrew.exe 또는 실행 파일 자체가 포함된 폴더입니다. 사용자별 및 모든 사용자 설치를 자동으로 감지하려면 비워 두세요.

# application
folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = kiro-cli.exe 또는 실행 파일 자체가 포함된 폴더입니다. 자동으로 찾으려면 비워두세요.

# application
off-45080e = { $v0 } (꺼짐)

# application
each-widget-is-one-icon-in-the-notification-area-indicators-show = 각 위젯은 알림 영역에 있는 하나의 아이콘입니다. 표시기는 공급자의 할당량을 숫자, 막대, 링 또는 재설정 시계로 표시합니다.

# application
a-reset-clock-follows-one-quota = 재설정 시계는 하나의 한도를 표시합니다.

# application
ai-tool = AI 도구

# application
msg-7-days = 7일

# application
msg-30-days = 30일

# application
msg-90-days = 90일

# application
yellow = 노랑

# application
custom = 사용자 지정

# application
not-available-for-manual-credentials-switch-source-to-config-fold = 수동 자격 증명에는 사용할 수 없습니다. 소스를 구성 폴더로 전환합니다.

# application
opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = OpenCode의 로컬 기록이 첫 번째 OpenCode 인스턴스에 의해 추적됩니다.

# application
this-provider-has-no-local-usage-history = 이 공급자에는 로컬 사용 기록이 없습니다.

# application
this-provider-has-no-session-window-to-start = 이 공급자에는 시작할 세션 한도 기간이 없습니다.

# application
this-provider-does-not-use-a-config-folder = 이 공급자는 구성 폴더를 사용하지 않습니다.

# application
could-not-save-provider-settings-error-restoring-its-previous-cre = 공급자 설정({ $error })을 저장할 수 없습니다. 이전 자격 증명 복원도 실패했습니다({ $rollback_error }).

# application
msg-5h-7d = 5시간 |  { $v0 } |  { $v1 }
    7일 |  { $v2 } |  { $v3 }

# API key count in the provider header.
api-key-count = API 키 { $v0 }개

# Application copy
widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = 홈과 사용량 화면을 넓힙니다. 홈 블록을 열 사이로 드래그할 수 있으며 공급자 탭은 간결하게 유지됩니다.

# Application copy
a-possible-codex-reset-is-scheduled-for-when-in-countdown = Codex 한도가 { $when }에 재설정될 수 있습니다 ({ $countdown } 후)

# Application copy
label-possible-reset-on-when-in-countdown = { $label }: { $when }에 재설정 가능 ({ $countdown } 후)

# Application copy
new-codex-reset-info = 새 Codex 재설정 정보

# Application copy
login-expires-soon = { $v0 } 로그인이 곧 만료됩니다.

# Application copy
it-stops-renewing-on-open-minibar-and-choose-sign-in-again = { $v0 }에서 갱신이 중지됩니다. Minibar를 열고 다시 로그인을 선택합니다.

# Application copy
could-not-clear-usage-data-error = 사용 데이터를 지울 수 없습니다: { $error }

# Application copy
succeeded-at = { $v0 }이(가) { $v1 }에서 성공했습니다.

# Application copy
failed-at-error = { $v0 }이(가) { $v1 }에서 실패했습니다: { $error }

# Application copy
key-39df89 = 키 { $v0 }

# Application copy
expired = 만료됨

# Application copy
jan = 1월

# Application copy
feb = 2월

# Application copy
mar = 3월

# Application copy
apr = 4월

# Application copy
may = 5월

# Application copy
jun = 6월

# Application copy
jul = 7월

# Application copy
aug = 8월

# Application copy
sep = 9월

# Application copy
oct = 10월

# Application copy
nov = 11월

# Application copy
dec = 12월

# { $value }% used
quota-percent-used = { $value }% 사용됨

# { $value }% left
quota-percent-left = { $value }% 남음

# { $amount } of { $limit } used
cloud-amount-used = { $limit } 중 { $amount } 사용

# { $amount } of { $limit } left
cloud-amount-left = { $limit } 중 { $amount } 남음

# Short usage range
range-24h = 24시간

# Short usage range
range-7d = 7일

# Short usage range
range-30d = 30일

# Short usage range
range-90d = 90일

# Home usage card title
home-usage-title = 사용량

# OpenRouter key administration

could-not-reach-openrouter = OpenRouter에 연결할 수 없습니다.


openrouter-keys-title = 키


openrouter-keys-new = 새 키


openrouter-keys-new-title = 새 키


openrouter-keys-back = 뒤로


openrouter-keys-updating = 업데이트 중…


openrouter-keys-loading = 키 로드 중…


openrouter-keys-load-failed = 키를 로드할 수 없습니다.


openrouter-keys-retry = 재시도


openrouter-keys-empty = 이 계정에는 아직 키가 없습니다.


openrouter-keys-this-app = 이 앱


openrouter-keys-no-limit = 제한 없음


openrouter-keys-of-limit = { $limit } 중 { $amount }


openrouter-keys-resets-daily = 매일 재설정됩니다.


openrouter-keys-resets-weekly = 매주 재설정됩니다.


openrouter-keys-resets-monthly = 매월 재설정


openrouter-keys-expires-on = { $date } 만료


openrouter-keys-expired-on = { $date } 만료됨


openrouter-keys-today = 오늘 { $amount }


openrouter-keys-usage-breakdown = 오늘 { $today } · 주 { $week } · 월 { $month } · 합계 { $total }


openrouter-keys-show-all = 키 { $count }개 모두 표시


openrouter-keys-show-fewer = 간략히 표시


openrouter-keys-spending-limit = 지출 한도


openrouter-keys-limit-hint = 제한이 없으면 비워 두세요.


openrouter-keys-resets = 재설정


openrouter-keys-reset-never = 재설정 안 함


openrouter-keys-reset-daily = 매일


openrouter-keys-reset-weekly = 주간


openrouter-keys-reset-monthly = 월간


openrouter-keys-byok = BYOK 사용량을 한도에 포함


openrouter-keys-enabled = 활성화됨


openrouter-keys-delete = 삭제


openrouter-keys-delete-tracked = Minibar도 추적을 중지합니다.


openrouter-keys-saving = 저장 중…


openrouter-keys-delete-title = "{ $name }"을(를) 삭제하시겠습니까?


openrouter-keys-delete-message = 이 키를 사용하는 모든 도구가 즉시 작동을 멈춥니다. 삭제는 취소할 수 없습니다. 대신 비활성화하면 나중에 다시 켤 수 있습니다.


openrouter-keys-keep = 키 유지


openrouter-keys-delete-confirm = 키 삭제


openrouter-keys-deleting = 삭제 중…


openrouter-keys-name = 이름


openrouter-keys-name-placeholder = 예: laptop-cursor


openrouter-keys-name-required = 키의 이름을 입력하세요.


openrouter-keys-invalid-amount = 25 또는 12.50과 같은 달러 금액을 입력하세요.


openrouter-keys-expires = 만료


openrouter-keys-expires-1-hour = 1시간


openrouter-keys-expires-1-day = 1일


openrouter-keys-expires-7-days = 7일


openrouter-keys-expires-30-days = 30일


openrouter-keys-expires-90-days = 90일


openrouter-keys-expires-180-days = 180일


openrouter-keys-expires-1-year = 1년


openrouter-keys-expires-never = 만료 없음


openrouter-keys-track = Minibar에서 이 키를 추적하세요


openrouter-keys-create = 키 생성


openrouter-keys-creating = 생성 중…


openrouter-keys-created = 키가 생성되었습니다.


openrouter-keys-copy = 키 복사


openrouter-keys-copied = 복사됨


openrouter-keys-done = 완료


openrouter-keys-once-title = 이 키는 다시는 볼 수 없습니다


openrouter-keys-once-message = OpenRouter가 새 키를 한 번만 표시합니다. 지금 복사하세요.


openrouter-keys-pinned = 완료를 누를 때까지 팝업이 계속 열려 있습니다.


openrouter-keys-tracking = Minibar에 키 추가 중…


openrouter-keys-tracked = 이제 Minibar가 이 키를 추적합니다.


openrouter-keys-track-failed = Minibar에 키를 추가할 수 없습니다: { $error }


openrouter-keys-no-management-key = 이 계정에는 관리 키가 없습니다.


openrouter-keys-management-key-rejected = OpenRouter가 관리 키를 거부했습니다. 설정에서 교체하세요.


openrouter-keys-not-a-management-key = 이 키는 다른 키를 관리할 수 없습니다. 관리 키를 사용하세요.


openrouter-keys-key-not-found = 이 키는 더 이상 존재하지 않습니다.


openrouter-keys-untrack = 앱에서 삭제


openrouter-keys-untracking = 제거 중…


openrouter-keys-untrack-title = “{ $name }” 추적을 중지하시겠습니까?


openrouter-keys-untrack-message = Minibar에서 이 키를 제거합니다. OpenRouter에서는 계속 작동합니다.


openrouter-keys-tracked-here = Minibar가 이 키를 추적합니다.


openrouter-keys-manage-hint = 키를 생성, 편집 및 삭제하려면 설정에 관리 키를 추가하세요.

# src/settings_window/notifications.rs
notification-sounds = 알림음

# src/settings_window/notifications.rs
plays-a-short-sound-with-each-notification = 각 알림마다 짧은 소리를 재생합니다.

# src/updater.rs
update-downloading = 업데이트 다운로드 중

# src/updater.rs
update-download-progress = { $total } 중 { $downloaded }

# src/updater.rs
update-downloaded = { $downloaded } 다운로드됨

# src/updater.rs
update-installing = 업데이트 설치 중

# src/updater.rs
update-unpacking = 파일 압축을 푸는 중…

# src/updater.rs
update-restarting = Codex Minibar 다시 시작

# src/updater.rs
update-restarting-body = 잠시 후 앱이 다시 열립니다.

# src/settings_window/general.rs
provider-status = 공급자 상태

# src/settings_window/general.rs
show-provider-incidents = 공급자 장애 표시

# src/settings_window/general.rs
show-provider-incidents-description = 5분마다 Codex, Claude, Cursor의 공개 상태 페이지를 확인하고 진행 중인 장애를 팝업에 표시합니다

# src/settings_window/general.rs
notify-about-new-incidents = 새 장애 알림

# src/settings_window/general.rs
notify-about-new-incidents-description = 활성화된 공급자의 상태 페이지에 새 장애가 보고되면 알림을 표시합니다

# src/popup_window/ui/cards.rs
open-status-page = 상태 페이지 열기

# src/notifications.rs
provider-incident-title = { $provider }: { $impact }

# src/popup_window/ui/cards.rs
status-since = { $time } 이후

# src/provider_status.rs
status-impact-maintenance = 유지보수

# src/provider_status.rs
status-impact-degraded = 성능 저하

# src/provider_status.rs
status-impact-partial-outage = 일부 서비스 중단

# src/provider_status.rs
status-impact-major-outage = 대규모 서비스 중단

# src/provider_status.rs
status-stage-investigating = 조사 중

# src/provider_status.rs
status-stage-identified = 원인 확인됨

# src/provider_status.rs
status-stage-monitoring = 모니터링

# src/provider_status.rs
status-stage-in-progress = 진행 중

# src/settings_window/notifications.rs
news-and-status = 뉴스 및 현황

# src/whats_new.rs
no-summary-for-this-version = 이 버전에는 아직 요약이 없습니다. 전체 릴리스 노트는 GitHub에 있습니다.

# src/whats_new.rs
couldn-t-load-what-s-new = 새로운 내용을 로드할 수 없습니다. 연결을 확인하거나 GitHub에서 릴리스 노트를 읽어보세요.

# src/whats_new.rs
full-release-notes = 전체 릴리스 노트

# src/settings_window/about.rs
show-what-s-new-after-updating = 업데이트 후 새로운 기능 표시

# src/settings_window/about.rs
a-short-summary-of-the-release-once-it-s-installed = 설치 후 릴리스에 대한 간략한 요약입니다.

# src/settings_window/about.rs
view = 보기

# src/settings_window/general.rs
popup-shortcut = 팝업 단축키
popup-shortcut-description = Windows 어디서나 이 단축키를 눌러 팝업을 표시하거나 숨깁니다.
shortcut-not-set = 설정되지 않음
press-a-shortcut = 단축키를 누르세요…
popup-shortcut-recording-hint = Esc를 누르면 취소하고 Backspace를 누르면 단축키를 제거합니다.
popup-shortcut-needs-modifier = Ctrl, Alt 또는 Win을 포함하거나 F13–F24를 사용하세요.
popup-shortcut-in-use = 다른 앱이 이미 이 단축키를 사용하고 있습니다. 다른 것을 선택하세요.
clear-shortcut = 단축키 제거
taskbar = 작업 표시줄
preview = 미리보기
taskbar-page-caption = Windows 작업 표시줄 내부 시계 옆에 실시간 사용량 위젯이 그려집니다. 작업 표시줄의 테마를 따릅니다. 하나를 클릭하면 공급자에 대한 팝업이 열립니다.
taskbar-show-widgets = 작업 표시줄에 위젯 표시
taskbar-show-widgets-description = Windows 작업 표시줄 내부에 아래 위젯을 그립니다.
taskbar-position = 위치
taskbar-position-description = 작업 표시줄의 위젯 위치입니다. 시작은 중앙 작업 표시줄 아이콘에 가장 적합합니다.
taskbar-position-tray = 알림 영역 옆
taskbar-position-start = 작업 표시줄 시작 부분
taskbar-widgets = 위젯
taskbar-no-widgets = 아직 위젯이 없습니다
taskbar-no-widgets-description = 작업 표시줄에 할당량을 표시하는 위젯을 추가합니다.
taskbar-show-provider-icon = 공급자 아이콘 표시
taskbar-style-glance = 한눈에
taskbar-style-glance-hint = 날씨 버튼과 같은 두 줄의 텍스트 옆에 있는 공급자 아이콘입니다.
taskbar-style-rings-hint = 옆에 값이 있는 할당량당 진행 링입니다.
taskbar-style-bars-hint = 공급자 아이콘 옆에 얇은 진행률 표시줄이 쌓였습니다.
taskbar-menu-open = Codex Minibar 열기
taskbar-menu-settings = 작업 표시줄 위젯 설정
taskbar-menu-hide = 작업 표시줄 위젯 숨기기
taskbar-card-left = { $value } 남음
taskbar-card-used = { $value } 사용됨
taskbar-card-resets = { $time } 후 재설정 · { $clock }
taskbar-card-no-data = 데이터를 기다리는 중
short-5h = 5시간
short-7d = 7일
taskbar-style-nested = 중첩된 링
taskbar-style-dials = 다이얼
taskbar-style-meters = 미터
taskbar-style-nested-hint = 원할 경우 옆에 값이 있는 하나의 공간에 동심원 고리가 있습니다.
taskbar-style-dials-hint = 내부에 숫자가 포함된 할당량당 작은 링입니다.
taskbar-style-meters-hint = 배터리 게이지처럼 슬림한 레벨의 기둥입니다.
taskbar-show-provider-icon-description = 위젯 시작 부분에 공급자 표시가 있습니다.
taskbar-show-labels = 라벨 표시
taskbar-show-labels-description = 값 옆에 5h 및 7d와 같은 짧은 이름.
taskbar-show-values = 백분율 표시
taskbar-show-values-description = 위젯의 할당량 백분율입니다.
taskbar-show-pace = 사용 속도 표시
taskbar-show-pace-description = 한도를 일정한 속도로 사용할 경우 현재 시점의 예상 사용량을 표시합니다.
taskbar-menu-edit = 이 위젯 편집
taskbar-content = 콘텐츠
taskbar-content-hint = 위젯이 그래픽 옆에 쓰는 내용.
taskbar-reset-time = 재설정 시간
taskbar-reset-time-description = 각 한도가 재설정되는 시각입니다.
taskbar-reset-off = 꺼짐
taskbar-reset-countdown = 남은 시간
taskbar-reset-clock = 시계 시간
taskbar-cycle = 스크롤 휠로 위젯 순환
taskbar-cycle-description = 한 번에 하나의 위젯을 표시합니다. 그 위로 스크롤하여 다음으로 전환하세요.
taskbar-card-tokens = 토큰 { $count }개
taskbar-click = 클릭 시 열기
taskbar-click-description = 위젯을 클릭하면 팝업 페이지가 열립니다.
taskbar-click-nearest = 홈 또는 홈에 없는 경우 탭
taskbar-click-tab = 공급자 탭
taskbar-click-home = 홈

# src/settings_window/notifications.rs
follow-do-not-disturb = 방해 금지 모드 중 일시 중지

# src/settings_window/notifications.rs
follow-do-not-disturb-description = Windows 방해 금지 또는 집중이 켜져 있는 동안 알림은 건너뛰고 소리는 꺼집니다. 시작한 작업의 결과는 계속 표시됩니다.

# src/settings_window/advanced.rs
performance = 성능

# src/settings_window/advanced.rs
game-mode = 게임 모드

# src/settings_window/advanced.rs
game-mode-description = 한도 및 사용량을 덜 자주 확인하고 낮은 우선순위로 실행하며 알림을 일시중지합니다. 게임이 실행되는 동안 게임이 시작되는 순간부터 종료될 때까지 자동으로 켜집니다.

# src/settings.rs
game-mode-off = 꺼짐

# src/settings.rs
game-mode-automatic = 자동

# src/settings.rs
game-mode-always = 항상 켜짐

# src/settings_window/notifications.rs
notify-game-mode-changes = 게임 모드 변경

# src/settings_window/notifications.rs
notify-game-mode-changes-description = 게임 모드가 켜지거나 꺼질 때 알림을 표시합니다

# src/notifications.rs
game-mode-on-title = 게임 모드 켜짐

# src/notifications.rs
game-mode-on-body = 한도 확인 빈도를 줄이고 알림을 일시 중지합니다

# src/notifications.rs
game-mode-on-game-body = { $game } 실행 중. 한도 확인 빈도를 줄이고 알림을 일시 중지합니다

# src/notifications.rs
not-a-game = 게임 아님

# src/settings_window/advanced.rs
add-a-game = 게임 추가

# src/settings_window/advanced.rs
add-a-game-description = Windows Game Bar에 등록된 게임과 Steam, Epic, GOG, Xbox 게임은 자동으로 감지합니다. 그 외 프로그램은 여기에 추가하세요.

# src/settings_window/advanced.rs
never-a-game-path = 게임으로 감지하지 않음 · { $path }

# src/notifications.rs
game-mode-off-title = 게임 모드 꺼짐

# src/notifications.rs
game-mode-off-body = 일반 확인 주기와 알림으로 돌아갑니다

# src/settings_window/command_palette.rs
command-palette = Command Palette

# src/settings_window/command_palette.rs
stream-deck = Stream Deck

# src/settings_window/command_palette.rs
install = 설치

# src/settings_window/command_palette.rs
cmdpal-extension = Command Palette 확장

# src/settings_window/command_palette.rs
cmdpal-description = PowerToys Command Palette에 Minibar를 추가합니다. 공급자 이름을 입력해 한도를 확인하거나 Dock에 밴드를 고정할 수 있습니다. 설치 시 관리자 승인이 필요합니다.

# src/settings_window/command_palette.rs
cmdpal-installed-hint = 설치되었습니다. Command Palette에 “minibar” 또는 공급자 이름을 입력하거나 Edit Dock에서 밴드를 고정하세요.

# src/settings_window/command_palette.rs
cmdpal-downloading = 확장 다운로드 중…

# src/settings_window/command_palette.rs
cmdpal-installing = 설치 프로그램을 기다리는 중입니다. 계속하려면 관리자 권한 요청을 승인하세요.

# src/settings_window/command_palette.rs
cmdpal-installing-short = 설치 중…

# src/settings_window/command_palette.rs
cmdpal-removing = 확장 제거 중…

# src/settings_window/command_palette.rs
cmdpal-failed = 확장을 설치하거나 제거할 수 없습니다.

# src/settings_window/command_palette.rs
cmdpal-reinstall = 다시 설치

# src/settings_window/command_palette.rs
cmdpal-dock-bands = Dock 밴드

# src/settings_window/command_palette.rs
cmdpal-dock-bands-caption = 각 밴드는 Command Palette의 Dock 설정에 표시됩니다. Edit Dock에서 고정한 다음 위치를 선택하세요.

# src/settings_window/command_palette.rs
cmdpal-no-bands = 밴드 없음

# src/settings_window/command_palette.rs
cmdpal-no-bands-description = 밴드를 추가해 공급자 한도를 Dock에 고정하세요.

# src/settings_window/command_palette.rs
cmdpal-add-band = 밴드 추가

# src/settings_window/command_palette.rs
cmdpal-band = 밴드 { $v0 }

# src/settings_window/command_palette.rs
cmdpal-band-removed = 밴드 제거됨

# src/settings_window/command_palette.rs
cmdpal-remove-band = 밴드 제거

# src/settings_window/command_palette.rs
cmdpal-content-hint = Dock은 밴드를 제목과 부제목으로 표시합니다.

# src/settings_window/command_palette.rs
cmdpal-indicators-hint = 최대 2개 한도: 하나를 선택하면 제목과 부제목에 표시하고, 두 개를 선택하면 한 줄씩 표시합니다.

# src/popup_window/ui/cards.rs
tibo-banked-reset = Tibo Banked Reset™

# src/popup_window/ui/cards.rs
banked-reset-credit = 적립된 재설정

# src/popup_window/ui/cards.rs
before-when = { $when } 이전

# src/popup_window/ui/cards.rs
time-ago = { $time } 전

# src/popup_window/ui/cards.rs
new-codex-banked-reset-info = 새 Codex 적립 재설정 정보

# src/popup_window/ui/cards.rs
a-possible-codex-reset-is-expected-before-when-within-countdown = Codex 한도가 { $when } 이전에 재설정될 수 있습니다 (최대 { $countdown } 이내)

# src/popup_window/ui/cards.rs
label-possible-reset-before-when-within-countdown = { $label }: { $when } 이전 재설정 가능 (최대 { $countdown } 이내)

# src/popup_window/ui/cards.rs
time-late = { $time } 지연

# Floating usage panel
popup-floating = 플로팅 패널
popup-floating-description = 패널을 열어 두고 헤더를 드래그하여 위치나 모니터를 변경합니다.
popup-always-on-top = 항상 위에 표시
popup-always-on-top-description = 패널을 다른 앱 위에 표시하고 앱을 전환해도 열린 상태로 유지합니다.
popup-position = 패널 위치
popup-position-description = 세션 간에 위치가 저장됩니다.
popup-reset-position = 위치 초기화
popup-drag-header = 드래그하여 이동
popup-close = 패널 닫기
popup-pop-out = 플로팅 패널로 분리
popup-dock = 작업 표시줄로 되돌리기

show-popup-action-bar = 팝업 작업 표시줄 표시
show-popup-action-bar-description = 일반 팝업에 새로 고침, 설정, 레이아웃 버튼을 표시합니다. 플로팅 패널에는 항상 표시됩니다.
