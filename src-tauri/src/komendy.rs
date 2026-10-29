//! Komendy wywoływane z frontendu. Wszystkie są `async`, więc działają poza
//! wątkiem głównym; zapis na Dysk czy do vaultu nie blokuje okna.

use std::sync::Arc;

use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::aplikacja::Aplikacja;
use crate::rdzen::Migawka;
use crate::ustawienia::{PodpowiedzKreatora, Ustawienia};

type Apl<'a> = State<'a, Arc<Aplikacja>>;

#[tauri::command]
pub async fn migawka(apl: Apl<'_>) -> Result<Migawka, String> {
    Ok(apl.migawka())
}

/// Pierwszy rysunek gotowy: rozmiar treści w pikselach logicznych.
#[tauri::command]
pub async fn gotowy(apl: Apl<'_>, szerokosc: f64, wysokosc: f64) -> Result<(), String> {
    apl.inner().gotowy(szerokosc, wysokosc);
    Ok(())
}

#[tauri::command]
pub async fn dopasuj_okno(apl: Apl<'_>, szerokosc: f64, wysokosc: f64) -> Result<(), String> {
    apl.inner().dopasuj(szerokosc, wysokosc);
    Ok(())
}

#[tauri::command]
pub async fn rozpocznij(apl: Apl<'_>, id: String) -> Result<(), String> {
    apl.inner().wykonaj(|r, t| r.rozpocznij(&id, t))
}

#[tauri::command]
pub async fn pauza(apl: Apl<'_>) -> Result<(), String> {
    apl.inner().wykonaj(|r, t| r.pauza(t));
    Ok(())
}

#[tauri::command]
pub async fn wznow(apl: Apl<'_>) -> Result<(), String> {
    apl.inner().wykonaj(|r, t| r.wznow(t));
    Ok(())
}

#[tauri::command]
pub async fn zrobione(apl: Apl<'_>) -> Result<(), String> {
    apl.inner().wykonaj(|r, t| r.zrobione(t));
    Ok(())
}

#[tauri::command]
pub async fn przerwij(apl: Apl<'_>) -> Result<(), String> {
    apl.inner().wykonaj(|r, t| r.przerwij(t));
    Ok(())
}

/// Odpowiedź na „Jesteś?”: `przerwa = true` odejmuje czas bezczynności.
#[tauri::command]
pub async fn odpowiedz_jestes(apl: Apl<'_>, przerwa: bool) -> Result<(), String> {
    apl.inner().wykonaj(|r, t| r.odpowiedz_jestes(przerwa, t));
    Ok(())
}

#[tauri::command]
pub async fn pokaz_liste(apl: Apl<'_>) -> Result<(), String> {
    apl.inner().wykonaj(|r, _| r.pokaz_liste());
    Ok(())
}

// ---- kreator, ustawienia i okno -----------------------------------------------

#[tauri::command]
pub async fn kreator_dane(apl: Apl<'_>) -> Result<PodpowiedzKreatora, String> {
    Ok(apl.podpowiedz_kreatora())
}

/// Kreator skończony: zapisuje ustawienia, w razie potrzeby tworzy plik zadań
/// z szablonu i otwiera go w Obsidianie.
#[tauri::command]
pub async fn kreator_zakoncz(apl: Apl<'_>, ustawienia: Ustawienia, utworz_plik: bool) -> Result<(), String> {
    apl.inner().zakoncz_kreator(ustawienia, utworz_plik)
}

#[tauri::command]
pub async fn ustawienia_pobierz(apl: Apl<'_>) -> Result<Ustawienia, String> {
    Ok(apl.ustawienia_biezace())
}

#[tauri::command]
pub async fn ustawienia_zapisz(apl: Apl<'_>, ustawienia: Ustawienia) -> Result<(), String> {
    apl.inner().zapisz_ustawienia(ustawienia)
}

#[tauri::command]
pub async fn otworz_ustawienia(apl: Apl<'_>) -> Result<(), String> {
    apl.inner().otworz_okno_ustawien();
    Ok(())
}

/// Czy pod tą ścieżką jest plik; kreator odróżnia nowy plik od istniejącego.
#[tauri::command]
pub async fn plik_istnieje(sciezka: String) -> Result<bool, String> {
    Ok(!sciezka.trim().is_empty() && std::path::Path::new(sciezka.trim()).is_file())
}

#[tauri::command]
pub async fn ustaw_fokus(apl: Apl<'_>, wlacz: bool) -> Result<(), String> {
    apl.inner().ustaw_fokus(wlacz);
    Ok(())
}

fn katalog_startowy(start: Option<&str>) -> Option<std::path::PathBuf> {
    let sciezka = std::path::PathBuf::from(start?.trim());
    if sciezka.is_dir() {
        Some(sciezka)
    } else {
        sciezka.parent().filter(|p| p.is_dir()).map(std::path::Path::to_path_buf)
    }
}

#[tauri::command]
pub async fn wybierz_plik(
    app: AppHandle,
    okno: WebviewWindow,
    start: Option<String>,
) -> Result<Option<String>, String> {
    let mut dialog =
        app.dialog().file().set_title("Wskaż plik zadań").add_filter("Markdown", &["md"]).set_parent(&okno);
    if let Some(katalog) = katalog_startowy(start.as_deref()) {
        dialog = dialog.set_directory(katalog);
    }
    Ok(dialog.blocking_pick_file().and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn wybierz_folder(
    app: AppHandle,
    okno: WebviewWindow,
    start: Option<String>,
    tytul: Option<String>,
) -> Result<Option<String>, String> {
    let mut dialog = app.dialog().file().set_title(tytul.as_deref().unwrap_or("Wskaż folder")).set_parent(&okno);
    if let Some(katalog) = katalog_startowy(start.as_deref()) {
        dialog = dialog.set_directory(katalog);
    }
    Ok(dialog.blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned()))
}
