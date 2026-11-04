//! Game mode: backs background work off while a game runs, or always when
//! the user asks for it.
//!
//! While active, provider polls and usage scans stretch their intervals
//! ([`stretch_limit_interval`], [`stretch_usage_interval`]), the process runs
//! at below-normal priority with EcoQoS, the idle bridge tick slows down and
//! unsolicited notifications are held back. A card announces each change
//! unless `notifications.game_mode_changes` is off.
//!
//! Automatic follows a game process, not the screen. A foreground window
//! starts it when its executable is a game: one Windows Game Bar knows (its
//! `GameConfigStore`), one installed in a store library, one running
//! exclusive full screen, or one the user added. It lasts until that process
//! exits, so Alt+Tab keeps it on and a full-screen video never starts it.

use std::collections::HashSet;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU8, Ordering},
    mpsc::Sender,
};
use std::time::{Duration, Instant};

use crate::settings::{GameMode, Settings};

/// How often Automatic looks at the foreground window and whether the game
/// it follows still runs. A few seconds is invisible next to a game launch.
const DETECTION_INTERVAL: Duration = Duration::from_secs(5);

/// How long the Game Bar catalog is trusted before it is read again; a game
/// launched for the first time appears there within seconds.
const CATALOG_REFRESH: Duration = Duration::from_secs(60);

/// Install folders of game stores, lowercase. Their launchers live outside
/// them (or in [`LIBRARY_EXCLUSIONS`]), so browsing a store is not a game.
const LIBRARY_FOLDERS: [&str; 5] = [
    "\\steamapps\\common\\",
    "\\epic games\\",
    "\\gog galaxy\\games\\",
    "\\xboxgames\\",
    // Heroic (Epic and GOG on Linux) installs under ~/Games/Heroic.
    "\\games\\heroic\\",
];
const LIBRARY_EXCLUSIONS: [&str; 1] = ["\\epic games\\launcher\\"];

/// Polls back off by this factor, never faster than the floors and never
/// slower than the ceilings below (or the configured interval, if longer),
/// so a long session still sees its quota move.
const STRETCH_FACTOR: u32 = 4;
const LIMIT_INTERVAL_FLOOR: Duration = Duration::from_secs(5 * 60);
const LIMIT_INTERVAL_CEILING: Duration = Duration::from_secs(60 * 60);
const USAGE_INTERVAL_FLOOR: Duration = Duration::from_secs(30 * 60);
const USAGE_INTERVAL_CEILING: Duration = Duration::from_secs(2 * 60 * 60);

static MODE: AtomicU8 = AtomicU8::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// Set on every activation change until the bridge resends poll intervals.
static TRANSITION: AtomicBool = AtomicBool::new(false);
static DETECTOR_STARTED: AtomicBool = AtomicBool::new(false);
/// Mirrors `notifications.game_mode_changes`.
static NOTIFY: AtomicBool = AtomicBool::new(false);
/// False until the first evaluation, which reflects the state at launch
/// rather than a change, so it shows a card only for a detected game.
static EVALUATED: AtomicBool = AtomicBool::new(false);
/// Serializes state changes so the process priority matches [`ACTIVE`].
static APPLY_LOCK: Mutex<()> = Mutex::new(());
/// Mirrors `game_mode_games` and `game_mode_ignored`.
static LISTS: Mutex<Lists> = Mutex::new(Lists::new());
/// The game Automatic follows. Held for a whole evaluation, so the detector
/// and settings commits never race on it.
static TRACKED: Mutex<Option<Game>> = Mutex::new(None);
static CATALOG: Mutex<Option<(Instant, HashSet<String>)>> = Mutex::new(None);
/// The serial settings writer, for "Not a game" on the card.
static SETTINGS_TX: OnceLock<Sender<Settings>> = OnceLock::new();

/// User overrides, as [`path_key`]s.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Lists {
    games: Vec<String>,
    ignored: Vec<String>,
}

impl Lists {
    const fn new() -> Self {
        Self {
            games: Vec::new(),
            ignored: Vec::new(),
        }
    }
}

/// A running game. Its process stays open, so the exit is seen even after
/// the id is reused.
struct Game {
    process: os::Process,
    /// Executable path as Windows reports it.
    path: String,
    /// Window title when it was detected, for the card.
    title: String,
}

/// The foreground window's process.
#[cfg(windows)]
struct Foreground {
    process: os::Process,
    path: String,
    title: String,
}

