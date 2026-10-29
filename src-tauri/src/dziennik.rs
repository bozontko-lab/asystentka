//! Dziennik pracy w Obsidianie (SPEC.md, B3): jedna notatka `RRRR-MM-DD.md`
//! na dzień i jedna linia na sesję. Widget zmienia tylko swoje pola we
//! frontmatterze i blok między znacznikami; reszta pliku zostaje nietknięta.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::czas::{self, Chwila};
use crate::plik_zadan::BEZ_PROJEKTU;
use crate::pliki;

pub const ZNACZNIK_START: &str = "<!-- widget:start -->";
pub const ZNACZNIK_KONIEC: &str = "<!-- widget:end -->";

/// Zakończona sesja w rejestrze dnia, z którego odtwarzany jest blok w dzienniku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WpisSesji {
    pub projekt: String,
    pub zadanie: String,
    pub szac_min: Option<u32>,
    pub start: Chwila,
    pub koniec: Chwila,
    pub sekundy: i64,
    pub minuty: u32,
    /// `true` po „Zrobione”, `false` po „Przerwij” i po końcu dnia („w toku”).
    pub zrobione: bool,
}

/// Sekcja „Podsumowanie dnia”, dopisywana przy końcu pracy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Podsumowanie {
    /// Nazwa reakcji, np. „lekko pochwalająca”.
    pub reakcja: String,
    /// Zadania z planu bez ptaszka.
    pub na_jutro: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DzienPracy {
    pub data: NaiveDate,
    pub sesje: Vec<WpisSesji>,
    pub cel_min: u32,
    pub zaplanowane: u32,
    pub zrobione: u32,
    pub podsumowanie: Option<Podsumowanie>,
}

impl Default for DzienPracy {
    fn default() -> Self {
        DzienPracy {
            data: NaiveDate::from_ymd_opt(1970, 1, 1).expect("poprawna data"),
            sesje: Vec::new(),
            cel_min: 240,
            zaplanowane: 0,
            zrobione: 0,
            podsumowanie: None,
        }
    }
}

impl DzienPracy {
    pub fn minuty(&self) -> u32 {
        self.sesje.iter().map(|s| s.minuty).sum()
    }

    pub fn blok(&self) -> String {
        let mut blok = format!("{ZNACZNIK_START}\n## Praca · {}\n\n", czas::data_pl(self.data));
        for sesja in &self.sesje {
            blok.push_str(&linia(sesja));
            blok.push('\n');
        }
        let zadan = if self.zaplanowane == 1 { "zadania" } else { "zadań" };
        blok.push_str(&format!(
            "\n**Razem:** {} z celu {} · **Plan:** {} z {} {zadan}\n",
            czas::minuty_tekst(self.minuty()),
            czas::minuty_tekst(self.cel_min),
            self.zrobione,
            self.zaplanowane,
        ));
        if let Some(p) = &self.podsumowanie {
            blok.push_str(&format!("\n### Podsumowanie dnia\nReakcja: {}\n", p.reakcja));
            if !p.na_jutro.is_empty() {
                blok.push_str(&format!("Na jutro: {}\n", p.na_jutro.join(", ")));
            }
        }
        blok.push_str(ZNACZNIK_KONIEC);
        blok
    }
}

