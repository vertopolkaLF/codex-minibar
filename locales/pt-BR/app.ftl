# Codex Minibar — pt-BR
# Full Fluent catalog; parameters and technical names remain unchanged.

luna-reserve = Reserva Luna

on-pace = No ritmo

delta-in-deficit = Déficit de { $delta }%

delta-in-reserve = Reserva de { $delta }%

msg-5-hour-limit-started = Limite de 5 horas iniciado

msg-5-hour-limit-reset-and-activated = Limite de 5 horas redefinido e ativado

msg-5-hour-limit-reset = Limite de 5 horas redefinido

weekly-limit-reset = Limite semanal redefinido

name-5-hour = { $name } · 5 horas

label-limit-is-low = O limite de { $label } está baixo

expired-at-time = expirou às { $time }

expired-at-time-6c39ff = expirou às { $time } { $v0 }

never = Nunca

unlimited = Ilimitado

unavailable = Indisponível

days-d-hours-h = { $days }d { $hours }h

days-d = { $days }d

hours-h-minutes-m = { $hours }h { $minutes }min

hours-h = { $hours }h

minutes-m = { $minutes }min

waiting-for-first-update = Aguardando a primeira atualização

just-now = agora mesmo

seconds-seconds-ago = há { $seconds } segundos

minutes-ago = há { $v0 } minutos

updated-elapsed = Atualizado { $elapsed }

the-request-timed-out-try-refreshing-again = A solicitação excedeu o tempo limite. Tente atualizar novamente.

the-secure-connection-could-not-be-verified-see-log-for-details = Não foi possível verificar a conexão segura. Consulte o Log para ver os detalhes.

the-provider-closed-the-connection-try-refreshing-again = O provedor encerrou a conexão. Tente atualizar novamente.

the-provider-s-address-could-not-be-resolved-check-your-connectio = Não foi possível resolver o endereço do provedor. Verifique sua conexão.

could-not-connect-to-the-provider-check-your-connection-and-try-a = Não foi possível conectar ao provedor. Verifique sua conexão e tente novamente.

the-management-key-was-rejected-update-it-in-settings = A chave de gerenciamento foi rejeitada. Atualize-a nas Configurações.

authentication-failed-sign-in-again-or-update-the-provider-key = Falha na autenticação. Entre novamente ou atualize a chave do provedor.

access-denied-by-the-provider-http-403 = Acesso negado pelo provedor (HTTP 403).

too-many-requests-wait-a-few-minutes-before-refreshing-again = Muitas solicitações. Aguarde alguns minutos antes de atualizar novamente.

the-provider-is-temporarily-unavailable-try-again-later = O provedor está temporariamente indisponível. Tente novamente mais tarde.

the-provider-rejected-the-request-see-log-for-details = O provedor rejeitou a solicitação. Consulte o Log para ver os detalhes.

the-requested-resource-was-not-found-see-log-for-details = O recurso solicitado não foi encontrado. Consulte o Log para ver os detalhes.

the-provider-returned-an-unexpected-response-try-refreshing-again = O provedor retornou uma resposta inesperada. Tente atualizar novamente.

the-request-failed-see-log-for-details = A solicitação falhou. Consulte o Log para ver os detalhes.

input = Entrada

cache = Cache

output = Saída

input-uncached = Entrada (sem cache)

cached-input = Entrada em cache

value-partially-priced = { $value } (preço parcial)

could-not-load-model-data-error = Não foi possível carregar os dados dos modelos: { $error }

usage-activity = Atividade de uso

waiting-for-cursor-s-usage-export-refresh-to-retry = Aguardando a exportação de uso do Cursor. Atualize para tentar novamente.

model-data-unavailable = Dados dos modelos indisponíveis

loading-models = Carregando modelos…

no-model-data = Sem dados dos modelos

no-usage-data = Sem dados de uso

no-series-selected = Nenhuma série selecionada

no-tokens-in-this-period = Sem tokens de { $v0 } neste período

loading-model-breakdown = Carregando detalhamento por modelo

group-tokens-or-cost-by-model = Agrupar tokens ou custo por modelo

model = Modelo

# src/popup_window/ui/usage.rs
project = Projeto

# src/usage_overview.rs
unknown-project = Projeto desconhecido

# src/popup_window/ui/usage.rs
no-project-usage = Sem dados de projetos neste período

tokens = Tokens

cost = Custo

no-cost-data-for-this-period = Sem dados de custo neste período

no-token-data-for-this-period = Sem dados de tokens neste período

daily-cost-in-usd = Custo diário em USD

daily-token-volume = Volume diário de tokens

of-total-models-scroll-for-more = { $v0 }–{ $v1 } de { $total } modelos · Role para ver mais

cost-usd-by-model = Custo (USD) por modelo

tokens-by-model = Tokens por modelo

today = Hoje

last-period-days = Últimos { $period } dias

no-data = Sem dados

loading-usage-statistics = Carregando estatísticas de uso…

available-balance = Saldo disponível

resets-in = Redefine em

session-not-started = Não iniciada

expires-in = Expira em

usage = Uso:

remove-key = Remover chave

msg-1-banked-reset = 1 redefinição acumulada

count-banked-resets = { $count ->
    [one] { $count } redefinição
   *[other] { $count } redefinições
    }

available-to-use = Disponível para uso

no-expiration-date = Sem data de expiração

banked-reset = Redefinição acumulada { $v0 }

codex-limits = Limites do Codex

open-announcement-source = Abrir a fonte do anúncio

source-not-provided = Fonte não informada

tibo-reset = Tibo Reset™

home = Início

usage-0bb186 = Uso

settings = Configurações

install-update = Instalar atualização

refreshing-limits-and-usage = Atualizando limites e uso…

updated = Atualizado{ " " }

refresh-last-updated-relative = Atualizar · Última atualização { $relative }

# src/popup_window/ui/footer.rs
popup-pin = Fixar no topo e manter aberto
popup-unpin = Desafixar e fechar ao perder o foco

edit-home = Editar início
home-card-layout = Layout
home-card-layout-cards = Cartões
home-card-layout-lines = Linhas
home-card-layout-rings = Anéis
home-card-layout-tiles = Blocos
home-card-layout-classic = Clássico
drag-to-reorder = Arraste para reordenar

drop-here = Solte aqui

usage-stats = Estatísticas de uso

loading-usage = Carregando uso…

update-failed = Falha na atualização

something-went-wrong = Algo deu errado

no-providers-enabled = Nenhum provedor ativado

turn-one-on-in-settings-providers = Ative um em Configurações > Provedores.

sign-in-again-to-keep-limits-updating = Entre novamente para continuar atualizando os limites.

sign-in-again = Entrar novamente

total = Total

enable-a-provider-in-settings-and-include-it-in-usage-stats-to-se = Ative um provedor nas Configurações e inclua-o nas Estatísticas de uso para ver o uso local da API.

hourly-cost = Custo por hora

hourly-processed-tokens = Tokens processados por hora

no-activity-in-this-range = Sem atividade neste intervalo

day = Dia

breakdown = Detalhamento

totals = Totais

processed-tokens = Tokens processados

uncached-input = Entrada sem cache

cache-savings = Economia com cache

share = Participação

hour = Hora

sat = Sáb

sun = Dom

msg-5h-session = Sessão de 5h

weekly = Semanal

cursor-models = Modelos do Cursor

other-models = Outros modelos

all-models = Todos os modelos

grok-bot = Grok Bot

monthly = Mensal

spending-limit = Limite de gastos

gemini = Gemini

claude-gpt = Claude + GPT

credits = Créditos

monthly-credits = Créditos mensais

codex = Codex

claude = Claude

cursor = Cursor

opencode-zen = OpenCode Zen

opencode-go = OpenCode Go

openrouter = OpenRouter

antigravity = Antigravity

grok = Grok

kiro = Kiro

banked-resets = Redefinições acumuladas

usage-stats-7b9e1a = Estatísticas de uso

spending = Gastos

separate-tabs = Abas separadas

grouped-switcher = Agrupado, com seletor

grouped-all-accounts = Agrupado, todas as contas

openrouter-account = Conta OpenRouter

yesterday = Ontem

history-retention-must-be-between-1-and-365-days = a retenção do histórico deve ser de 1 a 365 dias

session-low-usage-threshold-must-be-between-1-and-99-percent = o limite de uso baixo da sessão deve ser de 1 a 99 por cento

weekly-low-usage-threshold-must-be-between-1-and-99-percent = o limite de uso baixo semanal deve ser de 1 a 99 por cento

check-github-for-a-new-version = Verificar nova versão no GitHub

checking-for-updates = Verificando atualizações…

you-re-up-to-date = Você está na versão mais recente

update-available = Atualização { $v0 } disponível

installing-update = Instalando atualização…

couldn-t-check-for-updates = Não foi possível verificar atualizações

version = Versão { $v0 }

about-tagline = Companheiro gratuito e de código aberto para limites de uso, redefinições e gastos das suas assinaturas de IA, direto na bandeja do Windows.

