### Codex Minibar application messages.
### Turkish translation. Keep $parameters unchanged.

# src/provider_registry.rs
luna-reserve = Luna Reserve

# src/limits.rs
on-pace = Dengeli

# src/limits.rs
delta-in-deficit = %{ $delta } açık

# src/limits.rs
delta-in-reserve = %{ $delta } yedek

# src/notifications.rs
msg-5-hour-limit-started = 5 saatlik limit başladı

# src/notifications.rs
msg-5-hour-limit-reset-and-activated = 5 saatlik limit sıfırlandı ve etkinleştirildi

# src/notifications.rs
msg-5-hour-limit-reset = 5 saatlik limit sıfırlandı

# src/notifications.rs
weekly-limit-reset = Haftalık limit sıfırlandı

# src/notifications.rs
name-5-hour = { $name } 5 saatlik

# src/notifications.rs
label-limit-is-low = { $label } limiti azalıyor

# src/popup_window/formatting.rs
expired-at-time = { $time } itibarıyla süresi doldu

# src/popup_window/formatting.rs
expired-at-time-6c39ff = { $time } { $v0 } itibarıyla süresi doldu

# src/popup_window/state.rs
never = Hiçbir zaman

# src/popup_window/formatting.rs
unlimited = Sınırsız

# src/popup_window/ui/activity.rs
unavailable = Kullanılamıyor

# src/popup_window/formatting.rs
days-d-hours-h = { $days }g { $hours }sa

# src/popup_window/formatting.rs
days-d = { $days }g

# src/popup_window/formatting.rs
hours-h-minutes-m = { $hours }sa { $minutes }dk

# src/popup_window/formatting.rs
hours-h = { $hours }sa

# src/popup_window/formatting.rs
minutes-m = { $minutes }dk

# src/popup_window/formatting.rs
waiting-for-first-update = İlk güncelleme bekleniyor

# src/popup_window/formatting.rs
just-now = az önce

# src/popup_window/formatting.rs
seconds-seconds-ago = { $seconds } saniye önce

# src/popup_window/formatting.rs
minutes-ago = { $v0 } dakika önce

# src/popup_window/formatting.rs
updated-elapsed = Güncellendi: { $elapsed }

# src/popup_window/state.rs
the-request-timed-out-try-refreshing-again = İstek zaman aşımına uğradı. Yenilemeyi tekrar deneyin.

# src/popup_window/state.rs
the-secure-connection-could-not-be-verified-see-log-for-details = Güvenli bağlantı doğrulanamadı. Ayrıntılar için günlüğe bakın.

# src/popup_window/state.rs
the-provider-closed-the-connection-try-refreshing-again = Sağlayıcı bağlantıyı kapattı. Yenilemeyi tekrar deneyin.

# src/popup_window/state.rs
the-provider-s-address-could-not-be-resolved-check-your-connectio = Sağlayıcının adresi çözümlenemedi. Bağlantınızı kontrol edin.

# src/popup_window/state.rs
could-not-connect-to-the-provider-check-your-connection-and-try-a = Sağlayıcıya bağlanılamadı. Bağlantınızı kontrol edip tekrar deneyin.

# src/popup_window/state.rs
the-management-key-was-rejected-update-it-in-settings = Yönetim anahtarı reddedildi. Ayarlar'dan güncelleyin.

# src/popup_window/state.rs
authentication-failed-sign-in-again-or-update-the-provider-key = Kimlik doğrulama başarısız. Yeniden oturum açın veya sağlayıcı anahtarını güncelleyin.

# src/popup_window/state.rs
access-denied-by-the-provider-http-403 = Sağlayıcı erişimi reddetti (HTTP 403).

# src/popup_window/state.rs
too-many-requests-wait-a-few-minutes-before-refreshing-again = Çok fazla istek gönderildi. Yenilemeden önce birkaç dakika bekleyin.

# src/popup_window/state.rs
the-provider-is-temporarily-unavailable-try-again-later = Sağlayıcı geçici olarak kullanılamıyor. Daha sonra tekrar deneyin.

# src/popup_window/state.rs
the-provider-rejected-the-request-see-log-for-details = Sağlayıcı isteği reddetti. Ayrıntılar için günlüğe bakın.

# src/popup_window/state.rs
the-requested-resource-was-not-found-see-log-for-details = İstenen kaynak bulunamadı. Ayrıntılar için günlüğe bakın.

# src/popup_window/state.rs
the-provider-returned-an-unexpected-response-try-refreshing-again = Sağlayıcı beklenmeyen bir yanıt döndürdü. Yenilemeyi tekrar deneyin.

# src/popup_window/state.rs
the-request-failed-see-log-for-details = İstek başarısız. Ayrıntılar için günlüğe bakın.

# src/popup_window/ui/activity.rs
input = Girdi

# src/popup_window/ui/activity.rs
cache = Önbellek

# src/popup_window/ui/usage.rs
output = Çıktı

# src/popup_window/ui/activity.rs
input-uncached = Girdi (önbelleksiz)

# src/popup_window/ui/usage.rs
cached-input = Önbelleğe alınmış girdi

# src/popup_window/ui/activity.rs
value-partially-priced = { $value } (kısmen fiyatlandırıldı)

# src/popup_window/ui/activity.rs
could-not-load-model-data-error = Model verileri yüklenemedi: { $error }

# src/popup_window/ui/activity.rs
usage-activity = Kullanım etkinliği

# src/popup_window/ui/activity.rs
waiting-for-cursor-s-usage-export-refresh-to-retry = Cursor kullanım dışa aktarımı bekleniyor. Tekrar denemek için yenileyin.

# src/popup_window/ui/activity.rs
model-data-unavailable = Model verileri kullanılamıyor

# src/popup_window/ui/activity.rs
loading-models = Modeller yükleniyor…

# src/popup_window/ui/tooltip.rs
no-model-data = Model verisi yok

# src/popup_window/ui/activity.rs
no-usage-data = Kullanım verisi yok

# src/popup_window/ui/activity.rs
no-series-selected = Seri seçilmedi

# src/popup_window/ui/activity.rs
no-tokens-in-this-period = Bu dönemde { $v0 } token yok

# src/popup_window/ui/activity.rs
loading-model-breakdown = Model dökümü yükleniyor

# src/popup_window/ui/activity.rs
group-tokens-or-cost-by-model = Token veya maliyeti modele göre grupla

# src/popup_window/ui/activity.rs
split-type = Tür

# src/popup_window/ui/activity.rs
split-by-token-type = Çubukları token türüne göre böl

# src/popup_window/ui/usage.rs
model = Model

# src/popup_window/ui/usage.rs
project = Proje

# src/usage_overview.rs
unknown-project = Bilinmeyen proje

# src/popup_window/ui/usage.rs
no-project-usage = Bu dönemde proje verisi yok

# src/popup_window/ui/usage.rs
tokens = Tokenlar

# src/popup_window/ui/usage.rs
cost = Maliyet

# src/popup_window/ui/activity.rs
no-cost-data-for-this-period = Bu döneme ait maliyet verisi yok

# src/popup_window/ui/activity.rs
no-token-data-for-this-period = Bu döneme ait token verisi yok

# src/popup_window/ui/activity.rs
daily-cost-in-usd = USD cinsinden günlük maliyet

# src/popup_window/ui/activity.rs
daily-token-volume = Günlük token hacmi

# src/popup_window/ui/activity.rs
of-total-models-scroll-for-more = { $total } modelden { $v0 }–{ $v1 } · Daha fazlası için kaydırın

# src/popup_window/ui/activity.rs
cost-usd-by-model = Modele göre maliyet (USD)

# src/popup_window/ui/activity.rs
tokens-by-model = Modele göre tokenlar

# src/settings.rs
today = Bugün

# src/popup_window/ui/activity.rs
last-period-days = Son { $period } gün

# src/popup_window/ui/activity.rs
no-data = Veri yok

# src/popup_window/ui/cards.rs
loading-usage-statistics = Kullanım istatistikleri yükleniyor…

# src/popup_window/ui/cards.rs
available-balance = Kullanılabilir bakiye

# src/popup_window/ui/cards.rs
resets-in = Sıfırlanmaya kalan

# src/popup_window/ui/cards.rs
session-not-started = Başlamadı

# src/popup_window/ui/cards.rs
expires-in = Sürenin dolmasına kalan

# src/popup_window/ui/cards.rs
usage = Kullanım:

# src/settings_window/providers/page.rs
remove-key = Anahtarı kaldır

# src/popup_window/ui/cards.rs
msg-1-banked-reset = 1 Birikmiş Sıfırlama

# src/popup_window/ui/cards.rs
count-banked-resets = { $count } Sıfırlama

# src/popup_window/ui/cards.rs
available-to-use = Kullanıma hazır

# src/popup_window/ui/cards.rs
no-expiration-date = Son kullanma tarihi yok

# src/popup_window/ui/cards.rs
banked-reset = Birikmiş Sıfırlama { $v0 }

# src/popup_window/ui/cards.rs
codex-limits = Codex limitleri

# src/popup_window/ui/cards.rs
open-announcement-source = Duyuru kaynağını aç

# src/popup_window/ui/cards.rs
source-not-provided = Kaynak belirtilmedi

# src/popup_window/ui/cards.rs
tibo-reset = Tibo Reset™

# src/settings_window/customize.rs
home = Ana Sayfa

# src/popup_window/ui/usage.rs
usage-0bb186 = Kullanım

# src/popup_window/ui/footer.rs
settings = Ayarlar

# src/popup_window/ui/footer.rs
install-update = Güncellemeyi yükle

# src/popup_window/ui/footer.rs
refreshing-limits-and-usage = Limitler ve kullanım yenileniyor…

# src/popup_window/ui/footer.rs
updated = Güncellendi{ " " }

# src/popup_window/ui/footer.rs
refresh-last-updated-relative = Yenile | Son güncelleme: { $relative }

# src/popup_window/ui/footer.rs
popup-pin = Üste sabitle ve açık tut
popup-unpin = Sabitlemeyi kaldır ve odak kaybolunca kapat

# src/popup_window/ui/home.rs
edit-home = Ana Sayfayı düzenle
home-card-layout = Düzen
home-card-layout-cards = Kartlar
home-card-layout-lines = Satırlar
home-card-layout-rings = Halkalar
home-card-layout-tiles = Kutucuklar
home-card-layout-classic = Klasik
drag-to-reorder = Sıralamak için sürükleyin

# src/popup_window/ui/home.rs
drop-here = Buraya bırakın

# src/settings_window/general.rs
usage-stats = Kullanım İstatistikleri

# src/popup_window/ui/usage.rs
loading-usage = Kullanım yükleniyor…

# src/settings_window/window.rs
update-failed = Güncelleme başarısız

# src/popup_window/ui/root.rs
something-went-wrong = Bir sorun oluştu

# src/popup_window/ui/root.rs
no-providers-enabled = Etkin sağlayıcı yok

# src/popup_window/ui/root.rs
turn-one-on-in-settings-providers = Ayarlar > Sağlayıcılar bölümünden birini etkinleştirin.

# src/popup_window/ui/root.rs
sign-in-again-to-keep-limits-updating = Limitlerin güncellenmesi için yeniden oturum açın.

# src/settings_window/providers/page.rs
sign-in-again = Yeniden oturum aç

# src/popup_window/ui/tooltip.rs
total = Toplam

# src/popup_window/ui/usage.rs
enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = Yerel API kullanımını görmek için Ayarlar'dan bir sağlayıcıyı etkinleştirip Kullanım İstatistiklerine dahil edin.

# src/popup_window/ui/usage.rs
hourly-cost = Saatlik maliyet

# src/popup_window/ui/usage.rs
hourly-processed-tokens = Saatlik işlenen tokenlar

# src/popup_window/ui/usage.rs
no-activity-in-this-range = Bu aralıkta etkinlik yok

# src/popup_window/ui/usage.rs
day = Gün

# src/popup_window/ui/usage.rs
breakdown = Döküm

# src/popup_window/ui/usage.rs
totals = Toplamlar

# src/popup_window/ui/usage.rs
processed-tokens = İşlenen tokenlar

# src/popup_window/ui/usage.rs
uncached-input = Önbelleksiz girdi

# src/popup_window/ui/usage.rs
cache-savings = Önbellek tasarrufu

# src/popup_window/ui/usage.rs
share = Pay

# src/popup_window/ui/usage.rs
hour = Saat

# src/usage_overview.rs
sat = Cmt

# src/usage_overview.rs
sun = Paz

# src/provider_registry.rs
msg-5h-session = 5 sa. oturum

# src/provider_registry.rs
weekly = Haftalık

# src/provider_registry.rs
cursor-models = Cursor Modelleri

# src/settings_window/tray.rs
other-models = Diğer Modeller

# src/provider_registry.rs
all-models = Tüm Modeller

