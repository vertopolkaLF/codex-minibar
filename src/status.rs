//! `codex-minibar status`: quota readouts for status bars and scripts.
//!
//! The command is a thin client of the running app, like the Stream Deck
//! plugin: it reads the loopback bridge endpoint (`streamdeck-bridge.json`
//! next to the settings file), asks for a [`Report`] and prints it as plain
//! text, JSON, a Waybar custom-module object or a Polybar/i3blocks line.
//! `--watch` keeps printing a fresh line, which Waybar (`exec` without
//! `interval`) and Polybar (`tail = true`) consume as a stream.
//!
//! The same [`Report`] is published over D-Bus on Linux for panel widgets.

use std::{
    io::{self, BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{instances::ProviderId, limits::ProviderLimits, provider_registry, widget_data};

/// The first command-line argument that selects this command.
pub const COMMAND: &str = "status";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_WATCH_SECONDS: u64 = 30;
const MIN_WATCH_SECONDS: u64 = 2;
const DEFAULT_WARNING_PERCENT: u8 = 80;
const DEFAULT_CRITICAL_PERCENT: u8 = 95;

/// Everything a status client shows: one entry per enabled instance with a
/// quota, in display order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub observed_at: DateTime<Utc>,
    pub providers: Vec<ProviderReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderReport {
    /// Instance id; the primary instance keeps the driver id.
    pub id: String,
    /// Driver id, e.g. `claude`.
    pub kind: String,
    pub name: String,
    pub plan: Option<String>,
    /// The tray's default quotas on one line, as the app's language and
    /// remaining/used preference write them: `5h 46% · 7d 25%`.
    pub headline: String,
    pub quotas: Vec<QuotaReport>,
    /// Today's spend or tokens, when the provider records usage.
    pub today: Option<String>,
    /// PNG marks tinted for light and dark panels.
    pub icon: Option<IconReport>,
    pub sampled_at: DateTime<Utc>,
    /// The most used of the tray's default quotas, for warning colors.
    pub max_used_percent: Option<u8>,
    /// Raw quota windows of every resolved metric.
    pub metrics: Vec<MetricReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuotaReport {
    pub label: String,
    pub short_label: String,
    pub value: String,
    pub value_text: String,
    pub progress: Option<f32>,
    pub reset_in: Option<String>,
    pub reset_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconReport {
    pub light: String,
    pub dark: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricReport {
    pub id: String,
    pub label: String,
    pub used_percent: Option<u8>,
    pub remaining_percent: Option<u8>,
    pub resets_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<u32>,
}

/// Builds the report from the app's live limits. Runs inside the app.
pub fn report(limits: &ProviderLimits) -> Report {
    let payload = crate::cmdpal::payload(limits);
    let providers = payload
        .providers
        .into_iter()
        .map(|summary| {
            let provider = ProviderId::lookup(&summary.id);
            let snapshot = provider.map(|provider| widget_data::snapshot(provider, limits));
            let metrics = snapshot
                .as_ref()
                .map(|snapshot| {
                    snapshot
                        .metrics
                        .iter()
                        .map(|metric| MetricReport {
                            id: metric.id.clone(),
                            label: metric.label.clone(),
                            used_percent: metric.window.used_percent,
                            remaining_percent: metric.window.remaining_percent,
                            resets_at: metric.window.resets_at,
                            duration_minutes: metric.window.duration_minutes,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let default_metrics = provider
                .map(|provider| provider_registry::descriptor(provider.kind()).default_tray_metrics)
                .unwrap_or_default();
            ProviderReport {
                max_used_percent: max_used_percent(&metrics, default_metrics),
                sampled_at: snapshot
                    .as_ref()
                    .map_or_else(Utc::now, |snapshot| snapshot.sampled_at),
                id: summary.id,
                kind: summary.kind,
                name: summary.name,
                plan: summary.plan,
                headline: summary.headline,
                quotas: summary
                    .quotas
                    .into_iter()
                    .map(|quota| QuotaReport {
                        label: quota.label,
                        short_label: quota.short_label,
                        value: quota.value,
                        value_text: quota.value_text,
                        progress: quota.progress,
                        reset_in: quota.reset_in,
                        reset_at: quota.reset_at,
                    })
                    .collect(),
                today: summary.today,
                icon: summary.icon.map(|icon| IconReport {
                    light: icon.light,
                    dark: icon.dark,
                }),
                metrics,
            }
        })
        .collect();
    Report {
        observed_at: Utc::now(),
        providers,
    }
}

/// The highest used percentage among `preferred` metrics, or among all of
/// them when none of the preferred ones resolved.
fn max_used_percent(metrics: &[MetricReport], preferred: &[&str]) -> Option<u8> {
    let preferred_max = metrics
        .iter()
        .filter(|metric| preferred.contains(&metric.id.as_str()))
        .filter_map(|metric| metric.used_percent)
        .max();
    preferred_max.or_else(|| {
        metrics
            .iter()
            .filter_map(|metric| metric.used_percent)
            .max()
    })
}

// ---------------------------------------------------------------------------
// Command line
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Text,
    Json,
    Waybar,
    Polybar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Options {
    format: Format,
    providers: Vec<String>,
    watch: Option<Duration>,
    warning: u8,
    critical: u8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: Format::Text,
            providers: Vec::new(),
            watch: None,
            warning: DEFAULT_WARNING_PERCENT,
            critical: DEFAULT_CRITICAL_PERCENT,
        }
    }
}

enum Parsed {
    Run(Options),
    Help,
}

const HELP: &str = "\
Usage: codex-minibar status [options]

Prints the quotas of the running Codex Minibar.

Options:
  -f, --format <text|json|waybar|polybar>
                         Output format (default: text). --json, --waybar and
                         --polybar are shortcuts.
  -p, --provider <id>    Only this provider or account (repeatable), e.g.
                         codex, claude, claude-2.
  -w, --watch [seconds]  Keep printing a line every interval (default 30).
      --warning <percent>
                         Used percent for the warning class (default 80).
      --critical <percent>
                         Used percent for the critical class (default 95).
  -h, --help             Show this help.

Waybar module:
  \"custom/minibar\": { \"exec\": \"codex-minibar status --waybar --watch\",
                       \"return-type\": \"json\" }
";

/// Whether this process was started as `codex-minibar status`.
pub fn is_cli_request() -> bool {
    std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument.eq_ignore_ascii_case(COMMAND))
}

/// Runs the command and returns the process exit code.
pub fn run_cli() -> i32 {
    attach_parent_console();
    let arguments = std::env::args().skip(2).collect::<Vec<_>>();
    let options = match parse_args(&arguments) {
        Ok(Parsed::Run(options)) => options,
        Ok(Parsed::Help) => {
            print!("{HELP}");
            return 0;
        }
        Err(error) => {
            eprintln!("codex-minibar status: {error:#}\n\n{HELP}");
            return 2;
        }
    };
    let mut stdout = io::stdout();
    loop {
        let result = fetch_report().map(|report| filter(report, &options.providers));
        let line = render(&result, &options);
        if !(line.is_empty() && matches!(options.format, Format::Text))
            && writeln!(stdout, "{line}")
                .and_then(|()| stdout.flush())
                .is_err()
        {
            // The bar closed the pipe.
            return 0;
        }
        let Some(interval) = options.watch else {
            return match result {
                Ok(_) => 0,
                Err(error) => {
                    if matches!(options.format, Format::Text) {
                        eprintln!("codex-minibar status: {error:#}");
                    }
                    1
                }
            };
        };
        std::thread::sleep(interval);
    }
}

fn parse_args(arguments: &[String]) -> Result<Parsed> {
    let mut options = Options::default();
    let mut index = 0;
    let value = |index: &mut usize, flag: &str| -> Result<String> {
        *index += 1;
        arguments
            .get(*index)
            .cloned()
            .with_context(|| format!("{flag} needs a value"))
    };
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        let (flag, inline) = match argument.split_once('=') {
            Some((flag, inline)) if flag.starts_with("--") => (flag, Some(inline.to_owned())),
            _ => (argument, None),
        };
        match flag {
            "-h" | "--help" => return Ok(Parsed::Help),
            "--json" => options.format = Format::Json,
            "--waybar" => options.format = Format::Waybar,
            "--polybar" => options.format = Format::Polybar,
            "--text" => options.format = Format::Text,
            "-f" | "--format" => {
                let format = match inline {
                    Some(inline) => inline,
                    None => value(&mut index, flag)?,
                };
                options.format = match format.to_ascii_lowercase().as_str() {
                    "text" => Format::Text,
                    "json" => Format::Json,
                    "waybar" => Format::Waybar,
                    "polybar" | "i3blocks" => Format::Polybar,
                    other => bail!("unknown format `{other}`"),
                };
            }
            "-p" | "--provider" => {
                let provider = match inline {
                    Some(inline) => inline,
                    None => value(&mut index, flag)?,
                };
                options.providers.push(provider);
            }
            "-w" | "--watch" => {
                let seconds = match inline {
                    Some(inline) => Some(inline),
                    None => match arguments.get(index + 1) {
                        Some(next) if !next.starts_with('-') => {
                            index += 1;
                            Some(next.clone())
                        }
                        _ => None,
                    },
                };
                let seconds = match seconds {
                    Some(seconds) => seconds
                        .parse::<u64>()
                        .with_context(|| format!("`{seconds}` is not a number of seconds"))?,
                    None => DEFAULT_WATCH_SECONDS,
                };
                options.watch = Some(Duration::from_secs(seconds.max(MIN_WATCH_SECONDS)));
            }
            "--warning" | "--critical" => {
                let raw = match inline {
                    Some(inline) => inline,
                    None => value(&mut index, flag)?,
                };
                let percent = raw
                    .trim_end_matches('%')
                    .parse::<u8>()
                    .ok()
                    .filter(|percent| *percent <= 100)
                    .with_context(|| format!("`{raw}` is not a percentage"))?;
                if flag == "--warning" {
                    options.warning = percent;
                } else {
                    options.critical = percent;
                }
            }
            other => bail!("unknown option `{other}`"),
        }
        index += 1;
    }
    Ok(Parsed::Run(options))
}

fn filter(mut report: Report, providers: &[String]) -> Report {
    if !providers.is_empty() {
        report.providers.retain(|provider| {
            providers
                .iter()
                .any(|wanted| wanted.eq_ignore_ascii_case(&provider.id))
        });
    }
    report
}

#[derive(Debug, Deserialize)]
struct Endpoint {
    host: String,
    port: u16,
    token: String,
}

fn endpoint_path() -> Result<PathBuf> {
    Ok(crate::settings::Settings::default_path()?
        .with_file_name(crate::streamdeck::ENDPOINT_FILE_NAME))
}

/// Asks the running app for its report over the loopback bridge.
fn fetch_report() -> Result<Report> {
    fetch_report_from(&endpoint_path()?)
}

fn fetch_report_from(path: &std::path::Path) -> Result<Report> {
    let endpoint = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Endpoint>(&bytes).ok())
        .ok_or_else(|| anyhow!("Codex Minibar is not running"))?;
    let address = (endpoint.host.as_str(), endpoint.port)
        .to_socket_addrs()
        .context("resolve the Codex Minibar bridge")?
        .next()
        .context("resolve the Codex Minibar bridge")?;
    // A stale endpoint file outlives the app; a refused connection means
    // nothing is listening any more.
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)
        .map_err(|_| anyhow!("Codex Minibar is not running"))?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let request = serde_json::json!({ "op": "status", "token": endpoint.token });
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .context("read the Codex Minibar bridge response")?;
    parse_response(&line)
}

#[derive(Deserialize)]
struct Response {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    report: Option<Report>,
}

fn parse_response(line: &str) -> Result<Report> {
    let response: Response =
        serde_json::from_str(line.trim()).context("unexpected Codex Minibar bridge response")?;
    if !response.ok {
        let error = response.error.unwrap_or_default();
        if error.starts_with("invalid request") {
            bail!("the running Codex Minibar is older than this command; restart it")
        }
        bail!("Codex Minibar refused the request: {error}")
    }
    response
        .report
        .context("Codex Minibar sent no status report")
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const SEPARATOR: &str = " \u{00b7} ";

fn render(result: &Result<Report>, options: &Options) -> String {
    match options.format {
        Format::Text => match result {
            Ok(report) => render_text(report),
            Err(_) => String::new(),
        },
        Format::Json => render_json(result),
        Format::Waybar => render_waybar(result, options),
        Format::Polybar => match result {
            Ok(report) => render_line(report),
            Err(_) => String::new(),
        },
    }
}

fn provider_line(provider: &ProviderReport) -> String {
    if provider.headline.is_empty() {
        provider.name.clone()
    } else {
        format!("{} {}", provider.name, provider.headline)
    }
}

/// One line for single-line bars: `Codex 5h 46% · 7d 25% | Claude 5h 12%`.
fn render_line(report: &Report) -> String {
    report
        .providers
        .iter()
        .map(provider_line)
        .collect::<Vec<_>>()
        .join(" | ")
}

/// A readable block per provider for a terminal.
fn render_text(report: &Report) -> String {
    if report.providers.is_empty() {
        return "No enabled providers with quotas.".to_owned();
    }
    let mut lines = Vec::new();
    for provider in &report.providers {
        let mut title = provider.name.clone();
        if let Some(plan) = provider.plan.as_deref().filter(|plan| !plan.is_empty()) {
            title.push_str(&format!(" ({plan})"));
        }
        lines.push(title);
        for quota in &provider.quotas {
            let mut line = format!("  {}: {}", quota.label, quota.value_text);
            if let Some(reset) = &quota.reset_in {
                line.push_str(&format!("{SEPARATOR}{reset}"));
            }
            lines.push(line);
        }
        if let Some(today) = &provider.today {
            lines.push(format!("  {today}"));
        }
    }
    lines.join("\n")
}

fn render_json(result: &Result<Report>) -> String {
    let value = match result {
        Ok(report) => serde_json::json!({
            "ok": true,
            "observed_at": report.observed_at,
            "providers": report.providers,
        }),
        Err(error) => serde_json::json!({ "ok": false, "error": format!("{error:#}") }),
    };
    value.to_string()
}

fn level(percent: Option<u8>, options: &Options) -> &'static str {
    match percent {
        Some(percent) if percent >= options.critical => "critical",
        Some(percent) if percent >= options.warning => "warning",
        _ => "normal",
    }
}

/// Waybar's custom-module JSON: text, Pango tooltip, class and percentage.
fn render_waybar(result: &Result<Report>, options: &Options) -> String {
    let value = match result {
        Err(error) => serde_json::json!({
            "text": "",
            "tooltip": pango_escape(&format!("{error:#}")),
            "class": ["offline"],
            "alt": "offline",
        }),
        Ok(report) => {
            let max = report
                .providers
                .iter()
                .filter_map(|provider| provider.max_used_percent)
                .max();
            let level = level(max, options);
            let tooltip = report
                .providers
                .iter()
                .map(|provider| {
                    let mut block = format!("<b>{}</b>", pango_escape(&provider.name));
                    if let Some(plan) = provider.plan.as_deref().filter(|plan| !plan.is_empty()) {
                        block.push_str(&format!(" {}", pango_escape(plan)));
                    }
                    for quota in &provider.quotas {
                        block.push_str(&format!(
                            "\n{}: {}",
                            pango_escape(&quota.label),
                            pango_escape(&quota.value_text)
                        ));
                        if let Some(reset) = &quota.reset_in {
                            block.push_str(&format!("{SEPARATOR}{}", pango_escape(reset)));
                        }
                    }
                    if let Some(today) = &provider.today {
                        block.push_str(&format!("\n{}", pango_escape(today)));
                    }
                    block
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            let mut value = serde_json::json!({
                "text": pango_escape(&render_line(report)),
                "tooltip": tooltip,
                "class": [level],
                "alt": level,
            });
            if let Some(max) = max {
                value["percentage"] = serde_json::json!(max);
            }
            value
        }
    };
    value.to_string()
}

fn pango_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// The release binary is a GUI-subsystem program on Windows, so it has no
/// console of its own. Write to the terminal that started it, if any; when
/// output is redirected, the inherited handles already work.
#[cfg(windows)]
fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
    };
    // SAFETY: plain Win32 calls without pointers.
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if handle.is_null() {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn options(values: &[&str]) -> Options {
        match parse_args(&args(values)).unwrap() {
            Parsed::Run(options) => options,
            Parsed::Help => panic!("unexpected help"),
        }
    }

    fn sample() -> Report {
        Report {
            observed_at: DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
            providers: vec![
                ProviderReport {
                    id: "codex".into(),
                    kind: "codex".into(),
                    name: "Codex".into(),
                    plan: Some("Plus".into()),
                    headline: "5h 46% · 7d 25%".into(),
                    quotas: vec![QuotaReport {
                        label: "5h session".into(),
                        short_label: "5h".into(),
                        value: "46%".into(),
                        value_text: "46% left".into(),
                        progress: Some(0.46),
                        reset_in: Some("2h 10m".into()),
                        reset_at: Some("14:00".into()),
                    }],
                    today: None,
                    icon: None,
                    sampled_at: DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
                    max_used_percent: Some(75),
                    metrics: Vec::new(),
                },
                ProviderReport {
                    id: "claude-2".into(),
                    kind: "claude".into(),
                    name: "Claude <Work>".into(),
                    plan: None,
                    headline: "5h 4%".into(),
                    quotas: Vec::new(),
                    today: Some("$1.20 today".into()),
                    icon: None,
                    sampled_at: DateTime::from_timestamp(1_800_000_000, 0).unwrap(),
                    max_used_percent: Some(96),
                    metrics: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn parses_formats_providers_and_watch() {
        assert_eq!(options(&[]), Options::default());
        let parsed = options(&["--waybar", "-p", "codex", "--provider=claude-2", "--watch"]);
        assert_eq!(parsed.format, Format::Waybar);
        assert_eq!(parsed.providers, ["codex", "claude-2"]);
        assert_eq!(
            parsed.watch,
            Some(Duration::from_secs(DEFAULT_WATCH_SECONDS))
        );
        let parsed = options(&["--watch", "10", "--format", "polybar"]);
        assert_eq!(parsed.watch, Some(Duration::from_secs(10)));
        assert_eq!(parsed.format, Format::Polybar);
        assert_eq!(
            options(&["--watch=0"]).watch,
            Some(Duration::from_secs(MIN_WATCH_SECONDS))
        );
        let parsed = options(&["--warning", "50%", "--critical=90"]);
        assert_eq!((parsed.warning, parsed.critical), (50, 90));
        assert!(matches!(parse_args(&args(&["-h"])), Ok(Parsed::Help)));
        assert!(parse_args(&args(&["--format", "xml"])).is_err());
        assert!(parse_args(&args(&["--warning", "101"])).is_err());
        assert!(parse_args(&args(&["--bogus"])).is_err());
        assert!(parse_args(&args(&["--provider"])).is_err());
    }

    #[test]
    fn filters_providers_by_instance_id() {
        let report = filter(sample(), &["CLAUDE-2".to_owned()]);
        assert_eq!(report.providers.len(), 1);
        assert_eq!(report.providers[0].id, "claude-2");
        assert_eq!(filter(sample(), &[]).providers.len(), 2);
    }

    #[test]
    fn renders_text_and_single_line() {
        let report = sample();
        assert_eq!(
            render_line(&report),
            "Codex 5h 46% · 7d 25% | Claude <Work> 5h 4%"
        );
        assert_eq!(
            render_text(&report),
            "Codex (Plus)\n  5h session: 46% left · 2h 10m\nClaude <Work>\n  $1.20 today"
        );
        assert_eq!(
            render_text(&Report::default()),
            "No enabled providers with quotas."
        );
    }

    #[test]
    fn waybar_output_escapes_markup_and_picks_the_worst_level() {
        let options = Options::default();
        let value: serde_json::Value =
            serde_json::from_str(&render_waybar(&Ok(sample()), &options)).unwrap();
        assert_eq!(value["class"][0], "critical");
        assert_eq!(value["percentage"], 96);
        assert!(
            value["text"]
                .as_str()
                .unwrap()
                .contains("Claude &lt;Work&gt;")
        );
        assert!(
            value["tooltip"]
                .as_str()
                .unwrap()
                .starts_with("<b>Codex</b> Plus\n5h session: 46% left")
        );

        let offline: serde_json::Value = serde_json::from_str(&render_waybar(
            &Err(anyhow!("Codex Minibar is not running")),
            &options,
        ))
        .unwrap();
        assert_eq!(offline["class"][0], "offline");
        assert_eq!(offline["text"], "");
    }

    #[test]
    fn levels_follow_thresholds() {
        let options = options(&["--warning", "50", "--critical", "90"]);
        assert_eq!(level(None, &options), "normal");
        assert_eq!(level(Some(49), &options), "normal");
        assert_eq!(level(Some(50), &options), "warning");
        assert_eq!(level(Some(90), &options), "critical");
    }

    #[test]
    fn json_output_round_trips_and_reports_errors() {
        let report = sample();
        let value: serde_json::Value =
            serde_json::from_str(&render_json(&Ok(report.clone()))).unwrap();
        assert_eq!(value["ok"], true);
        let providers: Vec<ProviderReport> =
            serde_json::from_value(value["providers"].clone()).unwrap();
        assert_eq!(providers, report.providers);
        let error: serde_json::Value =
            serde_json::from_str(&render_json(&Err(anyhow!("down")))).unwrap();
        assert_eq!(error, serde_json::json!({ "ok": false, "error": "down" }));
    }

    #[test]
    fn bridge_responses_are_parsed() {
        let report = sample();
        let line =
            serde_json::json!({ "type": "status", "ok": true, "report": report }).to_string();
        assert_eq!(parse_response(&line).unwrap(), report);
        let old =
            r#"{"type":"error","ok":false,"error":"invalid request: unknown variant `status`"}"#;
        assert!(
            parse_response(old)
                .unwrap_err()
                .to_string()
                .contains("restart")
        );
        assert!(parse_response("garbage").is_err());
    }

    #[test]
    fn fetches_the_report_from_the_bridge_endpoint() {
        use std::net::TcpListener;

        let directory = tempfile::tempdir().unwrap();
        let endpoint = directory.path().join("streamdeck-bridge.json");
        assert_eq!(
            fetch_report_from(&endpoint).unwrap_err().to_string(),
            "Codex Minibar is not running"
        );

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::fs::write(
            &endpoint,
            serde_json::json!({
                "protocol": 1, "host": "127.0.0.1", "port": port,
                "token": "secret", "executable": "",
            })
            .to_string(),
        )
        .unwrap();
        let report = sample();
        let served = report.clone();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut request)
                .unwrap();
            let line = serde_json::json!({
                "type": "status", "ok": true, "protocol": 1, "report": served,
            });
            writeln!(&stream, "{line}").unwrap();
            request
        });
        assert_eq!(fetch_report_from(&endpoint).unwrap(), report);
        let request: serde_json::Value = serde_json::from_str(&server.join().unwrap()).unwrap();
        assert_eq!(
            request,
            serde_json::json!({ "op": "status", "token": "secret" })
        );

        // A stale endpoint whose port no longer listens.
        assert_eq!(
            fetch_report_from(&endpoint).unwrap_err().to_string(),
            "Codex Minibar is not running"
        );
    }

    #[test]
    fn max_used_prefers_the_default_tray_metrics() {
        let metric = |id: &str, used| MetricReport {
            id: id.into(),
            label: id.into(),
            used_percent: used,
            remaining_percent: used.map(|used: u8| 100 - used),
            resets_at: None,
            duration_minutes: None,
        };
        let metrics = vec![
            metric("codex.session", Some(30)),
            metric("codex.weekly", Some(60)),
            metric("codex.extra", Some(99)),
        ];
        assert_eq!(
            max_used_percent(&metrics, &["codex.session", "codex.weekly"]),
            Some(60)
        );
        assert_eq!(max_used_percent(&metrics, &["missing"]), Some(99));
        assert_eq!(max_used_percent(&[], &["codex.session"]), None);
    }
}