a-new-release-is-ready-to-install = Uma nova versão está pronta para instalar.

what-s-new = Novidades

update = Atualizar

check-for-updates = Verificar atualizações

check-for-updates-on-startup = Verificar atualizações ao iniciar

notify-when-a-new-version-is-found = Notificar quando uma nova versão for encontrada

github = GitHub

source-code = Código-fonte

releases = Versões

see-what-s-new = Ver novidades

report-an-issue = Relatar um problema

found-a-bug = Encontrou um bug?

author = Autor

updates = Atualizações

resources = Recursos

mon = Seg

tue = Ter

wed = Qua

thu = Qui

fri = Sex

every-day = Todos os dias

weekdays = Dias úteis

weekends = Fins de semana

no-days = Nenhum dia

start-5-hour-sessions-automatically = Iniciar sessões de 5 horas automaticamente

starts-a-new-session-as-soon-as-a-window-is-available-instead-of = Inicia uma nova sessão assim que a janela fica disponível, sem esperar sua primeira solicitação. Cada conta usa seu próprio login.

add-codex-or-claude-in-providers-first = Adicione Codex ou Claude em Provedores primeiro.

off-in-providers = Desativado em Provedores

quiet-periods = Períodos de pausa

don-t-auto-start-sessions-during-these-times = Não iniciar sessões automaticamente nestes horários.

add = Adicionar

all-day = Dia inteiro

from = De

until = Até

scheduled-activations = Ativações agendadas

start-a-5-hour-session-at-a-set-time = Iniciar uma sessão de 5 horas em um horário definido.

time = Horário

turn-on-codex-or-claude-in-providers-first-with-a-config-folder-l = Ative Codex ou Claude em Provedores primeiro, com login por pasta de configuração.

no-quiet-periods-yet = Ainda não há períodos de pausa.

no-scheduled-activations-yet = Ainda não há ativações agendadas.

unknown-provider = Provedor desconhecido

remove-quiet-period = Remover período de pausa

remove-activation = Remover ativação

provider = Provedor

days = Dias

export-settings = Exportar configurações

save-every-setting-to-a-toml-file-saved-keys-stay-in-windows-user = Salve todas as configurações em um arquivo .toml. As chaves salvas ficam no armazenamento do usuário do Windows.

export = Exportar

import-settings = Importar configurações

replace-the-current-settings-with-a-previously-exported-file = Substitua as configurações atuais por um arquivo exportado anteriormente.

import = Importar

clear-usage-data = Limpar dados de uso

delete-the-collected-usage-history-it-is-rebuilt-from-local-provi = Exclui o histórico de uso coletado. Ele é reconstruído a partir dos logs locais dos provedores na próxima varredura.

clear = Limpar

usage-data-clear-failed = Falha ao limpar dados de uso

the-background-worker-is-unavailable = O processo em segundo plano está indisponível.

usage-data-cleared = Dados de uso limpos.

reset-all-settings = Redefinir todas as configurações

restore-every-default-and-start-the-welcome-flow-again = Restaure os padrões e inicie a configuração inicial novamente.

reset = Redefinir

backup = Backup

data = Dados

rendering = Renderização

software-rendering = Renderização por software

software-rendering-description = Desenha as janelas com o processador em vez da placa de vídeo. Pode ajudar quando um jogo ocupa a GPU; as animações podem ficar menos suaves.

settings-exported = Configurações exportadas.

settings-export-failed = Falha ao exportar configurações

settings-imported = Configurações importadas.

settings-import-failed = Falha ao importar configurações

settings-reset-failed = Falha ao redefinir configurações

reset-all-settings-bbfe66 = Redefinir todas as configurações?

every-setting-returns-to-its-default-and-the-welcome-flow-opens-a = Todas as configurações voltam ao padrão e a configuração inicial abre novamente. As chaves salvas e os dados de uso são mantidos.

cancel = Cancelar

windows = Windows

light = Claro

dark = Escuro

color-theme = Tema de cores

applies-to-settings-the-popup-and-its-tray-menu = Aplica-se às Configurações, ao popup e ao menu da bandeja.

accent-color = Cor de destaque

windows-follows-your-system-accent = Windows segue a cor de destaque do sistema.


font = Fonte

any-font-installed-on-this-pc = Qualquer fonte instalada neste PC.
no-matching-fonts = Nenhuma fonte correspondente

windows-default = Padrão do Windows




monochrome = Monocromático

time-format = Formato de hora

msg-12-hour = 12 horas

msg-24-hour = 24 horas

popup-background = Fundo do popup
popup-theme = Tema do popup
popup-theme-description = Altera apenas o popup. As configurações mantêm a aparência do Windows.
popup-theme-fluent = Fluent
popup-theme-vercel = Vercel
popup-theme-built-in = Integrado
popup-theme-vscode = Tema do VS Code
vscode-themes = Temas do VS Code
vscode-themes-description = Use temas de cores do Visual Studio Code na janela pop-up: explore o Open VSX ou importe um arquivo de tema .vsix ou .json.
import-theme = Importar tema
importing-theme = Importando…
remove-theme = Remover tema
browse-open-vsx = Explorar Open VSX
browse-open-vsx-description = Encontre e instale temas de cores do registro open-vsx.org
search-color-themes = Pesquisar temas de cores
open-vsx-search = Pesquisar
open-vsx-browse = Explorar
open-vsx-searching = Pesquisando…
no-themes-found = Nenhum tema de cores encontrado
open-vsx-load-failed = Não foi possível carregar os temas do Open VSX
open-vsx-retry = Tentar novamente
open-vsx-preview-loading = Carregando prévia…
open-vsx-preview-hint = Clique em um tema para visualizá-lo no pop-up. Instalar mantém o pacote inteiro; o botão de download de um tema mantém apenas ele.
open-vsx-preview-empty = Esta extensão não inclui temas de cores
open-vsx-install = Instalar
open-vsx-install-one = Instalar apenas este tema
open-vsx-installed = Instalado
open-vsx-installing = Instalando…
theme-installed = Tema instalado
theme-install-failed = Não foi possível instalar o tema
theme-remove-failed = Não foi possível remover o tema

acrylic = Acrílico

mica = Mica

solid = Sólido

bottom-bar-size = Tamanho da barra inferior

comfortable = Confortável

compact = Compacto

popup-corner-radius = Raio dos cantos do popup
popup-borders = Bordas
popup-borders-description = Escolher quais partes do pop-up são contornadas
popup-card-borders = Cartões e controles
popup-window-border = Borda da janela
popup-theme-contrast = Contraste
popup-theme-contrast-description = Destacar mais ou menos o texto e os contornos dos temas do VS Code
popup-theme-tint = Tonalidade
popup-theme-tint-description = Quanto o fundo do tema cobre o vidro Mica ou Acrylic

animation-effects = Efeitos de animação

glide-transitions-in-the-popup-and-settings-windows-own-animation = Transições suaves no popup e nas Configurações. A preferência de animação do Windows também é respeitada.

popup = Popup

motion = Movimento

blue = Azul

purple = Roxo

pink = Rosa

red = Vermelho

orange = Laranja

green = Verde

teal = Verde-azulado

show-edit-button = Mostrar o botão Editar na barra inferior
show-edit-button-description = O modo de edição mostra alças no Início para arrastar os widgets para uma nova ordem. Inicie-o aqui para manter a barra inferior compacta.
start-editing = Começar a editar

use-two-columns = Usar duas colunas

several-accounts-of-one-provider = Várias contas do mesmo provedor

separate-tabs-gives-every-instance-its-own-tab-grouped-shows-one = Abas separadas dá uma aba a cada instância. Agrupado mostra uma aba por provedor, com um seletor de contas ou todas as contas empilhadas.

use-monochrome-icons = Usar ícones monocromáticos

draw-provider-marks-in-the-popup-without-brand-colors = Mostrar os ícones dos provedores no popup sem as cores das marcas.

show-used-instead-of-remaining = Mostrar usado em vez de restante

show-usage-in-values-when-possible = Mostrar uso em valores (quando possível)

adds-exact-used-limit-amounts-next-to-percentages-when-a-provider = Adiciona valores exatos de uso/limite ao lado das porcentagens quando o provedor os informa.

show-usage-pace = Mostrar ritmo de uso

marks-whether-you-re-burning-quota-faster-or-slower-than-an-even = Indica se você está consumindo a cota mais rápido ou mais devagar que um ritmo uniforme.



show-account-name = Mostrar nome da conta

show-on-home-tab = Mostrar na aba Início

layout = Layout

donut = Rosca

cards = Cartões

tabs = Abas

usage-widget = Widget de uso

card = Cartão

tab = Aba

popup-cards = Cartões do popup

choose-which-cards-this-account-shows-on-home-and-on-its-own-tab = Escolha os cartões que esta conta mostra no Início e na própria aba.

choose-which-cards-this-account-shows-on-its-own-tab-turn-on-show = Escolha os cartões que esta conta mostra na própria aba. Ative Mostrar no Início para escolher os cartões do Início.