# src/settings_window/tray.rs
grok-bot = Grok Bot

# src/provider_registry.rs
monthly = Aylık

# src/provider_registry.rs
spending-limit = Harcama limiti

# src/provider_registry.rs
gemini = Gemini

# src/provider_registry.rs
claude-gpt = Claude + GPT

# src/provider_registry.rs
credits = Krediler

# src/provider_registry.rs
monthly-credits = Aylık krediler

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
banked-resets = Birikmiş sıfırlamalar

# src/provider_registry.rs
usage-stats-7b9e1a = Kullanım istatistikleri

# src/provider_registry.rs
spending = Harcama

# src/settings.rs
separate-tabs = Ayrı sekmeler

# src/settings.rs
grouped-switcher = Gruplu, seçicili

# src/settings.rs
grouped-all-accounts = Gruplu, tüm hesaplar

# src/settings.rs
openrouter-account = OpenRouter hesabı

# src/settings.rs
yesterday = Dün

# src/settings.rs
history-retention-must-be-between-1-and-365-days = geçmiş saklama süresi 1 ile 365 gün arasında olmalıdır

# src/settings.rs
session-low-usage-threshold-must-be-between-1-and-99-percent = oturum düşük kullanım eşiği yüzde 1 ile 99 arasında olmalıdır

# src/settings.rs
weekly-low-usage-threshold-must-be-between-1-and-99-percent = haftalık düşük kullanım eşiği yüzde 1 ile 99 arasında olmalıdır

# src/settings_window/about.rs
check-github-for-a-new-version = GitHub'da yeni sürüm ara

# src/settings_window/about.rs
checking-for-updates = Güncellemeler denetleniyor…

# src/settings_window/about.rs
you-re-up-to-date = Güncelsiniz

# src/settings_window/about.rs
update-available = { $v0 } güncellemesi mevcut

# src/settings_window/about.rs
installing-update = Güncelleme yükleniyor…

# src/settings_window/about.rs
couldn-t-check-for-updates = Güncellemeler denetlenemedi

# src/settings_window/about.rs
version = Sürüm { $v0 }

# src/settings_window/about.rs
about-tagline = Yapay zekâ abonelik limitleri, sıfırlamalar ve harcamalar için ücretsiz, açık kaynaklı yardımcınız; doğrudan Windows sistem tepsisinde.

# src/settings_window/about.rs
a-new-release-is-ready-to-install = Yeni sürüm yüklenmeye hazır.

# src/settings_window/about.rs
what-s-new = Yenilikler

# src/settings_window/about.rs
update = Güncelle

# src/settings_window/about.rs
check-for-updates = Güncellemeleri denetle

# src/settings_window/about.rs
check-for-updates-on-startup = Başlangıçta güncellemeleri denetle

# src/settings_window/about.rs
notify-when-a-new-version-is-found = Yeni sürüm bulunduğunda bildir

# src/settings_window/about.rs
github = GitHub

# src/settings_window/about.rs
source-code = Kaynak kodu

# src/settings_window/about.rs
releases = Sürümler

# src/settings_window/about.rs
see-what-s-new = Yeniliklere göz at

# src/settings_window/about.rs
report-an-issue = Sorun bildir

# src/settings_window/about.rs
found-a-bug = Hata mı buldunuz?

# src/settings_window/about.rs
author = Geliştirici

# src/settings_window/onboarding.rs
updates = Güncellemeler

# src/settings_window/about.rs
resources = Kaynaklar

# src/usage_overview.rs
mon = Pzt

# src/usage_overview.rs
tue = Sal

# src/usage_overview.rs
wed = Çar

# src/usage_overview.rs
thu = Per

# src/usage_overview.rs
fri = Cum

# src/settings_window/activation.rs
every-day = Her gün

# src/settings_window/activation.rs
weekdays = Hafta içi

# src/settings_window/activation.rs
weekends = Hafta sonu

# src/settings_window/activation.rs
no-days = Gün seçilmedi

# src/settings_window/onboarding.rs
start-5-hour-sessions-automatically = 5 saatlik oturumları otomatik başlat

# src/settings_window/activation.rs
starts-a-new-session-as-soon-as-a-window-is-available-instead-of = İlk isteğinizi beklemek yerine, uygun bir zaman aralığı oluşur oluşmaz yeni oturum başlatır. Her hesap kendi oturum açma bilgilerini kullanır.

# src/settings_window/activation.rs
add-codex-or-claude-in-providers-first = Önce Sağlayıcılar bölümüne Codex veya Claude ekleyin.

# src/settings_window/activation.rs
off-in-providers = Sağlayıcılar bölümünde kapalı

# src/settings_window/activation.rs
quiet-periods = Sessiz dönemler

# src/settings_window/activation.rs
don-t-auto-start-sessions-during-these-times = Bu saatlerde oturumları otomatik başlatma.

# src/settings_window/providers/dialog.rs
add = Ekle

# src/settings_window/activation.rs
all-day = Tüm gün

# src/settings_window/activation.rs
from = Başlangıç

# src/settings_window/activation.rs
until = Bitiş

# src/settings_window/activation.rs
scheduled-activations = Zamanlanmış etkinleştirmeler

# src/settings_window/activation.rs
start-a-5-hour-session-at-a-set-time = Belirlenen saatte 5 saatlik oturum başlat.

# src/settings_window/activation.rs
time = Saat

# src/settings_window/activation.rs
turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = Önce Sağlayıcılar bölümünde yapılandırma klasörüyle oturum açarak Codex veya Claude'u etkinleştirin.

# src/settings_window/activation.rs
no-quiet-periods-yet = Henüz sessiz dönem yok.

# src/settings_window/activation.rs
no-scheduled-activations-yet = Henüz zamanlanmış etkinleştirme yok.

# src/settings_window/activation.rs
unknown-provider = Bilinmeyen sağlayıcı

# src/settings_window/activation.rs
remove-quiet-period = Sessiz dönemi kaldır

# src/settings_window/activation.rs
remove-activation = Etkinleştirmeyi kaldır

# src/settings_window/tray.rs
provider = Sağlayıcı

# src/settings_window/activation.rs
days = Günler

# src/settings_window/advanced.rs
export-settings = Ayarları dışa aktar

# src/settings_window/advanced.rs
save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = Tüm ayarları bir .toml dosyasına kaydet. Kayıtlı anahtarlar Windows kullanıcı depolamasında kalır.

# src/settings_window/advanced.rs
export = Dışa aktar

# src/settings_window/advanced.rs
import-settings = Ayarları içe aktar

# src/settings_window/advanced.rs
replace-the-current-settings-with-a-previously-exported-file = Mevcut ayarları daha önce dışa aktarılmış bir dosyayla değiştir.

# src/settings_window/advanced.rs
import = İçe aktar

# src/settings_window/advanced.rs
clear-usage-data = Kullanım verilerini temizle

# src/settings_window/advanced.rs
delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = Toplanan Kullanım geçmişini sil. Sonraki taramada yerel sağlayıcı günlüklerinden yeniden oluşturulur.

# src/settings_window/advanced.rs
clear = Temizle

# src/settings_window/advanced.rs
usage-data-clear-failed = Kullanım verileri temizlenemedi

# src/settings_window/advanced.rs
the-background-worker-is-unavailable = Arka plan işçisi kullanılamıyor.

# src/settings_window/advanced.rs
usage-data-cleared = Kullanım verileri temizlendi.

# src/settings_window/advanced.rs
reset-all-settings = Tüm ayarları sıfırla

# src/settings_window/advanced.rs
restore-every-default-and-start-the-welcome-flow-again = Tüm varsayılanları geri yükle ve karşılama akışını yeniden başlat.

# src/settings_window/advanced.rs
reset = Sıfırla

# src/settings_window/advanced.rs
backup = Yedekleme

# src/settings_window/advanced.rs
data = Veri

# src/settings_window/advanced.rs
rendering = İşleme

# src/settings_window/advanced.rs
software-rendering = Yazılımsal işleme

# src/settings_window/advanced.rs
software-rendering-description = Pencereleri ekran kartı yerine CPU ile çiz. Bir oyun GPU'yu meşgul ettiğinde yardımcı olabilir; animasyonlar daha az akıcı olabilir.

# src/settings_window/advanced.rs
settings-exported = Ayarlar dışa aktarıldı.

# src/settings_window/advanced.rs
settings-export-failed = Ayarlar dışa aktarılamadı

# src/settings_window/advanced.rs
settings-imported = Ayarlar içe aktarıldı.

# src/settings_window/advanced.rs
settings-import-failed = Ayarlar içe aktarılamadı

# src/settings_window/advanced.rs
settings-reset-failed = Ayarlar sıfırlanamadı

# src/settings_window/advanced.rs
reset-all-settings-bbfe66 = Tüm ayarlar sıfırlansın mı?

# src/settings_window/advanced.rs
every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = Tüm ayarlar varsayılan değerlerine döner ve karşılama akışı yeniden açılır. Kayıtlı anahtarlar ve Kullanım verileri korunur.

# src/settings_window/troubleshoot.rs
cancel = İptal
use-reset = Sıfırlama kullan
use-reset-short = Kullan
reset-using = Kullanılıyor…
reset-confirm-message = Bu hesap için bir birikmiş sıfırlama kullanılsın mı? Uygun hız limiti dönemleri temizlenir ve bu işlem geri alınamaz.
reset-applied = Sıfırlama uygulandı. Uygun limitleriniz temizlendi.
reset-nothing-to-reset = Şu anda sıfırlanacak bir şey yok.
reset-no-credit = Kullanılabilir sıfırlama kredisi kalmadı.
reset-already-redeemed = Bu sıfırlama zaten kullanıldı.
reset-unconfirmed = Sıfırlama doğrulanamadı. Aynı isteği kontrol etmek için tekrar deneyin.
reset-applied-refresh-failed = Sıfırlama uygulandı, ancak yeni limitler yüklenemedi. Kontrol etmek için yenileyin.
reset-in-progress = Bu hesap için zaten bir sıfırlama sürüyor.
reset-cooldown = Sıfırlamalar bekleme süresinde. Daha sonra tekrar deneyin.
reset-rate-limited = Çok fazla sıfırlama isteği. Daha sonra tekrar deneyin.
reset-sign-in-again = Sıfırlamaları kullanmak için Claude'da yeniden oturum açın.
reset-cli-login-required = Sıfırlamaları kullanmak için eşleşen Claude CLI oturumunu kullanın.
reset-invalid-grant = Claude geçersiz bir sıfırlama kredisi döndürdü.

# src/settings_window/appearance.rs
windows = Windows

# src/settings_window/appearance.rs
light = Açık

# src/settings_window/appearance.rs
dark = Koyu

# src/settings_window/appearance.rs
color-theme = Renk teması

# src/settings_window/appearance.rs
applies-to-settings-the-popup-and-its-tray-menu = Ayarlara, açılır panele ve tepsi menüsüne uygulanır.

# src/settings_window/appearance.rs
accent-color = Vurgu rengi

# src/settings_window/appearance.rs
windows-follows-your-system-accent = Windows seçeneği sisteminizin vurgu rengini izler.

# src/settings_window/appearance.rs

# src/settings_window/appearance.rs
font = Yazı tipi

# src/settings_window/appearance.rs
any-font-installed-on-this-pc = Bu bilgisayarda yüklü herhangi bir yazı tipi.
no-matching-fonts = Eşleşen yazı tipi yok

# src/settings_window/appearance.rs
windows-default = Windows varsayılanı

# src/settings_window/appearance.rs

# src/settings_window/appearance.rs

# src/settings_window/tray.rs
monochrome = Tek renk

# src/settings_window/appearance.rs
time-format = Saat biçimi

# src/settings_window/appearance.rs
msg-12-hour = 12 saat

# src/settings_window/appearance.rs
msg-24-hour = 24 saat

# src/settings_window/appearance.rs
popup-background = Açılır panel arka planı
popup-theme = Açılır panel teması
popup-theme-description = Yalnızca açılır paneli değiştirir. Ayarlar Windows görünümünü korur.
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel
popup-theme-built-in = Yerleşik
popup-theme-vscode = VS Code teması
vscode-themes = VS Code temaları
vscode-themes-description = Açılır panelde Visual Studio Code renk temalarını kullanın: Open VSX'e göz atın veya .vsix ya da .json tema dosyası içe aktarın.
import-theme = Tema içe aktar
importing-theme = İçe aktarılıyor…
remove-theme = Temayı kaldır
browse-open-vsx = Open VSX'e göz at
browse-open-vsx-description = open-vsx.org kayıt defterinden renk temaları bul ve yükle
search-color-themes = Renk teması ara
open-vsx-search = Ara
open-vsx-browse = Göz at
open-vsx-searching = Aranıyor…
no-themes-found = Renk teması bulunamadı
open-vsx-load-failed = Open VSX'ten temalar yüklenemedi
open-vsx-retry = Tekrar dene
open-vsx-preview-loading = Önizleme yükleniyor…
open-vsx-preview-hint = Açılır panelde önizlemek için bir temaya tıklayın. Yükle tüm paketi saklar; temanın indirme düğmesi yalnızca o temayı saklar.
open-vsx-preview-empty = Bu uzantı renk teması içermiyor
open-vsx-install = Yükle
open-vsx-install-one = Yalnızca bu temayı yükle
open-vsx-installed = Yüklendi
open-vsx-installing = Yükleniyor…
theme-installed = Tema yüklendi
theme-install-failed = Tema yüklenemedi
theme-remove-failed = Tema kaldırılamadı