fn mode() -> GameMode {
    GameMode::ALL
        .get(usize::from(MODE.load(Ordering::Relaxed)))
        .copied()
        .unwrap_or_default()
}

/// Lets "Not a game" on the card persist its choice. Called once at startup.
pub fn set_settings_writer(settings_tx: Sender<Settings>) {
    let _ = SETTINGS_TX.set(settings_tx);
}

/// Applies the settings immediately; called on startup and every commit.
pub fn apply(mode: GameMode, notify: bool, games: &[String], ignored: &[String]) {
    NOTIFY.store(notify, Ordering::Relaxed);
    let lists = Lists {
        games: games.iter().map(|path| path_key(path)).collect(),
        ignored: ignored.iter().map(|path| path_key(path)).collect(),
    };
    let lists_changed = {
        let mut current = LISTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let changed = *current != lists;
        *current = lists;
        changed
    };
    let previous = MODE.swap(mode.index() as u8, Ordering::Relaxed);
    if mode == GameMode::Automatic && !DETECTOR_STARTED.swap(true, Ordering::AcqRel) {
        let spawned = std::thread::Builder::new()
            .name("game-mode-detector".into())
            .spawn(detect);
        if let Err(error) = spawned {
            DETECTOR_STARTED.store(false, Ordering::Release);
            crate::logger::info(format!("Game mode detector failed to start: {error}"));
        }
    }
    // A new mode, the launch state or a new list applies at once; other
    // commits leave the detector alone.
    let mode_changed = previous != mode.index() as u8 || !EVALUATED.load(Ordering::Acquire);
    if mode_changed || (lists_changed && mode == GameMode::Automatic) {
        evaluate();
    }
}

/// "Not a game" on the card: never treats `path` as a game again. Runs off
/// the UI thread.
pub fn ignore_game(path: String) {
    std::thread::spawn(move || {
        let key = path_key(&path);
        {
            let mut lists = LISTS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            lists.games.retain(|game| *game != key);
            if !lists.ignored.contains(&key) {
                lists.ignored.push(key.clone());
            }
        }
        crate::logger::info(format!("Game mode: {path} is not a game"));
        evaluate();
        let Some(settings_tx) = SETTINGS_TX.get() else {
            return;
        };
        crate::settings_window::persist_update(settings_tx.clone(), move |settings| {
            settings
                .game_mode_games
                .retain(|game| path_key(game) != key);
            if !settings
                .game_mode_ignored
                .iter()
                .any(|ignored| path_key(ignored) == key)
            {
                settings.game_mode_ignored.push(path);
            }
        });
    });
}

/// Checks the followed game and the foreground window every few seconds.
fn detect() {
    loop {
        std::thread::sleep(DETECTION_INTERVAL);
        if mode() == GameMode::Automatic {
            evaluate();
        }
    }
}

/// Brings the activation state in line with the mode and, for Automatic,
/// with the followed game.
fn evaluate() {
    let mut tracked = TRACKED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match mode() {
        GameMode::Off => {
            *tracked = None;
            set_active(false, None);
        }
        GameMode::Always => {
            *tracked = None;
            set_active(true, None);
        }
        GameMode::Automatic => {
            let lists = LISTS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            let ended = tracked.as_ref().is_some_and(|game| {
                !game.process.is_running() || lists.ignored.contains(&path_key(&game.path))
            });
            if ended {
                *tracked = None;
            }
            if tracked.is_none() {
                *tracked = foreground_game(&lists);
            }
            set_active(tracked.is_some(), tracked.as_ref());
        }
    }
}

/// Linux has no foreground window to ask about on Wayland: any running
/// process of this user that counts as a game starts game mode. Steam's
/// launch environment stands in for Windows' exclusive full screen.
#[cfg(not(windows))]
fn foreground_game(lists: &Lists) -> Option<Game> {
    os::running_processes().into_iter().find_map(|running| {
        let key = path_key(&running.path);
        is_game(&key, lists, running.launched_by_steam, catalog_contains).then(|| Game {
            title: String::new(),
            process: running.process,
            path: running.path,
        })
    })
}

/// The foreground process, when it is a game.
#[cfg(windows)]
fn foreground_game(lists: &Lists) -> Option<Game> {
    let foreground = os::foreground()?;
    let key = path_key(&foreground.path);
    if !is_game(&key, lists, os::exclusive_full_screen(), catalog_contains) {
        return None;
    }
    Some(Game {
        process: foreground.process,
        path: foreground.path,
        title: foreground.title,
    })
}

