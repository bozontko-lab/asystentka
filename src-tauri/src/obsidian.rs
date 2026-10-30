//! Obsidian: lista vaultów, adres `obsidian://open` i szablon pliku zadań
//! (SPEC.md, B2 i B3).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{Duration, NaiveDate};

use crate::czas;

/// Vaulty z `obsidian.json` w katalogu konfiguracji (`%APPDATA%` na Windows).
/// Najpierw te otwierane ostatnio. Istniejące foldery tylko.
pub fn vaulty(katalog_konfiguracji: &Path) -> Vec<PathBuf> {
    for nazwa in ["Obsidian", "obsidian"] {
        let plik = katalog_konfiguracji.join(nazwa).join("obsidian.json");
        if let Ok(tekst) = fs::read_to_string(&plik) {
            return z_json(&tekst);
        }
    }
    Vec::new()
}

fn z_json(tekst: &str) -> Vec<PathBuf> {
    let tekst = tekst.strip_prefix('\u{feff}').unwrap_or(tekst);
    let Ok(dokument) = serde_json::from_str::<serde_json::Value>(tekst) else { return Vec::new() };
    let Some(vaulty) = dokument.get("vaults").and_then(|v| v.as_object()) else { return Vec::new() };
    let mut wpisy: Vec<(i64, PathBuf)> = vaulty
        .values()
        .filter_map(|v| {
            let sciezka = PathBuf::from(v.get("path")?.as_str()?);
            let ts = v.get("ts").and_then(|t| t.as_i64()).unwrap_or(0);
            sciezka.is_dir().then_some((ts, sciezka))
        })
        .collect();
    wpisy.sort_by_key(|w| std::cmp::Reverse(w.0));
    wpisy.into_iter().map(|(_, p)| p).collect()
}

pub fn w_vaulcie(plik: &Path, vaulty: &[PathBuf]) -> bool {
    vaulty.iter().any(|v| plik.starts_with(v))
}

/// `obsidian://open?path=` z zakodowaną ścieżką pliku ([Obsidian URI](https://obsidian.md/help/uri)).
pub fn adres_otwarcia(plik: &Path) -> String {
    let sciezka = plik.to_string_lossy();
    let mut wynik = String::from("obsidian://open?path=");
    for bajt in sciezka.bytes() {
        match bajt {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => wynik.push(bajt as char),
            _ => wynik.push_str(&format!("%{bajt:02X}")),
        }
    }
    wynik
}

/// Szablon tworzony przez kreator; termin przykładu to dziś + 14 dni, a w komentarzu
/// stoi imię postaci (B3).
pub fn szablon_zadan(dzis: NaiveDate, postac: &str) -> String {
    let termin = czas::data_iso(dzis + Duration::days(14));
    format!(
        "# Zadania

<!-- Jak pisać zadania dla {postac}:
- jedna linia to jedno zadanie: - [ ] Nazwa [szac:: minuty]
- nagłówek ## grupuje zadania w projekt
- opcjonalnie termin: [termin:: RRRR-MM-DD]
- [czas:: …] i ✅ dopisuje {postac} -->

## Przykładowy projekt
- [ ] Pierwsze zadanie [szac:: 30]
- [ ] Drugie zadanie [szac:: 60] [termin:: {termin}]

## Inbox
"
    )
}

/// Tworzy plik zadań z szablonu. Istniejącego pliku nigdy nie nadpisuje.
pub fn utworz_plik_zadan(plik: &Path, dzis: NaiveDate, postac: &str) -> io::Result<()> {
    if let Some(folder) = plik.parent().filter(|f| !f.as_os_str().is_empty()) {
        if !folder.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("folder {} nie istnieje", folder.display())));
        }
    }
    let mut otwarty = fs::OpenOptions::new().write(true).create_new(true).open(plik)?;
    otwarty.write_all(szablon_zadan(dzis, postac).as_bytes())?;
    otwarty.sync_all()
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn vaulty_z_obsidian_json_najpierw_ostatnio_uzywane() {
        let konfig = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        fs::create_dir_all(konfig.path().join("Obsidian")).unwrap();
        let json = serde_json::json!({
            "vaults": {
                "1": {"path": a.path(), "ts": 100},
                "2": {"path": b.path(), "ts": 200, "open": true},
                "3": {"path": "/nie/ma/takiego", "ts": 300}
            }
        });
        fs::write(konfig.path().join("Obsidian").join("obsidian.json"), format!("\u{feff}{json}")).unwrap();
        let v = vaulty(konfig.path());
        assert_eq!(v, vec![b.path().to_path_buf(), a.path().to_path_buf()]);
        assert!(w_vaulcie(&a.path().join("Zadania.md"), &v));
        assert!(!w_vaulcie(Path::new("/gdzie/indziej/Zadania.md"), &v));
        assert!(vaulty(&konfig.path().join("brak")).is_empty());
    }

    #[test]
    fn adres_z_zakodowana_sciezka() {
        assert_eq!(
            adres_otwarcia(Path::new(r"C:\Users\Ola\Second Brain\Zadania.md")),
            "obsidian://open?path=C%3A%5CUsers%5COla%5CSecond%20Brain%5CZadania.md"
        );
        assert_eq!(
            adres_otwarcia(Path::new("/home/ola/Mój vault/Zadania.md")),
            "obsidian://open?path=%2Fhome%2Fola%2FM%C3%B3j%20vault%2FZadania.md"
        );
    }

    #[test]
    fn szablon_jak_w_b3_z_terminem_za_14_dni() {
        let s = szablon_zadan(NaiveDate::from_ymd_opt(2026, 10, 29).unwrap(), "Sai");
        assert!(s.starts_with("# Zadania\n\n<!-- Jak pisać zadania dla Sai:\n"));
        assert!(s.contains("- [czas:: …] i ✅ dopisuje Sai -->\n"), "imię postaci także w drugiej linii komentarza");
        assert!(!s.contains("{postac}"));
        assert!(s.contains("- [ ] Drugie zadanie [szac:: 60] [termin:: 2026-11-12]\n"));
        assert!(s.ends_with("## Inbox\n"));
    }

    #[test]
    fn szablon_przyjmuje_dowolne_imie_postaci() {
        let s = szablon_zadan(NaiveDate::from_ymd_opt(2026, 10, 29).unwrap(), "才ちゃん");
        assert!(s.contains("Jak pisać zadania dla 才ちゃん:"), "{s}");
        assert!(s.contains("dopisuje 才ちゃん -->"), "{s}");
    }

    #[test]
    fn nie_nadpisuje_istniejacego_pliku() {
        let k = tempfile::tempdir().unwrap();
        let plik = k.path().join("Zadania.md");
        let dzis = NaiveDate::from_ymd_opt(2026, 10, 29).unwrap();
        utworz_plik_zadan(&plik, dzis, "Sai").unwrap();
        fs::write(&plik, "moje zadania").unwrap();
        assert_eq!(utworz_plik_zadan(&plik, dzis, "Sai").unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&plik).unwrap(), "moje zadania");
        assert_eq!(
            utworz_plik_zadan(&k.path().join("brak").join("Z.md"), dzis, "Sai").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }
}
