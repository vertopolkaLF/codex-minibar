"""Run a Codex prompt every five hours and show its status in the system tray."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
from datetime import datetime, timedelta
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
from PIL import Image, ImageDraw, ImageFont
from pystray._util import win32 as tray_win32


TRAY_MESSAGE = 0x8001
WM_DESTROY = 0x0002
WM_COMMAND = 0x0111
WM_DRAWITEM = 0x002B
WM_MEASUREITEM = 0x002C
WM_RBUTTONUP = 0x0205
NIM_ADD = 0x00000000
NIM_MODIFY = 0x00000001
NIM_DELETE = 0x00000002
NIF_MESSAGE = 0x00000001
NIF_ICON = 0x00000002
NIF_TIP = 0x00000004
MF_STRING = 0x0000
TPM_RIGHTBUTTON = 0x0002
IDI_APPLICATION = 32512
COMMAND_CLOSE = 1
COMMAND_REFRESH = 2
MIIM_ID = 0x00000002
MIIM_STRING = 0x00000040
IMAGE_ICON = 1
LR_LOADFROMFILE = 0x0010
ODT_MENU = 1
ODS_SELECTED = 0x0001
MFT_OWNERDRAW = 0x0100
MIIM_DATA = 0x0020
MIIM_FTYPE = 0x0100
DT_LEFT = 0x0000
DT_VCENTER = 0x0004
DT_SINGLELINE = 0x0020
TRANSPARENT = 1
DEFAULT_GUI_FONT = 17

CODEX_EXECUTABLE = Path(os.environ["LOCALAPPDATA"]) / "pnpm" / "codex.cmd"
ICON_FILE = Path(os.environ["LOCALAPPDATA"]) / "CodexFiveHourRunner" / "next_call.ico"
USAGE_ICON_FILE = Path(os.environ["LOCALAPPDATA"]) / "CodexFiveHourRunner" / "usage_left.ico"
CODEX_COMMAND = [
    str(CODEX_EXECUTABLE),
    "e",
    "respond only with letter a",
    "--skip-git-repo-check",
    "--model",
    "gpt-5.4-mini",
    "--config",
    "model_reasoning_effort=none",
]

user32 = ctypes.windll.user32
shell32 = ctypes.windll.shell32
kernel32 = ctypes.windll.kernel32
gdi32 = ctypes.windll.gdi32
gdi32.CreateSolidBrush.argtypes = [wintypes.DWORD]
gdi32.CreateSolidBrush.restype = wintypes.HANDLE
gdi32.GetStockObject.argtypes = [ctypes.c_int]
gdi32.GetStockObject.restype = wintypes.HANDLE
gdi32.SelectObject.argtypes = [wintypes.HDC, wintypes.HANDLE]
gdi32.SelectObject.restype = wintypes.HANDLE
gdi32.DeleteObject.argtypes = [wintypes.HANDLE]
gdi32.SetBkMode.argtypes = [wintypes.HDC, ctypes.c_int]
gdi32.SetTextColor.argtypes = [wintypes.HDC, wintypes.DWORD]
user32.CreatePopupMenu.restype = ctypes.c_void_p
user32.AppendMenuW.argtypes = [ctypes.c_void_p, wintypes.UINT, ctypes.c_size_t, wintypes.LPCWSTR]
user32.AppendMenuW.restype = wintypes.BOOL
user32.AppendMenuW.argtypes = [ctypes.c_void_p, wintypes.UINT, ctypes.c_size_t, wintypes.LPCWSTR]
user32.AppendMenuW.restype = wintypes.BOOL
user32.TrackPopupMenu.argtypes = [ctypes.c_void_p, wintypes.UINT, ctypes.c_int, ctypes.c_int, wintypes.UINT, wintypes.HWND, ctypes.c_void_p]
user32.DestroyMenu.argtypes = [ctypes.c_void_p]
user32.LoadImageW.argtypes = [wintypes.HANDLE, wintypes.LPCWSTR, wintypes.UINT, ctypes.c_int, ctypes.c_int, wintypes.UINT]
user32.LoadImageW.restype = wintypes.HANDLE


class NOTIFYICONDATAW(ctypes.Structure):
    _fields_ = [
        ("cbSize", wintypes.DWORD),
        ("hWnd", wintypes.HWND),
        ("uID", wintypes.UINT),
        ("uFlags", wintypes.UINT),
        ("uCallbackMessage", wintypes.UINT),
        ("hIcon", wintypes.HICON),
        ("szTip", wintypes.WCHAR * 128),
    ]


class WNDCLASSW(ctypes.Structure):
    _fields_ = [
        ("style", wintypes.UINT),
        ("lpfnWndProc", ctypes.c_void_p),
        ("cbClsExtra", ctypes.c_int),
        ("cbWndExtra", ctypes.c_int),
        ("hInstance", wintypes.HINSTANCE),
        ("hIcon", wintypes.HICON),
        ("hCursor", wintypes.HANDLE),
        ("hbrBackground", wintypes.HANDLE),
        ("lpszMenuName", wintypes.LPCWSTR),
        ("lpszClassName", wintypes.LPCWSTR),
    ]


class MEASUREITEMSTRUCT(ctypes.Structure):
    _fields_ = [
        ("CtlType", wintypes.UINT), ("CtlID", wintypes.UINT),
        ("itemID", wintypes.UINT), ("itemWidth", wintypes.UINT),
        ("itemHeight", wintypes.UINT), ("itemData", ctypes.c_size_t),
    ]


class DRAWITEMSTRUCT(ctypes.Structure):
    _fields_ = [
        ("CtlType", wintypes.UINT), ("CtlID", wintypes.UINT),
        ("itemID", wintypes.UINT), ("itemAction", wintypes.UINT),
        ("itemState", wintypes.UINT), ("hwndItem", wintypes.HWND),
        ("hDC", wintypes.HDC), ("rcItem", wintypes.RECT),
        ("itemData", ctypes.c_size_t),
    ]


stop_event = threading.Event()
refresh_event = threading.Event()
last_call: datetime | None = None
next_call: datetime | None = None
primary_used_percent: int | None = None
secondary_used_percent: int | None = None
secondary_reset: datetime | None = None
hwnd: int | None = None
time_icon_data: NOTIFYICONDATAW | None = None
usage_icon_data: NOTIFYICONDATAW | None = None
time_icon: int | None = None
usage_icon: int | None = None


def format_time(value: datetime | None, unavailable: str) -> str:
    if value is None:
        return unavailable
    return value.strftime("%H:%M" if value.date() == datetime.now().date() else "%H:%M %d.%m")


def tooltip() -> str:
    primary = format_time(next_call, "unavailable")
    primary_usage = "?" if primary_used_percent is None else f"{max(0, 100 - primary_used_percent)}%"
    weekly_usage = "?" if secondary_used_percent is None else f"{max(0, 100 - secondary_used_percent)}%"
    weekly = format_time(secondary_reset, "?")
    return f"5h  |  {primary_usage}  |  {primary}\n7d  |  {weekly_usage}  |  {weekly}"


def read_response(process: subprocess.Popen[str], request_id: int) -> dict:
    """Read the matching JSON-RPC response without leaving the tray app stuck forever."""
    while True:
        line_queue: queue.Queue[str] = queue.Queue(maxsize=1)
        threading.Thread(target=lambda: line_queue.put(process.stdout.readline()), daemon=True).start()
        try:
            line = line_queue.get(timeout=10)
        except queue.Empty as error:
            raise TimeoutError("Codex app-server did not reply within 10 seconds") from error
        if not line:
            raise RuntimeError(process.stderr.read())
        message = json.loads(line)
        if message.get("id") == request_id:
            return message


def read_rate_limits() -> None:
    """Fetch the actual account limit reset times from local Codex JSON-RPC."""
    global next_call, primary_used_percent, secondary_used_percent, secondary_reset
    process = subprocess.Popen(
        [str(CODEX_EXECUTABLE), "-s", "read-only", "-a", "untrusted", "app-server"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        creationflags=subprocess.CREATE_NO_WINDOW,
    )
    try:
        requests = (
            (1, "initialize", {"clientInfo": {"name": "Codex tray runner", "version": "1.0"}}),
            (2, "account/rateLimits/read", None),
        )
        result: dict | None = None
        for request_id, method, params in requests:
            process.stdin.write(json.dumps({"id": request_id, "method": method, "params": params}) + "\n")
            process.stdin.flush()
            response = read_response(process, request_id)
            if "error" in response:
                raise RuntimeError(str(response["error"]))
            result = response.get("result")
        limits = result["rateLimits"]
        primary = limits.get("primary") or {}
        secondary = limits.get("secondary") or {}
        primary_used_percent = primary.get("usedPercent")
        secondary_used_percent = secondary.get("usedPercent")
        next_call = datetime.fromtimestamp(primary["resetsAt"]) if primary.get("resetsAt") else None
        secondary_reset = datetime.fromtimestamp(secondary["resetsAt"]) if secondary.get("resetsAt") else None
    except (OSError, KeyError, RuntimeError, TimeoutError, json.JSONDecodeError):
        pass
    finally:
        process.terminate()
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()


def wait_for_refresh_or_stop(seconds: float) -> None:
    """Wait for the scheduler, but let the tray's Refresh action wake it early."""
    deadline = datetime.now().timestamp() + seconds
    while not stop_event.is_set() and datetime.now().timestamp() < deadline:
        remaining = deadline - datetime.now().timestamp()
        if refresh_event.wait(timeout=min(1, remaining)):
            refresh_event.clear()
            return


