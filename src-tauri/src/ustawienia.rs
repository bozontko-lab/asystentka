//! Ustawienia z `%APPDATA%\Asystentka\ustawienia.json` (SPEC.md, B7).
//!
//! Pola z wersji 1 (`folder_planow`, `plik_stanu`, `godzina_planu`) widget
//! pomija: nie czyta ich i nie usuwa z pliku. Zapis zachowuje wszystkie
//! klucze, których nie zna. `imie` i `folder_dziennika` z wersji 1 działają
//! dalej i wypełniają kreator.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveTime;
use serde::{Deserialize, Serialize};

use crate::czas;
use crate::pliki;
use crate::skrot;

pub const NAZWA_PLIKU_ZADAN: &str = "Zadania.md";
/// Imię postaci w interfejsie, dopóki nie wpiszesz innego (B7).
pub const NAZWA_POSTACI: &str = "Sai";
/// Najdłuższe imię postaci, żeby mieściło się w menu i podpowiedzi ikony.
const NAJDLUZSZA_NAZWA_POSTACI: usize = 30;
pub const NAZWA_FOLDERU_DZIENNIKA: &str = "Dziennik pracy";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ustawienia {
    /// Forma, w jakiej postać zwraca się do Ciebie (np. wołacz).
    pub imie: String,
    /// Imię postaci w interfejsie; zastępuje słowo „asystentka” w kreatorze, dymkach,
    /// menu, podpowiedzi ikony i tytule okienka ustawień. Przyjmuje też znaki japońskie.
    pub nazwa_postaci: String,
    pub plik_zadan: String,
    pub folder_dziennika: String,
    pub cel_dzienny_min: u32,
    pub godzina_konca_dnia: String,
    pub autostart: bool,
    pub prog_bezczynnosci_min: u32,
    pub przypomnienie_przerwy_min: u32,
    pub ukryj_przed_nagrywaniem: bool,
    pub chowaj_przy_pelnym_ekranie: bool,
    /// Dźwięk skupienia (dudnienie 40 Hz) w czasie pracy (B7).
    pub dzwiek_skupienia: bool,
    /// Głośność dźwięku skupienia, od 0 do 100.
    pub dzwiek_skupienia_glosnosc: u32,
    /// Gdy działa OBS, dźwięk milknie, żeby nie trafił do nagrania.
    pub cisza_przy_nagrywaniu: bool,
    pub skrot_pokaz_ukryj: String,
}

impl Default for Ustawienia {
    fn default() -> Self {
        Ustawienia {
            imie: String::new(),
            nazwa_postaci: NAZWA_POSTACI.into(),
            plik_zadan: String::new(),
            folder_dziennika: String::new(),
            cel_dzienny_min: 240,
            godzina_konca_dnia: "22:00".into(),
            autostart: true,
            prog_bezczynnosci_min: 15,
            przypomnienie_przerwy_min: 50,
            ukryj_przed_nagrywaniem: true,
            chowaj_przy_pelnym_ekranie: true,
            dzwiek_skupienia: true,
            dzwiek_skupienia_glosnosc: 25,
            cisza_przy_nagrywaniu: true,
            skrot_pokaz_ukryj: skrot::DOMYSLNY.into(),
        }
    }
}

/// Dane do wypełnienia kreatora: to, co już jest w ustawieniach, a czego
/// brakuje, podpowiada na podstawie vaulta Obsidiana.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PodpowiedzKreatora {
    pub ustawienia: Ustawienia,
    /// `true`, gdy wskazany plik zadań już istnieje, czyli kreator ma go użyć.
    pub plik_istnieje: bool,
    pub vaulty: Vec<String>,
}

impl Ustawienia {
    pub fn plik_zadan(&self) -> Option<PathBuf> {
        sciezka(&self.plik_zadan)
    }

    /// Głośność dźwięku skupienia jako mnożnik od 0 do 1; wartość spoza zakresu jest przycinana.
    pub fn glosnosc_dzwieku(&self) -> f32 {
        self.dzwiek_skupienia_glosnosc.min(100) as f32 / 100.0
    }

    /// Imię postaci do pokazania w interfejsie; puste pole znaczy „Sai”.
    pub fn postac(&self) -> &str {
        let nazwa = self.nazwa_postaci.trim();
        if nazwa.is_empty() {
            NAZWA_POSTACI
        } else {
            nazwa
        }
    }

