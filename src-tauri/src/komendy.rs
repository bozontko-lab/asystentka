//! Komendy wywoływane z frontendu. Wszystkie są `async`, więc działają poza
//! wątkiem głównym; zapis na Dysk czy do vaultu nie blokuje okna.

use std::sync::Arc;

use tauri::State;

use crate::aplikacja::Aplikacja;
use crate::rdzen::Migawka;

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
