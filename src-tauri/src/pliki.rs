//! Odczyt i zapis plików: zapis atomowy, BOM z Notatnika, wyrozumiały JSON.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Zapis atomowy z SPEC.md (B3): najpierw plik tymczasowy w tym samym
/// folderze, potem zamiana nazwy, żeby Dysk Google ani Obsidian nie
/// zobaczyły połowy pliku.
pub fn zapisz_atomowo(sciezka: &Path, tresc: &[u8]) -> io::Result<()> {
    let folder = match sciezka.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    if !folder.is_dir() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("folder {} jest niedostępny", folder.display())));
    }
    let nazwa = sciezka
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ścieżka bez nazwy pliku"))?
        .to_string_lossy();
    let tymczasowy = folder.join(format!(".{nazwa}.{}.tmp", std::process::id()));
    {
        let mut plik = fs::File::create(&tymczasowy)?;
        plik.write_all(tresc)?;
        plik.sync_all()?;
    }
    // Dysk albo Obsidian mogą na chwilę trzymać plik docelowy, więc są
    // trzy szybkie próby; dłuższe ponawianie robi kolejka zapisów co 30 s.
    let mut blad = None;
    for proba in 0..3u64 {
        match fs::rename(&tymczasowy, sciezka) {
            Ok(()) => return Ok(()),
            Err(e) => {
                blad = Some(e);
                thread::sleep(Duration::from_millis(50 * (proba + 1)));
            }
        }
    }
    let _ = fs::remove_file(&tymczasowy);
    Err(blad.unwrap_or_else(|| io::Error::other("nieudana zamiana nazwy")))
}

/// Czyta plik tekstowy i usuwa BOM, który dopisuje Notatnik.
pub fn czytaj_tekst(sciezka: &Path) -> io::Result<String> {
    let tekst = fs::read_to_string(sciezka)?;
    Ok(match tekst.strip_prefix('\u{feff}') {
        Some(bez) => bez.to_string(),
        None => tekst,
    })
}

/// Naprawia pojedyncze ukośniki w ścieżkach Windows wpisanych ręcznie do
/// JSON-a, np. `"C:\Users\Ja"`. Poprawne sekwencje (`\\`, `\"`, `\n`, `\u0041`
/// itd.) zostają bez zmian.
pub fn napraw_ukosniki(json: &str) -> String {
    let znaki: Vec<char> = json.chars().collect();
    let mut wynik = String::with_capacity(json.len() + 8);
    let mut w_napisie = false;
    let mut i = 0;
    while i < znaki.len() {
        let z = znaki[i];
        if !w_napisie {
            if z == '"' {
                w_napisie = true;
            }
            wynik.push(z);
            i += 1;
            continue;
        }
        match z {
            '"' => {
                w_napisie = false;
                wynik.push(z);
                i += 1;
            }
            '\\' => {
                let nastepny = znaki.get(i + 1).copied();
                let poprawna = match nastepny {
                    Some('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't') => true,
                    Some('u') => znaki.len() > i + 5 && znaki[i + 2..i + 6].iter().all(|c| c.is_ascii_hexdigit()),
                    _ => false,
                };
                if poprawna {
                    wynik.push('\\');
                    wynik.push(nastepny.unwrap_or_default());
                    i += 2;
                } else {
                    wynik.push_str("\\\\");
                    i += 1;
                }
            }
            _ => {
                wynik.push(z);
                i += 1;
            }
        }
    }
    wynik
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn zapis_atomowy_podmienia_plik_i_sprzata() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("stan-zadan.json");
        zapisz_atomowo(&plik, b"pierwszy").unwrap();
        zapisz_atomowo(&plik, b"drugi").unwrap();
        assert_eq!(fs::read_to_string(&plik).unwrap(), "drugi");
        let pliki: Vec<_> = fs::read_dir(katalog.path()).unwrap().collect();
        assert_eq!(pliki.len(), 1, "plik tymczasowy powinien zniknąć");
    }

    #[test]
    fn zapis_do_brakujacego_folderu_zwraca_blad() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("brak").join("plik.json");
        let blad = zapisz_atomowo(&plik, b"x").unwrap_err();
        assert_eq!(blad.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn bom_jest_usuwany() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("u.json");
        fs::write(&plik, "\u{feff}{\"a\":1}").unwrap();
        assert_eq!(czytaj_tekst(&plik).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn naprawa_ukosnikow_w_sciezkach() {
        let zle = r#"{"folder": "C:\Users\Ja\Dziennik pracy", "ok": "a\\b", "n": "x\ny", "u": "\u0041"}"#;
        let naprawione = napraw_ukosniki(zle);
        let v: serde_json::Value = serde_json::from_str(&naprawione).unwrap();
        assert_eq!(v["folder"], r"C:\Users\Ja\Dziennik pracy");
        assert_eq!(v["ok"], r"a\b");
        assert_eq!(v["n"], "x\ny");
        assert_eq!(v["u"], "A");
    }
}