msg-30-seconds = 30 segundos

msg-1-minute = 1 minuto

msg-2-minutes = 2 minutos

msg-5-minutes = 5 minutos

msg-10-minutes = 10 minutos

msg-15-minutes = 15 minutos

msg-30-minutes = 30 minutos

msg-45-minutes = 45 minutos

msg-60-minutes = 60 minutos

start-with-windows = Iniciar com o Windows

open-codex-minibar-in-the-tray-when-you-sign-in = Abrir o Codex Minibar na bandeja ao entrar no Windows.

refresh-interval = Intervalo de atualização

how-often-this-instance-s-quotas-are-read = Frequência de leitura das cotas desta instância. Intervalos maiores evitam os limites de requisições do provedor.

enable-usage-stats = Ativar estatísticas de uso

scan-local-provider-history-for-the-usage-tab-and-cost-totals = Ler o histórico local dos provedores para a aba Uso e os custos totais.

collection-period = Intervalo de coleta

how-often-local-provider-history-is-scanned = Frequência de leitura do histórico local dos provedores.

check-for-confirmed-tibo-resets = Verificar redefinições confirmadas do Tibo

reads-the-app-s-public-github-feed-and-keeps-the-latest-announcem = Lê o feed público do aplicativo no GitHub e mantém o último anúncio em cache.

notify-when-new-reset-info-arrives = Notificar quando houver novas informações de redefinição

shows-a-notification-when-the-feed-reports-a-possible-reset-never = Mostra uma notificação quando o feed informa uma possível redefinição, nunca no horário da redefinição.

check-every = Verificar a cada

the-feed-is-also-checked-immediately-when-the-app-starts-or-this = O feed também é verificado imediatamente ao iniciar o aplicativo ou ativar esta opção.

msg-1-hour = 1 hora

msg-3-hours = 3 horas

# src/settings_window/general.rs
msg-2-hours = 2 horas

# src/settings_window/general.rs
msg-5-hours = 5 horas

msg-6-hours = 6 horas

msg-12-hours = 12 horas

msg-24-hours = 24 horas

tibo-resets = Tibo Resets™

enable-a-provider-in-the-providers-tab-to-include-it-here = Ative um provedor na aba Provedores para incluí-lo aqui.

add-a-management-key = Adicionar chave de gerenciamento

usage-statistics-are-off-on-this-provider-s-page = As estatísticas de uso estão desativadas na página deste provedor

included-providers = Provedores incluídos

choose-which-accounts-count-toward-this-machine-s-usage-tab-and-h = Escolha as contas incluídas na aba Uso e no total do Início deste PC. As contas excluídas continuam coletando dados e mostram o uso na própria página.

download-the-latest-stream-deck-companion-from-github-and-open-it = Baixe o complemento mais recente do Stream Deck no GitHub e abra o instalador.

downloading-the-latest-stream-deck-companion-from-github = Baixando o complemento mais recente do Stream Deck no GitHub…

opening-the-stream-deck-installer = Abrindo o instalador do Stream Deck…

the-installer-is-open-finish-the-installation-in-stream-deck = O instalador está aberto. Conclua a instalação no Stream Deck.

the-plugin-could-not-be-downloaded-or-opened-try-again = Não foi possível baixar ou abrir o plugin. Tente novamente.

downloading = Baixando…

opening = Abrindo…

install-plugin = Instalar plugin

stream-deck-companion = Complemento do Stream Deck

check-your-connection-then-try-again = Verifique sua conexão e tente novamente.

on = Ativado

off = Desativado

more-options = Mais opções

custom-color = Cor personalizada

run-troubleshoot-with-ai = Executar diagnóstico com IA

let-an-installed-ai-cli-read-the-log-and-investigate-a-problem = Permita que uma CLI de IA instalada leia o log e investigue um problema.

choose-tool = Escolher ferramenta

application-log = Log do aplicativo

log-txt-in-the-app-data-folder = log.txt na pasta de dados do aplicativo.

open-log-txt = Abrir log.txt

could-not-open-log-txt = Não foi possível abrir log.txt

open-logs-folder = Abrir pasta de logs

could-not-open-logs-folder = Não foi possível abrir a pasta de logs

no-log-events-yet = Ainda não há eventos no log.

live-tail = Acompanhar ao vivo

no-supported-ai-tool-found = Nenhuma ferramenta de IA compatível encontrada

install-codex-or-claude-code-and-make-it-available-to-minibar = Instale Codex ou Claude Code e disponibilize-o para o Minibar.

general = Geral

appearance = Aparência

providers = Provedores

customize = Personalizar

limit-activation = Ativação de limites

tray = Bandeja

notifications = Notificações

advanced = Avançado

log = Log

integrations = Integrações

about-updates = Sobre e atualizações

successful-activations = Ativações bem-sucedidas

failed-activations = Ativações com falha

when-limits-reset = Quando os limites forem redefinidos

when-5-hour-remaining-hits = Quando o restante de 5 horas chegar a { $v0 }%

when-weekly-remaining-hits = Quando o restante semanal chegar a { $v0 }%

low-usage = Uso baixo

shows-a-notification-once-per-window-when-the-remaining-share-dro = Mostra uma notificação uma vez por janela quando a porcentagem restante cai até o limite.

threshold = Limite

found-in-opencode-auth-or-local-history = Encontrado na autenticação ou no histórico local do OpenCode.

not-found-turn-it-on-if-it-s-set-up-elsewhere = Não encontrado. Ative se estiver configurado em outro local.

account-credentials-are-already-set = As credenciais da conta já estão configuradas.

optional-add-accounts-later-in-providers = Opcional. Adicione contas depois em Provedores.

found-an-official-agy-sign-in-on-this-pc = Encontrado um login oficial do agy neste PC.

not-found-sign-in-with-agy-before-enabling-it = Não encontrado. Entre com agy antes de ativar.

found-an-official-grok-cli-sign-in-on-this-pc = Encontrado um login oficial da CLI do Grok neste PC.

not-found-run-grok-login-before-enabling-it = Não encontrado. Execute grok login antes de ativar.

# src/settings_window/onboarding.rs
found-an-official-command-code-cli-sign-in-on-this-pc = Encontrado um login oficial da CLI do Command Code neste PC.

# src/settings_window/onboarding.rs
not-found-run-cmdc-login-before-enabling-it = Não encontrado. Execute cmdc login antes de ativar.

found-kiro-ide-kiro-crew-or-a-signed-in-kiro-cli = Encontrado Kiro IDE, Kiro Crew ou uma CLI do Kiro conectada.

not-found-install-kiro-ide-or-kiro-crew-or-sign-in-to-kiro-cli = Não encontrado. Instale Kiro IDE ou Kiro Crew, ou entre na CLI do Kiro.

found-on-this-pc = Encontrado neste PC.

not-found-turn-it-on-if-it-s-installed-somewhere-else = Não encontrado. Ative se estiver instalado em outro local.

setup-could-not-be-saved = Não foi possível salvar a configuração inicial

detected = Detectado

starts-a-new-codex-or-claude-session-as-soon-as-a-window-is-avail = Inicia uma nova sessão do Codex ou Claude assim que a janela fica disponível, sem esperar sua primeira solicitação.

collect-usage-data = Coletar dados de uso

scans-local-provider-history-for-usage-stats = Lê o histórico local dos provedores para as estatísticas de uso.

startup = Inicialização

features = Recursos

customization = Personalização

when-a-new-version-is-found = Quando uma nova versão for encontrada

activity = Atividade

choose-providers = Escolher provedores
choose-a-theme = Escolher um tema
theme-step-description = Escolha as cores das Configurações e do pop-up. O pop-up mostra cada escolha na hora.

we-turned-on-the-providers-found-on-this-pc-you-can-change-this-l = Ativamos os provedores encontrados neste PC. Você pode alterar isso depois.

general-settings = Configurações gerais

you-can-change-these-later-in-settings = Você pode alterar isso depois nas Configurações.

turn-off-anything-you-don-t-want-to-hear-about-you-can-change-the = Desative as notificações que não quiser receber. Você pode alterar isso depois nas Configurações.

back = Voltar

continue = Continuar

done = Concluir

details = Detalhes

added = Adicionado

reads-the-session-and-weekly-limits-of-a-claude-subscription-with = Lê os limites de sessão e semanais de uma assinatura Claude sem login no Claude Code.

msg-1-open-claude-in-a-separate-browser-profile-or-private-window-sig = 1. Abra o Claude em um perfil separado do navegador ou em uma janela privada. Entre na conta que deseja acompanhar e confirme o e-mail nas configurações do Claude.

open-claude-ai = Abrir claude.ai

msg-2-in-chrome-or-edge-press-f12-open-application-storage-cookies-an = 2. No Chrome ou Edge, pressione F12. Abra Application > Storage > Cookies e selecione https://claude.ai.