# src/settings_window/appearance.rs
acrylic = Akrilik

# src/settings_window/appearance.rs
mica = Mika

# src/settings_window/appearance.rs
solid = Düz

# src/settings_window/appearance.rs
bottom-bar-size = Alt çubuk boyutu

# src/settings_window/appearance.rs
comfortable = Rahat

# src/settings_window/appearance.rs
compact = Kompakt

# src/settings_window/appearance.rs
popup-corner-radius = Açılır panel köşe yarıçapı
popup-borders = Kenarlıklar
popup-borders-description = Açılır panelin hangi bölümlerinin çerçeveleneceğini seçin
popup-card-borders = Kartlar ve denetimler
popup-window-border = Pencere kenarlığı
popup-theme-contrast = Kontrast
popup-theme-contrast-description = VS Code temalarının metin ve kenarlık belirginliğini ayarlayın
popup-theme-tint = Ton
popup-theme-tint-description = Tema arka planının Mika veya Akrilik camı ne kadar kapladığı

# src/settings_window/appearance.rs
animation-effects = Animasyon efektleri

# src/settings_window/appearance.rs
glide-transitions-in-the-popup-and-settings-windows-own-animation = Açılır panelde ve Ayarlar'da akıcı geçişler. Windows'un animasyon ayarına da uyulur.

# src/settings_window/appearance.rs
popup = Açılır panel

# src/settings_window/appearance.rs
motion = Hareket

# src/settings_window/appearance.rs
blue = Mavi

# src/settings_window/appearance.rs
purple = Mor

# src/settings_window/appearance.rs
pink = Pembe

# src/settings_window/appearance.rs
red = Kırmızı

# src/settings_window/appearance.rs
orange = Turuncu

# src/settings_window/appearance.rs
green = Yeşil

# src/settings_window/appearance.rs
teal = Turkuaz

# src/settings_window/customize.rs
show-edit-button = Alt çubukta Düzenle düğmesini göster
show-edit-button-description = Düzenleme modu, Ana Sayfadaki araçları sürükleyip sıralamak için tutamaçlar gösterir. Alt çubuğu kompakt tutmak için buradan başlatın.
start-editing = Düzenlemeye başla

# src/settings_window/customize.rs
use-two-columns = İki sütun kullan

# src/settings_window/customize.rs
several-accounts-of-one-provider = Tek sağlayıcının birden fazla hesabı

# src/settings_window/customize.rs
separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = Ayrı sekmeler her örneğe kendi sekmesini verir. Gruplu görünüm, hesap seçicisiyle veya tüm hesaplar alt alta olacak şekilde sağlayıcı başına tek sekme gösterir.

# src/settings_window/customize.rs
use-monochrome-icons = Tek renkli simgeler kullan

# src/settings_window/customize.rs
draw-provider-marks-in-the-popup-without-brand-colors = Açılır panelde sağlayıcı simgelerini marka renkleri olmadan çiz.

# src/settings_window/onboarding.rs
show-used-instead-of-remaining = Kalan yerine kullanılanı göster

# src/settings_window/customize.rs
show-usage-in-values-when-possible = Kullanımı sayısal değerlerle göster (mümkünse)

# src/settings_window/customize.rs
adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = Sağlayıcı bildirdiğinde yüzdelerin yanına kesin kullanılan/limit miktarlarını ekler.

# src/settings_window/onboarding.rs
show-usage-pace = Kullanım hızını göster

# src/settings_window/onboarding.rs
marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = Kotayı dengeli bir hızdan daha hızlı mı yoksa daha yavaş mı tükettiğinizi gösterir.

# src/settings_window/onboarding.rs
show-account-name = Hesap adını göster

# src/settings_window/customize.rs
show-on-home-tab = Ana Sayfa sekmesinde göster

# src/settings_window/customize.rs
layout = Düzen

# src/settings_window/customize.rs
donut = Halka

# src/settings_window/customize.rs
cards = Kartlar

# src/settings_window/customize.rs
tabs = Sekmeler

# src/settings_window/customize.rs
usage-widget = Kullanım Aracı

# src/settings_window/customize.rs
card = Kart

# src/settings_window/customize.rs
tab = Sekme

# src/settings_window/customize.rs
popup-cards = Açılır panel kartları

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = Bu hesabın Ana Sayfada ve kendi sekmesinde hangi kartları göstereceğini seçin.

# src/settings_window/customize.rs
choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = Bu hesabın kendi sekmesinde hangi kartları göstereceğini seçin. Ana Sayfa kartlarını seçmek için Ana Sayfada göster'i açın.

# src/settings_window/general.rs
msg-30-seconds = 30 saniye

# src/settings_window/general.rs
msg-1-minute = 1 dakika

# src/settings_window/general.rs
msg-2-minutes = 2 dakika

# src/settings_window/general.rs
msg-5-minutes = 5 dakika

# src/settings_window/general.rs
msg-10-minutes = 10 dakika

# src/settings_window/general.rs
msg-15-minutes = 15 dakika

# src/settings_window/general.rs
msg-30-minutes = 30 dakika

# src/settings_window/general.rs
msg-45-minutes = 45 dakika

# src/settings_window/general.rs
msg-60-minutes = 60 dakika

# src/settings_window/onboarding.rs
start-with-windows = Windows ile başlat

# src/settings_window/general.rs
open-codex-minibar-in-the-tray-when-you-sign-in = Oturum açtığınızda Codex Minibar'ı sistem tepsisinde aç.

# src/settings_window/onboarding.rs
refresh-interval = Yenileme aralığı

# src/settings_window/general.rs
how-often-this-instance-s-quotas-are-read = Bu örneğin kotalarının ne sıklıkla okunacağı. Daha uzun aralıklar sağlayıcı hız limitlerini önler.

# src/settings_window/general.rs
enable-usage-stats = Kullanım İstatistiklerini etkinleştir

# src/settings_window/general.rs
scan-local-provider-history-for-the-usage-tab-and-cost-totals = Kullanım sekmesi ve maliyet toplamları için yerel sağlayıcı geçmişini tara.

# src/settings_window/general.rs
collection-period = Toplama aralığı

# src/settings_window/general.rs
how-often-local-provider-history-is-scanned = Yerel sağlayıcı geçmişinin ne sıklıkla taranacağı.

# src/settings_window/general.rs
check-for-confirmed-tibo-resets = Doğrulanmış Tibo sıfırlamalarını denetle

# src/settings_window/general.rs
reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = Uygulamanın herkese açık GitHub akışını okur ve son duyuruyu önbellekte tutar.

# src/settings_window/general.rs
notify-when-new-reset-info-arrives = Yeni sıfırlama bilgisi geldiğinde bildir

# src/settings_window/general.rs
shows-a-notification-when-the-feed-reports-a-possible-reset-never = Akış olası bir sıfırlama bildirdiğinde bildirim gösterir; sıfırlama saatinde göstermez.

# src/settings_window/general.rs
check-every = Denetleme sıklığı

# src/settings_window/general.rs
the-feed-is-also-checked-immediately-when-the-app-starts-or-this = Akış, uygulama başladığında veya bu seçenek açıldığında da hemen denetlenir.

# src/settings_window/general.rs
msg-1-hour = 1 saat

# src/settings_window/general.rs
msg-3-hours = 3 saat

# src/settings_window/general.rs
msg-2-hours = 2 saat

# src/settings_window/general.rs
msg-5-hours = 5 saat

# src/settings_window/general.rs
msg-6-hours = 6 saat

# src/settings_window/general.rs
msg-12-hours = 12 saat

# src/settings_window/general.rs
msg-24-hours = 24 saat

# src/settings_window/general.rs
tibo-resets = Tibo Resets™

# src/settings_window/general.rs
enable-a-provider-in-the-providers-tab-to-include-it-here = Buraya dahil etmek için Sağlayıcılar sekmesinde bir sağlayıcıyı etkinleştirin.

# src/settings_window/general.rs
add-a-management-key = Yönetim anahtarı ekle

# src/settings_window/general.rs
usage-statistics-are-off-on-this-provider-s-page = Bu sağlayıcının sayfasında kullanım istatistikleri kapalı

# src/settings_window/general.rs
included-providers = Dahil edilen sağlayıcılar

# src/settings_window/general.rs
choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = Bu bilgisayarın Kullanım sekmesine ve Ana Sayfa toplamına hangi hesapların dahil edileceğini seçin. Hariç tutulan hesaplar veri toplamaya devam eder ve kendi sayfalarında kullanımı gösterir.

# src/settings_window/integrations.rs
download-the-latest-stream-deck-companion-from-github-and-open-it = En son Stream Deck eklentisini GitHub'dan indir ve yükleyicisini aç.

# src/settings_window/integrations.rs
downloading-the-latest-stream-deck-companion-from-github = En son Stream Deck eklentisi GitHub'dan indiriliyor…

# src/settings_window/integrations.rs
opening-the-stream-deck-installer = Stream Deck yükleyicisi açılıyor…

# src/settings_window/integrations.rs
the-installer-is-open-finish-the-installation-in-stream-deck = Yükleyici açık. Yüklemeyi Stream Deck'te tamamlayın.

# src/settings_window/integrations.rs
the-plugin-could-not-be-downloaded-or-opened-try-again = Eklenti indirilemedi veya açılamadı. Tekrar deneyin.

# src/settings_window/integrations.rs
downloading = İndiriliyor…

# src/settings_window/integrations.rs
opening = Açılıyor…

# src/settings_window/integrations.rs
install-plugin = Eklentiyi yükle

# src/settings_window/integrations.rs
stream-deck-companion = Stream Deck eklentisi

# src/settings_window/integrations.rs
check-your-connection-then-try-again = Bağlantınızı kontrol edip tekrar deneyin.

# src/settings_window/providers/page.rs
on = Açık

# src/settings_window/providers/page.rs
off = Kapalı

# src/settings_window/kit.rs
more-options = Diğer seçenekler

# src/settings_window/kit.rs
custom-color = Özel renk

# src/settings_window/troubleshoot.rs
run-troubleshoot-with-ai = Yapay zekâ ile sorun gidermeyi çalıştır

# src/settings_window/log.rs
let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = Yüklü bir yapay zekâ CLI aracının günlüğü okuyup sorunu araştırmasına izin ver.

# src/settings_window/log.rs
choose-tool = Araç seç

# src/settings_window/log.rs
application-log = Uygulama günlüğü

# src/settings_window/log.rs
log-txt-in-the-app-data-folder = Uygulama veri klasöründeki log.txt.

# src/settings_window/log.rs
open-log-txt = log.txt dosyasını aç

# src/settings_window/log.rs
could-not-open-log-txt = log.txt açılamadı

# src/settings_window/log.rs
open-logs-folder = Günlük klasörünü aç

# src/settings_window/log.rs
could-not-open-logs-folder = Günlük klasörü açılamadı

# src/settings_window/log.rs
no-log-events-yet = Henüz günlük olayı yok.

# src/settings_window/log.rs
live-tail = Canlı takip

# src/settings_window/log.rs
no-supported-ai-tool-found = Desteklenen yapay zekâ aracı bulunamadı

# src/settings_window/log.rs
install-codex-or-claude-code-and-make-it-available-to-minibar = Codex veya Claude Code yükleyip Minibar'ın erişebilmesini sağlayın.

# src/settings_window/window.rs
general = Genel

# src/settings_window/window.rs
appearance = Görünüm

# src/settings_window/window.rs
providers = Sağlayıcılar

# src/settings_window/window.rs
customize = Özelleştir

# src/settings_window/window.rs
limit-activation = Limit etkinleştirme

# src/settings_window/window.rs
tray = Sistem tepsisi

# src/settings_window/window.rs
notifications = Bildirimler

# src/settings_window/window.rs
advanced = Gelişmiş

# src/settings_window/window.rs
log = Günlük

# src/settings_window/window.rs
integrations = Entegrasyonlar

# src/settings_window/nav.rs
about-updates = Hakkında ve Güncellemeler

# src/settings_window/onboarding.rs
successful-activations = Başarılı etkinleştirmeler