def create_next_call_icon() -> int:
    """Render the next call's hour and minute as a two-line tray icon."""
    when = next_call or datetime.now()
    image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    font_path = Path(os.environ["WINDIR"]) / "Fonts" / "consolab.ttf"
    font = ImageFont.truetype(font_path, 34)
    draw.text((32, 15), when.strftime("%H"), font=font, anchor="mm", fill="white")
    draw.text((32, 47), when.strftime("%M"), font=font, anchor="mm", fill="white")
    ICON_FILE.parent.mkdir(parents=True, exist_ok=True)
    image.save(ICON_FILE, format="ICO", sizes=[(16, 16), (32, 32), (64, 64)])
    return user32.LoadImageW(None, str(ICON_FILE), IMAGE_ICON, 32, 32, LR_LOADFROMFILE)


def create_usage_icon() -> int:
    """Render 5-hour remaining percentage, colored by the remaining amount."""
    left = 0 if primary_used_percent is None else max(0, min(100, 100 - primary_used_percent))
    color = (230, 74, 72, 255) if left <= 15 else (245, 158, 11, 255) if left <= 50 else (49, 196, 141, 255)
    image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    font_path = Path(os.environ["WINDIR"]) / "Fonts" / "arialnb.ttf"
    font = ImageFont.truetype(font_path, 37 if left == 100 else 42)
    draw.text((32, 32), str(left), font=font, anchor="mm", fill=color)
    USAGE_ICON_FILE.parent.mkdir(parents=True, exist_ok=True)
    image.save(USAGE_ICON_FILE, format="ICO", sizes=[(16, 16), (32, 32), (64, 64)])
    return user32.LoadImageW(None, str(USAGE_ICON_FILE), IMAGE_ICON, 32, 32, LR_LOADFROMFILE)