msg-3-find-sessionkey-copy-its-value-not-its-name-or-the-whole-cookie = 3. Encontre sessionKey. Copie o campo Value, não o nome nem a tabela inteira de cookies, e cole no campo acima.

how-to-view-cookies-in-chrome = Como ver cookies no Chrome

a-cookie-header-containing-sessionkey-also-works-when-the-session = Um cabeçalho Cookie contendo sessionKey também funciona. Quando a sessão expirar, cole um novo aqui.

use-the-access-token-from-a-claude-code-subscription-login-miniba = Use o token de acesso de um login de assinatura Claude Code. O Minibar não pode renovar um token colado; prefira Fonte: Pasta de configuração quando possível.

copy-only-claudeaioauth-accesstoken-starts-with-sk-ant-oat-from-t = Copie apenas claudeAiOauth.accessToken (começa com sk-ant-oat) do .credentials.json desse login, sem aspas. Não use claude setup-token: ele pode não ter acesso ao uso.

claude-code-log-in-with-multiple-accounts = Claude Code: entrar com várias contas

requires-a-claude-subscription-login-with-usage-access-api-keys-a = Requer login de assinatura Claude com acesso ao uso. Chaves de API e da Admin API não mostram os limites da assinatura.

this-provider = este provedor

track-another-account-or-a-provider-minibar-has-not-shown-yet = Acompanhe outra conta ou um provedor que o Minibar ainda não mostrou.

e-g-work = ex.: Trabalho

name = Nome

shown-on-its-tab-home-card-tray-and-notifications = Mostrado na aba, no cartão do Início, na bandeja e nas notificações.

badge = Identificador

auto = Automático

badge-color = Cor do identificador

up-to-three-letters-empty-uses-the-name-s-initials-badges-show-wh = Até três letras; em branco usa as iniciais do nome. Os identificadores aparecem quando mais de uma instância do provedor está ativada.

add-provider = Adicionar provedor

next = Próximo

minibar-stops-reading-name-and-forgets-its-saved-keys-schedules-t = O Minibar para de ler { $name } e esquece suas chaves salvas, agendamentos, indicadores da bandeja e posição no Início.{ $v0 }

its-config-folder-stays-on-disk = { " " }A pasta de configuração permanece no disco.

delete-name = Excluir { $name }?

delete = Excluir

finish-signing-in-in-your-browser-cancel-stops-this-login = Conclua o login no navegador. Cancelar interrompe este login.

runs-claude-code-s-own-login-for-name-with-its-config-folder-as-c = Executa o login do próprio Claude Code para { $name }, usando sua pasta de configuração como CLAUDE_CONFIG_DIR. O login fica nessa pasta, onde o Claude Code o mantém atualizado. Requer o Claude Code nativo para Windows.

runs-codex-s-own-login-for-name-with-its-config-folder-as-codex-h = Executa o login do próprio Codex para { $name }, usando sua pasta de configuração como CODEX_HOME. O login fica nessa pasta, onde o Codex o mantém atualizado. Requer a CLI ou o aplicativo desktop nativo do Codex.

sign-in-to-name = Entrar em { $name }

sign-in = Entrar

for = Para { $v0 }.

browser-session = Sessão do navegador

oauth-token = Token OAuth

session-key = Chave de sessão

paste-the-sessionkey-value = Cole o valor de sessionKey

oauth-access-token = Token de acesso OAuth

the-saved-credential-is-replaced-only-after-the-new-one-passes-th = A credencial salva só é substituída depois que a nova passa na verificação.

claude-credential = Credencial do Claude

check-and-save = Verificar e salvar

key-name-optional = Nome da chave (opcional)

e-g-personal = ex.: Pessoal

leave-blank-to-use-the-name-from-openrouter = Deixe em branco para usar o nome do OpenRouter.

minibar-checks-the-key-with-openrouter-before-saving-it = O Minibar verifica a chave com o OpenRouter antes de salvar.

replace-api-key = Substituir chave de API

add-api-key = Adicionar chave de API

management-key = Chave de gerenciamento

create-one-under-settings-management-keys-on-openrouter-ai = Crie uma em Settings → Management keys no openrouter.ai.

replace-management-key = Substituir chave de gerenciamento

add-management-key = Adicionar chave de gerenciamento

key-name = Nome da chave

rename-key = Renomear chave

save = Salvar

this-key = esta chave

minibar-stops-tracking-hint-the-key-keeps-working-on-openrouter = O Minibar para de acompanhar { $hint }. A chave continua funcionando no OpenRouter.

remove-api-key = Remover chave de API?

remove = Remover

minibar-stops-showing-credit-balance-and-usage-history-for-the-ke = O Minibar para de mostrar saldo de créditos e histórico de uso de { $v0 }. A chave continua funcionando no OpenRouter.

remove-management-key = Remover chave de gerenciamento?

saved-in-windows-user-storage-never-in-the-settings-file = Salva no armazenamento do usuário do Windows, nunca no arquivo de configurações.

save-key = Salvar chave

minibar-forgets-the-saved-key-of-it-keeps-working-with-opencode = O Minibar esquece a chave salva de { $v0 }. Ela continua funcionando com o OpenCode.

signing-in = Entrando…

checking = Verificando…

choose-a-provider = Escolha um provedor.

reason-it-is-already-in-the-list = { $reason }. Já está na lista.

added-name = { $name } adicionado.

could-not-add-the-provider-error = Não foi possível adicionar o provedor: { $error }

this-provider-no-longer-exists = Este provedor não existe mais.

could-not-delete-the-provider-error = Não foi possível excluir o provedor: { $error }

paste-a-credential-first = Cole uma credencial primeiro.

credential-saved-in-windows-user-storage = Credencial salva no armazenamento do usuário do Windows.

paste-a-key-first = Cole uma chave primeiro.

this-account-no-longer-exists = Esta conta não existe mais.

api-key-saved-in-windows-user-storage = Chave de API salva no armazenamento do usuário do Windows.

management-key-replaced = Chave de gerenciamento substituída.

management-key-added = Chave de gerenciamento adicionada.

could-not-rename-the-key-error = Não foi possível renomear a chave: { $error }

api-key-renamed = Chave de API renomeada.

could-not-remove-the-key-error = Não foi possível remover a chave: { $error }

api-key-removed = Chave de API removida.

management-key-removed = Chave de gerenciamento removida.

could-not-save-the-key-error = Não foi possível salvar a chave: { $error }

api-key-saved = Chave de API salva.

switch-source-to-config-folder-to-sign-in = Mude Fonte para Pasta de configuração para entrar.

this-provider-has-no-config-folder = Este provedor não tem pasta de configuração.

another-instance-already-reads-this-config-folder-choose-a-differ = Outra instância já lê esta pasta de configuração. Escolha outra pasta primeiro.

this-provider-has-no-sign-in = Este provedor não tem login.

signed-in-its-cli-keeps-the-login-fresh-in-its-config-folder = { $v0 } conectado. Sua CLI mantém o login atualizado na pasta de configuração.

no-keys-yet = Ainda não há chaves

management-key-5c8cf2 = { " " }· chave de gerenciamento

using-a-saved-credential = Usando uma credencial salva

paste-a-credential-under-account = Cole uma credencial em Conta

needs-an-api-key-or-opencode-sign-in = Requer chave de API ou login no OpenCode

needs-an-api-key = Requer uma chave de API

not-found-set-its-folder-under-runtime = Não encontrado. Defina a pasta em Ambiente de execução.

using-a-saved-api-key = Usando uma chave de API salva

using-opencode-sign-in-or-local-history = Usando login no OpenCode ou histórico local

reading-cli = Lendo { $cli }

reading-crew = Lendo { $crew }

reading-app = Lendo { $app }

checking-kiro-ide-kiro-crew-and-cli = Verificando Kiro IDE, Kiro Crew e CLI…

checking-installed-app-and-cli = Verificando o aplicativo e a CLI instalados…

checking-cli = Verificando CLI…

checking-installed-app = Verificando o aplicativo instalado…

no-providers-yet-add-one-to-start-reading-limits = Ainda não há provedores. Adicione um para começar a ler os limites.

is-off-so-it-doesn-t-appear-in-the-minibar-or-tray-turn-it-on-to = { $v0 } está desativado e não aparece no minibar nem na bandeja. Ative para começar a ler o uso.

replace-chatgpt-logo-with-codex = Substituir logo do ChatGPT pelo Codex

limits-only = Somente limites

delete-provider = Excluir provedor

display-name = Nome de exibição

shown-on-popup-tabs-home-cards-the-tray-and-notifications = Mostrado nas abas do popup, nos cartões do Início, na bandeja e nas notificações.

up-to-three-letters-leave-empty-to-use-the-name-s-initials-shown = Até três letras. Deixe em branco para usar as iniciais do nome. Aparece quando mais de uma instância do provedor está ativada.

auto-uses-a-neutral-plate-that-follows-the-theme = Automático usa um fundo neutro que segue o tema.

show-on-home = Mostrar no Início