    /// Folder dziennika z ustawień, a gdy go brak, `Dziennik pracy` obok pliku zadań.
    pub fn folder_dziennika(&self) -> Option<PathBuf> {
        sciezka(&self.folder_dziennika).or_else(|| {
            let plik = self.plik_zadan()?;
            Some(plik.parent()?.join(NAZWA_FOLDERU_DZIENNIKA))
        })
    }

    pub fn koniec_dnia(&self) -> NaiveTime {
        czas::parsuj_godzine(&self.godzina_konca_dnia)
            .unwrap_or_else(|| NaiveTime::from_hms_opt(22, 0, 0).expect("stała godzina"))
    }

    pub fn do_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("ustawienia zawsze dają się zserializować") + "\n"
    }

    /// Sprawdza pola, które można ocenić bez systemu plików i bez skrótu
    /// globalnego (ten sprawdza warstwa Tauri). Zwraca komunikaty dla użytkownika.
    pub fn waliduj(&self) -> Vec<String> {
        let mut bledy = Vec::new();
        if self.imie.trim().is_empty() {
            bledy.push("Wpisz, jak mam się do Ciebie zwracać.".to_string());
        }
        if self.nazwa_postaci.trim().is_empty() {
            bledy.push("Wpisz imię postaci.".to_string());
        } else if self.nazwa_postaci.trim().chars().count() > NAJDLUZSZA_NAZWA_POSTACI {
            bledy.push(format!("Imię postaci może mieć najwyżej {NAJDLUZSZA_NAZWA_POSTACI} znaków."));
        }
        if self.plik_zadan().is_none() {
            bledy.push("Wskaż plik zadań.".to_string());
        }
        if self.folder_dziennika().is_none() {
            bledy.push("Wskaż folder dziennika.".to_string());
        }
        if !(15..=1440).contains(&self.cel_dzienny_min) {
            bledy.push("Cel dzienny musi mieć od 15 minut do 24 godzin.".to_string());
        }
        if czas::parsuj_godzine(&self.godzina_konca_dnia).is_none() {
            bledy.push("Godzina zamknięcia dnia ma format GG:MM, np. 22:00.".to_string());
        }
        if self.prog_bezczynnosci_min > 240 {
            bledy.push("Próg bezczynności nie może przekraczać 240 minut.".to_string());
        }
        if self.przypomnienie_przerwy_min > 480 {
            bledy.push("Przypomnienie o przerwie nie może przekraczać 480 minut.".to_string());
        }
        if self.dzwiek_skupienia_glosnosc > 100 {
            bledy.push("Głośność dźwięku skupienia to liczba od 0 do 100.".to_string());
        }
        if let Err(blad) = skrot::analizuj(&self.skrot_pokaz_ukryj) {
            bledy.push(blad);
        }
        bledy
    }
}

/// Ścieżka z ustawień albo `None`, gdy pole jest puste albo stoi w nim
/// znacznik `<Dysk Google>` / `<vault>` z wartości domyślnych wersji 1.
/// Zmienne w stylu `%USERPROFILE%` są rozwijane.
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

fn wczytaj_dokument(sciezka: &Path) -> Result<serde_json::Value, String> {
    let tekst = pliki::czytaj_tekst(sciezka).map_err(|e| format!("nie mogę przeczytać {}: {e}", sciezka.display()))?;
    serde_json::from_str(&tekst)
        .or_else(|pierwszy| serde_json::from_str(&pliki::napraw_ukosniki(&tekst)).map_err(|_| pierwszy))
        .map_err(|e| format!("błąd w {}: {e}", sciezka.display()))
}

/// Czyta ustawienia; brakujące pola biorą wartości domyślne, a pojedyncze
/// ukośniki w ścieżkach są naprawiane.
pub fn wczytaj(sciezka: &Path) -> Result<Ustawienia, String> {
    let dokument = wczytaj_dokument(sciezka)?;
    serde_json::from_value(dokument).map_err(|e| format!("błąd w {}: {e}", sciezka.display()))
}

