//! Dziennik pracy w Obsidianie (SPEC.md, B3): jeden plik `RRRR-MM-DD.md` na
//! dzień, jedna linia na sesję. Widget zmienia tylko swoje pola we
//! frontmatterze i blok między znacznikami; reszta pliku zostaje nietknięta.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::czas::{self, Chwila};
use crate::pliki;
use crate::zadania::ZROBIONE;

pub const ZNACZNIK_START: &str = "<!-- widget:start -->";
pub const ZNACZNIK_KONIEC: &str = "<!-- widget:end -->";

/// Zakończona sesja w lokalnym rejestrze dnia, z którego odtwarzany jest blok.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WpisSesji {
    pub id: String,
    /// Nazwa z dzisiejszym fragmentem, np. `Montaż: plansze 1–5`.
    pub nazwa: String,
    pub szac_min: Option<u32>,
    pub start: Chwila,
    pub koniec: Chwila,
    pub sekundy: i64,
    pub minuty: u32,
    /// `Zrobione` albo `W toku`.
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DzienPracy {
    pub data: NaiveDate,
    pub sesje: Vec<WpisSesji>,
}

impl DzienPracy {
    pub fn nowy(data: NaiveDate) -> DzienPracy {
        DzienPracy { data, sesje: Vec::new() }
    }

    pub fn minuty(&self) -> u32 {
        self.sesje.iter().map(|s| s.minuty).sum()
    }

    pub fn minuty_zadania(&self, id: &str) -> u32 {
        self.sesje.iter().filter(|s| s.id == id).map(|s| s.minuty).sum()
    }

    /// Nazwy zrobionych dziś zadań, bez powtórzeń, w kolejności zakończenia.
    pub fn zrobione(&self) -> Vec<String> {
        let mut widziane = BTreeSet::new();
        self.sesje
            .iter()
            .filter(|s| s.status == ZROBIONE && widziane.insert(s.id.clone()))
            .map(|s| s.nazwa.clone())
            .collect()
    }

    pub fn blok(&self) -> String {
        let mut blok = format!("{ZNACZNIK_START}\n## Praca · {}\n\n", czas::data_pl(self.data));
        for sesja in &self.sesje {
            blok.push_str(&linia(sesja));
            blok.push('\n');
        }
        blok.push_str(&format!("\n**Razem:** {}\n{ZNACZNIK_KONIEC}", czas::minuty_tekst(self.minuty())));
        blok
    }
}

/// `- [x] 09:12–10:05 · Scenariusz: intro · 47 min (szac. 60) · #12`
pub fn linia(s: &WpisSesji) -> String {
    let godziny = format!("{}–{}", czas::godzina_minuta(s.start), czas::godzina_minuta(s.koniec));
    if s.status == ZROBIONE {
        let szacunek = s.szac_min.map(|m| format!(" (szac. {m})")).unwrap_or_default();
        format!("- [x] {godziny} · {} · {} min{szacunek} · #{}", s.nazwa, s.minuty, s.id)
    } else {
        format!("- [ ] {godziny} · {} · {} min · w toku · #{}", s.nazwa, s.minuty, s.id)
    }
}

/// Dzieli tekst na linie frontmattera i resztę pliku.
fn rozdziel_frontmatter(tekst: &str) -> (Option<Vec<&str>>, &str) {
    let Some(po_otwarciu) = tekst.strip_prefix("---\n") else {
        return (None, tekst);
    };
    let mut pozycja = 0;
    for linia in po_otwarciu.split_inclusive('\n') {
        if linia.trim_end_matches('\n').trim_end() == "---" {
            let linie = po_otwarciu[..pozycja].lines().collect();
            return (Some(linie), &po_otwarciu[pozycja + linia.len()..]);
        }
        pozycja += linia.len();
    }
    (None, tekst)
}

fn ustaw_pola(linie: Vec<&str>, pola: &[(&str, String)]) -> Vec<String> {
    let mut wynik: Vec<String> = linie.into_iter().map(str::to_string).collect();
    for (klucz, wartosc) in pola {
        let nowa = format!("{klucz}: {wartosc}");
        let prefiks = format!("{klucz}:");
        match wynik.iter_mut().find(|l| l.starts_with(&prefiks)) {
            Some(linia) => *linia = nowa,
            None => wynik.push(nowa),
        }
    }
    wynik
}