its-provider-tab-stays-available-when-hidden-from-home = A aba do provedor continua disponível quando oculto do Início.

credential = Credencial

signed-in-as-identity = Conectado como { $identity }

saved-in-windows-user-storage = Salva no armazenamento do usuário do Windows

paste-a-sessionkey-or-an-oauth-access-token = Cole uma sessionKey ou um token de acesso OAuth.

reads-limits-only-minibar-cannot-refresh-a-pasted-credential = Lê apenas os limites. O Minibar não pode renovar uma credencial colada.

replace-credential = Substituir credencial

add-credential = Adicionar credencial

signed-in-account = Conta conectada

not-signed-in-yet-or-no-limits-read-so-far = Ainda não conectado, ou nenhum limite lido até agora.

account = Conta

runtime = Ambiente de execução

minibar-finds-these-automatically = { $v0 } O Minibar encontra estes itens automaticamente.

in-use = Em uso

found = Encontrado

copy-path = Copiar caminho

open-folder = Abrir pasta

path-copied = Caminho copiado.

not-installed-or-installed-somewhere-minibar-doesn-t-look = Não instalado, ou instalado em um local que o Minibar não verifica.

not-found = Não encontrado

choose-folder = Escolher pasta…

select-folder = Selecionar pasta

choose-folder-1838a4 = Escolher pasta

only-needed-if-automatic-detection-misses-your-install = Necessário apenas se a detecção automática não encontrar a instalação.

source = Fonte

config-folder-reads-the-claude-code-login-in-claude-config-dir-ma = Pasta de configuração lê o login do Claude Code em CLAUDE_CONFIG_DIR. Credencial manual lê apenas os limites a partir de uma credencial colada.

config-folder = Pasta de configuração

manual = Manual

passed-to-the-cli-as-env-leave-empty-to-use = Passado à CLI como { $env }. Deixe em branco para usar { $v0 }.

the-standard-folder = a pasta padrão

a-folder-minibar-creates-for-this-instance = uma pasta que o Minibar cria para esta instância

other-already-reads-this-folder-two-instances-must-not-share-a-lo = { $other } já lê esta pasta. Duas instâncias não devem compartilhar um login, ou o uso será contado duas vezes.

automatic-activation = Ativação automática

starts-this-account-s-5-hour-window-when-it-resets-using-its-own = Inicia a janela de 5 horas desta conta quando ela é redefinida, usando seu próprio login. Agendamentos e pausas ficam em Ativação de limites.

usage-statistics = Estatísticas de uso

scans-this-instance-s-local-history-for-its-usage-card-turn-off-t = Lê o histórico local desta instância para o cartão de uso. Desative para parar totalmente a coleta.

opencode-sign-in-or-local-history = Login no OpenCode ou histórico local

found-in-opencode-auth-environment-a-saved-key-or-local-history = Encontrado na autenticação do OpenCode, no ambiente, em uma chave salva ou no histórico local.

nothing-found-in-opencode-auth-environment-or-local-history = Nada encontrado na autenticação do OpenCode, no ambiente ou no histórico local.

replace-key = Substituir chave

optional-only-needed-without-opencode-sign-in-on-this-pc = Opcional. Necessário apenas sem login no OpenCode neste PC.

add-the-key-of-the-account-this-instance-tracks = Adicione a chave da conta que esta instância acompanha.

keys = Chaves

a-management-key-shows-credit-balance-and-usage-history-api-keys = Uma chave de gerenciamento mostra saldo de créditos e histórico de uso. Chaves de API mostram os gastos por chave.

could-not-read-the-saved-key-reopen-this-page-to-retry = Não foi possível ler a chave salva. Reabra esta página para tentar novamente.

credit-balance-and-account-wide-usage-history = Saldo de créditos e histórico de uso de toda a conta

not-added-add-one-to-see-credit-balance-and-usage-history = Não adicionada. Adicione uma para ver o saldo de créditos e o histórico de uso.

last-updated = Última atualização { $v0 }, { $v1 }

no-api-keys-yet-add-one-to-track-spend-per-key = Ainda não há chaves de API. Adicione uma para acompanhar os gastos por chave.

key = Chave

spend = Gastos

limit = Limite

error-reopen-this-page-to-retry = { $error }. Reabra esta página para tentar novamente.

could-not-read-key = Não foi possível ler a chave

not-saved = Não salva

unnamed = Sem nome

of-the-limit = { $v0 } do limite de { $v1 }

no-spend-limit-key-spend-account-credits-purchased = Sem limite de gastos. Gastos da chave: { $v0 }. Créditos comprados da conta: { $v1 }.

none = Nenhum

add-key = Adicionar chave

openrouter-account-credentials-are-configured = As credenciais da conta OpenRouter estão configuradas

saved-api-key = Chave de API salva

saved-credential = Credencial salva

opencode-auth-json-or-local-database = auth.json ou banco de dados local do OpenCode

that-doesn-t-look-like-an-openrouter-key-keys-start-with-sk-or = Isso não parece uma chave do OpenRouter. As chaves começam com sk-or-.

codex-cli-folder = Pasta da CLI do Codex

claude-code-cli-folder = Pasta da CLI do Claude Code

cursor-app-folder = Pasta do aplicativo Cursor

grok-cli-folder = Pasta da CLI do Grok

# src/settings_window/providers.rs
command-code-cli-folder = Pasta da CLI do Command Code

kiro-ide-folder = Pasta do Kiro IDE

kiro-crew-app-path = Caminho do aplicativo Kiro Crew

kiro-cli-folder = Pasta da CLI do Kiro

reads-the-signed-in-codex-cli-or-desktop-app = Lê a CLI ou o aplicativo desktop do Codex conectado.

reads-your-existing-claude-code-login = Lê seu login existente do Claude Code.

reads-the-signed-in-cursor-app-for-this-billing-cycle = Lê o aplicativo Cursor conectado para este ciclo de cobrança.

reads-zen-auth-and-local-opencode-history = Lê a autenticação do Zen e o histórico local do OpenCode.

reads-go-quota-windows-and-local-opencode-history = Lê as janelas de cota do Go e o histórico local do OpenCode.

reads-api-key-usage-and-spend-limits-a-management-key-also-enable = Lê o uso das chaves de API e os limites de gastos. Uma chave de gerenciamento também permite ver histórico de uso e saldo de créditos.

reads-subscription-quota-from-your-existing-official-agy-windows = Lê a cota da assinatura a partir do seu login oficial existente do agy no Windows.

reads-supergrok-subscription-credits-from-your-existing-official = Lê os créditos da assinatura SuperGrok a partir do seu login oficial existente da CLI do Grok.

# src/settings_window/providers.rs
reads-command-code-5h-weekly-and-monthly-credits-from-your-existin = Lê os créditos de 5 horas, semanais e mensais do Command Code a partir do seu login oficial existente da CLI.

fetches-kiro-s-live-monthly-credits-with-its-shared-sign-in-recog = Obtém os créditos mensais atuais do Kiro com seu login compartilhado; reconhece instalações do IDE, Crew e CLI.

codex-desktop-app = Aplicativo desktop do Codex

codex-cli = Codex CLI

claude-desktop-app = Aplicativo desktop do Claude

claude-code-cli = Claude Code CLI

cursor-app = Aplicativo Cursor

antigravity-app = Aplicativo Antigravity

grok-cli = Grok CLI

# src/settings_window/providers.rs
command-code-cli = Command Code CLI

kiro-ide = Kiro IDE

kiro-crew = Kiro Crew

kiro-cli = Kiro CLI

supports-one-instance = { $v0 } permite uma instância

numbers = Números

progress-bars = Barras de progresso

rings = Anéis

reset-time = Horário de redefinição

countdown = Contagem regressiva

status = Status

fixed = Fixa

app-accent = Cor de destaque do aplicativo

percentages-as-digits = Porcentagens em números

one-bar-per-indicator = Uma barra por indicador

nested-rings-one-per-indicator = Anéis concêntricos, um por indicador

when-the-limit-resets = Quando o limite é redefinido

time-left-until-the-reset = Tempo restante até a redefinição

used = Usado

remaining = Restante

app-icon = Ícone do aplicativo

widget = Widget { $v0 }

the-codex-minibar-icon = O ícone do Codex Minibar

style-no-indicators = { $style } · Sem indicadores

widget-removed = Widget removido

undo = Desfazer

tray-icon = Ícone da bandeja

shows-the-app-icon = Mostra o ícone do aplicativo.

add-widget = Adicionar widget

add-app-icon = Adicionar ícone do aplicativo

move-up = Mover para cima

move-down = Mover para baixo

edit = Editar

duplicate = Duplicar

shows-the-codex-minibar-icon-in-the-notification-area-it-has-no-i = Mostra o ícone do Codex Minibar na área de notificação. Não há indicadores para configurar.

style = Estilo

up-to-max-indicators-quotas-drawn-in-this-order-expand-one-to-cha = Até { $max_indicators } cotas, desenhadas nesta ordem. Expanda uma para alterá-la.