pub enum Wynik {
    /// Brak pliku albo brak pola `plik_zadan`: uruchamia się kreator.
    Kreator(Ustawienia),
    Wczytane(Ustawienia),
    /// Uszkodzony plik zostaje nietknięty, a widget działa na wartościach domyślnych.
    Blad(Ustawienia, String),
}

impl Wynik {
    pub fn ustawienia(&self) -> &Ustawienia {
        match self {
            Wynik::Kreator(u) | Wynik::Wczytane(u) | Wynik::Blad(u, _) => u,
        }
    }
}

/// Przy starcie. Plik ustawień z wersji 1 nie ma pola `plik_zadan`, więc
/// kreator otwiera się sam i dostaje to, co już w nim jest.
pub fn wczytaj_przy_starcie(sciezka: &Path) -> Wynik {
    if !sciezka.exists() {
        return Wynik::Kreator(Ustawienia::default());
    }
    let dokument = match wczytaj_dokument(sciezka) {
        Ok(d) => d,
        Err(e) => return Wynik::Blad(Ustawienia::default(), e),
    };
    let ma_plik_zadan = dokument.get("plik_zadan").and_then(|v| v.as_str()).is_some_and(|s| !s.trim().is_empty());
    match serde_json::from_value::<Ustawienia>(dokument) {
        Ok(mut u) => {
            zamien_stary_skrot(sciezka, &mut u);
            if ma_plik_zadan {
                Wynik::Wczytane(u)
            } else {
                Wynik::Kreator(u)
            }
        }
        Err(e) => Wynik::Blad(Ustawienia::default(), format!("błąd w {}: {e}", sciezka.display())),
    }
}

/// Stary domyślny skrót (Ctrl+Alt+Spacja) zajmuje szybkie okno aplikacji Claude,
/// więc przy pierwszym starcie wersji 2.1 zamieniamy go w pliku na nowy (B6).
/// Inne skróty zostają, jakie są.
fn zamien_stary_skrot(sciezka: &Path, ustawienia: &mut Ustawienia) {
    if !skrot::to_stary_domyslny(&ustawienia.skrot_pokaz_ukryj) {
        return;
    }
    ustawienia.skrot_pokaz_ukryj = skrot::DOMYSLNY.to_string();
    match zapisz_pole(sciezka, "skrot_pokaz_ukryj", serde_json::Value::String(skrot::DOMYSLNY.to_string())) {
        Ok(()) => log::info!("skrót pokaż/ukryj: {} zamieniony na {}", skrot::STARY_DOMYSLNY, skrot::DOMYSLNY),
        Err(e) => log::warn!("nie mogę zapisać nowego skrótu w {}: {e}", sciezka.display()),
    }
}

/// Zapisuje ustawienia, zachowując klucze, których widget nie zna (także
/// stare pola z wersji 1). Kolejność kluczy w nowym pliku jak w B7.
pub fn zapisz(sciezka: &Path, ustawienia: &Ustawienia) -> Result<(), String> {
    let mut dokument = match wczytaj_dokument(sciezka) {
        Ok(d) if d.is_object() => d,
        _ if !sciezka.exists() => serde_json::Value::Object(Default::default()),
        Ok(_) => return Err(format!("{} nie zawiera obiektu JSON", sciezka.display())),
        Err(e) => return Err(e),
    };
    let nowe = serde_json::to_value(ustawienia).map_err(|e| e.to_string())?;
    let (Some(cel), Some(zrodlo)) = (dokument.as_object_mut(), nowe.as_object()) else {
        return Err("ustawienia nie są obiektem JSON".into());
    };
    for (klucz, wartosc) in zrodlo {
        cel.insert(klucz.clone(), wartosc.clone());
    }
    if let Some(folder) = sciezka.parent() {
        fs::create_dir_all(folder).map_err(|e| format!("nie mogę utworzyć {}: {e}", folder.display()))?;
    }
    let tekst = serde_json::to_string_pretty(&dokument).map_err(|e| e.to_string())? + "\n";
    pliki::zapisz_atomowo(sciezka, tekst.as_bytes()).map_err(|e| format!("nie mogę zapisać {}: {e}", sciezka.display()))
}

