//! Ustawienia z `%APPDATA%\Asystentka\ustawienia.json` (SPEC.md, B7).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveTime;
use serde::{Deserialize, Serialize};

use crate::czas;
use crate::pliki;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ustawienia {
    pub imie: String,
    pub folder_planow: String,
    pub plik_stanu: String,
    pub folder_dziennika: String,
    pub godzina_planu: String,
    pub godzina_konca_dnia: String,
    pub prog_bezczynnosci_min: u32,
    pub przypomnienie_przerwy_min: u32,
    pub ukryj_przed_nagrywaniem: bool,
    pub chowaj_przy_pelnym_ekranie: bool,
    pub skrot_pokaz_ukryj: String,
}

impl Default for Ustawienia {
    fn default() -> Self {
        Ustawienia {
            imie: "Dżiordżio".into(),
            folder_planow: r"<Dysk Google>\Asystentka\plany".into(),
            plik_stanu: r"<Dysk Google>\Asystentka\stan-zadan.json".into(),
            folder_dziennika: r"<vault>\Dziennik pracy".into(),
            godzina_planu: "07:40".into(),
            godzina_konca_dnia: "22:00".into(),
            prog_bezczynnosci_min: 15,
            przypomnienie_przerwy_min: 50,
            ukryj_przed_nagrywaniem: true,
            chowaj_przy_pelnym_ekranie: true,
            skrot_pokaz_ukryj: "Ctrl+Alt+Space".into(),
        }
    }
}

impl Ustawienia {
    pub fn folder_planow(&self) -> Option<PathBuf> {
        sciezka(&self.folder_planow)
    }

    pub fn plik_stanu(&self) -> Option<PathBuf> {
        sciezka(&self.plik_stanu)
    }

    pub fn folder_dziennika(&self) -> Option<PathBuf> {
        sciezka(&self.folder_dziennika)
    }

    pub fn koniec_dnia(&self) -> NaiveTime {
        czas::parsuj_godzine(&self.godzina_konca_dnia)
            .unwrap_or_else(|| NaiveTime::from_hms_opt(22, 0, 0).expect("stała godzina"))
    }

    pub fn do_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("ustawienia zawsze dają się zserializować") + "\n"
    }
}

/// Ścieżka z ustawień albo `None`, gdy nadal stoi tam znacznik
/// `<Dysk Google>` / `<vault>` z wartości domyślnych. Zmienne w stylu
/// `%USERPROFILE%` są rozwijane.
fn sciezka(tekst: &str) -> Option<PathBuf> {
    let tekst = tekst.trim();
    if tekst.is_empty() || tekst.contains('<') || tekst.contains('>') {
        return None;
    }
    Some(PathBuf::from(rozwin_zmienne(tekst)))
}

fn rozwin_zmienne(tekst: &str) -> String {
    let mut wynik = String::new();
    let mut reszta = tekst;
    while let Some(start) = reszta.find('%') {
        let po = &reszta[start + 1..];
        match po.find('%') {
            Some(koniec) if koniec > 0 => {
                let nazwa = &po[..koniec];
                wynik.push_str(&reszta[..start]);
                match std::env::var(nazwa) {
                    Ok(wartosc) => wynik.push_str(&wartosc),
                    Err(_) => wynik.push_str(&reszta[start..start + koniec + 2]),
                }
                reszta = &po[koniec + 1..];
            }
            _ => break,
        }
    }
    wynik.push_str(reszta);
    if let Some(bez) = wynik.strip_prefix("~/") {
        if let Some(dom) = std::env::var_os("HOME") {
            return Path::new(&dom).join(bez).to_string_lossy().into_owned();
        }
    }
    wynik
}

/// Czyta ustawienia; brakujące pola biorą wartości domyślne, a pojedyncze
/// ukośniki w ścieżkach są naprawiane.
pub fn wczytaj(sciezka: &Path) -> Result<Ustawienia, String> {
    let tekst = pliki::czytaj_tekst(sciezka).map_err(|e| format!("nie mogę przeczytać {}: {e}", sciezka.display()))?;
    match serde_json::from_str(&tekst) {
        Ok(u) => Ok(u),
        Err(pierwszy) => serde_json::from_str(&pliki::napraw_ukosniki(&tekst))
            .map_err(|_| format!("błąd w {}: {pierwszy}", sciezka.display())),
    }
}

pub enum Wynik {
    Utworzone,
    Wczytane,
    Blad(String),
}

/// Przy starcie: tworzy plik z wartościami ze SPEC.md, gdy go brak.
/// Uszkodzonego pliku nie nadpisuje, tylko zwraca domyślne wartości i błąd.
pub fn wczytaj_lub_utworz(sciezka: &Path) -> (Ustawienia, Wynik) {
    if !sciezka.exists() {
        let domyslne = Ustawienia::default();
        if let Some(folder) = sciezka.parent() {
            let _ = fs::create_dir_all(folder);
        }
        return match pliki::zapisz_atomowo(sciezka, domyslne.do_json().as_bytes()) {
            Ok(()) => (domyslne, Wynik::Utworzone),
            Err(e) => (domyslne, Wynik::Blad(format!("nie mogę utworzyć {}: {e}", sciezka.display()))),
        };
    }
    match wczytaj(sciezka) {
        Ok(u) => (u, Wynik::Wczytane),
        Err(e) => (Ustawienia::default(), Wynik::Blad(e)),
    }
}