# src/settings_window/onboarding.rs
failed-activations = Başarısız etkinleştirmeler

# src/settings_window/onboarding.rs
when-limits-reset = Limitler sıfırlandığında

# src/settings_window/onboarding.rs
when-5-hour-remaining-hits = Kalan 5 saatlik kota %{ $v0 } olduğunda

# src/settings_window/onboarding.rs
when-weekly-remaining-hits = Kalan haftalık kota %{ $v0 } olduğunda

# src/settings_window/onboarding.rs
low-usage = Düşük kullanım

# src/settings_window/notifications.rs
shows-a-notification-once-per-window-when-the-remaining-share-dro = Kalan pay eşiğe düştüğünde her dönem için bir kez bildirim gösterir.

# src/settings_window/notifications.rs
threshold = Eşik

# src/settings_window/onboarding.rs
found-in-opencode-auth-or-local-history = OpenCode kimlik doğrulamasında veya yerel geçmişte bulundu.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-set-up-elsewhere = Bulunamadı. Başka bir yerde kuruluysa etkinleştirin.

# src/settings_window/onboarding.rs
account-credentials-are-already-set = Hesap kimlik bilgileri zaten ayarlanmış.

# src/settings_window/onboarding.rs
optional-add-accounts-later-in-providers = İsteğe bağlı. Hesapları daha sonra Sağlayıcılar bölümünden ekleyin.

# src/settings_window/onboarding.rs
found-an-official-agy-sign-in-on-this-pc = Bu bilgisayarda resmî agy oturumu bulundu.

# src/settings_window/onboarding.rs
not-found-sign-in-with-agy-before-enabling-it = Bulunamadı. Etkinleştirmeden önce agy ile oturum açın.

# src/settings_window/onboarding.rs
found-an-official-grok-cli-sign-in-on-this-pc = Bu bilgisayarda resmî Grok CLI oturumu bulundu.

# src/settings_window/onboarding.rs
not-found-run-grok-login-before-enabling-it = Bulunamadı. Etkinleştirmeden önce grok login çalıştırın.

# src/settings_window/onboarding.rs
found-an-official-command-code-cli-sign-in-on-this-pc = Bu bilgisayarda resmî Command Code CLI oturumu bulundu.

# src/settings_window/onboarding.rs
not-found-run-cmdc-login-before-enabling-it = Bulunamadı. Etkinleştirmeden önce cmdc login çalıştırın.

# src/settings_window/onboarding.rs
found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Kiro IDE, Kiro Crew veya oturum açılmış Kiro CLI bulundu.

# src/settings_window/onboarding.rs
not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = Bulunamadı. Kiro IDE veya Kiro Crew yükleyin ya da Kiro CLI'da oturum açın.

# src/settings_window/onboarding.rs
found-on-this-pc = Bu bilgisayarda bulundu.

# src/settings_window/onboarding.rs
not-found-turn-it-on-if-it-s-installed-somewhere-else = Bulunamadı. Başka bir yerde yüklüyse etkinleştirin.

# src/settings_window/onboarding.rs
setup-could-not-be-saved = Kurulum kaydedilemedi

# src/settings_window/onboarding.rs
detected = Algılandı

# src/settings_window/onboarding.rs
starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = İlk isteğinizi beklemek yerine, uygun bir zaman aralığı oluşur oluşmaz yeni Codex veya Claude oturumu başlatır.

# src/settings_window/onboarding.rs
collect-usage-data = Kullanım verilerini topla

# src/settings_window/onboarding.rs
scans-local-provider-history-for-usage-stats = Kullanım İstatistikleri için yerel sağlayıcı geçmişini tarar.

# src/settings_window/onboarding.rs
startup = Başlangıç

# src/settings_window/providers/page.rs
features = Özellikler

# src/settings_window/onboarding.rs
customization = Özelleştirme

# src/settings_window/onboarding.rs
when-a-new-version-is-found = Yeni sürüm bulunduğunda

# src/settings_window/onboarding.rs
activity = Etkinlik

# src/settings_window/onboarding.rs
choose-providers = Sağlayıcıları seçin
choose-a-theme = Tema seçin
theme-step-description = Ayarlar ve açılır panel için renkleri seçin. Açılır panel her seçimi anında gösterir.

# src/settings_window/onboarding.rs
we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = Bu bilgisayarda bulunan sağlayıcıları etkinleştirdik. Bunu daha sonra değiştirebilirsiniz.

# src/settings_window/onboarding.rs
general-settings = Genel ayarlar

# src/settings_window/onboarding.rs
you-can-change-these-later-in-settings = Bunları daha sonra Ayarlar'dan değiştirebilirsiniz.

# src/settings_window/onboarding.rs
turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = Bildirim almak istemediğiniz seçenekleri kapatın. Bunları daha sonra Ayarlar'dan değiştirebilirsiniz.

# src/settings_window/window.rs
back = Geri

# src/settings_window/onboarding.rs
continue = Devam

# src/settings_window/tray.rs
done = Bitti

# src/settings_window/providers/dialog.rs
details = Ayrıntılar

# src/settings_window/providers/dialog.rs
added = Eklendi

# src/settings_window/providers/dialog.rs
reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Claude Code oturumu açmadan Claude aboneliğinin oturum ve haftalık limitlerini okur.

# src/settings_window/providers/dialog.rs
msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. Claude'u ayrı bir tarayıcı profilinde veya gizli pencerede açın. İzlemek istediğiniz hesapta oturum açıp Claude ayarlarından e-posta adresini doğrulayın.

# src/settings_window/providers/dialog.rs
open-claude-ai = claude.ai'ı aç

# src/settings_window/providers/dialog.rs
msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. Chrome veya Edge'de F12'ye basın. Application > Storage > Cookies bölümünü açıp https://claude.ai adresini seçin.

# src/settings_window/providers/dialog.rs
msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. sessionKey öğesini bulun. Adını veya tüm çerez tablosunu değil, Value değerini kopyalayıp yukarıdaki alana yapıştırın.

# src/settings_window/providers/dialog.rs
how-to-view-cookies-in-chrome = Chrome'da çerezler nasıl görüntülenir

# src/settings_window/providers/dialog.rs
a-cookie-header-containing-sessionkey-also-works-when-the-session = sessionKey içeren bir Cookie başlığı da kullanılabilir. Oturumun süresi dolduğunda buraya yenisini yapıştırın.

# src/settings_window/providers/dialog.rs
use-the-access-token-from-a-claude-code-subscription-login-miniba = Claude Code abonelik oturumunun erişim tokenını kullanın. Minibar yapıştırılan tokenı yenileyemez; mümkünse Kaynak: Yapılandırma klasörü seçeneğini tercih edin.

# src/settings_window/providers/dialog.rs
copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = İlgili oturumun .credentials.json dosyasından yalnızca claudeAiOauth.accessToken değerini (sk-ant-oat ile başlar) tırnak işareti olmadan kopyalayın. claude setup-token kullanmayın: kullanım erişimi olmayabilir.

# src/settings_window/providers/dialog.rs
claude-code-log-in-with-multiple-accounts = Claude Code: birden fazla hesapla oturum açma

# src/settings_window/providers/dialog.rs
requires-a-claude-subscription-login-with-usage-access-api-keys-a = Kullanım erişimi olan bir Claude abonelik oturumu gerektirir. API anahtarları ve Admin API anahtarları abonelik limitlerini göstermez.

# src/settings_window/providers/dialog.rs
this-provider = bu sağlayıcı

# src/settings_window/providers/dialog.rs
track-another-account-or-a-provider-minibar-has-not-shown-yet = Başka bir hesabı veya Minibar'ın henüz göstermediği bir sağlayıcıyı izleyin.

# src/settings_window/providers/dialog.rs
e-g-work = ör. İş

# src/settings_window/providers/page.rs
name = Ad

# src/settings_window/providers/dialog.rs
shown-on-its-tab-home-card-tray-and-notifications = Sekmesinde, Ana Sayfa kartında, sistem tepsisinde ve bildirimlerde gösterilir.

# src/settings_window/providers/page.rs
badge = Rozet

# src/settings_window/providers/dialog.rs
auto = Otomatik

# src/settings_window/providers/page.rs
badge-color = Rozet rengi

# src/settings_window/providers/dialog.rs
up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = En fazla üç harf; boşsa adın baş harfleri kullanılır. Rozetler bir sağlayıcının birden fazla örneği etkin olduğunda gösterilir.

# src/settings_window/window.rs
add-provider = Sağlayıcı ekle

# src/settings_window/providers/dialog.rs
next = İleri

# src/settings_window/providers/dialog.rs
minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = Minibar { $name } verilerini okumayı durdurur; kayıtlı anahtarlarını, zamanlamalarını, tepsi göstergelerini ve Ana Sayfa konumunu unutur.{ $v0 }

# src/settings_window/providers/dialog.rs
its-config-folder-stays-on-disk = { " " }Yapılandırma klasörü diskte kalır.

# src/settings_window/providers/dialog.rs
delete-name = { $name } silinsin mi?

# src/settings_window/providers/dialog.rs
delete = Sil

# src/settings_window/providers/dialog.rs
finish-signing-in-in-your-browser-cancel-stops-this-login = Oturum açmayı tarayıcınızda tamamlayın. İptal bu oturum açmayı durdurur.

# src/settings_window/providers/dialog.rs
runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = { $name } için Claude Code'un kendi oturum açma işlemini yapılandırma klasörünü CLAUDE_CONFIG_DIR olarak kullanarak çalıştırır. Oturum bu klasörde kalır ve Claude Code tarafından güncel tutulur. Yerel Windows Claude Code gerektirir.

# src/settings_window/providers/dialog.rs
runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = { $name } için Codex'in kendi oturum açma işlemini yapılandırma klasörünü CODEX_HOME olarak kullanarak çalıştırır. Oturum bu klasörde kalır ve Codex tarafından güncel tutulur. Yerel Codex CLI veya masaüstü uygulaması gerektirir.

# src/settings_window/providers/dialog.rs
sign-in-to-name = { $name } hesabında oturum aç

# src/settings_window/providers/page.rs
sign-in = Oturum aç

# src/settings_window/providers/dialog.rs
for = { $v0 } için.

# src/settings_window/providers/dialog.rs
browser-session = Tarayıcı oturumu

# src/settings_window/providers/dialog.rs
oauth-token = OAuth tokenı

# src/settings_window/providers/dialog.rs
session-key = Oturum anahtarı

# src/settings_window/providers/dialog.rs
paste-the-sessionkey-value = sessionKey değerini yapıştırın

# src/settings_window/providers/dialog.rs
oauth-access-token = OAuth erişim tokenı

# src/settings_window/providers/dialog.rs
the-saved-credential-is-replaced-only-after-the-new-one-passes-th = Kayıtlı kimlik bilgisi yalnızca yenisi denetimi geçtikten sonra değiştirilir.

# src/settings_window/providers/dialog.rs
claude-credential = Claude kimlik bilgisi

# src/settings_window/providers/dialog.rs
check-and-save = Denetle ve kaydet

# src/settings_window/providers/dialog.rs
key-name-optional = Anahtar adı (isteğe bağlı)

# src/settings_window/providers/dialog.rs
e-g-personal = ör. Kişisel

# src/settings_window/providers/dialog.rs
leave-blank-to-use-the-name-from-openrouter = OpenRouter'daki adı kullanmak için boş bırakın.

# src/settings_window/providers/dialog.rs
minibar-checks-the-key-with-openrouter-before-saving-it = Minibar kaydetmeden önce anahtarı OpenRouter ile denetler.

# src/settings_window/providers/dialog.rs
replace-api-key = API anahtarını değiştir

# src/settings_window/providers/page.rs
add-api-key = API anahtarı ekle

# src/settings_window/providers/page.rs
management-key = Yönetim anahtarı

# src/settings_window/providers/dialog.rs
create-one-under-settings-management-keys-on-openrouter-ai = openrouter.ai sitesinde Settings → Management keys bölümünden oluşturun.

# src/settings_window/providers/dialog.rs
replace-management-key = Yönetim anahtarını değiştir

# src/settings_window/providers/page.rs
add-management-key = Yönetim anahtarı ekle

# src/settings_window/providers/dialog.rs
key-name = Anahtar adı

# src/settings_window/providers/page.rs
rename-key = Anahtarı yeniden adlandır

# src/settings_window/providers/dialog.rs
save = Kaydet

# src/settings_window/providers/dialog.rs
this-key = bu anahtar

# src/settings_window/providers/dialog.rs
minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = Minibar { $hint } izlemeyi durdurur. Anahtar OpenRouter'da çalışmaya devam eder.

# src/settings_window/providers/dialog.rs
remove-api-key = API anahtarı kaldırılsın mı?

# src/settings_window/tray.rs
remove = Kaldır