/// Zmienia jedno pole w pliku i zostawia resztę.
pub fn zapisz_pole(sciezka: &Path, klucz: &str, wartosc: serde_json::Value) -> Result<(), String> {
    let mut dokument = match wczytaj_dokument(sciezka) {
        Ok(d) => d,
        Err(_) if !sciezka.exists() => serde_json::to_value(Ustawienia::default()).expect("domyślne ustawienia"),
        Err(e) => return Err(e),
    };
    let obiekt = dokument.as_object_mut().ok_or_else(|| format!("{} nie zawiera obiektu JSON", sciezka.display()))?;
    obiekt.insert(klucz.to_string(), wartosc);
    let tekst = serde_json::to_string_pretty(&dokument).expect("JSON") + "\n";
    pliki::zapisz_atomowo(sciezka, tekst.as_bytes()).map_err(|e| e.to_string())
}

/// Wypełnia kreator tym, co już jest w ustawieniach, a brakujące ścieżki
/// podpowiada na podstawie folderu dziennika z wersji 1 albo vaultów Obsidiana.
pub fn podpowiedz_kreatora(ustawienia: &Ustawienia, vaulty: &[PathBuf]) -> PodpowiedzKreatora {
    let mut u = ustawienia.clone();
    if u.plik_zadan().is_none() {
        let folder =
            u.folder_dziennika().and_then(|d| d.parent().map(Path::to_path_buf)).or_else(|| vaulty.first().cloned());
        if let Some(folder) = folder {
            u.plik_zadan = folder.join(NAZWA_PLIKU_ZADAN).to_string_lossy().into_owned();
        }
    }
    if sciezka(&u.folder_dziennika).is_none() {
        u.folder_dziennika = u.folder_dziennika().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    }
    let plik_istnieje = u.plik_zadan().is_some_and(|p| p.is_file());
    PodpowiedzKreatora {
        ustawienia: u,
        plik_istnieje,
        vaulty: vaulty.iter().map(|v| v.to_string_lossy().into_owned()).collect(),
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    const V1: &str = r#"{
  "imie": "Dżiordżio",
  "folder_planow": "G:\\Mój dysk\\Asystentka\\plany",
  "plik_stanu": "G:\\Mój dysk\\Asystentka\\stan-zadan.json",
  "folder_dziennika": "D:\\Second Brain\\Dziennik pracy",
  "godzina_planu": "07:40",
  "godzina_konca_dnia": "21:30",
  "prog_bezczynnosci_min": 10,
  "przypomnienie_przerwy_min": 45,
  "ukryj_przed_nagrywaniem": false,
  "chowaj_przy_pelnym_ekranie": true,
  "skrot_pokaz_ukryj": "Ctrl+Alt+Space"
}"#;

    fn plik_v1() -> (tempfile::TempDir, PathBuf) {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("ustawienia.json");
        fs::write(&plik, V1).unwrap();
        (katalog, plik)
    }

    #[test]
    fn brak_pliku_otwiera_kreator_z_wartosciami_domyslnymi() {
        let katalog = tempfile::tempdir().unwrap();
        let wynik = wczytaj_przy_starcie(&katalog.path().join("ustawienia.json"));
        assert!(matches!(&wynik, Wynik::Kreator(u) if *u == Ustawienia::default()));
        assert_eq!(wynik.ustawienia().cel_dzienny_min, 240);
        assert!(wynik.ustawienia().autostart);
    }

    #[test]
    fn plik_z_wersji_1_otwiera_kreator_z_dotychczasowymi_ustawieniami() {
        let (_k, plik) = plik_v1();
        let Wynik::Kreator(u) = wczytaj_przy_starcie(&plik) else { panic!("powinien być kreator") };
        assert_eq!(u.imie, "Dżiordżio");
        assert_eq!(u.folder_dziennika, r"D:\Second Brain\Dziennik pracy");
        assert_eq!(u.godzina_konca_dnia, "21:30");
        assert_eq!((u.prog_bezczynnosci_min, u.przypomnienie_przerwy_min), (10, 45));
        assert!(!u.ukryj_przed_nagrywaniem);
        assert_eq!(u.cel_dzienny_min, 240, "nowe pole dostaje wartość domyślną");
        assert!(u.autostart);
    }

    #[test]
    fn kreator_podpowiada_plik_zadan_obok_dziennika_z_wersji_1() {
        let vault = tempfile::tempdir().unwrap();
        let dziennik = vault.path().join("Dziennik pracy");
        let u = Ustawienia { folder_dziennika: dziennik.to_string_lossy().into(), ..Ustawienia::default() };
        let p = podpowiedz_kreatora(&u, &[PathBuf::from("/inny/vault")]);
        assert_eq!(p.ustawienia.plik_zadan(), Some(vault.path().join("Zadania.md")));
        assert!(!p.plik_istnieje);
        fs::write(vault.path().join("Zadania.md"), "# Zadania\n").unwrap();
        assert!(podpowiedz_kreatora(&u, &[]).plik_istnieje);
        // Znacznik z wartości domyślnych wersji 1 to brak ścieżki: podpowiedź z vaulta.
        let znacznik = Ustawienia { folder_dziennika: r"<vault>\Dziennik pracy".into(), ..Ustawienia::default() };
        let p = podpowiedz_kreatora(&znacznik, &[vault.path().to_path_buf()]);
        assert_eq!(p.ustawienia.plik_zadan(), Some(vault.path().join("Zadania.md")));
        assert_eq!(
            p.ustawienia.folder_dziennika(),
            Some(vault.path().join("Zadania.md").parent().unwrap().join("Dziennik pracy"))
        );
    }

    #[test]
    fn folder_dziennika_domyslnie_obok_pliku_zadan() {
        let u = Ustawienia { plik_zadan: "/v/Zadania.md".into(), ..Ustawienia::default() };
        assert_eq!(u.folder_dziennika(), Some(PathBuf::from("/v/Dziennik pracy")));
        let u = Ustawienia { folder_dziennika: "/inny/Log".into(), ..u };
        assert_eq!(u.folder_dziennika(), Some(PathBuf::from("/inny/Log")));
    }

    #[test]
    fn zapis_zachowuje_nieznane_klucze_i_nie_zapisuje_pol_z_wersji_1() {
        let (_k, plik) = plik_v1();
        let mut u = wczytaj(&plik).unwrap();
        u.plik_zadan = "D:\\Second Brain\\Zadania.md".into();
        u.cel_dzienny_min = 300;
        zapisz(&plik, &u).unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&plik).unwrap()).unwrap();
        assert_eq!(v["plik_zadan"], "D:\\Second Brain\\Zadania.md");
        assert_eq!(v["cel_dzienny_min"], 300);
        assert_eq!(v["imie"], "Dżiordżio");
        // Stare klucze zostają w pliku, ale widget ich nie dotyka.
        assert_eq!(v["folder_planow"], "G:\\Mój dysk\\Asystentka\\plany");
        assert!(matches!(wczytaj_przy_starcie(&plik), Wynik::Wczytane(_)));
    }

    #[test]
    fn nowy_plik_ma_pola_z_b7_w_kolejnosci() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("A").join("ustawienia.json");
        let u = Ustawienia { imie: "Ola".into(), plik_zadan: "/v/Zadania.md".into(), ..Ustawienia::default() };
        zapisz(&plik, &u).unwrap();
        let klucze: Vec<String> =
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&fs::read_to_string(&plik).unwrap())
                .unwrap()
                .keys()
                .cloned()
                .collect();
        assert_eq!(
            klucze,
            [
                "imie",
                "nazwa_postaci",
                "plik_zadan",
                "folder_dziennika",
                "cel_dzienny_min",
                "godzina_konca_dnia",
                "autostart",
                "prog_bezczynnosci_min",
                "przypomnienie_przerwy_min",
                "ukryj_przed_nagrywaniem",
                "chowaj_przy_pelnym_ekranie",
                "dzwiek_skupienia",
                "dzwiek_skupienia_glosnosc",
                "cisza_przy_nagrywaniu",
                "skrot_pokaz_ukryj"
            ]
        );
    }

    #[test]
    fn uszkodzony_plik_nie_jest_nadpisywany() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("ustawienia.json");
        fs::write(&plik, "{ to nie jest json").unwrap();
        assert!(matches!(wczytaj_przy_starcie(&plik), Wynik::Blad(..)));
        assert!(zapisz(&plik, &Ustawienia::default()).is_err());
        assert_eq!(fs::read_to_string(&plik).unwrap(), "{ to nie jest json");
    }

    #[test]
    fn pojedyncze_ukosniki_w_sciezkach_sa_naprawiane() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("ustawienia.json");
        fs::write(&plik, "\u{feff}{\"imie\": \"Ola\", \"plik_zadan\": \"D:\\Vault\\Zadania.md\"}").unwrap();
        let Wynik::Wczytane(u) = wczytaj_przy_starcie(&plik) else { panic!("powinny się wczytać") };
        assert_eq!(u.plik_zadan(), Some(PathBuf::from(r"D:\Vault\Zadania.md")));
    }

    #[test]
    fn walidacja_zwraca_komunikaty() {
        let ok = Ustawienia { imie: "Ola".into(), plik_zadan: "/v/Zadania.md".into(), ..Ustawienia::default() };
        assert!(ok.waliduj().is_empty());
        let zle = Ustawienia {
            imie: "  ".into(),
            plik_zadan: String::new(),
            cel_dzienny_min: 5,
            godzina_konca_dnia: "25:99".into(),
            prog_bezczynnosci_min: 999,
            ..Ustawienia::default()
        };
        assert_eq!(zle.waliduj().len(), 6);
    }

    #[test]
    fn zmienne_srodowiskowe_w_sciezkach() {
        std::env::set_var("ASYSTENTKA_TEST_VAULT", "/vault");
        assert_eq!(rozwin_zmienne("%ASYSTENTKA_TEST_VAULT%/Dziennik pracy"), "/vault/Dziennik pracy");
        assert_eq!(rozwin_zmienne("%NIEMA_TAKIEJ_ZMIENNEJ_X%/a"), "%NIEMA_TAKIEJ_ZMIENNEJ_X%/a");
        assert_eq!(rozwin_zmienne("100% pewne"), "100% pewne");
    }

    #[test]
    fn domyslny_plik_jak_w_b7() {
        let u = Ustawienia { imie: "Dżiordżio".into(), ..Ustawienia::default() };
        let v: serde_json::Value = serde_json::from_str(&u.do_json()).unwrap();
        assert_eq!(v["cel_dzienny_min"], 240);
        assert_eq!(v["godzina_konca_dnia"], "22:00");
        assert_eq!(v["autostart"], true);
        assert!(v.get("folder_planow").is_none());
    }

    #[test]
    fn stary_domyslny_skrot_jest_zamieniany_w_pliku_przy_pierwszym_starcie() {
        let (_k, plik) = plik_v1();
        let Wynik::Kreator(u) = wczytaj_przy_starcie(&plik) else { panic!("powinien być kreator") };
        assert_eq!(u.skrot_pokaz_ukryj, "Ctrl+Alt+Shift+Space");
        let zapisany: serde_json::Value = serde_json::from_str(&fs::read_to_string(&plik).unwrap()).unwrap();
        assert_eq!(zapisany["skrot_pokaz_ukryj"], "Ctrl+Alt+Shift+Space", "zamiana trafia do pliku");
        assert_eq!(zapisany["imie"], "Dżiordżio", "reszta pliku zostaje");
        assert_eq!(zapisany["folder_planow"], "G:\\Mój dysk\\Asystentka\\plany", "także stare klucze");
    }

    #[test]
    fn wlasny_skrot_zostaje_nietkniety() {
        let katalog = tempfile::tempdir().unwrap();
        let plik = katalog.path().join("ustawienia.json");
        fs::write(&plik, r#"{"imie": "Ola", "plik_zadan": "/v/Zadania.md", "skrot_pokaz_ukryj": "Ctrl+Shift+F9"}"#)
            .unwrap();
        let Wynik::Wczytane(u) = wczytaj_przy_starcie(&plik) else { panic!("powinny być wczytane") };
        assert_eq!(u.skrot_pokaz_ukryj, "Ctrl+Shift+F9");
        assert!(fs::read_to_string(&plik).unwrap().contains("Ctrl+Shift+F9"));
    }

    #[test]
    fn nowy_domyslny_skrot_i_walidacja_skrotu() {
        assert_eq!(Ustawienia::default().skrot_pokaz_ukryj, "Ctrl+Alt+Shift+Space");
        let dobre = Ustawienia { imie: "Ola".into(), plik_zadan: "/v/Zadania.md".into(), ..Ustawienia::default() };
        assert!(dobre.waliduj().is_empty(), "{:?}", dobre.waliduj());
        let altgr = Ustawienia { skrot_pokaz_ukryj: "Ctrl+Alt+A".into(), ..dobre.clone() };
        assert!(altgr.waliduj().iter().any(|b| b.contains("AltGr")));
        let pusty = Ustawienia { skrot_pokaz_ukryj: "  ".into(), ..dobre };
        assert_eq!(pusty.waliduj().len(), 1);
    }

    #[test]
    fn imie_postaci_domyslnie_sai_i_przyjmuje_znaki_japonskie() {
        let u = Ustawienia::default();
        assert_eq!((u.nazwa_postaci.as_str(), u.postac()), ("Sai", "Sai"));
        let dobre = Ustawienia { imie: "Ola".into(), plik_zadan: "/v/Zadania.md".into(), ..u };
        let japonskie = Ustawienia { nazwa_postaci: "  才ちゃん  ".into(), ..dobre.clone() };
        assert!(japonskie.waliduj().is_empty(), "{:?}", japonskie.waliduj());
        assert_eq!(japonskie.postac(), "才ちゃん");
        let json = japonskie.do_json();
        assert!(json.contains("才ちゃん"), "{json}");
        assert!(json.find("\"nazwa_postaci\"").unwrap() < json.find("\"plik_zadan\"").unwrap(), "kolejność jak w B7");
        let pusta = Ustawienia { nazwa_postaci: "   ".into(), ..dobre.clone() };
        assert!(pusta.waliduj().iter().any(|b| b.contains("imię postaci")));
        assert_eq!(pusta.postac(), "Sai", "puste pole nie psuje interfejsu");
        let za_dluga = Ustawienia { nazwa_postaci: "x".repeat(31), ..dobre };
        assert!(za_dluga.waliduj().iter().any(|b| b.contains("najwyżej 30 znaków")));
    }

    #[test]
    fn plik_ustawien_bez_imienia_postaci_dostaje_sai_a_zapis_je_dopisuje() {
        let (_k, plik) = plik_v1();
        let Wynik::Kreator(u) = wczytaj_przy_starcie(&plik) else { panic!("powinien być kreator") };
        assert_eq!(u.nazwa_postaci, "Sai");
        let nowe = Ustawienia { nazwa_postaci: "Bajtka".into(), ..u };
        zapisz(&plik, &nowe).unwrap();
        let zapisany: serde_json::Value = serde_json::from_str(&fs::read_to_string(&plik).unwrap()).unwrap();
        assert_eq!(zapisany["nazwa_postaci"], "Bajtka");
        assert_eq!(wczytaj(&plik).unwrap().nazwa_postaci, "Bajtka");
    }

    #[test]
    fn dzwiek_skupienia_ma_domyslne_ustawienia_z_b7_i_sprawdzana_glosnosc() {
        let u = Ustawienia::default();
        assert!(u.dzwiek_skupienia && u.cisza_przy_nagrywaniu);
        assert_eq!(u.dzwiek_skupienia_glosnosc, 25);
        assert_eq!(u.glosnosc_dzwieku(), 0.25);
        let dobre = Ustawienia { imie: "Ola".into(), plik_zadan: "/v/Zadania.md".into(), ..u };
        assert!(Ustawienia { dzwiek_skupienia_glosnosc: 100, ..dobre.clone() }.waliduj().is_empty());
        assert!(Ustawienia { dzwiek_skupienia_glosnosc: 0, ..dobre.clone() }.waliduj().is_empty());
        let za_glosno = Ustawienia { dzwiek_skupienia_glosnosc: 101, ..dobre };
        assert!(za_glosno.waliduj().iter().any(|b| b.contains("od 0 do 100")));
        assert_eq!(za_glosno.glosnosc_dzwieku(), 1.0, "przy odtwarzaniu przycinane do 100");
    }

    #[test]
    fn stary_plik_ustawien_bez_pol_dzwieku_dostaje_wartosci_domyslne() {
        let (_k, plik) = plik_v1();
        let Wynik::Kreator(u) = wczytaj_przy_starcie(&plik) else { panic!("powinien być kreator") };
        assert!(u.dzwiek_skupienia && u.cisza_przy_nagrywaniu);
        assert_eq!(u.dzwiek_skupienia_glosnosc, 25);
    }
}