/// `- [x] 09:12–10:05 · Scenariusz: intro · 47 min (szac. 60) · Film #1 · FIFA`
pub fn linia(s: &WpisSesji) -> String {
    let godziny = format!("{}–{}", czas::godzina_minuta(s.start), czas::godzina_minuta(s.koniec));
    let projekt =
        if s.projekt.is_empty() || s.projekt == BEZ_PROJEKTU { String::new() } else { format!(" · {}", s.projekt) };
    if s.zrobione {
        let szacunek = s.szac_min.map(|m| format!(" (szac. {m})")).unwrap_or_default();
        format!("- [x] {godziny} · {} · {} min{szacunek}{projekt}", s.zadanie, s.minuty)
    } else {
        format!("- [ ] {godziny} · {} · {} min · w toku{projekt}", s.zadanie, s.minuty)
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
        ("cel_min", dzien.cel_min.to_string()),
        ("zaplanowane", dzien.zaplanowane.to_string()),
        ("zrobione", dzien.zrobione.to_string()),
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

    /// Przykład z B3 (lekko pochwalająca, plan 2 z 4).
    const PRZYKLAD: &str = "---
data: 2026-10-30
minuty: 132
cel_min: 240
zaplanowane: 4
zrobione: 2
---
<!-- widget:start -->
## Praca · 30.10.2026

- [x] 09:12–10:05 · Scenariusz: intro · 47 min (szac. 60) · Film #1 · FIFA
- [ ] 10:20–11:05 · Montaż: plansze 1–5 · 45 min · w toku · Film #1 · FIFA
- [x] 11:30–12:10 · Miniatura · 40 min (szac. 30) · Film #1 · FIFA

**Razem:** 2 h 12 min z celu 4 h · **Plan:** 2 z 4 zadań

### Podsumowanie dnia
Reakcja: lekko pochwalająca
Na jutro: Montaż: plansze 1–5, Odpisać na maila od montażysty
<!-- widget:end -->
";

    fn wpis(zadanie: &str, szac: u32, start: &str, koniec: &str, minuty: u32, zrobione: bool) -> WpisSesji {
        WpisSesji {
            projekt: "Film #1 · FIFA".into(),
            zadanie: zadanie.into(),
            szac_min: Some(szac),
            start: chwila(start),
            koniec: chwila(koniec),
            sekundy: i64::from(minuty) * 60,
            minuty,
            zrobione,
        }
    }

    fn dzien_z_przykladu() -> DzienPracy {
        DzienPracy {
            data: NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
            sesje: vec![
                wpis("Scenariusz: intro", 60, "2026-10-30T09:12:00+01:00", "2026-10-30T10:05:00+01:00", 47, true),
                wpis("Montaż: plansze 1–5", 120, "2026-10-30T10:20:00+01:00", "2026-10-30T11:05:00+01:00", 45, false),
                wpis("Miniatura", 30, "2026-10-30T11:30:00+01:00", "2026-10-30T12:10:00+01:00", 40, true),
            ],
            cel_min: 240,
            zaplanowane: 4,
            zrobione: 2,
            podsumowanie: Some(Podsumowanie {
                reakcja: "lekko pochwalająca".into(),
                na_jutro: vec!["Montaż: plansze 1–5".into(), "Odpisać na maila od montażysty".into()],
            }),
        }
    }

    #[test]
    fn nowy_plik_jak_w_specyfikacji() {
        assert_eq!(aktualizuj_tresc(None, &dzien_z_przykladu()), PRZYKLAD);
    }

    #[test]
    fn bez_podsumowania_blok_konczy_sie_linia_razem() {
        let dzien = DzienPracy { podsumowanie: None, ..dzien_z_przykladu() };
        let tresc = aktualizuj_tresc(None, &dzien);
        assert!(tresc.contains("**Plan:** 2 z 4 zadań\n<!-- widget:end -->\n"));
        assert!(!tresc.contains("Podsumowanie dnia"));
    }

    #[test]
    fn wroc_do_pracy_usuwa_podsumowanie_i_zachowuje_notatki() {
        let stara = format!("# Moje notatki\n\n{PRZYKLAD}\n## Wieczór\n- coś\n");
        let dzien = DzienPracy { podsumowanie: None, ..dzien_z_przykladu() };
        let nowa = aktualizuj_tresc(Some(&stara), &dzien);
        assert!(!nowa.contains("Podsumowanie dnia"));
        assert!(nowa.ends_with("<!-- widget:end -->\n\n## Wieczór\n- coś\n"));
        // Frontmatter bez notatek powstaje przed nimi.
        assert!(
            nowa.starts_with("---\ndata: 2026-10-30\nminuty: 132\ncel_min: 240\nzaplanowane: 4\nzrobione: 2\n---\n")
        );
    }

    #[test]
    fn notatka_z_wersji_1_dostaje_nowe_pola_we_frontmatterze() {
        let stara = "---\ndata: 2026-10-30\nminuty: 92\nzrobione: 1\ntags: [praca]\n---\n<!-- widget:start -->\nstary blok\n<!-- widget:end -->\n";
        let nowa = aktualizuj_tresc(Some(stara), &dzien_z_przykladu());
        assert!(nowa.starts_with(
            "---\ndata: 2026-10-30\nminuty: 132\nzrobione: 2\ntags: [praca]\ncel_min: 240\nzaplanowane: 4\n---\n<!-- widget:start -->\n## Praca"
        ));
        assert!(!nowa.contains("stary blok"));
    }

    #[test]
    fn plik_bez_bloku_i_bez_frontmattera() {
        let nowa = aktualizuj_tresc(Some("Tylko notatka\n"), &dzien_z_przykladu());
        assert!(nowa.starts_with("---\ndata: 2026-10-30\n"));
        assert!(nowa.ends_with("<!-- widget:end -->\n\nTylko notatka\n"));
    }

    #[test]
    fn zachowuje_crlf_i_bom() {
        let stary = format!("\u{feff}{}", PRZYKLAD.replace('\n', "\r\n")).replace("2 h 12 min z celu", "1 min z celu");
        let nowy = aktualizuj_tresc(Some(&stary), &dzien_z_przykladu());
        assert_eq!(nowy, format!("\u{feff}{}", PRZYKLAD.replace('\n', "\r\n")));
    }

    #[test]
    fn zadanie_bez_projektu_i_jedno_zaplanowane() {
        let mut w = wpis("Luźne", 10, "2026-10-30T09:00:00+01:00", "2026-10-30T09:10:00+01:00", 10, true);
        w.projekt = BEZ_PROJEKTU.into();
        assert_eq!(linia(&w), "- [x] 09:00–09:10 · Luźne · 10 min (szac. 10)");
        let d = DzienPracy { sesje: vec![w], zaplanowane: 1, zrobione: 1, ..DzienPracy::default() };
        assert!(d.blok().contains("**Plan:** 1 z 1 zadania"));
        w = wpis("Bez szacunku", 0, "2026-10-30T09:00:00+01:00", "2026-10-30T09:05:00+01:00", 5, true);
        w.szac_min = None;
        assert_eq!(linia(&w), "- [x] 09:00–09:05 · Bez szacunku · 5 min · Film #1 · FIFA");
    }

    #[test]
    fn zapis_tworzy_tylko_folder_dziennika() {
        let vault = tempfile::tempdir().unwrap();
        let folder = vault.path().join("Dziennik pracy");
        zapisz(&folder, &dzien_z_przykladu()).unwrap();
        assert_eq!(std::fs::read_to_string(folder.join("2026-10-30.md")).unwrap(), PRZYKLAD);
        let brak = vault.path().join("brak").join("Dziennik pracy");
        assert!(zapisz(&brak, &dzien_z_przykladu()).is_err());
        assert!(!brak.exists());
    }
}