# src/settings_window/providers/dialog.rs
minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = Minibar { $v0 } için kredi bakiyesini ve kullanım geçmişini göstermeyi durdurur. Anahtar OpenRouter'da çalışmaya devam eder.

# src/settings_window/providers/dialog.rs
remove-management-key = Yönetim anahtarı kaldırılsın mı?

# src/settings_window/providers/dialog.rs
saved-in-windows-user-storage-never-in-the-settings-file = Windows kullanıcı depolamasına kaydedilir; ayar dosyasına asla yazılmaz.

# src/settings_window/providers/dialog.rs
save-key = Anahtarı kaydet

# src/settings_window/providers/dialog.rs
minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = Minibar { $v0 } için kayıtlı anahtarı unutur. Anahtar OpenCode ile çalışmaya devam eder.

# src/settings_window/providers/dialog.rs
signing-in = Oturum açılıyor…

# src/settings_window/providers/page.rs
checking = Denetleniyor…

# src/settings_window/providers/dialog.rs
choose-a-provider = Bir sağlayıcı seçin.

# src/settings_window/providers/dialog.rs
reason-it-is-already-in-the-list = { $reason }. Zaten listede.

# src/settings_window/providers/dialog.rs
added-name = { $name } eklendi.

# src/settings_window/providers/dialog.rs
could-not-add-the-provider-error = Sağlayıcı eklenemedi: { $error }

# src/settings_window/providers.rs
this-provider-no-longer-exists = Bu sağlayıcı artık mevcut değil.

# src/settings_window/providers/dialog.rs
could-not-delete-the-provider-error = Sağlayıcı silinemedi: { $error }

# src/settings_window/providers/dialog.rs
paste-a-credential-first = Önce bir kimlik bilgisi yapıştırın.

# src/settings_window/providers/dialog.rs
credential-saved-in-windows-user-storage = Kimlik bilgisi Windows kullanıcı depolamasına kaydedildi.

# src/settings_window/providers/dialog.rs
paste-a-key-first = Önce bir anahtar yapıştırın.

# src/settings_window/providers/dialog.rs
this-account-no-longer-exists = Bu hesap artık mevcut değil.

# src/settings_window/providers/dialog.rs
api-key-saved-in-windows-user-storage = API anahtarı Windows kullanıcı depolamasına kaydedildi.

# src/settings_window/providers/dialog.rs
management-key-replaced = Yönetim anahtarı değiştirildi.

# src/settings_window/providers/dialog.rs
management-key-added = Yönetim anahtarı eklendi.

# src/settings_window/providers/dialog.rs
could-not-rename-the-key-error = Anahtar yeniden adlandırılamadı: { $error }

# src/settings_window/providers/dialog.rs
api-key-renamed = API anahtarı yeniden adlandırıldı.

# src/settings_window/providers/dialog.rs
could-not-remove-the-key-error = Anahtar kaldırılamadı: { $error }

# src/settings_window/providers/dialog.rs
api-key-removed = API anahtarı kaldırıldı.

# src/settings_window/providers/dialog.rs
management-key-removed = Yönetim anahtarı kaldırıldı.

# src/settings_window/providers/dialog.rs
could-not-save-the-key-error = Anahtar kaydedilemedi: { $error }

# src/settings_window/providers/dialog.rs
api-key-saved = API anahtarı kaydedildi.

# src/settings_window/providers/dialog.rs
switch-source-to-config-folder-to-sign-in = Oturum açmak için Kaynak seçeneğini Yapılandırma klasörü olarak değiştirin.

# src/settings_window/providers/dialog.rs
this-provider-has-no-config-folder = Bu sağlayıcının yapılandırma klasörü yok.

# src/settings_window/providers/dialog.rs
another-instance-already-reads-this-config-folder-choose-a-differ = Bu yapılandırma klasörünü başka bir örnek zaten okuyor. Önce farklı bir klasör seçin.

# src/settings_window/providers/dialog.rs
this-provider-has-no-sign-in = Bu sağlayıcının oturum açma özelliği yok.

# src/settings_window/providers/dialog.rs
signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 } oturumu açıldı. CLI, yapılandırma klasöründeki oturumu güncel tutar.

# src/settings_window/providers/page.rs
no-keys-yet = Henüz anahtar yok

# src/settings_window/providers/page.rs
management-key-5c8cf2 = { " " }· yönetim anahtarı

# src/settings_window/providers/page.rs
using-a-saved-credential = Kayıtlı kimlik bilgisi kullanılıyor

# src/settings_window/providers/page.rs
paste-a-credential-under-account = Hesap bölümüne bir kimlik bilgisi yapıştırın

# src/settings_window/providers/page.rs
needs-an-api-key-or-opencode-sign-in = API anahtarı veya OpenCode oturumu gerekli

# src/settings_window/providers/page.rs
needs-an-api-key = API anahtarı gerekli

# src/settings_window/providers/page.rs
not-found-set-its-folder-under-runtime = Bulunamadı. Çalışma Zamanı bölümünden klasörünü ayarlayın.

# src/settings_window/providers/page.rs
using-a-saved-api-key = Kayıtlı API anahtarı kullanılıyor

# src/settings_window/providers/page.rs
using-opencode-sign-in-or-local-history = OpenCode oturumu veya yerel geçmiş kullanılıyor

# src/settings_window/providers/page.rs
reading-cli = { $cli } okunuyor

# src/settings_window/providers/page.rs
reading-crew = { $crew } okunuyor

# src/settings_window/providers/page.rs
reading-app = { $app } okunuyor

# src/settings_window/providers/page.rs
checking-kiro-ide-kiro-crew-and-cli = Kiro IDE, Kiro Crew ve CLI denetleniyor…

# src/settings_window/providers/page.rs
checking-installed-app-and-cli = Yüklü uygulama ve CLI denetleniyor…

# src/settings_window/providers/page.rs
checking-cli = CLI denetleniyor…

# src/settings_window/providers/page.rs
checking-installed-app = Yüklü uygulama denetleniyor…

# src/settings_window/providers/page.rs
no-providers-yet-add-one-to-start-reading-limits = Henüz sağlayıcı yok. Limitleri okumaya başlamak için bir tane ekleyin.

# src/settings_window/providers/page.rs
is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } kapalı olduğundan minibar veya sistem tepsisinde görünmüyor. Kullanımı okumaya başlamak için etkinleştirin.

# src/settings_window/providers/page.rs
replace-chatgpt-logo-with-codex = ChatGPT logosunu Codex ile değiştir

# src/settings_window/providers/page.rs
limits-only = Yalnızca limitler

# src/settings_window/providers/page.rs
delete-provider = Sağlayıcıyı sil

# src/settings_window/providers/page.rs
display-name = Görünen ad

# src/settings_window/providers/page.rs
shown-on-popup-tabs-home-cards-the-tray-and-notifications = Açılır panel sekmelerinde, Ana Sayfa kartlarında, sistem tepsisinde ve bildirimlerde gösterilir.

# src/settings_window/providers/page.rs
up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = En fazla üç harf. Adın baş harflerini kullanmak için boş bırakın. Bir sağlayıcının birden fazla örneği etkin olduğunda gösterilir.

# src/settings_window/providers/page.rs
auto-uses-a-neutral-plate-that-follows-the-theme = Otomatik, temayı izleyen nötr bir zemin kullanır.

# src/settings_window/providers/page.rs
show-on-home = Ana Sayfada göster

# src/settings_window/providers/page.rs
its-provider-tab-stays-available-when-hidden-from-home = Ana Sayfadan gizlendiğinde sağlayıcı sekmesi kullanılabilir kalır.

# src/settings_window/providers/page.rs
credential = Kimlik bilgisi

# src/settings_window/providers/page.rs
signed-in-as-identity = { $identity } olarak oturum açıldı

# src/settings_window/providers/page.rs
saved-in-windows-user-storage = Windows kullanıcı depolamasına kaydedildi

# src/settings_window/providers/page.rs
paste-a-sessionkey-or-an-oauth-access-token = Bir sessionKey veya OAuth erişim tokenı yapıştırın.

# src/settings_window/providers/page.rs
reads-limits-only-minibar-cannot-refresh-a-pasted-credential = Yalnızca limitleri okur. Minibar yapıştırılan kimlik bilgisini yenileyemez.

# src/settings_window/providers/page.rs
replace-credential = Kimlik bilgisini değiştir

# src/settings_window/providers/page.rs
add-credential = Kimlik bilgisi ekle

# src/settings_window/providers/page.rs
signed-in-account = Oturum açılan hesap

# src/settings_window/providers/page.rs
not-signed-in-yet-or-no-limits-read-so-far = Henüz oturum açılmadı veya henüz limit okunmadı.

# src/settings_window/providers/page.rs
account = Hesap

# src/settings_window/providers/page.rs
runtime = Çalışma Zamanı

# src/settings_window/providers/page.rs
minibar-finds-these-automatically = { $v0 } Minibar bunları otomatik bulur.

# src/settings_window/providers/page.rs
in-use = Kullanımda

# src/settings_window/providers/page.rs
found = Bulundu

# src/settings_window/providers/page.rs
copy-path = Yolu kopyala

# src/settings_window/providers/page.rs
open-folder = Klasörü aç

# src/settings_window/providers/page.rs
path-copied = Yol kopyalandı.

# src/settings_window/providers/page.rs
not-installed-or-installed-somewhere-minibar-doesn-t-look = Yüklü değil veya Minibar'ın aramadığı bir konumda yüklü.

# src/settings_window/providers/page.rs
not-found = Bulunamadı

# src/settings_window/providers/page.rs
choose-folder = Klasör seç…

# src/settings_window/providers/page.rs
select-folder = Klasör seçin

# src/settings_window/providers/page.rs
choose-folder-1838a4 = Klasör seç

# src/settings_window/providers/page.rs
only-needed-if-automatic-detection-misses-your-install = Yalnızca otomatik algılama kurulumunuzu bulamazsa gereklidir.

# src/settings_window/providers/page.rs
source = Kaynak

# src/settings_window/providers/page.rs
config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = Yapılandırma klasörü, CLAUDE_CONFIG_DIR içindeki Claude Code oturumunu okur. Manuel kimlik bilgisi yalnızca yapıştırılan kimlik bilgisinden limitleri okur.

# src/settings_window/providers/page.rs
config-folder = Yapılandırma klasörü

# src/settings_window/providers/page.rs
manual = Manuel

# src/settings_window/providers/page.rs
passed-to-the-cli-as-env-leave-empty-to-use = CLI'a { $env } olarak aktarılır. { $v0 } kullanmak için boş bırakın.

# src/settings_window/providers/page.rs
the-standard-folder = standart klasör

# src/settings_window/providers/page.rs
a-folder-minibar-creates-for-this-instance = Minibar'ın bu örnek için oluşturduğu klasör

# src/settings_window/providers/page.rs
other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } bu klasörü zaten okuyor. İki örnek aynı oturumu paylaşmamalıdır; aksi halde kullanım iki kez sayılır.

# src/settings_window/providers/page.rs
automatic-activation = Otomatik etkinleştirme

# src/settings_window/providers/page.rs
starts-this-account-s-5-hour-window-when-it-resets-using-its-own = Bu hesabın 5 saatlik dönemini sıfırlandığında kendi oturumunu kullanarak başlatır. Zamanlamalar ve duraklatmalar Limit etkinleştirme bölümündedir.

# src/settings_window/providers/page.rs
usage-statistics = Kullanım istatistikleri

# src/settings_window/providers/page.rs
scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = Kullanım kartı için bu örneğin yerel geçmişini tarar. Toplamayı tamamen durdurmak için kapatın.

# src/settings_window/providers/page.rs
opencode-sign-in-or-local-history = OpenCode oturumu veya yerel geçmiş

# src/settings_window/providers/page.rs
found-in-opencode-auth-environment-a-saved-key-or-local-history = OpenCode kimlik doğrulamasında, ortamda, kayıtlı anahtarda veya yerel geçmişte bulundu.

# src/settings_window/providers/page.rs
nothing-found-in-opencode-auth-environment-or-local-history = OpenCode kimlik doğrulamasında, ortamda veya yerel geçmişte bir şey bulunamadı.

# src/settings_window/providers/page.rs
replace-key = Anahtarı değiştir

# src/settings_window/providers/page.rs
optional-only-needed-without-opencode-sign-in-on-this-pc = İsteğe bağlı. Yalnızca bu bilgisayarda OpenCode oturumu yoksa gereklidir.

# src/settings_window/providers/page.rs
add-the-key-of-the-account-this-instance-tracks = Bu örneğin izlediği hesabın anahtarını ekleyin.

# src/settings_window/providers/page.rs
keys = Anahtarlar

