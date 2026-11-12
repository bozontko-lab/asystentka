//! Odczyt i zapis plików: zapis atomowy, BOM z Notatnika, wyrozumiały JSON.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

/// Zapisuje treść do pliku tymczasowego w folderze pliku docelowego i zwraca
/// jego ścieżkę. Zamianę nazwy robi wywołujący.
fn zapisz_tymczasowy(sciezka: &Path, tresc: &[u8]) -> io::Result<PathBuf> {
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
    // Każdy zapis ma własny plik tymczasowy, także dwa naraz w jednym procesie.
    static NUMER: AtomicU64 = AtomicU64::new(0);
    let numer = NUMER.fetch_add(1, Ordering::Relaxed);
    let tymczasowy = folder.join(format!(".{nazwa}.{}.{numer}.tmp", std::process::id()));
    let zapis = fs::File::create(&tymczasowy).and_then(|mut plik| {
        plik.write_all(tresc)?;
        plik.sync_all()
    });
    if let Err(e) = zapis {
        let _ = fs::remove_file(&tymczasowy);
        return Err(e);
    }
    Ok(tymczasowy)
}

/// Zapis atomowy z SPEC.md (B3): najpierw plik tymczasowy w tym samym
/// folderze, potem zamiana nazwy, żeby Dysk Google ani Obsidian nie
/// zobaczyły połowy pliku.
pub fn zapisz_atomowo(sciezka: &Path, tresc: &[u8]) -> io::Result<()> {
    let tymczasowy = zapisz_tymczasowy(sciezka, tresc)?;
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

/// Zapis atomowy pod warunkiem, że plik docelowy ma wciąż treść `oczekiwana`,
/// czyli tę, z której powstała `tresc`. Sprawdzenie jest tuż przed zamianą
/// nazwy, a zamiana ma jedną próbę: gdy plik zmienił w międzyczasie ktoś inny
/// (Hub w Obsidianie, synchronizacja) albo plik jest zajęty, wywołujący czyta
/// go od nowa i powtarza całą zmianę, zamiast nadpisać cudzy zapis starą treścią.
///
/// `Ok(false)` znaczy: plik się zmienił i nic nie zostało zapisane.
pub fn zapisz_atomowo_bez_zmian(sciezka: &Path, tresc: &[u8], oczekiwana: &[u8]) -> io::Result<bool> {
    let tymczasowy = zapisz_tymczasowy(sciezka, tresc)?;
    let wynik = match fs::read(sciezka) {
        Ok(obecna) if obecna == oczekiwana => fs::rename(&tymczasowy, sciezka).map(|()| true),
        Ok(_) => Ok(false),
        Err(e) => Err(e),
    };
    if !matches!(wynik, Ok(true)) {
        let _ = fs::remove_file(&tymczasowy);
    }
    wynik
}

/// Ile kopii jednego pliku zostaje w folderze kopii. Plik zadań ma kilka
/// kilobajtów, więc to kilka tygodni historii w około megabajcie.
pub const KOPII_MAX: usize = 200;

/// Odkłada kopię zapasową pliku: `tresc` trafia do `folder` jako
/// `<nazwa>-<znacznik>-<numer>.<rozszerzenie>`, np. `Zadania-20261112-124501-0003.md`.
/// Nazwy ułożone alfabetycznie są ułożone w czasie. Treść taka sama jak
/// w najnowszej kopii nie jest odkładana drugi raz, a w folderze zostaje
/// `zostaw` najnowszych kopii tego pliku.
pub fn odloz_kopie(folder: &Path, plik: &Path, tresc: &[u8], znacznik: &str, zostaw: usize) -> io::Result<()> {
    let nazwa =
        plik.file_stem().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ścieżka bez nazwy pliku"))?;
    let poczatek = format!("{}-", nazwa.to_string_lossy());
    let koniec = plik.extension().map(|r| format!(".{}", r.to_string_lossy())).unwrap_or_default();
    fs::create_dir_all(folder)?;
    let mut kopie: Vec<String> = fs::read_dir(folder)?
        .filter_map(|wpis| wpis.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with(&poczatek) && n.ends_with(&koniec) && !n.starts_with('.'))
        .collect();
    kopie.sort();
    if let Some(najnowsza) = kopie.last() {
        if fs::read(folder.join(najnowsza)).is_ok_and(|stara| stara == tresc) {
            return Ok(());
        }
    }
    // Numer rozróżnia kopie z tej samej sekundy.
    static NUMER: AtomicU64 = AtomicU64::new(0);
    let numer = NUMER.fetch_add(1, Ordering::Relaxed);
    zapisz_atomowo(&folder.join(format!("{poczatek}{znacznik}-{numer:04}{koniec}")), tresc)?;
    let za_duzo = (kopie.len() + 1).saturating_sub(zostaw.max(1));
    for stara in kopie.iter().take(za_duzo) {
        let _ = fs::remove_file(folder.join(stara));
    }
    Ok(())
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
    fn rownolegle_zapisy_tego_samego_pliku_sie_nie_zderzaja() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = std::sync::Arc::new(katalog.path().join("ustawienia.json"));
        let watki: Vec<_> = (0..8)
            .map(|nr| {
                let plik = std::sync::Arc::clone(&plik);
                std::thread::spawn(move || {
                    let tresc = format!("{{\"watek\": {nr}}}").repeat(2000);
                    (0..40).map(|_| zapisz_atomowo(&plik, tresc.as_bytes()).is_ok()).filter(|ok| !ok).count()
                })
            })
            .collect();
        let bledy: usize = watki.into_iter().map(|w| w.join().unwrap()).sum();
        assert_eq!(bledy, 0, "każdy zapis ma własny plik tymczasowy");
        let tresc = fs::read_to_string(&*plik).unwrap();
        assert!(
            (0..8).any(|nr| tresc == format!("{{\"watek\": {nr}}}").repeat(2000)),
            "plik jest cały z jednego zapisu"
        );
        assert_eq!(fs::read_dir(katalog.path()).unwrap().count(), 1, "bez pozostawionych plików tymczasowych");
    }

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
    fn zapis_bez_zmian_odmawia_gdy_plik_zmienil_sie_od_odczytu() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("Zadania.md");
        fs::write(&plik, b"odczytane").unwrap();
        // Plik jest taki, jak przy odczycie: zapis przechodzi.
        assert!(zapisz_atomowo_bez_zmian(&plik, b"nowe", b"odczytane").unwrap());
        assert_eq!(fs::read_to_string(&plik).unwrap(), "nowe");
        // Ktoś zapisał plik po odczycie: nic nie nadpisujemy.
        fs::write(&plik, b"cudzy zapis").unwrap();
        assert!(!zapisz_atomowo_bez_zmian(&plik, b"jeszcze nowsze", b"nowe").unwrap());
        assert_eq!(fs::read_to_string(&plik).unwrap(), "cudzy zapis");
        assert_eq!(fs::read_dir(katalog.path()).unwrap().count(), 1, "plik tymczasowy znika");
    }

    #[test]
    fn kopie_bez_powtorek_w_kolejnosci_i_z_limitem() {
        let katalog = tempfile::tempdir().unwrap();
        let folder = katalog.path().join("kopie");
        let plik = katalog.path().join("vault").join("Zadania.md");
        let nazwy = || {
            let mut n: Vec<String> =
                fs::read_dir(&folder).unwrap().map(|w| w.unwrap().file_name().into_string().unwrap()).collect();
            n.sort();
            n
        };
        odloz_kopie(&folder, &plik, b"wersja 1", "20261112-090000", 3).unwrap();
        // Ta sama treść drugi raz (np. ponowiony zapis): bez nowej kopii.
        odloz_kopie(&folder, &plik, b"wersja 1", "20261112-090001", 3).unwrap();
        assert_eq!(nazwy().len(), 1);
        assert!(nazwy()[0].starts_with("Zadania-20261112-090000-") && nazwy()[0].ends_with(".md"), "{:?}", nazwy());
        // Obcy plik w folderze kopii zostaje nietknięty i nie liczy się do limitu.
        fs::write(folder.join("notatka.txt"), "moje").unwrap();
        for (i, znacznik) in ["20261112-090002", "20261112-090002", "20261113-080000"].iter().enumerate() {
            odloz_kopie(&folder, &plik, format!("wersja {}", i + 2).as_bytes(), znacznik, 3).unwrap();
        }
        let kopie: Vec<String> = nazwy().into_iter().filter(|n| n.starts_with("Zadania-")).collect();
        assert_eq!(kopie.len(), 3, "zostają trzy najnowsze: {kopie:?}");
        let tresci: Vec<String> = kopie.iter().map(|n| fs::read_to_string(folder.join(n)).unwrap()).collect();
        assert_eq!(
            tresci,
            ["wersja 2", "wersja 3", "wersja 4"],
            "najstarsza znika, kolejność nazw to kolejność w czasie"
        );
        assert_eq!(fs::read_to_string(folder.join("notatka.txt")).unwrap(), "moje");
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