/// User choices win; then exclusive full screen, the store libraries and,
/// last because it may read the registry, the Game Bar catalog.
fn is_game(key: &str, lists: &Lists, exclusive: bool, known: impl Fn(&str) -> bool) -> bool {
    if lists.ignored.iter().any(|ignored| ignored == key) {
        return false;
    }
    lists.games.iter().any(|game| game == key) || exclusive || in_library(key) || known(key)
}

fn in_library(key: &str) -> bool {
    // Library recognition is a name heuristic; explicit executable identities
    // below must still preserve the Unix filesystem's case and separators.
    #[cfg(not(windows))]
    let key = key.replace('/', "\\").to_lowercase();
    LIBRARY_FOLDERS.iter().any(|folder| key.contains(folder))
        && !LIBRARY_EXCLUSIONS
            .iter()
            .any(|excluded| key.contains(excluded))
}

fn catalog_contains(key: &str) -> bool {
    let mut catalog = CATALOG
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fresh = catalog
        .as_ref()
        .is_some_and(|(read, _)| read.elapsed() < CATALOG_REFRESH);
    if !fresh {
        let games = os::game_bar_catalog()
            .iter()
            .map(|path| path_key(path))
            .collect();
        *catalog = Some((Instant::now(), games));
    }
    catalog
        .as_ref()
        .is_some_and(|(_, games)| games.contains(key))
}

/// Windows path comparisons ignore case and slash direction. Unix paths keep
/// their case, backslashes and whitespace, which can identify different files.
pub fn path_key(path: &str) -> String {
    if cfg!(windows) {
        path.trim().replace('/', "\\").to_lowercase()
    } else {
        path.to_owned()
    }
}

/// The window title when it reads like a name, otherwise the file name.
fn display_name(title: &str, path: &str) -> String {
    let title = title.trim();
    if !title.is_empty() && title.chars().count() <= 48 {
        return title.to_owned();
    }
    file_stem(path)
}

/// `C:\Games\Hollow Knight\hollow_knight.exe` → `hollow_knight`.
/// Splits on both `\` and `/` so Windows-style paths work on every platform.
pub fn file_stem(path: &str) -> String {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[..dot].to_owned(),
        _ if name.is_empty() => path.to_owned(),
        _ => name.to_owned(),
    }
}

/// Records the activation state and applies a change.
fn set_active(active: bool, game: Option<&Game>) {
    let _guard = APPLY_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let first = !EVALUATED.swap(true, Ordering::AcqRel);
    if ACTIVE.swap(active, Ordering::AcqRel) == active {
        return;
    }
    TRANSITION.store(true, Ordering::Release);
    set_background_priority(active);
    crate::logger::info(match (active, game) {
        (true, Some(game)) => format!(
            "Game mode on for {}: background work slowed down",
            game.path
        ),
        (true, None) => "Game mode on: background work slowed down".to_owned(),
        (false, _) => "Game mode off: background work back to normal".to_owned(),
    });
    // At launch only a detected game is worth a card: it names what turned
    // game mode on and offers "Not a game" when that is wrong.
    if NOTIFY.load(Ordering::Relaxed) && (!first || game.is_some()) {
        crate::notifications::show_game_mode_changed(
            active,
            game.map(|game| (display_name(&game.title, &game.path), game.path.clone())),
        );
    }
}

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// True once after each activation change; the bridge then resends the
/// stretched (or restored) poll intervals to every worker.
pub fn take_transition() -> bool {
    TRANSITION.swap(false, Ordering::AcqRel)
}

/// The quota poll interval to use right now for a configured one.
pub fn stretch_limit_interval(configured: Duration) -> Duration {
    stretch(configured, LIMIT_INTERVAL_FLOOR, LIMIT_INTERVAL_CEILING)
}

/// The usage scan interval to use right now for a configured one.
pub fn stretch_usage_interval(configured: Duration) -> Duration {
    stretch(configured, USAGE_INTERVAL_FLOOR, USAGE_INTERVAL_CEILING)
}

fn stretch(configured: Duration, floor: Duration, ceiling: Duration) -> Duration {
    if is_active() {
        stretched(configured, floor, ceiling)
    } else {
        configured
    }
}

fn stretched(configured: Duration, floor: Duration, ceiling: Duration) -> Duration {
    configured
        .saturating_mul(STRETCH_FACTOR)
        .max(floor)
        .min(ceiling.max(configured))
}