# src/settings_window/providers/page.rs
a-management-key-shows-credit-balance-and-usage-history-api-keys = Yönetim anahtarı kredi bakiyesini ve kullanım geçmişini gösterir. API anahtarları anahtar başına harcamayı gösterir.

# src/settings_window/providers/page.rs
could-not-read-the-saved-key-reopen-this-page-to-retry = Kayıtlı anahtar okunamadı. Tekrar denemek için bu sayfayı yeniden açın.

# src/settings_window/providers/page.rs
credit-balance-and-account-wide-usage-history = Kredi bakiyesi ve hesap genelindeki kullanım geçmişi

# src/settings_window/providers/page.rs
not-added-add-one-to-see-credit-balance-and-usage-history = Eklenmedi. Kredi bakiyesini ve kullanım geçmişini görmek için ekleyin.

# src/settings_window/providers/page.rs
last-updated = Son güncelleme: { $v0 }, { $v1 }

# src/settings_window/providers/page.rs
no-api-keys-yet-add-one-to-track-spend-per-key = Henüz API anahtarı yok. Anahtar başına harcamayı izlemek için bir tane ekleyin.

# src/settings_window/providers/page.rs
key = Anahtar

# src/settings_window/providers/page.rs
spend = Harcama

# src/settings_window/providers/page.rs
limit = Limit

# src/settings_window/providers/page.rs
error-reopen-this-page-to-retry = { $error }. Tekrar denemek için bu sayfayı yeniden açın.

# src/settings_window/providers/page.rs
could-not-read-key = Anahtar okunamadı

# src/settings_window/providers/page.rs
not-saved = Kaydedilmedi

# src/settings_window/providers/page.rs
unnamed = Adsız

# src/settings_window/providers/page.rs
of-the-limit = { $v1 } limitinin { $v0 } kadarı

# src/settings_window/providers/page.rs
no-spend-limit-key-spend-account-credits-purchased = Harcama limiti yok. Anahtar harcaması: { $v0 }. Satın alınan hesap kredileri: { $v1 }.

# src/settings_window/providers/page.rs
none = Yok

# src/settings_window/providers/page.rs
add-key = Anahtar ekle

# src/settings_window/providers.rs
openrouter-account-credentials-are-configured = OpenRouter hesap kimlik bilgileri yapılandırıldı

# src/settings_window/providers.rs
saved-api-key = Kayıtlı API anahtarı

# src/settings_window/providers.rs
saved-credential = Kayıtlı kimlik bilgisi

# src/settings_window/providers.rs
opencode-auth-json-or-local-database = OpenCode auth.json veya yerel veritabanı

# src/settings_window/providers.rs
that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = Bu bir OpenRouter anahtarına benzemiyor. Anahtarlar sk-or- ile başlar.

# src/settings_window/providers.rs
codex-cli-folder = Codex CLI klasörü

# src/settings_window/providers.rs
claude-code-cli-folder = Claude Code CLI klasörü

# src/settings_window/providers.rs
cursor-app-folder = Cursor uygulama klasörü

# src/settings_window/providers.rs
grok-cli-folder = Grok CLI klasörü

# src/settings_window/providers.rs
command-code-cli-folder = Command Code CLI klasörü

# src/settings_window/providers.rs
kiro-ide-folder = Kiro IDE klasörü

# src/settings_window/providers.rs
kiro-crew-app-path = Kiro Crew uygulama yolu

# src/settings_window/providers.rs
kiro-cli-folder = Kiro CLI klasörü

# src/settings_window/providers.rs
reads-the-signed-in-codex-cli-or-desktop-app = Oturum açılmış Codex CLI veya masaüstü uygulamasını okur.

# src/settings_window/providers.rs
reads-your-existing-claude-code-login = Mevcut Claude Code oturumunuzu okur.

# src/settings_window/providers.rs
reads-the-signed-in-cursor-app-for-this-billing-cycle = Bu faturalama dönemi için oturum açılmış Cursor uygulamasını okur.

# src/settings_window/providers.rs
reads-zen-auth-and-local-opencode-history = Zen kimlik doğrulamasını ve yerel OpenCode geçmişini okur.

# src/settings_window/providers.rs
reads-go-quota-windows-and-local-opencode-history = Go kota dönemlerini ve yerel OpenCode geçmişini okur.

# src/settings_window/providers.rs
reads-api-key-usage-and-spend-limits-a-management-key-also-enable = API anahtarı kullanımını ve harcama limitlerini okur. Yönetim anahtarı ayrıca kullanım geçmişini ve kredi bakiyesini etkinleştirir.

# src/settings_window/providers.rs
reads-subscription-quota-from-your-existing-official-agy-windows = Mevcut resmî agy Windows oturumunuzdan abonelik kotasını okur.

# src/settings_window/providers.rs
reads-supergrok-subscription-credits-from-your-existing-official = Mevcut resmî Grok CLI oturumunuzdan SuperGrok abonelik kredilerini okur.

# src/settings_window/providers.rs
reads-command-code-5h-weekly-and-monthly-credits-from-your-existin = Mevcut resmî CLI oturumunuzdan Command Code 5 saatlik, haftalık ve aylık kredilerini okur.

# src/settings_window/providers.rs
fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = Kiro'nun güncel aylık kredilerini paylaşılan oturumuyla alır; IDE, Crew ve CLI kurulumlarını tanır.

# src/settings_window/providers.rs
codex-desktop-app = Codex masaüstü uygulaması

# src/settings_window/providers.rs
codex-cli = Codex CLI

# src/settings_window/providers.rs
claude-desktop-app = Claude masaüstü uygulaması

# src/settings_window/providers.rs
claude-code-cli = Claude Code CLI

# src/settings_window/providers.rs
cursor-app = Cursor uygulaması

# src/settings_window/providers.rs
antigravity-app = Antigravity uygulaması

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
supports-one-instance = { $v0 } tek örneği destekler

# src/settings_window/tray.rs
numbers = Sayılar

# src/settings_window/tray.rs
progress-bars = İlerleme çubukları

# src/settings_window/tray.rs
rings = Halkalar

# src/settings_window/tray.rs
reset-time = Sıfırlama zamanı

# src/settings_window/tray.rs
countdown = Geri sayım

# src/settings_window/tray.rs
status = Durum

# src/settings_window/tray.rs
fixed = Sabit

# src/settings_window/tray.rs
app-accent = Uygulama vurgusu

# src/settings_window/tray.rs
percentages-as-digits = Rakamlarla yüzdeler

# src/settings_window/tray.rs
one-bar-per-indicator = Gösterge başına bir çubuk

# src/settings_window/tray.rs
nested-rings-one-per-indicator = Gösterge başına bir iç içe halka

# src/settings_window/tray.rs
when-the-limit-resets = Limitin sıfırlanacağı zaman

# src/settings_window/tray.rs
time-left-until-the-reset = Sıfırlanmaya kalan süre

# src/settings_window/tray.rs
used = Kullanılan

# src/settings_window/tray.rs
remaining = Kalan

# src/settings_window/tray.rs
app-icon = Uygulama simgesi

# src/settings_window/tray.rs
widget = Araç { $v0 }

# src/settings_window/tray.rs
the-codex-minibar-icon = Codex Minibar simgesi

# src/settings_window/tray.rs
style-no-indicators = { $style } · Gösterge yok

# src/settings_window/tray.rs
widget-removed = Araç kaldırıldı

# src/settings_window/tray.rs
undo = Geri al

# src/settings_window/tray.rs
tray-icon = Sistem tepsisi simgesi

# src/settings_window/tray.rs
shows-the-app-icon = Uygulama simgesini gösterir.

# src/settings_window/tray.rs
add-widget = Araç ekle

# src/settings_window/tray.rs
add-app-icon = Uygulama simgesi ekle

# src/settings_window/tray.rs
move-up = Yukarı taşı

# src/settings_window/tray.rs
move-down = Aşağı taşı

# src/settings_window/tray.rs
edit = Düzenle

# src/settings_window/tray.rs
duplicate = Çoğalt

# src/settings_window/tray.rs
shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = Bildirim alanında Codex Minibar simgesini gösterir. Ayarlanacak göstergesi yoktur.

# src/settings_window/tray.rs
style = Stil

# src/settings_window/tray.rs
up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = Bu sırayla çizilen en fazla { $max_indicators } kota. Değiştirmek için birini genişletin.

# src/settings_window/tray.rs
indicators = Göstergeler

# src/settings_window/tray.rs
remove-widget = Aracı kaldır

# src/settings_window/tray.rs
add-indicator = Gösterge ekle

# src/settings_window/tray.rs
unsupported = Desteklenmiyor { $v0 }

# src/settings_window/tray.rs
remove-indicator = Göstergeyi kaldır

# src/settings_window/tray.rs
unsupported-bba2a8 = Desteklenmiyor ({ $v0 })

# src/settings_window/tray.rs
unavailable-0c5d75 = Kullanılamıyor ({ $v0 })

# src/settings_window/tray.rs
metric = Ölçüt

# src/settings_window/tray.rs
show = Göster

# src/settings_window/tray.rs
color = Renk

# src/settings_window/tray.rs
fixed-color = Sabit renk

# src/settings_window/troubleshoot.rs
troubleshooting-could-not-start = Sorun giderme başlatılamadı

# src/settings_window/troubleshoot.rs
choose-which-installed-ai-tool-should-investigate-the-problem = Sorunu hangi yüklü yapay zekâ aracının araştıracağını seçin.

# src/settings_window/troubleshoot.rs
open-terminal = Terminali aç

# src/settings_window/window.rs
version-is-available = { $version } mevcut

# src/settings_window/window.rs
update-now = Şimdi güncelle

# src/settings_window/window.rs
missing-a-provider = Bir sağlayıcı eksik mi?

# src/settings_window/window.rs
ask-for-it-or-build-it-yourself = Talep edin veya kendiniz geliştirin.

# src/settings_window/window.rs
request-a-provider = Sağlayıcı talep et

# src/settings_window/window.rs
contribute-one = Sağlayıcı katkısında bulun

# src/tray.rs
disabled = Devre dışı

# src/updater.rs
update-complete = Güncelleme tamamlandı

# src/updater.rs
now-running-version = Şu an { $version } çalışıyor.

# src/usage_overview.rs
past-24h = Son 24 saat

# application
language = Dil

# application
applies-immediately-to-every-app-window-and-notification-auto-fol = Tüm uygulama pencerelerine ve bildirimlere hemen uygulanır. Otomatik seçeneği Windows görüntüleme dilini izler.

# application
auto-windows = Otomatik (Windows)

# application
english = English

# application
msg-russian = Русский

# application
codex-minibar-settings = Codex Minibar Ayarları

# application
welcome-to-codex-minibar = Codex Minibar'a Hoş Geldiniz

# application
exit = Çıkış

# application
update-available-67fd3a = Güncelleme Mevcut

# application
monthly-credits-976559 = Aylık Krediler

# application
msg-5h-session-de7ce8 = 5 Sa. Oturum

# application
name-weekly = { $name } haftalık

# application
tokens-94e0b9 = { $v0 }: { $v1 } token

# application
requests-priced = { $v0 } istek · { $v1 } fiyatlandırıldı

# application
requests = { $v0 } istek

# application
credits-338f52 = KREDİLER

# application
cloud-session-credits = BULUT OTURUM KREDİLERİ

# application
name-login-expires-in-days-left = { $name } oturumunun süresi { $days_left } gün sonra doluyor

# application
name-error = { $name } hatası

# application
sessions = { $v0 } oturum

# application
api-estimate = API tahmini

# application
start-to-end = { $start } – { $end }

# application
to = { $v0 } – { $v1 }

# application
share-1-of-other = { $v0 } toplamının %{ $share } kadarı · { $other }

# application
excluded-other = Hariç · { $other }

# application
exclude-from-usage-stats = Kullanım istatistiklerinden çıkarmak için tıklayın

# application
include-in-usage-stats = Kullanım istatistiklerine dahil etmek için tıklayın

# application
cost-885dc4 = maliyet

# application
tokens-339143 = token

# application
sessions-0e5e29 = · { $v0 } oturum

# application
api-key = API anahtarı

# application
api-keys = API anahtarları

# application
name-deleted = { $name } silindi.

# application
openrouter-api-key-no-longer-exists = OpenRouter API anahtarı artık mevcut değil

# application
openrouter-account-no-longer-exists = OpenRouter hesabı artık mevcut değil

# application
credit = { $v0 } kredi

# application
agy-cli-folder = agy CLI klasörü

# application
folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = codex.exe, codex.cmd veya codex.ps1 içeren klasör. Otomatik bulmak için boş bırakın.

# application
folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = claude.exe, claude.cmd veya claude.ps1 içeren klasör. Otomatik bulmak için boş bırakın.

# application
folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = Cursor.exe içeren klasör. Otomatik bulmak için boş bırakın. Kullanım verisi yine oturum açılan profilden gelir.