/// Zmienia jedno pole w pliku i zostawia resztę, także nieznane klucze.
pub fn zapisz_pole(sciezka: &Path, klucz: &str, wartosc: serde_json::Value) -> Result<(), String> {
    let mut dokument: serde_json::Value = match pliki::czytaj_tekst(sciezka) {
        Ok(tekst) => serde_json::from_str(&tekst)
            .or_else(|_| serde_json::from_str(&pliki::napraw_ukosniki(&tekst)))
            .map_err(|e| format!("błąd w {}: {e}", sciezka.display()))?,
        Err(_) => serde_json::to_value(Ustawienia::default()).expect("domyślne ustawienia"),
    };
    let obiekt = dokument.as_object_mut().ok_or_else(|| format!("{} nie zawiera obiektu JSON", sciezka.display()))?;
    obiekt.insert(klucz.to_string(), wartosc);
    let tekst = serde_json::to_string_pretty(&dokument).expect("JSON") + "\n";
    pliki::zapisz_atomowo(sciezka, tekst.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn domyslny_plik_jak_w_specyfikacji() {
        let oczekiwany = r#"{
  "imie": "Dżiordżio",
  "folder_planow": "<Dysk Google>\\Asystentka\\plany",
  "plik_stanu": "<Dysk Google>\\Asystentka\\stan-zadan.json",
  "folder_dziennika": "<vault>\\Dziennik pracy",
  "godzina_planu": "07:40",
  "godzina_konca_dnia": "22:00",
  "prog_bezczynnosci_min": 15,
  "przypomnienie_przerwy_min": 50,
  "ukryj_przed_nagrywaniem": true,
  "chowaj_przy_pelnym_ekranie": true,
  "skrot_pokaz_ukryj": "Ctrl+Alt+Space"
}
"#;
        assert_eq!(Ustawienia::default().do_json(), oczekiwany);
    }

    #[test]
    fn znaczniki_oznaczaja_brak_sciezki() {
        let u = Ustawienia::default();
        assert_eq!(u.folder_planow(), None);
        assert_eq!(u.plik_stanu(), None);
        assert_eq!(u.folder_dziennika(), None);
    }

    #[test]
    fn tworzy_brakujacy_plik_i_czyta_go_ponownie() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("Asystentka").join("ustawienia.json");
        let (u, wynik) = wczytaj_lub_utworz(&plik);
        assert!(matches!(wynik, Wynik::Utworzone));
        assert_eq!(u, Ustawienia::default());
        let (u2, wynik2) = wczytaj_lub_utworz(&plik);
        assert!(matches!(wynik2, Wynik::Wczytane));
        assert_eq!(u2, u);
    }

    #[test]
    fn brakujace_pola_i_pojedyncze_ukosniki() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("ustawienia.json");
        fs::write(&plik, "\u{feff}{\"imie\": \"Ola\", \"folder_dziennika\": \"D:\\Vault\\Dziennik pracy\"}").unwrap();
        let u = wczytaj(&plik).unwrap();
        assert_eq!(u.imie, "Ola");
        assert_eq!(u.folder_dziennika().unwrap(), PathBuf::from(r"D:\Vault\Dziennik pracy"));
        assert_eq!(u.prog_bezczynnosci_min, 15);
    }

    #[test]
    fn uszkodzony_plik_nie_jest_nadpisywany() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("ustawienia.json");
        fs::write(&plik, "{ to nie jest json").unwrap();
        let (u, wynik) = wczytaj_lub_utworz(&plik);
        assert!(matches!(wynik, Wynik::Blad(_)));
        assert_eq!(u, Ustawienia::default());
        assert_eq!(fs::read_to_string(&plik).unwrap(), "{ to nie jest json");
    }

    #[test]
    fn zapis_pola_zachowuje_reszte() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("ustawienia.json");
        fs::write(&plik, "{\"imie\": \"Ola\", \"wlasne\": 7}").unwrap();
        zapisz_pole(&plik, "ukryj_przed_nagrywaniem", serde_json::Value::Bool(false)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&plik).unwrap()).unwrap();
        assert_eq!(v["imie"], "Ola");
        assert_eq!(v["wlasne"], 7);
        assert_eq!(v["ukryj_przed_nagrywaniem"], false);
    }

    #[test]
    fn zmienne_srodowiskowe_w_sciezkach() {
        std::env::set_var("ASYSTENTKA_TEST_VAULT", "/vault");
        assert_eq!(rozwin_zmienne("%ASYSTENTKA_TEST_VAULT%/Dziennik pracy"), "/vault/Dziennik pracy");
        assert_eq!(rozwin_zmienne("%NIEMA_TAKIEJ_ZMIENNEJ_X%/a"), "%NIEMA_TAKIEJ_ZMIENNEJ_X%/a");
        assert_eq!(rozwin_zmienne("100% pewne"), "100% pewne");
    }
}