/// Nowa treść pliku dziennika na podstawie starej (albo `None` dla nowego pliku).
pub fn aktualizuj_tresc(stara: Option<&str>, dzien: &DzienPracy) -> String {
    let stara = stara.unwrap_or_default();
    let bom = stara.starts_with('\u{feff}');
    let tekst = stara.trim_start_matches('\u{feff}');
    let crlf = tekst.contains("\r\n");
    let tekst = tekst.replace("\r\n", "\n");

    let pola = [
        ("data", czas::data_iso(dzien.data)),
        ("minuty", dzien.minuty().to_string()),
        ("zrobione", dzien.zrobione().len().to_string()),
    ];
    let (frontmatter, reszta) = rozdziel_frontmatter(&tekst);
    let frontmatter = ustaw_pola(frontmatter.unwrap_or_default(), &pola);

    let blok = dzien.blok();
    let poczatek_bloku = reszta.find(ZNACZNIK_START);
    let koniec_bloku =
        poczatek_bloku.and_then(|a| reszta[a..].find(ZNACZNIK_KONIEC).map(|b| a + b + ZNACZNIK_KONIEC.len()));
    let reszta = match (poczatek_bloku, koniec_bloku) {
        (Some(a), Some(b)) => format!("{}{blok}{}", &reszta[..a], &reszta[b..]),
        _ => {
            let notatki = reszta.trim_start_matches('\n');
            if notatki.is_empty() {
                format!("{blok}\n")
            } else {
                format!("{blok}\n\n{notatki}")
            }
        }
    };

    let mut wynik = format!("---\n{}\n---\n{reszta}", frontmatter.join("\n"));
    if !wynik.ends_with('\n') {
        wynik.push('\n');
    }
    if crlf {
        wynik = wynik.replace('\n', "\r\n");
    }
    if bom {
        wynik.insert(0, '\u{feff}');
    }
    wynik
}

pub fn sciezka_pliku(folder: &Path, data: NaiveDate) -> PathBuf {
    folder.join(format!("{}.md", czas::data_iso(data)))
}

/// Zapisuje dzień do pliku w vaulcie. Tworzy sam folder dziennika, jeśli
/// istnieje folder nad nim (vault), ale nie zakłada całych ścieżek.
pub fn zapisz(folder: &Path, dzien: &DzienPracy) -> Result<(), String> {
    if !folder.is_dir() {
        match folder.parent() {
            Some(vault) if vault.is_dir() => {
                std::fs::create_dir(folder).map_err(|e| format!("nie mogę utworzyć {}: {e}", folder.display()))?
            }
            _ => return Err(format!("folder dziennika {} jest niedostępny", folder.display())),
        }
    }
    let plik = sciezka_pliku(folder, dzien.data);
    let stara = match pliki::czytaj_tekst(&plik) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("nie mogę przeczytać {}: {e}", plik.display())),
    };
    let nowa = aktualizuj_tresc(stara.as_deref(), dzien);
    if stara.as_deref() == Some(nowa.as_str()) {
        return Ok(());
    }
    pliki::zapisz_atomowo(&plik, nowa.as_bytes()).map_err(|e| format!("nie mogę zapisać {}: {e}", plik.display()))
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::czas::chwila;
    use crate::zadania::W_TOKU;

    const PRZYKLAD: &str = "---
data: 2026-10-30
minuty: 132
zrobione: 2
---
<!-- widget:start -->
## Praca · 30.10.2026

- [x] 09:12–10:05 · Scenariusz: intro · 47 min (szac. 60) · #12
- [ ] 10:20–11:05 · Montaż: plansze 1–5 · 45 min · w toku · #15
- [x] 11:30–12:10 · Miniatura: szkic · 40 min (szac. 30) · #18