# application
folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = agy.exe, agy.cmd veya agy.ps1 içeren klasör. Otomatik bulmak için boş bırakın.

# application
folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = grok.exe, grok.cmd veya grok.ps1 içeren klasör. Otomatik bulmak için boş bırakın.

# src/settings_window/providers.rs
folder-with-cmdc-cmd-or-cmdc-ps1-leave-empty-to-find-it-automati = cmdc.cmd veya cmdc.ps1 içeren klasör. Otomatik bulmak için boş bırakın.

# application
folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = Kiro.exe içeren klasör veya yürütülebilir dosyanın kendisi. Otomatik bulmak için boş bırakın.

# application
folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = KiroCrew.exe içeren klasör veya yürütülebilir dosyanın kendisi. Kullanıcıya özel ve tüm kullanıcılar için kurulumları otomatik algılamak için boş bırakın.

# application
folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = kiro-cli.exe içeren klasör veya yürütülebilir dosyanın kendisi. Otomatik bulmak için boş bırakın.

# application
off-45080e = { $v0 } (kapalı)

# application
each-widget-is-one-icon-in-the-notification-area-indicators-show = Her araç bildirim alanında bir simgedir. Göstergeler sağlayıcı kotasını sayı, çubuk, halka veya sıfırlama saati olarak gösterir.

# application
a-reset-clock-follows-one-quota = Sıfırlama saati tek bir kotayı izler.

# application
ai-tool = Yapay zekâ aracı

# application
msg-7-days = 7 gün

# application
msg-30-days = 30 gün

# application
msg-90-days = 90 gün

# application
yellow = Sarı

# application
custom = Özel

# application
not-available-for-manual-credentials-switch-source-to-config-fold = Manuel kimlik bilgileri için kullanılamaz. Kaynak seçeneğini Yapılandırma klasörü olarak değiştirin.

# application
opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = OpenCode yerel geçmişi ilk OpenCode örneği tarafından izlenir.

# application
this-provider-has-no-local-usage-history = Bu sağlayıcının yerel kullanım geçmişi yok.

# application
this-provider-has-no-session-window-to-start = Bu sağlayıcının başlatılacak oturum dönemi yok.

# application
this-provider-does-not-use-a-config-folder = Bu sağlayıcı yapılandırma klasörü kullanmıyor.

# application
could-not-save-provider-settings-error-restoring-its-previous-cre = Sağlayıcı ayarları kaydedilemedi ({ $error }); önceki kimlik bilgisini geri yükleme de başarısız oldu ({ $rollback_error }).

# application
msg-5h-7d = 5sa | { $v0 } | { $v1 }
    7g  | { $v2 } | { $v3 }

# API key count in the provider header.
api-key-count = { $v0 } API anahtarı

# Application copy
widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = Ana Sayfa ve Kullanım görünümünü genişlet. Ana Sayfa bloklarını sütunlar arasında sürükleyin; sağlayıcı sekmeleri kompakt kalır.

# Application copy
a-possible-codex-reset-is-scheduled-for-when-in-countdown = Olası bir Codex sıfırlaması { $when } için planlandı ({ $countdown } sonra)

# Application copy
label-possible-reset-on-when-in-countdown = { $label }: olası sıfırlama { $when } ({ $countdown } sonra)

# Application copy
new-codex-reset-info = Yeni Codex sıfırlama bilgisi

# Application copy
login-expires-soon = { $v0 } oturumunun süresi yakında doluyor

# Application copy
it-stops-renewing-on-open-minibar-and-choose-sign-in-again = { $v0 } tarihinde yenilenmeyi durdurur. Minibar'ı açıp Yeniden oturum aç'ı seçin.

# Application copy
could-not-clear-usage-data-error = Kullanım verileri temizlenemedi: { $error }

# Application copy
succeeded-at = { $v0 }, { $v1 } zamanında başarılı oldu

# Application copy
failed-at-error = { $v0 }, { $v1 } zamanında başarısız oldu: { $error }

# Application copy
key-39df89 = Anahtar { $v0 }

# Application copy
expired = Süresi doldu

# Application copy
jan = Oca

# Application copy
feb = Şub

# Application copy
mar = Mar

# Application copy
apr = Nis

# Application copy
may = May

# Application copy
jun = Haz

# Application copy
jul = Tem

# Application copy
aug = Ağu

# Application copy
sep = Eyl

# Application copy
oct = Eki

# Application copy
nov = Kas

# Application copy
dec = Ara

# { $value }% used
quota-percent-used = %{ $value } kullanıldı

# { $value }% left
quota-percent-left = %{ $value } kaldı

# { $amount } of { $limit } used
cloud-amount-used = { $limit } toplamının { $amount } kadarı kullanıldı

# { $amount } of { $limit } left
cloud-amount-left = { $limit } toplamının { $amount } kadarı kaldı

# Short usage range
range-24h = 24sa

# Short usage range
range-7d = 7g

# Short usage range
range-30d = 30g

# Short usage range
range-90d = 90g

# Home usage card title
home-usage-title = Kullanım

# OpenRouter key administration

could-not-reach-openrouter = OpenRouter'a ulaşılamadı


openrouter-keys-title = Anahtarlar


openrouter-keys-new = Yeni anahtar


openrouter-keys-new-title = Yeni anahtar


openrouter-keys-back = Geri


openrouter-keys-updating = Güncelleniyor…


openrouter-keys-loading = Anahtarlar yükleniyor…


openrouter-keys-load-failed = Anahtarlar yüklenemedi


openrouter-keys-retry = Tekrar dene


openrouter-keys-empty = Bu hesabın henüz anahtarı yok.


openrouter-keys-this-app = Bu uygulama


openrouter-keys-no-limit = Limit yok


openrouter-keys-of-limit = { $limit } toplamının { $amount } kadarı


openrouter-keys-resets-daily = günlük sıfırlanır


openrouter-keys-resets-weekly = haftalık sıfırlanır


openrouter-keys-resets-monthly = aylık sıfırlanır


openrouter-keys-expires-on = süresi { $date } tarihinde dolar


openrouter-keys-expired-on = süresi { $date } tarihinde doldu


openrouter-keys-today = bugün { $amount }


openrouter-keys-usage-breakdown = Bugün { $today } · hafta { $week } · ay { $month } · toplam { $total }


openrouter-keys-show-all = { $count } anahtarın tümünü göster


openrouter-keys-show-fewer = Daha az göster


openrouter-keys-spending-limit = Harcama limiti


openrouter-keys-limit-hint = Limitsiz olması için boş bırakın.


openrouter-keys-resets = Sıfırlama


openrouter-keys-reset-never = Hiçbir zaman


openrouter-keys-reset-daily = Günlük


openrouter-keys-reset-weekly = Haftalık


openrouter-keys-reset-monthly = Aylık


openrouter-keys-byok = BYOK kullanımını limite dahil et


openrouter-keys-enabled = Etkin


openrouter-keys-delete = Sil


openrouter-keys-delete-tracked = Minibar da izlemeyi durdurur.


openrouter-keys-saving = Kaydediliyor…


openrouter-keys-delete-title = “{ $name }” silinsin mi?


openrouter-keys-delete-message = Bu anahtarı kullanan her şey hemen çalışmayı durdurur. Bu işlem geri alınamaz; anahtarı devre dışı bırakmak ise geri alınabilir.


openrouter-keys-keep = Anahtarı koru


openrouter-keys-delete-confirm = Anahtarı sil


openrouter-keys-deleting = Siliniyor…


openrouter-keys-name = Ad


openrouter-keys-name-placeholder = ör. dizustu-cursor


openrouter-keys-name-required = Anahtar için bir ad girin.


openrouter-keys-invalid-amount = 25 veya 12.50 gibi bir dolar tutarı girin.


openrouter-keys-expires = Süre sonu


openrouter-keys-expires-1-hour = 1 saat


openrouter-keys-expires-1-day = 1 gün


openrouter-keys-expires-7-days = 7 gün


openrouter-keys-expires-30-days = 30 gün


openrouter-keys-expires-90-days = 90 gün


openrouter-keys-expires-180-days = 180 gün


openrouter-keys-expires-1-year = 1 yıl


openrouter-keys-expires-never = Süresiz


openrouter-keys-track = Bu anahtarı Minibar'da izle


openrouter-keys-create = Anahtar oluştur


openrouter-keys-creating = Oluşturuluyor…


openrouter-keys-created = Anahtar oluşturuldu


openrouter-keys-copy = Anahtarı kopyala


openrouter-keys-copied = Kopyalandı


openrouter-keys-done = Bitti


openrouter-keys-once-title = Bu anahtarı bir daha göremeyeceksiniz


openrouter-keys-once-message = OpenRouter yeni anahtarı yalnızca bir kez gösterir. Şimdi kopyalayın.


openrouter-keys-pinned = Bitti'ye basana kadar açılır panel açık kalır.


openrouter-keys-tracking = Anahtar Minibar'a ekleniyor…


openrouter-keys-tracked = Minibar artık bu anahtarı izliyor.


openrouter-keys-track-failed = Anahtar Minibar'a eklenemedi: { $error }


openrouter-keys-no-management-key = Bu hesabın yönetim anahtarı yok.


openrouter-keys-management-key-rejected = OpenRouter yönetim anahtarını reddetti. Ayarlar'dan değiştirin.


openrouter-keys-not-a-management-key = Bu anahtar diğer anahtarları yönetemez. Yönetim anahtarı kullanın.


openrouter-keys-key-not-found = Bu anahtar artık mevcut değil.


openrouter-keys-untrack = Uygulamadan kaldır


openrouter-keys-untracking = Kaldırılıyor…


openrouter-keys-untrack-title = “{ $name }” izlenmesi durdurulsun mu?


openrouter-keys-untrack-message = Minibar bu anahtarı unutur. Anahtar OpenRouter'da çalışmaya devam eder.


openrouter-keys-tracked-here = Minibar bu anahtarı izliyor.


openrouter-keys-manage-hint = Anahtar oluşturmak, düzenlemek ve silmek için Ayarlar'dan yönetim anahtarı ekleyin.

# src/settings_window/notifications.rs
notification-sounds = Bildirim sesleri

# src/settings_window/notifications.rs
plays-a-short-sound-with-each-notification = Her bildirimde kısa bir ses çalar

# src/updater.rs
update-downloading = Güncelleme indiriliyor

# src/updater.rs
update-download-progress = { $total } toplamının { $downloaded } kadarı

# src/updater.rs
update-downloaded = { $downloaded } indirildi

# src/updater.rs
update-installing = Güncelleme yükleniyor

# src/updater.rs
update-unpacking = Dosyalar çıkarılıyor…

# src/updater.rs
update-restarting = Codex Minibar yeniden başlatılıyor

# src/updater.rs
update-restarting-body = Uygulama birazdan yeniden açılacak.

# src/settings_window/general.rs
provider-status = Sağlayıcı durumu

# src/settings_window/general.rs
show-provider-incidents = Sağlayıcı olaylarını göster

# src/settings_window/general.rs
show-provider-incidents-description = Codex, Claude ve Cursor'ın herkese açık durum sayfalarını her 5 dakikada denetler ve devam eden olayları açılır panelde gösterir

# src/settings_window/general.rs
notify-about-new-incidents = Yeni olayları bildir

# src/settings_window/general.rs
notify-about-new-incidents-description = Durum sayfası etkin bir sağlayıcı için yeni olay bildirdiğinde bildirim gösterir

# src/popup_window/ui/cards.rs
open-status-page = Durum sayfasını aç

# src/notifications.rs
provider-incident-title = { $provider }: { $impact }

# src/popup_window/ui/cards.rs
status-since = Başlangıç: { $time }

# src/provider_status.rs
status-impact-maintenance = Bakım

# src/provider_status.rs
status-impact-degraded = Performans düşüşü

# src/provider_status.rs
status-impact-partial-outage = Kısmi kesinti

# src/provider_status.rs
status-impact-major-outage = Büyük kesinti

# src/provider_status.rs
status-stage-investigating = Araştırılıyor

# src/provider_status.rs
status-stage-identified = Belirlendi

# src/provider_status.rs
status-stage-monitoring = İzleniyor

# src/provider_status.rs
status-stage-in-progress = Devam ediyor

# src/settings_window/notifications.rs
news-and-status = Haberler ve durum

# src/whats_new.rs
no-summary-for-this-version = Bu sürüm için henüz özet yok. Tam sürüm notları GitHub'da.

# src/whats_new.rs
couldn-t-load-what-s-new = Yenilikler yüklenemedi. Bağlantınızı kontrol edin veya GitHub'daki sürüm notlarını okuyun.

# src/whats_new.rs
full-release-notes = Tam sürüm notları

# src/settings_window/about.rs
show-what-s-new-after-updating = Güncellemeden sonra yenilikleri göster