indicators = Indicadores

remove-widget = Remover widget

add-indicator = Adicionar indicador

unsupported = Não compatível: { $v0 }

remove-indicator = Remover indicador

unsupported-bba2a8 = Não compatível ({ $v0 })

unavailable-0c5d75 = Indisponível ({ $v0 })

metric = Métrica

show = Mostrar

color = Cor

fixed-color = Cor fixa

troubleshooting-could-not-start = Não foi possível iniciar o diagnóstico

choose-which-installed-ai-tool-should-investigate-the-problem = Escolha qual ferramenta de IA instalada deve investigar o problema.

open-terminal = Abrir terminal

version-is-available = { $version } disponível

update-now = Atualizar agora

missing-a-provider = Falta um provedor?

ask-for-it-or-build-it-yourself = Solicite ou crie você mesmo.

request-a-provider = Solicitar um provedor

contribute-one = Contribuir com um

disabled = Desativado

update-complete = Atualização concluída

now-running-version = Executando agora { $version }.

past-24h = Últimas 24h

language = Idioma

applies-immediately-to-every-app-window-and-notification-auto-fol = Aplica-se imediatamente a todas as janelas e notificações do aplicativo. Automático segue o idioma de exibição do Windows.

auto-windows = Automático (Windows)

english = English

msg-russian = Русский

codex-minibar-settings = Configurações do Codex Minibar

welcome-to-codex-minibar = Bem-vindo ao Codex Minibar

exit = Sair

update-available-67fd3a = Atualização disponível

monthly-credits-976559 = Créditos mensais

msg-5h-session-de7ce8 = Sessão de 5h

name-weekly = { $name } semanal

tokens-94e0b9 = { $v0 }: { $v1 } tokens

requests-priced = { $v0 ->
    [one] { $v0 } solicitação · { $v1 } com preço
   *[other] { $v0 } solicitações · { $v1 } com preço
    }

requests = { $v0 ->
    [one] { $v0 } solicitação
   *[other] { $v0 } solicitações
    }

credits-338f52 = CRÉDITOS

cloud-session-credits = CRÉDITOS DE SESSÃO NA NUVEM

name-login-expires-in-days-left = { $days_left ->
    [one] O login de { $name } expira em { $days_left } dia
   *[other] O login de { $name } expira em { $days_left } dias
    }

name-error = Erro de { $name }

sessions = { $v0 ->
    [one] { $v0 } sessão
   *[other] { $v0 } sessões
    }

api-estimate = Estimativa da API

start-to-end = { $start } a { $end }

to = { $v0 } a { $v1 }

share-1-of-other = { $share }% de { $v0 } · { $other }

# application
excluded-other = Excluído · { $other }

# application
exclude-from-usage-stats = Clique para excluir das estatísticas

# application
include-in-usage-stats = Clique para incluir nas estatísticas

cost-885dc4 = custo

tokens-339143 = tokens

sessions-0e5e29 = { $v0 ->
    [one] · { $v0 } sessão
   *[other] · { $v0 } sessões
    }

api-key = Chave de API

api-keys = Chaves de API

name-deleted = { $name } excluído.

openrouter-api-key-no-longer-exists = A chave de API do OpenRouter não existe mais

openrouter-account-no-longer-exists = A conta OpenRouter não existe mais

credit = { $v0 } crédito

agy-cli-folder = Pasta da CLI do agy

folder-with-codex-exe-codex-cmd-or-codex-ps1-leave-empty-to-find = Pasta com codex.exe, codex.cmd ou codex.ps1. Deixe em branco para encontrar automaticamente.

folder-with-claude-exe-claude-cmd-or-claude-ps1-leave-empty-to-fi = Pasta com claude.exe, claude.cmd ou claude.ps1. Deixe em branco para encontrar automaticamente.

folder-with-cursor-exe-leave-empty-to-find-it-automatically-usage = Pasta com Cursor.exe. Deixe em branco para encontrar automaticamente. O uso continua vindo do perfil conectado.

folder-with-agy-exe-agy-cmd-or-agy-ps1-leave-empty-to-find-it-aut = Pasta com agy.exe, agy.cmd ou agy.ps1. Deixe em branco para encontrar automaticamente.

folder-with-grok-exe-grok-cmd-or-grok-ps1-leave-empty-to-find-it = Pasta com grok.exe, grok.cmd ou grok.ps1. Deixe em branco para encontrar automaticamente.

# src/settings_window/providers.rs
folder-with-cmdc-cmd-or-cmdc-ps1-leave-empty-to-find-it-automati = Pasta com cmdc.cmd ou cmdc.ps1. Deixe em branco para encontrar automaticamente.

folder-containing-kiro-exe-or-the-executable-itself-leave-empty-t = Pasta com Kiro.exe ou o próprio executável. Deixe em branco para encontrar automaticamente.

folder-containing-kirocrew-exe-or-the-executable-itself-leave-emp = Pasta com KiroCrew.exe ou o próprio executável. Deixe em branco para detectar automaticamente instalações para um usuário ou todos os usuários.

folder-containing-kiro-cli-exe-or-the-executable-itself-leave-emp = Pasta com kiro-cli.exe ou o próprio executável. Deixe em branco para encontrar automaticamente.

off-45080e = { $v0 } (desativado)

each-widget-is-one-icon-in-the-notification-area-indicators-show = Cada widget é um ícone na área de notificação. Os indicadores mostram a cota de um provedor como números, barras, anéis ou relógio de redefinição.

a-reset-clock-follows-one-quota = Um relógio de redefinição acompanha uma cota.

ai-tool = Ferramenta de IA

msg-7-days = 7 dias

msg-30-days = 30 dias

msg-90-days = 90 dias

yellow = Amarelo

custom = Personalizado

not-available-for-manual-credentials-switch-source-to-config-fold = Indisponível para credenciais manuais. Mude Fonte para Pasta de configuração.

opencode-s-local-history-is-tracked-by-the-first-opencode-instanc = O histórico local do OpenCode é acompanhado pela primeira instância do OpenCode.

this-provider-has-no-local-usage-history = Este provedor não tem histórico local de uso.

this-provider-has-no-session-window-to-start = Este provedor não tem janela de sessão para iniciar.

this-provider-does-not-use-a-config-folder = Este provedor não usa pasta de configuração.

could-not-save-provider-settings-error-restoring-its-previous-cre = Não foi possível salvar as configurações do provedor ({ $error }); a restauração da credencial anterior também falhou ({ $rollback_error }).

msg-5h-7d = 5h  ·  { $v0 }  ·  { $v1 }
    7d  ·  { $v2 }  ·  { $v3 }

api-key-count = { $v0 ->
    [one] { $v0 } chave de API
   *[other] { $v0 } chaves de API
    }

widen-home-and-usage-drag-home-blocks-between-columns-provider-ta = Amplie o Início e o Uso. Arraste blocos do Início entre colunas; as abas dos provedores continuam compactas.

a-possible-codex-reset-is-scheduled-for-when-in-countdown = Uma possível redefinição do Codex está agendada para { $when } (em { $countdown })

label-possible-reset-on-when-in-countdown = { $label }: possível redefinição em { $when } (em { $countdown })

new-codex-reset-info = Novas informações de redefinição do Codex

login-expires-soon = O login de { $v0 } expira em breve

it-stops-renewing-on-open-minibar-and-choose-sign-in-again = A renovação para em { $v0 }. Abra o Minibar e escolha Entrar novamente.

could-not-clear-usage-data-error = Não foi possível limpar os dados de uso: { $error }

succeeded-at = { $v0 } concluído às { $v1 }

failed-at-error = { $v0 } falhou às { $v1 }: { $error }

key-39df89 = Chave { $v0 }

expired = Expirado

jan = jan

feb = fev

mar = mar

apr = abr

may = mai

jun = jun

jul = jul

aug = ago

sep = set

oct = out

nov = nov

dec = dez

quota-percent-used = { $value }% usado

quota-percent-left = { $value }% restante

cloud-amount-used = { $amount } de { $limit } usados

cloud-amount-left = { $amount } de { $limit } restantes

range-24h = 24h

range-7d = 7d

range-30d = 30d

range-90d = 90d

home-usage-title = Uso

# Activity chart grouping
split-type = Tipo

split-by-token-type = Dividir barras por tipo de token

