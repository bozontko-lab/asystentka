//! Windows: zdarzenia i stan systemu przez crate `windows` (SPEC.md, B5 i B6).

use std::mem::size_of;
use std::sync::OnceLock;

use tauri::WebviewWindow;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClassNameW, GetCursorPos, GetDesktopWindow,
    GetForegroundWindow, GetMessageW, GetShellWindow, GetWindowLongW, GetWindowRect, IsWindow, IsZoomed,
    RegisterClassW, SetForegroundWindow, SetWindowDisplayAffinity, SetWindowPos, TranslateMessage, GWL_STYLE, MSG,
    PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND, SWP_NOACTIVATE, SWP_NOZORDER, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    WINDOW_EX_STYLE, WM_ENDSESSION, WM_POWERBROADCAST, WM_QUERYENDSESSION, WM_WTSSESSION_CHANGE, WNDCLASSW, WS_CAPTION,
    WS_OVERLAPPED, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
};

use super::{Obsluga, ZdarzenieSystemowe};

static OBSLUGA: OnceLock<Obsluga> = OnceLock::new();

/// Uruchamia wątek z niewidocznym oknem, które odbiera zdarzenia sesji i zasilania.
pub fn nasluchuj(obsluga: Obsluga) {
    if OBSLUGA.set(obsluga).is_err() {
        return;
    }
    let watek = std::thread::Builder::new().name("zdarzenia-windows".into()).spawn(|| {
        if let Err(e) = unsafe { petla_komunikatow() } {
            log::error!("nasłuch blokady i uśpienia nie działa: {e}");
        }
    });
    if let Err(e) = watek {
        log::error!("nie mogę uruchomić wątku zdarzeń Windows: {e}");
    }
}

unsafe fn petla_komunikatow() -> windows::core::Result<()> {
    let instancja = GetModuleHandleW(None)?;
    let klasa = w!("AsystentkaZdarzeniaSystemowe");
    let opis = WNDCLASSW {
        lpfnWndProc: Some(procedura_okna),
        hInstance: instancja.into(),
        lpszClassName: klasa,
        ..Default::default()
    };
    if RegisterClassW(&opis) == 0 {
        return Err(windows::core::Error::from_thread());
    }
    // Zwykłe niewidoczne okno najwyższego poziomu: okna „message-only” nie
    // dostają rozgłaszanego WM_POWERBROADCAST.
    let okno = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        klasa,
        w!("Asystentka"),
        WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        None,
        None,
        Some(instancja.into()),
        None,
    )?;
    WTSRegisterSessionNotification(okno, NOTIFY_FOR_THIS_SESSION)?;
    log::info!("nasłuch blokady i uśpienia działa");
    let mut komunikat = MSG::default();
    while GetMessageW(&mut komunikat, None, 0, 0).0 > 0 {
        let _ = TranslateMessage(&komunikat);
        DispatchMessageW(&komunikat);
    }
    Ok(())
}

fn zglos(zdarzenie: ZdarzenieSystemowe) {
    if let Some(obsluga) = OBSLUGA.get() {
        obsluga(zdarzenie);
    }
}