# src/settings_window/about.rs
a-short-summary-of-the-release-once-it-s-installed = Yüklendikten sonra sürümün kısa bir özeti.

# src/settings_window/about.rs
view = Görüntüle

# src/settings_window/general.rs
popup-shortcut = Açılır panel kısayolu
popup-shortcut-description = Açılır paneli göstermek veya gizlemek için Windows'ta herhangi bir yerde basın.
shortcut-not-set = Ayarlanmadı
press-a-shortcut = Bir kısayola basın…
popup-shortcut-recording-hint = Esc iptal eder. Backspace kısayolu kaldırır.
popup-shortcut-needs-modifier = Ctrl, Alt veya Win ekleyin ya da F13–F24 kullanın.
popup-shortcut-in-use = Başka bir uygulama bu kısayolu kullanıyor. Farklı bir kısayol seçin.
clear-shortcut = Kısayolu kaldır
taskbar = Görev Çubuğu
preview = Önizleme
taskbar-page-caption = Windows görev çubuğunda, saatin yanında çizilen canlı kullanım araçları. Görev çubuğunun temasını izlerler; sağlayıcının açılır panelini açmak için birine tıklayın.
taskbar-show-widgets = Görev çubuğunda araçları göster
taskbar-show-widgets-description = Aşağıdaki araçları Windows görev çubuğunda çizer.
taskbar-position = Konum
taskbar-position-description = Araçların görev çubuğundaki konumu. Başlangıç konumu ortalanmış görev çubuğu simgelerine en iyi uyar.
taskbar-position-tray = Bildirim alanının yanında
taskbar-position-start = Görev çubuğunun başında
taskbar-widgets = Araçlar
taskbar-no-widgets = Henüz araç yok
taskbar-no-widgets-description = Görev çubuğunda kota göstermek için bir araç ekleyin.
taskbar-show-provider-icon = Sağlayıcı simgesini göster
taskbar-style-glance = Bir bakışta
taskbar-style-glance-hint = Hava durumu düğmesi gibi, iki satır metnin yanında sağlayıcı simgesi.
taskbar-style-rings-hint = Her kota için yanında değeri olan bir ilerleme halkası.
taskbar-style-bars-hint = Sağlayıcı simgesinin yanında üst üste ince ilerleme çubukları.
taskbar-menu-open = Codex Minibar'ı aç
taskbar-menu-settings = Görev çubuğu aracı ayarları
taskbar-menu-hide = Görev çubuğu araçlarını gizle
taskbar-card-left = { $value } kaldı
taskbar-card-used = { $value } kullanıldı
taskbar-card-resets = Sıfırlanmaya kalan: { $time } · { $clock }
taskbar-card-no-data = Veri bekleniyor
short-5h = 5sa
short-7d = 7g
taskbar-style-nested = İç içe halkalar
taskbar-style-dials = Kadranlar
taskbar-style-meters = Seviye göstergeleri
taskbar-style-nested-hint = Tek halka alanında iç içe halkalar; isterseniz yanlarında değerler.
taskbar-style-dials-hint = Her kota için içinde sayısı olan küçük bir halka.
taskbar-style-meters-hint = Pil göstergesi gibi ince seviye sütunları.
taskbar-show-provider-icon-description = Aracın başındaki sağlayıcı simgesi.
taskbar-show-labels = Etiketleri göster
taskbar-show-labels-description = Değerlerin yanında 5sa ve 7g gibi kısa adlar.
taskbar-show-values = Yüzdeleri göster
taskbar-show-values-description = Araç üzerindeki kota yüzdeleri.
taskbar-show-pace = Kullanım hızını göster
taskbar-show-pace-description = Dengeli tüketimde kullanımın şu anda nerede olacağını gösteren bir işaret.
taskbar-menu-edit = Bu aracı düzenle
taskbar-content = İçerik
taskbar-content-hint = Aracın grafiğinin yanına ne yazacağı.
taskbar-reset-time = Sıfırlama zamanı
taskbar-reset-time-description = Her kotanın ne zaman sıfırlanacağı.
taskbar-reset-off = Kapalı
taskbar-reset-countdown = Kalan süre
taskbar-reset-clock = Saat
taskbar-cycle = Kaydırma tekerleğiyle araçlar arasında geçiş yap
taskbar-cycle-description = Aynı anda tek araç gösterir; sonrakine geçmek için üzerinde kaydırın.
taskbar-card-tokens = { $count } token
taskbar-click = Tıklama açar
taskbar-click-description = Araca tıklanınca açılacak açılır panel sayfası.
taskbar-click-nearest = Ana Sayfadaysa Ana Sayfa, değilse sekmesi
taskbar-click-tab = Sağlayıcı sekmesi
taskbar-click-home = Ana Sayfa

# src/settings_window/notifications.rs
follow-do-not-disturb = Rahatsız Etmeyin sırasında duraklat

# src/settings_window/notifications.rs
follow-do-not-disturb-description = Windows Rahatsız Etmeyin veya Odak açıkken bildirimler atlanır ve sesler kapalı kalır. Başlattığınız işlemlerin sonuçları yine gösterilir.

# src/settings_window/advanced.rs
performance = Performans

# src/settings_window/advanced.rs
game-mode = Oyun modu

# src/settings_window/advanced.rs
game-mode-description = Limitleri ve kullanımı daha seyrek denetler, düşük öncelikle çalışır ve bildirimleri duraklatır. Otomatik seçeneği, oyun öne geldiği andan kapanana kadar bunu etkinleştirir.

# src/settings.rs
game-mode-off = Kapalı

# src/settings.rs
game-mode-automatic = Otomatik

# src/settings.rs
game-mode-always = Her zaman açık

# src/settings_window/notifications.rs
notify-game-mode-changes = Oyun modu değişiklikleri

# src/settings_window/notifications.rs
notify-game-mode-changes-description = Oyun modunun açılıp kapandığını gösterir

# src/notifications.rs
game-mode-on-title = Oyun modu açık

# src/notifications.rs
game-mode-on-body = Limitler daha seyrek denetleniyor ve bildirimler duraklatılıyor

# src/notifications.rs
game-mode-on-game-body = { $game } çalışıyor. Limitler daha seyrek denetleniyor ve bildirimler duraklatılıyor

# src/notifications.rs
not-a-game = Oyun değil

# src/settings_window/advanced.rs
add-a-game = Oyun ekle

# src/settings_window/advanced.rs
add-a-game-description = Windows Game Bar'ın tanıdığı oyunlar ile Steam, Epic, GOG ve Xbox oyunları otomatik algılanır. Diğer programları buraya ekleyin.

# src/settings_window/advanced.rs
never-a-game-path = Asla oyun değil · { $path }

# src/notifications.rs
game-mode-off-title = Oyun modu kapalı

# src/notifications.rs
game-mode-off-body = Normal denetimlere ve bildirimlere dönüldü

# src/settings_window/command_palette.rs
command-palette = Command Palette

# src/settings_window/command_palette.rs
stream-deck = Stream Deck

# src/settings_window/command_palette.rs
install = Yükle

# src/settings_window/command_palette.rs
cmdpal-extension = Command Palette uzantısı

# src/settings_window/command_palette.rs
cmdpal-description = Minibar'ı PowerToys Command Palette'e ekler: limitlerini görmek için bir sağlayıcı yazın veya Dock'a bant sabitleyin. Yükleme yönetici onayı ister.

# src/settings_window/command_palette.rs
cmdpal-installed-hint = Yüklendi. Command Palette'e “minibar” veya sağlayıcı adı yazın ya da Edit Dock'tan bant sabitleyin.

# src/settings_window/command_palette.rs
cmdpal-downloading = Uzantı indiriliyor…

# src/settings_window/command_palette.rs
cmdpal-installing = Yükleyici bekleniyor. Devam etmek için yönetici istemini onaylayın.

# src/settings_window/command_palette.rs
cmdpal-installing-short = Yükleniyor…

# src/settings_window/command_palette.rs
cmdpal-removing = Uzantı kaldırılıyor…

# src/settings_window/command_palette.rs
cmdpal-failed = Uzantı yüklenemedi veya kaldırılamadı.

# src/settings_window/command_palette.rs
cmdpal-reinstall = Yeniden yükle

# src/settings_window/command_palette.rs
cmdpal-dock-bands = Dock bantları

# src/settings_window/command_palette.rs
cmdpal-dock-bands-caption = Her bant Command Palette Dock ayarlarında görünür. Edit Dock'tan sabitleyip konumunu seçin.

# src/settings_window/command_palette.rs
cmdpal-no-bands = Bant yok

# src/settings_window/command_palette.rs
cmdpal-no-bands-description = Sağlayıcı limitlerini Dock'a sabitlemek için bant ekleyin.

# src/settings_window/command_palette.rs
cmdpal-add-band = Bant ekle

# src/settings_window/command_palette.rs
cmdpal-band = Bant { $v0 }

# src/settings_window/command_palette.rs
cmdpal-band-removed = Bant kaldırıldı

# src/settings_window/command_palette.rs
cmdpal-remove-band = Bandı kaldır

# src/settings_window/command_palette.rs
cmdpal-content-hint = Dock, bandı alt başlık üzerinde bir başlık olarak gösterir.

# src/settings_window/command_palette.rs
cmdpal-indicators-hint = En fazla 2 kota: biri başlık ve alt başlığı doldurur, iki kota birer satır kullanır.

# src/popup_window/ui/cards.rs
tibo-banked-reset = Tibo Banked Reset™

# src/popup_window/ui/cards.rs
banked-reset-credit = Birikmiş sıfırlama kredisi

# src/popup_window/ui/cards.rs
before-when = { $when } öncesinde

# src/popup_window/ui/cards.rs
time-ago = { $time } önce

# src/popup_window/ui/cards.rs
new-codex-banked-reset-info = Yeni Codex birikmiş sıfırlama bilgisi

# src/popup_window/ui/cards.rs
a-possible-codex-reset-is-expected-before-when-within-countdown = { $when } öncesinde olası bir Codex sıfırlaması bekleniyor ({ $countdown } içinde)

# src/popup_window/ui/cards.rs
label-possible-reset-before-when-within-countdown = { $label }: { $when } öncesinde olası sıfırlama ({ $countdown } içinde)

# src/popup_window/ui/cards.rs
time-late = { $time } gecikti

# Floating usage panel
popup-floating = Yüzen panel
popup-floating-description = Paneli açık tutun ve başlığını başka bir konuma veya monitöre sürükleyin.
popup-always-on-top = Her zaman üstte
popup-always-on-top-description = Paneli diğer uygulamaların üzerinde ve uygulama değiştirirken açık tut.
popup-position = Panel konumu
popup-position-description = Konum oturumlar arasında hatırlanır.
popup-reset-position = Konumu sıfırla
popup-drag-header = Taşımak için sürükleyin
popup-close = Paneli kapat

show-popup-action-bar = Açılır panel eylem çubuğunu göster
show-popup-action-bar-description = Normal açılır panelde uygulama başlığını ve eylem düğmelerini göster. Yüzen paneller bu çubuğu her zaman gösterir.

# src/settings_window/activation.rs
use-banked-resets-automatically = Birikmiş sıfırlamaları otomatik kullan

# src/settings_window/activation.rs
spends-a-banked-reset-shortly-before-it-expires = Birikmiş bir sıfırlamayı boşa gitmemesi için süresi dolmadan kısa süre önce uygular. Limitlerde temizlenecek bir şey yokken atlanır.

# src/settings_window/activation.rs
use-before-expiry = Süre dolmadan kullan

# src/settings_window/activation.rs
how-long-before-a-banked-reset-expires-it-is-applied = Birikmiş sıfırlamanın süresi dolmadan ne kadar önce uygulanacağı.

# src/settings_window/providers/page.rs
automatic-resets = Otomatik sıfırlamalar

# src/popup_window/auto_reset.rs
banked-reset-applied-automatically = Birikmiş sıfırlama otomatik uygulandı

# src/instances.rs
this-provider-has-no-banked-resets = Bu sağlayıcının birikmiş sıfırlaması yok.

# src/popup_window/auto_reset.rs
banked-reset-applies-in-minutes = Birikmiş sıfırlama { $minutes } dk içinde uygulanacak

# src/settings_window/notifications.rs
before-an-automatic-reset = Otomatik sıfırlamadan önce

# src/settings_window/notifications.rs
warns-before-a-banked-reset-is-applied-automatically = Birikmiş bir sıfırlama otomatik uygulanmadan 10 dakika önce uyarır.

# src/popup_window/auto_reset.rs
apply-reset-now = Şimdi uygula

# src/popup_window/auto_reset.rs
skip-automatic-reset = İptal

# src/popup_window/auto_reset.rs
banked-reset-applied = Birikmiş sıfırlama uygulandı