def replace_tray_icon(data: NOTIFYICONDATAW, new_icon: int, text: str) -> None:
    data.hIcon = new_icon
    data.uFlags = NIF_TIP | NIF_ICON
    data.szTip = text
    shell32.Shell_NotifyIconW(NIM_MODIFY, ctypes.byref(data))


def update_tooltip() -> None:
    global time_icon, usage_icon
    if time_icon_data is None or usage_icon_data is None:
        return
    new_time_icon = create_next_call_icon()
    if new_time_icon:
        previous_icon = time_icon
        replace_tray_icon(time_icon_data, new_time_icon, tooltip())
        time_icon = new_time_icon
        if previous_icon:
            user32.DestroyIcon(previous_icon)
    new_usage_icon = create_usage_icon()
    if new_usage_icon:
        previous_icon = usage_icon
        replace_tray_icon(usage_icon_data, new_usage_icon, tooltip())
        usage_icon = new_usage_icon
        if previous_icon:
            user32.DestroyIcon(previous_icon)


def codex_worker() -> None:
    global last_call
    while not stop_event.is_set():
        read_rate_limits()
        update_tooltip()

        # Recheck once a minute so a changed server-side reset time is honored.
        # The command runs one minute after the real 5-hour window resets.
        if next_call is None:
            wait_for_refresh_or_stop(60)
            continue
        run_at = next_call + timedelta(minutes=1)
        seconds_until_run = (run_at - datetime.now()).total_seconds()
        if seconds_until_run > 0:
            wait_for_refresh_or_stop(min(seconds_until_run, 60))
            continue

        last_call = datetime.now()
        update_tooltip()
        try:
            subprocess.run(
                CODEX_COMMAND,
                check=False,
                creationflags=subprocess.CREATE_NO_WINDOW,
            )
        except FileNotFoundError:
            pass
        read_rate_limits()
        update_tooltip()
        wait_for_refresh_or_stop(60)