#[cfg(windows)]
mod os {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, STILL_ACTIVE};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_ANY, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW,
        RegGetValueW, RegOpenKeyExW,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, GetExitCodeProcess, OpenProcess, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::Shell::{
        QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    use super::Foreground;

    pub(super) struct Process(OwnedHandle);

    impl Process {
        /// A process that can no longer be queried counts as gone, so game
        /// mode never sticks.
        pub(super) fn is_running(&self) -> bool {
            let mut code = 0;
            let queried = unsafe { GetExitCodeProcess(self.0.as_raw_handle(), &mut code) } != 0;
            queried && code == STILL_ACTIVE as u32
        }
    }

    pub(super) fn foreground() -> Option<Foreground> {
        let window = unsafe { GetForegroundWindow() };
        if window.is_null() {
            return None;
        }
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if pid == 0 || pid == unsafe { GetCurrentProcessId() } {
            return None;
        }
        // Limited query access is granted even for elevated games.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let process = Process(unsafe { OwnedHandle::from_raw_handle(handle) });
        let mut path = [0_u16; 1024];
        let mut len = path.len() as u32;
        let queried = unsafe {
            QueryFullProcessImageNameW(
                process.0.as_raw_handle(),
                PROCESS_NAME_WIN32,
                path.as_mut_ptr(),
                &mut len,
            )
        } != 0;
        if !queried {
            return None;
        }
        let mut title = [0_u16; 256];
        let title_len = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
        Some(Foreground {
            process,
            path: String::from_utf16_lossy(&path[..len as usize]),
            title: String::from_utf16_lossy(&title[..title_len.max(0) as usize]),
        })
    }

    /// A Direct3D app owns the screen exclusively. Borderless games and
    /// full-screen video both report "busy", which tells them apart from
    /// nothing.
    pub(super) fn exclusive_full_screen() -> bool {
        let mut state = 0;
        let status = unsafe { SHQueryUserNotificationState(&mut state) };
        status == 0 && state == QUNS_RUNNING_D3D_FULL_SCREEN
    }

    /// Executables Game Bar knows as games. Undocumented but stable
    /// since Windows 10; any failure reads as an empty catalog.
    pub(super) fn game_bar_catalog() -> Vec<String> {
        let wide = |value: &str| {
            value
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>()
        };
        let mut key: HKEY = std::ptr::null_mut();
        let opened = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide("System\\GameConfigStore\\Children").as_ptr(),
                0,
                KEY_READ,
                &mut key,
            )
        };
        if opened != ERROR_SUCCESS {
            return Vec::new();
        }
        let value = wide("MatchedExeFullPath");
        let identity = [wide("TitleId"), wide("GameDVR_GameGUID")];
        let mut paths = Vec::new();
        for index in 0.. {
            let mut name = [0_u16; 256];
            let mut name_len = name.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(
                    key,
                    index,
                    name.as_mut_ptr(),
                    &mut name_len,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if status != ERROR_SUCCESS {
                break;
            }
            // The enumerated name comes back null-terminated.
            let mut path = [0_u16; 1024];
            let mut bytes = std::mem::size_of_val(&path) as u32;
            let status = unsafe {
                RegGetValueW(
                    key,
                    name.as_ptr(),
                    value.as_ptr(),
                    RRF_RT_REG_SZ,
                    std::ptr::null_mut(),
                    path.as_mut_ptr().cast(),
                    &mut bytes,
                )
            };
            if status != ERROR_SUCCESS {
                continue;
            }
            // Games carry a store title id or a Game DVR id (Game Bar adds
            // one when the user marks an app as a game). Entries with
            // neither are shortcuts Windows keeps for other reasons, such
            // as Chrome's.
            let identified = identity.iter().any(|value| {
                let status = unsafe {
                    RegGetValueW(
                        key,
                        name.as_ptr(),
                        value.as_ptr(),
                        RRF_RT_ANY,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                status == ERROR_SUCCESS
            });
            if !identified {
                continue;
            }
            let len = path
                .iter()
                .position(|&unit| unit == 0)
                .unwrap_or(path.len());
            if len > 0 {
                paths.push(String::from_utf16_lossy(&path[..len]));
            }
        }
        unsafe { RegCloseKey(key) };
        paths
    }
}

#[cfg(not(windows))]
mod os {
    use std::path::Path;

    /// A process identified by pid and start time, so a recycled pid is not
    /// mistaken for the game that exited.
    pub(super) struct Process {
        pid: u32,
        start_time: u64,
    }

    impl Process {
        pub(super) fn is_running(&self) -> bool {
            start_time(self.pid) == Some(self.start_time)
        }
    }

    pub(super) struct Running {
        pub(super) process: Process,
        pub(super) path: String,
        pub(super) launched_by_steam: bool,
    }

    /// Processes of this user with a readable executable.
    pub(super) fn running_processes() -> Vec<Running> {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        // SAFETY: getpid has no preconditions and cannot fail.
        let own = unsafe { libc::getpid() } as u32;
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
            .filter(|pid| *pid != own)
            .filter_map(|pid| {
                let proc = Path::new("/proc").join(pid.to_string());
                // Other users' executables are unreadable; skip them.
                let path = std::fs::read_link(proc.join("exe")).ok()?;
                let start_time = start_time(pid)?;
                Some(Running {
                    process: Process { pid, start_time },
                    path: path.to_string_lossy().into_owned(),
                    launched_by_steam: launched_by_steam(&proc),
                })
            })
            .collect()
    }

    /// Steam starts games with their app id in the environment.
    fn launched_by_steam(proc: &Path) -> bool {
        std::fs::read(proc.join("environ")).is_ok_and(|environ| {
            environ.split(|byte| *byte == 0).any(|variable| {
                variable
                    .strip_prefix(b"SteamGameId=")
                    .is_some_and(|id| !id.is_empty() && id != b"0")
            })
        })
    }

    /// Field 22 of `/proc/<pid>/stat`, only while the process is alive.
    fn start_time(pid: u32) -> Option<u64> {
        let stat = std::fs::read(format!("/proc/{pid}/stat")).ok()?;
        live_start_time(&stat)
    }

    fn live_start_time(stat: &[u8]) -> Option<u64> {
        // comm can contain spaces, parentheses and arbitrary Unix bytes.
        // Only the fields after its closing parenthesis are ASCII numbers.
        let after_name = &stat[stat.iter().rposition(|byte| *byte == b')')? + 1..];
        let mut fields = after_name
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty());
        match fields.next()? {
            [b'Z' | b'X' | b'x'] => return None,
            [_] => {}
            _ => return None,
        }
        std::str::from_utf8(fields.nth(18)?).ok()?.parse().ok()
    }

    pub(super) fn game_bar_catalog() -> Vec<String> {
        Vec::new()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn proc_stat_handles_arbitrary_names_and_excludes_exited_processes() {
            for state in [b'R', b'S', b'T', b'Z', b'X', b'x'] {
                let mut stat = b"123 (name-\xff with ) parentheses) ".to_vec();
                stat.push(state);
                stat.extend_from_slice(format!(" {} 12345 0", ["0"; 18].join(" ")).as_bytes());
                assert_eq!(
                    live_start_time(&stat),
                    match state {
                        b'Z' | b'X' | b'x' => None,
                        _ => Some(12345),
                    }
                );
            }
            assert_eq!(live_start_time(b"123 (truncated) S 0"), None);
        }

        #[test]
        fn an_unreaped_child_no_longer_counts_as_a_running_game() {
            use std::{
                process::{Command, Stdio},
                time::{Duration, Instant},
            };
            let mut child = Command::new("/bin/sh")
                .args(["-c", "read ignored"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let process = Process {
                pid: child.id(),
                start_time: start_time(child.id()).unwrap(),
            };
            assert!(process.is_running());
            drop(child.stdin.take());
            let deadline = Instant::now() + Duration::from_secs(2);
            while process.is_running() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            let stopped = !process.is_running();
            let still_in_proc = Path::new(&format!("/proc/{}/stat", child.id())).is_file();
            if !stopped {
                let _ = child.kill();
            }
            child.wait().unwrap();
            assert!(
                stopped,
                "an exited but unreaped child still counted as running"
            );
            assert!(
                still_in_proc,
                "the test must cover a process still present in /proc"
            );
        }
    }
}

/// Below-normal priority keeps the app off the game's cores; EcoQoS lets
/// Windows run its threads on efficient cores at low clocks.
#[cfg(windows)]
fn set_background_priority(background: bool) {
    use windows_sys::Win32::System::Threading::{
        BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, NORMAL_PRIORITY_CLASS,
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetPriorityClass,
        SetProcessInformation,
    };
    let process = unsafe { GetCurrentProcess() };
    let class = if background {
        BELOW_NORMAL_PRIORITY_CLASS
    } else {
        NORMAL_PRIORITY_CLASS
    };
    if unsafe { SetPriorityClass(process, class) } == 0 {
        crate::logger::info("Game mode could not change the process priority");
    }
    // An empty control mask hands throttling back to the system's own policy.
    let throttling = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: if background {
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
        StateMask: if background {
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
    };
    let applied = unsafe {
        SetProcessInformation(
            process,
            ProcessPowerThrottling,
            (&raw const throttling).cast(),
            size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        )
    };
    if applied == 0 {
        crate::logger::info("Game mode could not change EcoQoS");
    }
}

#[cfg(not(windows))]
fn set_background_priority(_background: bool) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretched_intervals_stay_between_floor_and_ceiling() {
        let limits = |minutes: u64| {
            stretched(
                Duration::from_secs(minutes * 60),
                LIMIT_INTERVAL_FLOOR,
                LIMIT_INTERVAL_CEILING,
            )
        };
        assert_eq!(limits(0), LIMIT_INTERVAL_FLOOR);
        assert_eq!(limits(10), Duration::from_secs(40 * 60));
        assert_eq!(limits(30), LIMIT_INTERVAL_CEILING);
        // A configured interval above the ceiling is never shortened.
        assert_eq!(limits(300), Duration::from_secs(300 * 60));
        assert_eq!(
            stretched(
                Duration::from_secs(5 * 60),
                USAGE_INTERVAL_FLOOR,
                USAGE_INTERVAL_CEILING
            ),
            USAGE_INTERVAL_FLOOR
        );
    }

    #[test]
    fn user_choices_override_detection() {
        let steam = path_key(
            r"C:\Program Files (x86)\Steam\steamapps\common\Hollow Knight\hollow_knight.exe",
        );
        let browser = path_key(r"C:\Program Files\Google\Chrome\Application\chrome.exe");
        let none = Lists::default();
        let unknown = |_: &str| false;
        assert!(is_game(&steam, &none, false, unknown));
        assert!(!is_game(&browser, &none, false, unknown));
        assert!(is_game(&browser, &none, false, |key| key == browser));
        assert!(is_game(&browser, &none, true, unknown));
        let ignored = Lists {
            games: Vec::new(),
            ignored: vec![steam.clone()],
        };
        assert!(!is_game(&steam, &ignored, true, |_| true));
        let added = Lists {
            games: vec![browser.clone()],
            ignored: Vec::new(),
        };
        assert!(is_game(&browser, &added, false, unknown));
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_game_overrides_only_match_the_selected_executable() {
        let selected = path_key("/games/Custom/Game");
        let ignored = Lists {
            games: Vec::new(),
            ignored: vec![selected.clone()],
        };
        assert!(!is_game(&selected, &ignored, true, |_| true));
        for distinct in [
            "/games/Custom/game",
            "/games/Custom/Game ",
            r"/games/Custom\Game",
        ] {
            let key = path_key(distinct);
            assert_ne!(key, selected);
            assert!(is_game(&key, &ignored, true, |_| false));
        }
        assert!(in_library(&path_key(
            "/home/me/.steam/steamapps/common/Game/game"
        )));
        assert!(in_library(&path_key("/home/me/Games/Heroic/Game/game")));
    }

    #[test]
    fn store_launchers_are_not_games() {
        assert!(in_library(&path_key(
            r"D:\Epic Games\Fortnite\FortniteClient-Win64-Shipping.exe"
        )));
        assert!(!in_library(&path_key(
            r"C:\Program Files (x86)\Epic Games\Launcher\Portal\Binaries\Win64\EpicGamesLauncher.exe"
        )));
        assert!(!in_library(&path_key(
            r"C:\Program Files (x86)\Steam\steam.exe"
        )));
    }

    #[test]
    fn names_prefer_short_window_titles() {
        let path = r"C:\Games\hollow_knight.exe";
        assert_eq!(display_name("Hollow Knight", path), "Hollow Knight");
        assert_eq!(display_name("  ", path), "hollow_knight");
        assert_eq!(display_name(&"x".repeat(80), path), "hollow_knight");
    }

    #[test]
    fn mode_round_trips_through_its_index() {
        for mode in GameMode::ALL {
            assert_eq!(GameMode::from_index(mode.index()), mode);
        }
    }
}
