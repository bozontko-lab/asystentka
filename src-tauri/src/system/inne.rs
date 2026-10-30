//! Zaślepki dla systemów innych niż Windows (praca nad interfejsem na Linuksie i macOS).

use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

use super::{Obsluga, StanPelnegoEkranu};

pub fn nasluchuj(_obsluga: Obsluga) {
    log::info!("zdarzenia blokady i uśpienia są obsługiwane tylko na Windows");
}

pub fn sekundy_bezczynnosci() -> Option<u64> {
    None
}

pub fn pelny_ekran(_wlasne_okno: Option<isize>) -> StanPelnegoEkranu {
    StanPelnegoEkranu::default()
}

pub fn kursor() -> Option<(i32, i32)> {
    None
}

pub fn uchwyt(_okno: &WebviewWindow) -> Option<isize> {
    None
}

pub fn wyklucz_z_przechwytywania(okno: &WebviewWindow, wlacz: bool) -> Result<(), String> {
    okno.set_content_protected(wlacz).map_err(|e| e.to_string())
}

pub fn ustaw_prostokat(okno: &WebviewWindow, x: i32, y: i32, szer: u32, wys: u32) {
    let _ = okno.set_size(PhysicalSize::new(szer, wys));
    let _ = okno.set_position(PhysicalPosition::new(x, y));
}

pub fn pokaz_bez_aktywacji(okno: &WebviewWindow) {
    let _ = okno.show();
}