def show_menu() -> None:
    point = wintypes.POINT()
    tray_win32.GetCursorPos(ctypes.byref(point))
    menu = tray_win32.CreatePopupMenu()
    try:
        mask = tray_win32.MIIM_ID | MIIM_FTYPE | MIIM_DATA
        for position, (command_id, label) in enumerate(((COMMAND_REFRESH, "Refresh"), (COMMAND_CLOSE, "Close"))):
            menu_item = tray_win32.MENUITEMINFO(
                cbSize=ctypes.sizeof(tray_win32.MENUITEMINFO),
                fMask=mask,
                fType=MFT_OWNERDRAW,
                fState=0,
                wID=command_id,
                hSubMenu=None,
                dwTypeData=None,
                dwItemData=command_id,
            )
            tray_win32.InsertMenuItem(menu, position, True, ctypes.byref(menu_item))
        tray_win32.SetForegroundWindow(hwnd)
        command = tray_win32.TrackPopupMenuEx(
            menu,
            tray_win32.TPM_RIGHTALIGN | tray_win32.TPM_BOTTOMALIGN | tray_win32.TPM_RETURNCMD,
            point.x,
            point.y,
            hwnd,
            None,
        )
        if command == COMMAND_REFRESH:
            refresh_event.set()
        elif command == COMMAND_CLOSE:
            user32.DestroyWindow(hwnd)
    finally:
        tray_win32.DestroyMenu(menu)