**Razem:** 2 h 12 min
<!-- widget:end -->
";

    fn wpis(id: &str, nazwa: &str, szac: u32, start: &str, koniec: &str, minuty: u32, status: &str) -> WpisSesji {
        WpisSesji {
            id: id.into(),
            nazwa: nazwa.into(),
            szac_min: Some(szac),
            start: chwila(start),
            koniec: chwila(koniec),
            sekundy: i64::from(minuty) * 60,
            minuty,
            status: status.into(),
        }
    }

    fn dzien_z_przykladu() -> DzienPracy {
        DzienPracy {
            data: NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
            sesje: vec![
                wpis(
                    "12",
                    "Scenariusz: intro",
                    60,
                    "2026-10-30T09:12:00+01:00",
                    "2026-10-30T10:05:00+01:00",
                    47,
                    ZROBIONE,
                ),
                wpis(
                    "15",
                    "Montaż: plansze 1–5",
                    120,
                    "2026-10-30T10:20:00+01:00",
                    "2026-10-30T11:05:00+01:00",
                    45,
                    W_TOKU,
                ),
                wpis(
                    "18",
                    "Miniatura: szkic",
                    30,
                    "2026-10-30T11:30:00+01:00",
                    "2026-10-30T12:10:00+01:00",
                    40,
                    ZROBIONE,
                ),
            ],
        }
    }

    #[test]
    fn nowy_plik_jak_w_specyfikacji() {
        assert_eq!(aktualizuj_tresc(None, &dzien_z_przykladu()), PRZYKLAD);
    }

    #[test]
    fn notatki_poza_blokiem_zostaja() {
        let mut dzien = dzien_z_przykladu();
        let pierwszy = dzien.sesje.remove(2);
        let stary = format!(
            "---\ndata: 2026-10-30\ntags: [praca]\nminuty: 92\nzrobione: 1\n---\n# Moje notatki\n\nRano kawa.\n\n{}\n\n## Wieczór\n- [ ] własne zadanie\n",
            DzienPracy { sesje: dzien.sesje.clone(), ..dzien.clone() }.blok()
        );
        dzien.sesje.push(pierwszy);
        let nowy = aktualizuj_tresc(Some(&stary), &dzien);
        assert!(nowy.starts_with("---\ndata: 2026-10-30\ntags: [praca]\nminuty: 132\nzrobione: 2\n---\n# Moje notatki\n\nRano kawa.\n\n<!-- widget:start -->"));
        assert!(nowy.ends_with("<!-- widget:end -->\n\n## Wieczór\n- [ ] własne zadanie\n"));
        assert!(nowy.contains("Miniatura: szkic · 40 min (szac. 30) · #18"));
        assert_eq!(nowy.matches(ZNACZNIK_START).count(), 1);
    }

    #[test]
    fn plik_bez_bloku_dostaje_go_po_frontmatterze() {
        let stary = "---\ntags: [dziennik]\n---\nNotatka z rana.\n";
        let nowy = aktualizuj_tresc(Some(stary), &dzien_z_przykladu());
        assert!(nowy.starts_with(
            "---\ntags: [dziennik]\ndata: 2026-10-30\nminuty: 132\nzrobione: 2\n---\n<!-- widget:start -->"
        ));
        assert!(nowy.ends_with("<!-- widget:end -->\n\nNotatka z rana.\n"));
    }

    #[test]
    fn plik_bez_frontmattera() {
        let nowy = aktualizuj_tresc(Some("Tylko notatka\n"), &dzien_z_przykladu());
        assert!(nowy.starts_with("---\ndata: 2026-10-30\nminuty: 132\nzrobione: 2\n---\n<!-- widget:start -->"));
        assert!(nowy.ends_with("<!-- widget:end -->\n\nTylko notatka\n"));
    }

    #[test]
    fn zachowuje_crlf_i_bom() {
        let stary =
            format!("\u{feff}{}", PRZYKLAD.replace('\n', "\r\n")).replace("Razem:** 2 h 12 min", "Razem:** stare");
        let nowy = aktualizuj_tresc(Some(&stary), &dzien_z_przykladu());
        assert_eq!(nowy, format!("\u{feff}{}", PRZYKLAD.replace('\n', "\r\n")));
    }

    #[test]
    fn podsumowanie_dnia() {
        let dzien = dzien_z_przykladu();
        assert_eq!(dzien.minuty(), 132);
        assert_eq!(dzien.minuty_zadania("15"), 45);
        assert_eq!(dzien.zrobione(), vec!["Scenariusz: intro", "Miniatura: szkic"]);
    }

    #[test]
    fn zapis_tworzy_tylko_folder_dziennika() {
        let vault = tempfile::tempdir().unwrap();
        let folder = vault.path().join("Dziennik pracy");
        zapisz(&folder, &dzien_z_przykladu()).unwrap();
        let tresc = std::fs::read_to_string(folder.join("2026-10-30.md")).unwrap();
        assert_eq!(tresc, PRZYKLAD);
        let brak = vault.path().join("brak").join("Dziennik pracy");
        assert!(zapisz(&brak, &dzien_z_przykladu()).is_err());
        assert!(!brak.exists());
    }
}