unsafe extern "system" fn procedura_okna(okno: HWND, komunikat: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match komunikat {
        WM_WTSSESSION_CHANGE => {
            match wparam.0 as u32 {
                WTS_SESSION_LOCK => zglos(ZdarzenieSystemowe::Blokada),
                WTS_SESSION_UNLOCK => zglos(ZdarzenieSystemowe::Odblokowanie),
                _ => {}
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            match wparam.0 as u32 {
                PBT_APMSUSPEND => zglos(ZdarzenieSystemowe::Uspienie),
                PBT_APMRESUMEAUTOMATIC => zglos(ZdarzenieSystemowe::Wybudzenie),
                _ => {}
            }
            LRESULT(1)
        }
        WM_QUERYENDSESSION => LRESULT(1),
        WM_ENDSESSION => {
            if wparam.0 != 0 {
                zglos(ZdarzenieSystemowe::KoniecSesji);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(okno, komunikat, wparam, lparam),
    }
}

/// Sekundy bez myszy i klawiatury (GetLastInputInfo).
pub fn sekundy_bezczynnosci() -> Option<u64> {
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe {
        if !GetLastInputInfo(&mut info).as_bool() {
            return None;
        }
        Some(u64::from(GetTickCount().wrapping_sub(info.dwTime)) / 1000)
    }
}

/// Pełny ekran według SHQueryUserNotificationState, a w zapasie: okno na
/// pierwszym planie zajmuje cały monitor (bez pulpitu i paska zadań).
pub fn pelny_ekran(wlasne_okno: Option<isize>) -> bool {
    unsafe {
        if let Ok(stan) = SHQueryUserNotificationState() {
            if stan == QUNS_BUSY || stan == QUNS_RUNNING_D3D_FULL_SCREEN || stan == QUNS_PRESENTATION_MODE {
                return true;
            }
        }
        okno_na_pierwszym_planie_zajmuje_monitor(wlasne_okno)
    }
}

unsafe fn okno_na_pierwszym_planie_zajmuje_monitor(wlasne_okno: Option<isize>) -> bool {
    let okno = GetForegroundWindow();
    if okno.is_invalid() || Some(okno.0 as isize) == wlasne_okno {
        return false;
    }
    if okno == GetDesktopWindow() || okno == GetShellWindow() {
        return false;
    }
    let mut bufor = [0u16; 64];
    let dlugosc = GetClassNameW(okno, &mut bufor).max(0) as usize;
    let klasa = String::from_utf16_lossy(&bufor[..dlugosc]);
    if matches!(klasa.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
        return false;
    }
    // Zmaksymalizowane okna z paskiem tytułu to zwykła praca, nie pełny ekran.
    if IsZoomed(okno).as_bool() {
        return false;
    }
    let styl = GetWindowLongW(okno, GWL_STYLE) as u32;
    if styl & WS_CAPTION.0 == WS_CAPTION.0 {
        return false;
    }
    let mut prostokat = RECT::default();
    if GetWindowRect(okno, &mut prostokat).is_err() {
        return false;
    }
    let monitor = MonitorFromWindow(okno, MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !GetMonitorInfoW(monitor, &mut info).as_bool() {
        return false;
    }
    let m = info.rcMonitor;
    prostokat.left <= m.left && prostokat.top <= m.top && prostokat.right >= m.right && prostokat.bottom >= m.bottom
}

pub fn kursor() -> Option<(i32, i32)> {
    let mut punkt = POINT::default();
    unsafe { GetCursorPos(&mut punkt).ok()? };
    Some((punkt.x, punkt.y))
}

fn hwnd(okno: &WebviewWindow) -> Option<HWND> {
    // Tauri może używać innej wersji crate `windows`, więc przez surowy wskaźnik.
    okno.hwnd().ok().map(|h| HWND(h.0 as _))
}

pub fn uchwyt(okno: &WebviewWindow) -> Option<isize> {
    hwnd(okno).map(|h| h.0 as isize)
}

/// SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE): widget widać na
/// monitorze, ale nie w OBS ani na udostępnianym ekranie (Windows 10 2004+).
pub fn wyklucz_z_przechwytywania(okno: &WebviewWindow, wlacz: bool) -> Result<(), String> {
    let uchwyt = hwnd(okno).ok_or("brak uchwytu okna")?;
    let tryb = if wlacz { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
    unsafe { SetWindowDisplayAffinity(uchwyt, tryb) }.map_err(|e| e.to_string())
}

/// Pozycja i rozmiar jednym wywołaniem, żeby dolna krawędź nie drgała.
pub fn ustaw_prostokat(okno: &WebviewWindow, x: i32, y: i32, szer: u32, wys: u32) {
    if let Some(uchwyt) = hwnd(okno) {
        let wynik = unsafe { SetWindowPos(uchwyt, None, x, y, szer as i32, wys as i32, SWP_NOZORDER | SWP_NOACTIVATE) };
        if let Err(e) = wynik {
            log::warn!("SetWindowPos: {e}");
        }
    }
}

/// Pokazuje okno i oddaje fokus oknu, które miało go wcześniej.
pub fn pokaz_bez_aktywacji(okno: &WebviewWindow) {
    let poprzednie = unsafe { GetForegroundWindow() };
    let _ = okno.show();
    let Some(nasze) = hwnd(okno) else { return };
    unsafe {
        if GetForegroundWindow() == nasze
            && !poprzednie.is_invalid()
            && poprzednie != nasze
            && IsWindow(Some(poprzednie)).as_bool()
        {
            let _ = SetForegroundWindow(poprzednie);
        }
    }
}