@ctypes.WINFUNCTYPE(ctypes.c_ssize_t, wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)
def window_proc(window, message, wparam, lparam):
    if message == WM_MEASUREITEM:
        item = ctypes.cast(lparam, ctypes.POINTER(MEASUREITEMSTRUCT)).contents
        if item.CtlType == ODT_MENU:
            dpi = user32.GetDpiForWindow(window) or 96
            item.itemWidth = round(110 * dpi / 96)
            item.itemHeight = round(28 * dpi / 96)
            return 1
    if message == WM_DRAWITEM:
        item = ctypes.cast(lparam, ctypes.POINTER(DRAWITEMSTRUCT)).contents
        if item.CtlType == ODT_MENU:
            label = {COMMAND_REFRESH: "Refresh", COMMAND_CLOSE: "Close"}.get(item.itemID, "")
            selected = bool(item.itemState & ODS_SELECTED)
            color = (62, 62, 62) if selected else (43, 43, 43)
            colorref = color[0] | (color[1] << 8) | (color[2] << 16)
            brush = gdi32.CreateSolidBrush(colorref)
            user32.FillRect(item.hDC, ctypes.byref(item.rcItem), brush)
            gdi32.DeleteObject(brush)
            gdi32.SetBkMode(item.hDC, TRANSPARENT)
            gdi32.SetTextColor(item.hDC, 0x00FFFFFF)
            font = gdi32.GetStockObject(DEFAULT_GUI_FONT)
            previous_font = gdi32.SelectObject(item.hDC, font)
            dpi = user32.GetDpiForWindow(window) or 96
            text_rect = wintypes.RECT(item.rcItem.left + round(12 * dpi / 96), item.rcItem.top, item.rcItem.right, item.rcItem.bottom)
            user32.DrawTextW(item.hDC, label, -1, ctypes.byref(text_rect), DT_LEFT | DT_VCENTER | DT_SINGLELINE)
            gdi32.SelectObject(item.hDC, previous_font)
            return 1
    if message == TRAY_MESSAGE and lparam == WM_RBUTTONUP:
        show_menu()
        return 0
    if message == WM_COMMAND:
        command = wparam & 0xFFFF
        if command == COMMAND_REFRESH:
            refresh_event.set()
            return 0
        if command == COMMAND_CLOSE:
            user32.DestroyWindow(window)
            return 0
    if message == WM_DESTROY:
        stop_event.set()
        if time_icon_data is not None:
            shell32.Shell_NotifyIconW(NIM_DELETE, ctypes.byref(time_icon_data))
        if usage_icon_data is not None:
            shell32.Shell_NotifyIconW(NIM_DELETE, ctypes.byref(usage_icon_data))
        if time_icon:
            user32.DestroyIcon(time_icon)
        if usage_icon:
            user32.DestroyIcon(usage_icon)
        user32.PostQuitMessage(0)
        return 0
    return user32.DefWindowProcW(window, message, wparam, lparam)


def main() -> None:
    global hwnd, time_icon_data, usage_icon_data
    if not CODEX_EXECUTABLE.is_file():
        return

    instance = kernel32.GetModuleHandleW(None)
    class_name = "CodexFiveHourRunner"
    window_class = WNDCLASSW()
    window_class.lpfnWndProc = ctypes.cast(window_proc, ctypes.c_void_p).value
    window_class.hInstance = instance
    window_class.lpszClassName = class_name
    user32.RegisterClassW(ctypes.byref(window_class))
    hwnd = user32.CreateWindowExW(0, class_name, class_name, tray_win32.WS_POPUP, 0, 0, 0, 0, 0, 0, instance, None)
    default_icon = user32.LoadIconW(None, IDI_APPLICATION)
    for icon_id in (1, 2):
        data = NOTIFYICONDATAW()
        data.cbSize = ctypes.sizeof(data)
        data.hWnd, data.uID = hwnd, icon_id
        data.uFlags, data.uCallbackMessage = NIF_MESSAGE | NIF_ICON | NIF_TIP, TRAY_MESSAGE
        data.hIcon, data.szTip = default_icon, tooltip()
        shell32.Shell_NotifyIconW(NIM_ADD, ctypes.byref(data))
        if icon_id == 1:
            time_icon_data = data
        else:
            usage_icon_data = data
    threading.Thread(target=codex_worker, daemon=True).start()
    message = wintypes.MSG()
    while user32.GetMessageW(ctypes.byref(message), None, 0, 0) > 0:
        user32.TranslateMessage(ctypes.byref(message))
        user32.DispatchMessageW(ctypes.byref(message))


if __name__ == "__main__":
    main()