# OpenRouter key administration
could-not-reach-openrouter = Não foi possível conectar ao OpenRouter
openrouter-keys-title = Chaves
openrouter-keys-new = Nova chave
openrouter-keys-new-title = Nova chave
openrouter-keys-back = Voltar
openrouter-keys-updating = Atualizando…
openrouter-keys-loading = Carregando chaves…
openrouter-keys-load-failed = Não foi possível carregar as chaves
openrouter-keys-retry = Tentar novamente
openrouter-keys-empty = Esta conta ainda não tem chaves.
openrouter-keys-this-app = Este app
openrouter-keys-no-limit = Sem limite
openrouter-keys-of-limit = { $amount } de { $limit }
openrouter-keys-resets-daily = reinicia diariamente
openrouter-keys-resets-weekly = reinicia semanalmente
openrouter-keys-resets-monthly = reinicia mensalmente
openrouter-keys-expires-on = expira em { $date }
openrouter-keys-expired-on = expirou em { $date }
openrouter-keys-today = hoje { $amount }
openrouter-keys-usage-breakdown = Hoje { $today } · semana { $week } · mês { $month } · total { $total }
openrouter-keys-show-all = Mostrar todas as chaves ({ $count })
openrouter-keys-show-fewer = Mostrar menos
openrouter-keys-spending-limit = Limite de gastos
openrouter-keys-limit-hint = Deixe vazio para não ter limite.
openrouter-keys-resets = Reinicia
openrouter-keys-reset-never = Nunca
openrouter-keys-reset-daily = Diariamente
openrouter-keys-reset-weekly = Semanalmente
openrouter-keys-reset-monthly = Mensalmente
openrouter-keys-byok = Contar uso BYOK no limite
openrouter-keys-enabled = Ativada
openrouter-keys-delete = Excluir
openrouter-keys-delete-tracked = O Minibar também deixará de acompanhá-la.
openrouter-keys-saving = Salvando…
openrouter-keys-delete-title = Excluir “{ $name }”?
openrouter-keys-delete-message = Tudo o que ainda usa esta chave para de funcionar imediatamente. Isso não pode ser desfeito; desativar a chave é reversível.
openrouter-keys-keep = Manter chave
openrouter-keys-delete-confirm = Excluir chave
openrouter-keys-deleting = Excluindo…
openrouter-keys-name = Nome
openrouter-keys-name-placeholder = ex.: laptop-cursor
openrouter-keys-name-required = Digite um nome para a chave.
openrouter-keys-invalid-amount = Digite um valor em dólares, como 25 ou 12.50.
openrouter-keys-expires = Expira
openrouter-keys-expires-1-hour = 1 hora
openrouter-keys-expires-1-day = 1 dia
openrouter-keys-expires-7-days = 7 dias
openrouter-keys-expires-30-days = 30 dias
openrouter-keys-expires-90-days = 90 dias
openrouter-keys-expires-180-days = 180 dias
openrouter-keys-expires-1-year = 1 ano
openrouter-keys-expires-never = Sem expiração
openrouter-keys-track = Acompanhar esta chave no Minibar
openrouter-keys-create = Criar chave
openrouter-keys-creating = Criando…
openrouter-keys-created = Chave criada
openrouter-keys-copy = Copiar chave
openrouter-keys-copied = Copiada
openrouter-keys-done = Concluído
openrouter-keys-once-title = Você não verá esta chave novamente
openrouter-keys-once-message = O OpenRouter mostra uma nova chave apenas uma vez. Copie-a agora.
openrouter-keys-pinned = O pop-up fica aberto até você clicar em Concluído.
openrouter-keys-tracking = Adicionando a chave ao Minibar…
openrouter-keys-tracked = O Minibar agora acompanha esta chave.
openrouter-keys-track-failed = Não foi possível adicionar a chave ao Minibar: { $error }
openrouter-keys-no-management-key = Esta conta não tem chave de gerenciamento.
openrouter-keys-management-key-rejected = O OpenRouter rejeitou a chave de gerenciamento. Substitua-a nas Configurações.
openrouter-keys-not-a-management-key = Esta chave não pode gerenciar outras chaves. Use uma chave de gerenciamento.
openrouter-keys-key-not-found = Esta chave não existe mais.
openrouter-keys-untrack = Remover do app
openrouter-keys-untracking = Removendo…
openrouter-keys-untrack-title = Parar de acompanhar “{ $name }”?
openrouter-keys-untrack-message = O Minibar esquece esta chave. Ela continua funcionando no OpenRouter.
openrouter-keys-tracked-here = O Minibar acompanha esta chave.
openrouter-keys-manage-hint = Adicione uma chave de gerenciamento nas Configurações para criar, editar e excluir chaves.

# src/settings_window/notifications.rs
notification-sounds = Sons de notificação

# src/settings_window/notifications.rs
plays-a-short-sound-with-each-notification = Toca um som curto a cada notificação

use-reset = Usar redefinição
use-reset-short = Usar
reset-using = Aplicando…
reset-confirm-message = Usar uma redefinição acumulada nesta conta? Isso limpa os limites elegíveis e não pode ser desfeito.
reset-applied = Redefinição aplicada. Os limites elegíveis foram limpos.
reset-nothing-to-reset = Nada para redefinir agora.
reset-no-credit = Não há mais redefinições disponíveis.
reset-already-redeemed = Esta redefinição já foi usada.
reset-unconfirmed = Não foi possível confirmar a redefinição. Tente novamente para verificar a mesma solicitação.
reset-applied-refresh-failed = Redefinição aplicada, mas os novos limites não foram carregados. Atualize para verificar.
reset-in-progress = Uma redefinição já está em andamento nesta conta.
reset-cooldown = As redefinições estão em espera. Tente mais tarde.
reset-rate-limited = Muitas solicitações de redefinição. Tente mais tarde.
reset-sign-in-again = Entre novamente no Claude para usar redefinições.
reset-cli-login-required = Use o login do Claude CLI da mesma conta para aplicar redefinições.
reset-invalid-grant = O Claude retornou uma redefinição inválida.

# src/updater.rs
update-downloading = Baixando atualização

# src/updater.rs
update-download-progress = { $downloaded } de { $total }

# src/updater.rs
update-downloaded = { $downloaded } baixados

# src/updater.rs
update-installing = Instalando atualização

# src/updater.rs
update-unpacking = Descompactando arquivos…

# src/updater.rs
update-restarting = Reiniciando o Codex Minibar

# src/updater.rs
update-restarting-body = O app será reaberto em instantes.

# src/settings_window/general.rs
provider-status = Status dos provedores

# src/settings_window/general.rs
show-provider-incidents = Mostrar incidentes dos provedores

# src/settings_window/general.rs
show-provider-incidents-description = Verifica as páginas de status públicas do Codex, Claude e Cursor a cada 5 minutos e mostra incidentes em andamento no pop-up

# src/settings_window/general.rs
notify-about-new-incidents = Notificar sobre novos incidentes

# src/settings_window/general.rs
notify-about-new-incidents-description = Mostra uma notificação quando uma página de status relata um novo incidente de um provedor ativado

# src/popup_window/ui/cards.rs
open-status-page = Abrir página de status

# src/notifications.rs
provider-incident-title = { $provider }: { $impact }

# src/popup_window/ui/cards.rs
status-since = Desde { $time }

# src/provider_status.rs
status-impact-maintenance = Manutenção

# src/provider_status.rs
status-impact-degraded = Desempenho degradado

# src/provider_status.rs
status-impact-partial-outage = Interrupção parcial

# src/provider_status.rs
status-impact-major-outage = Interrupção grave

# src/provider_status.rs
status-stage-investigating = Investigando

# src/provider_status.rs
status-stage-identified = Identificado

# src/provider_status.rs
status-stage-monitoring = Monitorando

# src/provider_status.rs
status-stage-in-progress = Em andamento

# src/settings_window/notifications.rs
news-and-status = Novidades e status

# src/whats_new.rs
no-summary-for-this-version = Ainda não há um resumo para esta versão. As notas completas estão no GitHub.

# src/whats_new.rs
couldn-t-load-what-s-new = Não foi possível carregar as novidades. Verifique a conexão ou leia as notas no GitHub.

# src/whats_new.rs
full-release-notes = Notas completas

# src/settings_window/about.rs
show-what-s-new-after-updating = Mostrar novidades após atualizar

# src/settings_window/about.rs
a-short-summary-of-the-release-once-it-s-installed = Um breve resumo da versão, logo após a instalação.

# src/settings_window/about.rs
view = Ver

# src/settings_window/general.rs
popup-shortcut = Atalho do popup
popup-shortcut-description = Pressione-o em qualquer lugar do Windows para mostrar ou ocultar o popup.
shortcut-not-set = Não definido
press-a-shortcut = Pressione um atalho…
popup-shortcut-recording-hint = Esc cancela. Backspace remove o atalho.
popup-shortcut-needs-modifier = Inclua Ctrl, Alt ou Win, ou use F13–F24.
popup-shortcut-in-use = Outro aplicativo já usa este atalho. Escolha outro.
clear-shortcut = Remover atalho
taskbar = Barra de tarefas
preview = Pré-visualização
taskbar-page-caption = Widgets de uso ao vivo desenhados dentro da barra de tarefas do Windows, ao lado do relógio. Eles seguem o tema da barra de tarefas; clique em um para abrir a janela no provedor dele.
taskbar-show-widgets = Mostrar widgets na barra de tarefas
taskbar-show-widgets-description = Desenha os widgets abaixo dentro da barra de tarefas do Windows.
taskbar-position = Posição
taskbar-position-description = Onde os widgets ficam na barra de tarefas. O início combina melhor com ícones centralizados.
taskbar-position-tray = Ao lado da área de notificação
taskbar-position-start = No início da barra de tarefas
taskbar-position-in-tray = Dentro da área de notificação (experimental)
taskbar-widgets = Widgets
taskbar-no-widgets = Nenhum widget ainda
taskbar-no-widgets-description = Adicione um widget para mostrar uma cota na barra de tarefas.
taskbar-show-provider-icon = Mostrar o ícone do provedor
taskbar-style-glance = Resumo
taskbar-style-glance-hint = O ícone do provedor ao lado de duas linhas de texto, como o botão do clima.
taskbar-style-rings-hint = Um anel de progresso por cota, com o valor ao lado.
taskbar-style-bars-hint = Barras de progresso finas ao lado do ícone do provedor.
taskbar-menu-open = Abrir o Codex Minibar
taskbar-menu-settings = Configurações dos widgets da barra de tarefas
taskbar-menu-hide = Ocultar os widgets da barra de tarefas
taskbar-card-left = Restam { $value }
taskbar-card-used = { $value } usado
taskbar-card-resets = Redefine em { $time } · { $clock }
taskbar-card-no-data = Aguardando dados
short-5h = 5 h
short-7d = 7 d
taskbar-style-nested = Anéis aninhados
taskbar-style-dials = Mostradores
taskbar-style-meters = Medidores
taskbar-style-nested-hint = Anéis concêntricos no espaço de um, com os valores ao lado se quiser.
taskbar-style-dials-hint = Um pequeno anel por cota com o número dentro.
taskbar-style-meters-hint = Colunas de nível finas, como um indicador de bateria.
taskbar-show-provider-icon-description = A marca do provedor no início do widget.
taskbar-show-labels = Mostrar rótulos
taskbar-show-labels-description = Nomes curtos como 5 h e 7 d ao lado dos valores.
taskbar-show-values = Mostrar porcentagens
taskbar-show-values-description = As porcentagens das cotas no widget.
taskbar-show-pace = Mostrar ritmo
taskbar-show-pace-description = Uma marca onde o uso estaria agora com um consumo uniforme.
taskbar-menu-edit = Editar este widget
taskbar-content = Conteúdo
taskbar-content-hint = O que o widget escreve ao lado do gráfico.
taskbar-reset-time = Horário de redefinição
taskbar-reset-time-description = Quando cada cota é redefinida.
taskbar-reset-off = Desativado
taskbar-reset-countdown = Tempo restante
taskbar-reset-clock = Horário
taskbar-cycle = Alternar widgets com a roda do mouse
taskbar-cycle-description = Mostra um widget por vez; role a roda sobre ele para passar ao próximo.
taskbar-card-tokens = { $count } tokens
taskbar-click = Clique abre
taskbar-click-description = A página do popup que um clique no widget abre.
taskbar-click-nearest = Início, ou a aba se não estiver no Início
taskbar-click-tab = Aba do provedor
taskbar-click-home = Início

# src/settings_window/notifications.rs
follow-do-not-disturb = Pausar no modo Não incomodar

# src/settings_window/notifications.rs
follow-do-not-disturb-description = Enquanto o Não incomodar ou o Foco do Windows estiver ativo, as notificações são ignoradas e os sons ficam desligados. Resultados de ações suas continuam aparecendo.

# src/settings_window/advanced.rs
performance = Desempenho

# src/settings_window/advanced.rs
game-mode = Modo jogo

# src/settings_window/advanced.rs
game-mode-description = Verifica limites e uso com menos frequência, roda com prioridade baixa e pausa as notificações. Automático o ativa quando um jogo vem para a frente e o mantém até o jogo fechar.

# src/settings.rs
game-mode-off = Desligado

# src/settings.rs
game-mode-automatic = Automático

# src/settings.rs
game-mode-always = Sempre

# src/settings_window/notifications.rs
notify-game-mode-changes = Mudanças do modo jogo

# src/settings_window/notifications.rs
notify-game-mode-changes-description = Avisa quando o modo jogo liga ou desliga

# src/notifications.rs
game-mode-on-title = Modo jogo ligado

# src/notifications.rs
game-mode-on-body = Limites verificados com menos frequência e notificações pausadas

# src/notifications.rs
game-mode-on-game-body = { $game } está em execução. Limites verificados com menos frequência e notificações pausadas

# src/notifications.rs
not-a-game = Não é um jogo

# src/settings_window/advanced.rs
add-a-game = Adicionar jogo

# src/settings_window/advanced.rs
add-a-game-description = Jogos conhecidos pela Game Bar do Windows e jogos da Steam, Epic, GOG e Xbox são detectados sozinhos. Adicione aqui qualquer outro programa.

# src/settings_window/advanced.rs
never-a-game-path = Nunca é um jogo · { $path }

# src/notifications.rs
game-mode-off-title = Modo jogo desligado

# src/notifications.rs
game-mode-off-body = Verificações e notificações normais retomadas

# src/settings_window/command_palette.rs
command-palette = Command Palette

# src/settings_window/command_palette.rs
stream-deck = Stream Deck

# src/settings_window/command_palette.rs
install = Instalar

# src/settings_window/command_palette.rs
cmdpal-extension = Extensão da Command Palette

# src/settings_window/command_palette.rs
cmdpal-description = Adiciona o Minibar à Command Palette do PowerToys: digite um provedor para ver os limites ou fixe faixas no Dock. A instalação pede aprovação de administrador.

# src/settings_window/command_palette.rs
cmdpal-installed-hint = Instalada. Digite “minibar” ou o nome de um provedor na Command Palette, ou fixe faixas em Editar Dock.

# src/settings_window/command_palette.rs
cmdpal-downloading = Baixando a extensão…

# src/settings_window/command_palette.rs
cmdpal-installing = Aguardando o instalador. Aprove a solicitação de administrador para continuar.

# src/settings_window/command_palette.rs
cmdpal-installing-short = Instalando…

# src/settings_window/command_palette.rs
cmdpal-removing = Removendo a extensão…

# src/settings_window/command_palette.rs
cmdpal-failed = Não foi possível instalar ou remover a extensão.

# src/settings_window/command_palette.rs
cmdpal-reinstall = Reinstalar

# src/settings_window/command_palette.rs
cmdpal-dock-bands = Faixas do Dock

# src/settings_window/command_palette.rs
cmdpal-dock-bands-caption = Cada faixa aparece nas configurações do Dock da Command Palette. Fixe-a em Editar Dock e escolha onde ela fica.

# src/settings_window/command_palette.rs
cmdpal-no-bands = Nenhuma faixa

# src/settings_window/command_palette.rs
cmdpal-no-bands-description = Adicione uma faixa para fixar os limites de um provedor no Dock.

# src/settings_window/command_palette.rs
cmdpal-add-band = Adicionar faixa

# src/settings_window/command_palette.rs
cmdpal-band = Faixa { $v0 }

# src/settings_window/command_palette.rs
cmdpal-band-removed = Faixa removida

# src/settings_window/command_palette.rs
cmdpal-remove-band = Remover faixa

# src/settings_window/command_palette.rs
cmdpal-content-hint = O Dock mostra uma faixa como um título acima de um subtítulo.

# src/settings_window/command_palette.rs
cmdpal-indicators-hint = Até 2 cotas: uma preenche o título e o subtítulo; duas ocupam uma linha cada.

# src/popup_window/ui/cards.rs
tibo-banked-reset = Tibo Banked Reset™

# src/popup_window/ui/cards.rs
banked-reset-credit = Redefinição guardada

# src/popup_window/ui/cards.rs
before-when = Antes de { $when }

# src/popup_window/ui/cards.rs
time-ago = há { $time }

# src/popup_window/ui/cards.rs
new-codex-banked-reset-info = Novas informações de redefinição guardada do Codex

# src/popup_window/ui/cards.rs
a-possible-codex-reset-is-expected-before-when-within-countdown = Uma possível redefinição do Codex é esperada antes de { $when } (em até { $countdown })

# src/popup_window/ui/cards.rs
label-possible-reset-before-when-within-countdown = { $label }: possível redefinição antes de { $when } (em até { $countdown })

# src/popup_window/ui/cards.rs
time-late = { $time } de atraso

# Floating usage panel
popup-floating = Painel flutuante
popup-floating-description = Mantenha o painel aberto e arraste o cabeçalho para outra posição ou monitor.
popup-always-on-top = Sempre no topo
popup-always-on-top-description = Manter o painel acima de outros aplicativos e aberto ao alternar entre eles.
popup-position = Posição do painel
popup-position-description = A posição é lembrada entre sessões.
popup-reset-position = Redefinir posição
popup-drag-header = Arraste para mover
popup-close = Fechar painel

show-popup-action-bar = Mostrar barra de ações do popup
show-popup-action-bar-description = Mostrar o título e as ações no popup normal. Painéis flutuantes sempre mostram esta barra.
