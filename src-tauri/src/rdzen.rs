//! Rdzeń widgetu: stany postaci i przejścia z SPEC.md (B4, B5).
//!
//! Rdzeń nie zna Tauri ani Windows. Dostaje zdarzenia z czasem `teraz`,
//! zmienia stan, zapisuje pliki i zbiera dymki oraz efekty dla frontendu.
//! Dzięki temu cały przepływ dnia da się sprawdzić testami.

use std::collections::BTreeSet;

use chrono::{Duration, NaiveTime};
use serde::Serialize;

use crate::czas::{self, Chwila};
use crate::dzien::{DzienStan, Edycja, Reakcja};
use crate::dziennik::{self, DzienPracy, Podsumowanie, WpisSesji};
use crate::magazyn::Magazyn;
use crate::okno::{KotwicaOkna, Naroznik, Pion, Strona};
use crate::plik_zadan::{self, BladZmiany, Klucz, PlikZadan, Zadanie};
use crate::sesja::{PowodPauzy, Sesja, ZadanieSesji};
use crate::skrot;
use crate::ustawienia::Ustawienia;

/// Jak długo trwa powitanie, zanim postać przejdzie dalej.
const POWITANIE_S: i64 = 6;
/// Gratulacje wracają do listy po 10 s albo po kliknięciu w dymek; tyle też
/// działa Cofnij (B4, B5).
const GRATULACJE_S: i64 = 10;
/// Zapis do pliku zadań, który się nie udał (plik chwilowo zajęty), ponawiamy po
/// kilku sekundach, a nie dopiero w cyklu co 30 s: ptaszek ma pojawić się od razu (B5).
const PONOWIENIA_ZAPISU_S: [i64; 6] = [1, 1, 2, 3, 5, 10];
/// Krótszej sesji przerwanej przez „Przerwij” nie zapisujemy.
const MIN_SESJA_S: i64 = 60;
/// Po 5 s ręcznej przerwy postać przestaje się przeciągać, gra na konsolce i mówi o tym (B4, B7).
const PAUZA_GRA_PO_S: i64 = 5;
/// Dymek o graniu ma sens tylko tuż po wciśnięciu Pauzy, a nie po spóźnionym ticku.
const PAUZA_GRA_NAJPOZNIEJ_S: i64 = 30;
/// Wybór zadań liczy co najmniej tyle minut na zadanie (B4).
const MIN_NA_ZADANIE_MIN: u32 = 10;
/// Wybór przekracza cel, gdy suma jest większa o ponad 20% (B4).
const PROG_ZA_DUZO: f64 = 1.2;
/// Jak długo trwa reakcja na koniec dnia, zanim postać pomacha na pożegnanie.
const REAKCJA_S: i64 = 8;
/// Pożegnanie („Do jutra!”) trwa tyle, zanim okno schowa się do zasobnika.
const POZEGNANIE_S: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stan {
    Konfiguracja,
    Powitanie,
    WyborZadan,
    Lista,
    Praca,
    Pauza,
    Jestes,
    Gratulacje,
    KoniecDnia,
}

impl Stan {
    pub fn nazwa(self) -> &'static str {
        match self {
            Stan::Konfiguracja => "konfiguracja",
            Stan::Powitanie => "powitanie",
            Stan::WyborZadan => "wybor_zadan",
            Stan::Lista => "lista",
            Stan::Praca => "praca",
            Stan::Pauza => "pauza",
            Stan::Jestes => "jestes",
            Stan::Gratulacje => "gratulacje",
            Stan::KoniecDnia => "koniec_dnia",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GratulacjeInfo {
    pub zadanie: String,
    /// Łączny czas zadania (plik plus ta sesja).
    pub czas_min: u32,
    pub szac_min: Option<u32>,
    pub czas_dnia_min: u32,
    pub plan_wykonany: bool,
    /// Przez pierwsze 10 s dymek ma przycisk Cofnij.
    #[serde(default)]
    pub mozna_cofnac: bool,
}

/// Faza poza sesją. W czasie sesji stan wynika z samej sesji (Praca, Pauza, Jesteś?).
#[derive(Debug, Clone, PartialEq)]
enum Faza {
    Konfiguracja,
    Powitanie { do_: Chwila },
    Wybor,
    Lista,
    Gratulacje { do_: Option<Chwila>, info: GratulacjeInfo, klucz: Klucz },
    KoniecDnia { etap: EtapKonca, reakcja: Reakcja },
}

/// Przebieg końca dnia: reakcja, pożegnanie i schowanie do zasobnika; po
/// schowaniu postać „śpi”, a przy starcie po wyłączonym komputerze pokazuje
/// podsumowanie wczorajszego dnia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EtapKonca {
    Reakcja { do_: Chwila },
    Pozegnanie { do_: Chwila },
    Spi,
    Wczoraj,
}

impl EtapKonca {
    fn nazwa(self) -> &'static str {
        match self {
            EtapKonca::Reakcja { .. } => "reakcja",
            EtapKonca::Pozegnanie { .. } => "pozegnanie",
            EtapKonca::Spi => "spi",
            EtapKonca::Wczoraj => "wczoraj",
        }
    }
}

/// Podsumowanie wczorajszego dnia zamkniętego za Ciebie, do pokazania po powitaniu.
#[derive(Debug, Clone, PartialEq)]
struct Wczoraj {
    widok: PodsumowanieWidok,
    reakcja: Reakcja,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "rodzaj", rename_all = "snake_case")]
pub enum Zdarzenie {
    /// Klucz z `src/kwestie.json` i wstawki do podstawienia.
    Dymek {
        klucz: String,
        wstawki: std::collections::BTreeMap<String, String>,
    },
    Efekt {
        nazwa: String,
    },
}

/// Nakładka „Ukryta”: okno schowane, a licznik liczy dalej.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ukrycie {
    pub recznie: bool,
    pub pelny_ekran: bool,
    /// Pokazane skrótem albo z zasobnika mimo pełnego ekranu.
    pub wymuszone: bool,
}

// ---- widoki dla frontendu ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ZadanieWidok {
    pub projekt: String,
    pub zadanie: String,
    pub szac_min: Option<u32>,
    /// Łączny czas z pliku (i z zapisów, które na niego czekają).
    pub czas_min: u32,
    pub zrobione: bool,
    pub w_toku: bool,
    pub zaznaczone: bool,
    pub termin: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjektWidok {
    pub nazwa: String,
    pub zadania: Vec<ZadanieWidok>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WyborWidok {
    pub projekty: Vec<ProjektWidok>,
    pub suma_min: u32,
    pub cel_min: u32,
    pub przekroczony: bool,
    pub zaznaczonych: usize,
    /// Jest już plan na dziś, więc można z wyboru wrócić do listy.
    pub mozna_wrocic: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListaWidok {
    pub zadania: Vec<ZadanieWidok>,
    pub zrobione: u32,
    pub razem: u32,
    pub pozostalo_min: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SesjaWidok {
    pub projekt: String,
    pub zadanie: String,
    pub szac_min: Option<u32>,
    /// Czas z pliku z chwili startu plus zamknięte odcinki; frontend dolicza otwarty odcinek.
    pub sekundy_zamkniete: i64,
    pub odcinek_od_ms: Option<i64>,
    pub pauza_od_ms: Option<i64>,
    pub powod_pauzy: Option<PowodPauzy>,
    pub bezczynna_od_ms: Option<i64>,
}

/// Czas jednego zadania w danym dniu względem szacunku.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZadanieDniaWidok {
    pub zadanie: String,
    pub projekt: String,
    pub czas_min: u32,
    pub szac_min: Option<u32>,
    pub zrobione: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PodsumowanieWidok {
    pub zrobione: u32,
    pub zaplanowane: u32,
    pub czas_dnia_min: u32,
    pub cel_min: u32,
    pub zadania: Vec<ZadanieDniaWidok>,
    pub na_jutro: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KoniecWidok {
    /// `reakcja`, `pozegnanie`, `spi` albo `wczoraj`.
    pub etap: &'static str,
    pub reakcja: Reakcja,
    pub podsumowanie: PodsumowanieWidok,
}

#[derive(Debug, Clone, Serialize)]
pub struct DzienWidok {
    pub minuty: u32,
    pub cel_min: u32,
}

/// Wszystko, czego frontend potrzebuje do narysowania bieżącego stanu.
#[derive(Debug, Clone, Serialize)]
pub struct Migawka {
    pub stan: &'static str,
    pub widoczna: bool,
    /// Widget stoi na lewej (postać odbita lustrzanie) albo prawej połowie swojego monitora.
    pub strona: Strona,
    /// Przy której krawędzi monitora stoi widget; od niej panel rośnie w stronę środka (B6).
    pub pion: Pion,
    pub imie: String,
    pub wybor: Option<WyborWidok>,
    pub lista: Option<ListaWidok>,
    pub sesja: Option<SesjaWidok>,
    pub gratulacje: Option<GratulacjeInfo>,
    pub koniec: Option<KoniecWidok>,
    pub dzien: DzienWidok,
    /// Problem z plikiem zadań (brak ścieżki, nieczytelny plik).
    pub blad_pliku: Option<String>,
    /// Projekty do wyboru w „Dodaj zadanie”: nagłówki z pliku i Inbox.
    pub projekty_do_dopisania: Vec<String>,
}

/// Co wolno z menu pod prawym przyciskiem i z zasobnika (B6) w bieżącym stanie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mozliwosci {
    pub koniec_pracy: bool,
    pub zmien_plan: bool,
    pub dodaj_zadanie: bool,
    /// `None`: brak sesji; `Some(true)`: sesja zapauzowana.
    pub pauza: Option<bool>,
    pub widoczna: bool,
}

pub struct Rdzen {
    pub ust: Ustawienia,
    magazyn: Magazyn,
    dz: DzienStan,
    zadania: PlikZadan,
    blad_pliku: Option<String>,
    /// Zapisane położenie widgetu (`okno.json`).
    okno: KotwicaOkna,
    /// Róg, który naprawdę obowiązuje: inny niż zapisany, gdy monitor widgetu zniknął.
    naroznik: Naroznik,
    faza: Faza,
    wybor: BTreeSet<Klucz>,
    wybor_przekroczony: bool,
    pub ukrycie: Ukrycie,
    zdarzenia: Vec<Zdarzenie>,
    brudny: bool,
    kreator: bool,
    obserwacja_pliku: bool,
    ostatni_dymek_powrotu: Option<Chwila>,
    wczoraj: Option<Wczoraj>,
    /// Kiedy ponowić zapis w pliku zadań, który się nie udał, i które to już ponowienie.
    zapis_ponow: Option<Chwila>,
    proby_zapisu: usize,
    /// Skrót pokaż/ukryj nie zarejestrował się: dymek o tym czeka na swoją chwilę (raz).
    skrot_zajety: Option<(String, Chwila)>,
    /// Początek przerwy, dla której dymek `pauza_gra` już padł (albo jest spóźniony).
    pauza_gra: Option<Chwila>,
}

impl Rdzen {
    pub fn nowy(magazyn: Magazyn, ust: Ustawienia, teraz: Chwila) -> Rdzen {
        let okno = magazyn.wczytaj_okno();
        let naroznik = okno.kotwica;
        Rdzen {
            ust,
            dz: DzienStan::nowy(czas::dzien(teraz)),
            magazyn,
            zadania: PlikZadan::default(),
            blad_pliku: None,
            okno,
            naroznik,
            faza: Faza::Lista,
            wybor: BTreeSet::new(),
            wybor_przekroczony: false,
            ukrycie: Ukrycie::default(),
            zdarzenia: Vec::new(),
            brudny: false,
            kreator: false,
            obserwacja_pliku: false,
            ostatni_dymek_powrotu: None,
            wczoraj: None,
            zapis_ponow: None,
            proby_zapisu: 0,
            skrot_zajety: None,
            pauza_gra: None,
        }
    }

    // ---- odczyt -------------------------------------------------------------------------

    pub fn stan(&self) -> Stan {
        if let Some(s) = &self.dz.sesja {
            return if s.pauza.is_some() {
                Stan::Pauza
            } else if s.bezczynna_od.is_some() {
                Stan::Jestes
            } else {
                Stan::Praca
            };
        }
        match self.faza {
            Faza::Konfiguracja => Stan::Konfiguracja,
            Faza::Powitanie { .. } => Stan::Powitanie,
            Faza::Wybor => Stan::WyborZadan,
            Faza::Lista => Stan::Lista,
            Faza::Gratulacje { .. } => Stan::Gratulacje,
            Faza::KoniecDnia { .. } => Stan::KoniecDnia,
        }
    }

    pub fn widoczna(&self) -> bool {
        !self.ukrycie.recznie && !self.schowana_przez_pelny_ekran()
    }

    fn schowana_przez_pelny_ekran(&self) -> bool {
        self.ukrycie.pelny_ekran && self.ust.chowaj_przy_pelnym_ekranie && !self.ukrycie.wymuszone
    }

    pub fn sesja_trwa(&self) -> bool {
        self.dz.sesja.as_ref().is_some_and(|s| s.pauza.is_none())
    }

    /// `None`: brak sesji; `Some(true)`: sesja zapauzowana.
    pub fn sesja_pauza(&self) -> Option<bool> {
        self.dz.sesja.as_ref().map(|s| s.pauza.is_some())
    }

    pub fn mozliwosci(&self) -> Mozliwosci {
        let stan = self.stan();
        let otwarty = !self.dz.zamkniety && !self.kreator && stan != Stan::Konfiguracja;
        Mozliwosci {
            koniec_pracy: otwarty,
            zmien_plan: otwarty && matches!(stan, Stan::Lista | Stan::Gratulacje),
            dodaj_zadanie: otwarty
                && matches!(stan, Stan::WyborZadan | Stan::Lista | Stan::Praca | Stan::Pauza | Stan::Gratulacje),
            pauza: self.sesja_pauza(),
            widoczna: self.widoczna(),
        }
    }

    pub fn kotwica(&self) -> KotwicaOkna {
        self.okno.clone()
    }

    pub fn naroznik(&self) -> Naroznik {
        self.naroznik
    }

    pub fn wez_zdarzenia(&mut self) -> Vec<Zdarzenie> {
        std::mem::take(&mut self.zdarzenia)
    }

    pub fn ustawienia_sciezka(&self) -> std::path::PathBuf {
        self.magazyn.ustawienia()
    }

    /// Najbliższa chwila, w której `tick` ma coś do zrobienia. Uwzględnia
    /// tylko terminy, na które `tick` zareaguje, żeby zaległy termin nie
    /// budził wątku w kółko.
    pub fn najblizszy_termin(&self, teraz: Chwila) -> Chwila {
        let polnoc = (self.dz.data + Duration::days(1)).and_time(NaiveTime::MIN);
        let mut termin = lokalna(polnoc, teraz).unwrap_or(teraz + Duration::hours(1));
        let mut wez = |t: Chwila| {
            if t < termin {
                termin = t;
            }
        };
        match &self.faza {
            Faza::Powitanie { do_ } => wez(*do_),
            Faza::Gratulacje { do_: Some(do_), .. } => wez(*do_),
            Faza::KoniecDnia { etap: EtapKonca::Reakcja { do_ } | EtapKonca::Pozegnanie { do_ }, .. } => wez(*do_),
            _ => {}
        }
        if let Some(t) = self.czas_automatycznego_zamkniecia(teraz) {
            wez(t);
        }
        if let Some(t) = self.zapis_ponow {
            wez(t);
        }
        if let (Some((_, t)), true) = (&self.skrot_zajety, self.mozna_mowic_o_skrocie()) {
            wez(*t);
        }
        if let Some((_, t)) = self.pauza_gra_termin() {
            wez(t);
        }
        let co = self.ust.przypomnienie_przerwy_min;
        if let (Some(s), Stan::Praca, true) = (&self.dz.sesja, self.stan(), co > 0) {
            if let Some(od) = s.otwarty_od() {
                wez(od + Duration::minutes(i64::from((s.przypomnienia + 1) * co)));
            }
        }
        termin.max(teraz)
    }

    // ---- plik zadań -----------------------------------------------------------------------

    /// Wczytuje `Zadania.md`. Gdy plik chwilowo nie da się przeczytać
    /// (Obsidian właśnie go zapisuje), zostaje ostatnia dobra wersja.
    fn wczytaj_zadania(&mut self) -> bool {
        let (nowe, blad) = match self.ust.plik_zadan() {
            None => (PlikZadan::default(), Some("Nie wskazano pliku zadań. Ustaw go w ustawieniach.".to_string())),
            Some(sciezka) => match plik_zadan::wczytaj_i_parsuj(&sciezka) {
                Ok(p) => (p, None),
                Err(e) => (self.zadania.clone(), Some(format!("Nie mogę wczytać {}: {e}", sciezka.display()))),
            },
        };
        let zmiana = nowe != self.zadania || blad != self.blad_pliku;
        if zmiana {
            if let (Some(b), true) = (&blad, blad != self.blad_pliku) {
                log::warn!("{b}");
            }
            self.zadania = nowe;
            self.blad_pliku = blad;
            let otwarte: BTreeSet<Klucz> = self.zadania.otwarte().map(|z| z.klucz.clone()).collect();
            self.wybor.retain(|k| otwarte.contains(k));
            self.brudny = true;
        }
        zmiana
    }

    /// Rejestracja skrótu pokaż/ukryj się nie udała (zajmuje go inny program). Widget
    /// działa dalej bez skrótu i raz mówi o tym w dymku `skrot_zajety` (B6, B7),
    /// nie wcześniej niż `najwczesniej`, żeby nie zagłuszyć powitania.
    pub fn skrot_zajety(&mut self, skrot: &str, najwczesniej: Chwila) {
        self.skrot_zajety = Some((skrot.to_string(), najwczesniej));
    }

    /// Początek ręcznej przerwy i chwila, w której pojawi się dymek `pauza_gra`.
    /// Przerwa z blokady ekranu albo uśpienia nie dostaje dymku: nikt na nią nie patrzy.
    fn pauza_gra_termin(&self) -> Option<(Chwila, Chwila)> {
        let p = self.dz.sesja.as_ref()?.pauza.as_ref()?;
        if p.powod != PowodPauzy::Reczna || self.pauza_gra == Some(p.od) {
            return None;
        }
        Some((p.od, p.od + Duration::seconds(PAUZA_GRA_PO_S)))
    }

    /// „To ja w tym czasie pogram!”: raz, 5 s po wciśnięciu Pauzy.
    fn dymek_pauza_gra(&mut self, teraz: Chwila) {
        let Some((od, termin)) = self.pauza_gra_termin() else { return };
        if teraz < termin {
            return;
        }
        self.pauza_gra = Some(od);
        if teraz - od <= Duration::seconds(PAUZA_GRA_NAJPOZNIEJ_S) {
            self.dymek("pauza_gra", &[]);
        }
    }

    /// Dymek o skrócie nie przerywa kreatora ani powitania.
    fn mozna_mowic_o_skrocie(&self) -> bool {
        !self.kreator && !matches!(self.faza, Faza::Konfiguracja | Faza::Powitanie { .. })
    }

    fn dymek_o_zajetym_skrocie(&mut self, teraz: Chwila) {
        let Some((skrot, od)) = self.skrot_zajety.clone() else { return };
        if teraz < od || !self.mozna_mowic_o_skrocie() {
            return;
        }
        self.skrot_zajety = None;
        self.dymek("skrot_zajety", &[("skrot", skrot::dla_czlowieka(&skrot))]);
    }

    /// Obserwacja pliku zadań działa, więc nie trzeba go odpytywać.
    pub fn ustaw_obserwacje_pliku(&mut self, dziala: bool) {
        self.obserwacja_pliku = dziala;
    }

    /// Plik zadań zmienił się (zapis z Obsidiana albo naszej sesji).
    pub fn odswiez_zadania(&mut self) -> bool {
        self.wczytaj_zadania()
    }

    fn zadanie(&self, klucz: &Klucz) -> Option<&Zadanie> {
        self.zadania.zadanie(klucz)
    }

    /// Czas zadania z pliku plus zapisy, które na niego jeszcze czekają.
    fn czas_zadania(&self, z: &Zadanie) -> u32 {
        z.czas_min + self.dz.oczekujace.iter().filter(|e| e.klucz == z.klucz).map(|e| e.minuty).sum::<u32>()
    }

    /// Pozostały czas zadania: szacunek minus przepracowany czas, co najmniej
    /// 10 min. Zadanie bez szacunku liczy się jako 0 (B3).
    fn pozostaly_czas(&self, z: &Zadanie) -> u32 {
        z.szac_min.map_or(0, |s| s.saturating_sub(self.czas_zadania(z)).max(MIN_NA_ZADANIE_MIN))
    }

    fn plan_zadania(&self) -> Vec<&Zadanie> {
        self.dz.plan.iter().filter_map(|k| self.zadanie(k)).collect()
    }

    /// Zrobione i zaplanowane zadania z planu. Zadanie, które zniknęło z
    /// pliku (zmieniony tekst), liczy się tylko wtedy, gdy zrobiła je dziś sesja.
    fn licz_plan(&self) -> (u32, u32) {
        let (mut zrobione, mut zaplanowane) = (0, 0);
        for k in &self.dz.plan {
            let dzis_zrobione =
                self.dz.sesje.iter().any(|s| s.zrobione && s.projekt == k.projekt && s.zadanie == k.zadanie);
            match self.zadanie(k) {
                Some(z) => {
                    zaplanowane += 1;
                    zrobione += u32::from(z.zrobione);
                }
                None if dzis_zrobione => {
                    zaplanowane += 1;
                    zrobione += 1;
                }
                None => {}
            }
        }
        (zrobione, zaplanowane)
    }

    fn plan_wykonany(&self) -> bool {
        let (zrobione, zaplanowane) = self.licz_plan();
        zaplanowane > 0 && zrobione == zaplanowane
    }

    // ---- start, cykl i zegar ----------------------------------------------------------------

    /// Pierwsze uruchomienie wersji 2: zamiast powitania otwiera się kreator.
    pub fn wymus_kreator(&mut self) {
        self.kreator = true;
    }

    /// Kreator skończony: ustawienia są zapisane, plik zadań istnieje.
    pub fn zakoncz_kreator(&mut self, nowe: Ustawienia, teraz: Chwila) {
        self.kreator = false;
        self.dz.powitano = true;
        self.zmien_ustawienia(nowe, teraz);
        self.faza = Faza::Wybor;
        self.wybor = self.domyslny_wybor();
        self.wybor_przekroczony = self.przekroczony();
        self.dymek("konfiguracja_gotowe", &[]);
        if let Some((_, od)) = self.skrot_zajety.as_mut() {
            // Najpierw „Gotowe!”, dopiero potem uwaga o skrócie.
            *od = (*od).max(teraz + Duration::seconds(8));
        }
        self.brudny = true;
        self.zapisz_stan(teraz);
    }

    pub fn start(&mut self, teraz: Chwila) {
        let dzis = czas::dzien(teraz);
        self.wczytaj_zadania();
        match self.magazyn.wczytaj_dzien() {
            Some(d) if d.data == dzis => self.dz = d,
            Some(stary) => {
                self.dz = stary;
                let (przeniesione, wczoraj) = self.zamknij_stary_dzien(teraz);
                self.dz = DzienStan { przeniesione, ..DzienStan::nowy(dzis) };
                self.wczoraj = wczoraj;
            }
            None => {
                self.dz = DzienStan::nowy(dzis);
                // Przejście z wersji 1 w środku dnia nie gubi linii z dziennika.
                self.dz.sesje = self.magazyn.sesje_z_wersji_1(dzis);
                self.dz.powitano = !self.dz.sesje.is_empty();
            }
        }
        let ostatni_zapis = self.dz.zapisano.unwrap_or(teraz);
        if let Some(s) = self.dz.sesja.as_mut() {
            s.odtworz(ostatni_zapis);
            log::info!("odtworzona sesja: {} (pauza)", s.zadanie.nazwa());
        }
        self.efekt("glitch_wejscie");
        if self.kreator && self.dz.sesja.is_none() {
            self.faza = Faza::Konfiguracja;
            self.dz.powitano = true;
            self.dymek("konfiguracja_imie", &[]);
        } else if self.dz.sesja.is_some() {
            self.faza = Faza::Lista;
            self.dymek("po_blokadzie", &[]);
        } else if self.dz.zamkniety {
            // Dzień zamknięty wcześniej: postać „śpi” z podsumowaniem i przyciskiem „Wróć do pracy”.
            let reakcja = self.dz.reakcja.unwrap_or_else(|| self.reakcja_dnia());
            self.faza = Faza::KoniecDnia { etap: EtapKonca::Spi, reakcja };
        } else if !self.dz.powitano {
            self.powitaj(teraz);
        } else if self.plan_zadania().is_empty() {
            self.otworz_wybor();
        } else {
            self.faza = Faza::Lista;
            self.dymek("powrot", &[]);
        }
        self.zastosuj_edycje(teraz);
        self.brudny = true;
        self.zapisz_stan(teraz);
    }

    /// Wywoływany co sekundę albo w `najblizszy_termin`.
    pub fn tick(&mut self, teraz: Chwila) {
        self.zmiana_dnia(teraz);
        match &self.faza {
            Faza::Powitanie { do_ } if teraz >= *do_ => self.po_powitaniu(),
            Faza::Gratulacje { do_: Some(do_), .. } if teraz >= *do_ => self.po_gratulacjach(),
            Faza::KoniecDnia { etap: EtapKonca::Reakcja { do_ }, .. } if teraz >= *do_ => {
                self.rozpocznij_pozegnanie(teraz)
            }
            Faza::KoniecDnia { etap: EtapKonca::Pozegnanie { do_ }, .. } if teraz >= *do_ => {
                self.schowaj_do_zasobnika()
            }
            _ => {}
        }
        // O godzinie zamknięcia dzień kończy się sam (B4); po wyłączonym komputerze zaraz po starcie.
        if self.czas_automatycznego_zamkniecia(teraz).is_some_and(|t| teraz >= t) {
            self.koniec_pracy(teraz);
        }
        self.przypomnienie_o_przerwie(teraz);
        self.dymek_o_zajetym_skrocie(teraz);
        self.dymek_pauza_gra(teraz);
        if self.zapis_ponow.is_some_and(|t| teraz >= t) {
            self.zastosuj_edycje(teraz);
            self.zapisz_stan(teraz);
        }
    }

    /// Co 30 s: ponowienie zapisów z kolejki i zapis stanu dnia.
    pub fn cykl(&mut self, teraz: Chwila) {
        self.tick(teraz);
        // Przy działającej obserwacji pliku zadań nie odpytujemy go co 30 s.
        if !(self.obserwacja_pliku && self.ust.plik_zadan().is_some()) {
            self.wczytaj_zadania();
        }
        if !self.dz.oczekujace.is_empty() {
            self.zastosuj_edycje(teraz);
        }
        if self.dz.dziennik_do_zapisu {
            self.zapisz_dziennik();
        }
        self.ponow_zalegle_dzienniki();
        if self.sesja_trwa() {
            self.brudny = true;
        }
        self.zapisz_stan(teraz);
    }

    fn zmiana_dnia(&mut self, teraz: Chwila) {
        let dzis = czas::dzien(teraz);
        // Trwająca sesja liczy się dalej i należy do dnia, w którym się zaczęła (B5).
        if dzis == self.dz.data || self.dz.sesja.is_some() {
            return;
        }
        log::info!("nowy dzień: {dzis}");
        let (przeniesione, wczoraj) = self.zamknij_stary_dzien(teraz);
        self.dz = DzienStan { przeniesione, powitano: true, ..DzienStan::nowy(dzis) };
        self.wczytaj_zadania();
        self.wczoraj = wczoraj;
        self.faza = Faza::Lista;
        if !self.kreator {
            if self.wczoraj.is_some() {
                self.pokaz_wczoraj();
            } else {
                self.otworz_wybor();
            }
        }
        self.brudny = true;
    }

    /// Kończy dzień, który minął: trwająca sesja zapisuje się jak „Przerwij”,
    /// a dziennik dostaje ostatni stan. Dzień, którego nie zamknąłeś, zamyka
    /// się za Ciebie z podsumowaniem. Zwraca niedokończone zadania z planu i
    /// podsumowanie do pokazania.
    fn zamknij_stary_dzien(&mut self, teraz: Chwila) -> (Vec<Klucz>, Option<Wczoraj>) {
        if let Some(mut s) = self.dz.sesja.take() {
            let koniec = s.pauza.as_ref().map_or(s.koniec(), |p| p.od);
            s.zakoncz(koniec);
            self.dz.sesja = Some(s);
            self.zakoncz_sesje(false, koniec);
        }
        self.zastosuj_edycje(teraz);
        let przeniesione: Vec<Klucz> =
            self.dz.plan.iter().filter(|k| self.zadanie(k).is_some_and(|z| !z.zrobione)).cloned().collect();
        let wczoraj = if !self.dz.zamkniety && self.dzien_mial_aktywnosc() {
            let reakcja = self.domknij_dzien(teraz);
            Some(Wczoraj { widok: self.podsumowanie_widok(), reakcja })
        } else {
            if !self.dz.sesje.is_empty() {
                self.zapisz_dziennik();
            }
            None
        };
        if self.dz.dziennik_do_zapisu {
            let dzien = self.dzien_pracy();
            self.dz.zalegle.push(dzien);
        }
        (przeniesione, wczoraj)
    }

    // ---- koniec dnia -------------------------------------------------------------------------------

    /// Dzień bez planu i bez sesji nie ma czego podsumowywać.
    fn dzien_mial_aktywnosc(&self) -> bool {
        !self.dz.plan.is_empty() || !self.dz.sesje.is_empty()
    }

    /// Godzina zamknięcia dnia dzisiaj, o ile dzień ma się zamknąć sam: jest
    /// otwarty, miał aktywność i nie wróciłeś do pracy po zamknięciu.
    fn czas_automatycznego_zamkniecia(&self, teraz: Chwila) -> Option<Chwila> {
        if self.dz.zamkniety
            || self.dz.nie_zamykaj_automatycznie
            || self.kreator
            || matches!(self.faza, Faza::Konfiguracja)
            || !self.dzien_mial_aktywnosc()
        {
            return None;
        }
        lokalna(self.dz.data.and_time(self.ust.koniec_dnia()), teraz)
    }

    fn na_jutro(&self) -> Vec<String> {
        self.plan_zadania().into_iter().filter(|z| !z.zrobione).map(|z| z.klucz.zadanie.clone()).collect()
    }

    fn reakcja_dnia(&self) -> Reakcja {
        Reakcja::dla(self.dz.minuty_sesji(), self.ust.cel_dzienny_min, self.plan_wykonany())
    }

    /// Czas każdego zadania z dzisiejszych sesji względem szacunku, plan i lista na jutro.
    fn podsumowanie_widok(&self) -> PodsumowanieWidok {
        let (zrobione, zaplanowane) = self.licz_plan();
        let mut zadania: Vec<ZadanieDniaWidok> = Vec::new();
        for s in &self.dz.sesje {
            match zadania.iter_mut().find(|z| z.projekt == s.projekt && z.zadanie == s.zadanie) {
                Some(z) => {
                    z.czas_min += s.minuty;
                    z.zrobione |= s.zrobione;
                }
                None => zadania.push(ZadanieDniaWidok {
                    zadanie: s.zadanie.clone(),
                    projekt: s.projekt.clone(),
                    czas_min: s.minuty,
                    szac_min: s.szac_min,
                    zrobione: s.zrobione,
                }),
            }
        }
        for z in &mut zadania {
            z.zrobione |= self.zadanie(&Klucz::new(&z.projekt, &z.zadanie)).is_some_and(|f| f.zrobione);
        }
        PodsumowanieWidok {
            zrobione,
            zaplanowane,
            czas_dnia_min: self.dz.minuty_sesji(),
            cel_min: self.ust.cel_dzienny_min,
            zadania,
            na_jutro: self.na_jutro(),
        }
    }

    /// Zamyka dzień: trwająca sesja zapisuje się jak „Przerwij”, a dziennik
    /// dostaje podsumowanie z reakcją.
    fn domknij_dzien(&mut self, teraz: Chwila) -> Reakcja {
        self.zastosuj_edycje(teraz);
        let reakcja = self.reakcja_dnia();
        self.dz.zamkniety = true;
        self.dz.reakcja = Some(reakcja);
        self.zapisz_dziennik();
        self.brudny = true;
        reakcja
    }

    /// „Koniec pracy na dziś” z przycisku, menu i o godzinie zamknięcia dnia.
    pub fn koniec_pracy(&mut self, teraz: Chwila) {
        if self.dz.zamkniety || self.kreator || matches!(self.faza, Faza::Konfiguracja) {
            return;
        }
        if self.dz.sesja.is_some() {
            self.zakoncz_sesje(false, teraz);
        }
        let reakcja = self.domknij_dzien(teraz);
        self.faza =
            Faza::KoniecDnia { etap: EtapKonca::Reakcja { do_: teraz + Duration::seconds(REAKCJA_S) }, reakcja };
        self.dymek(reakcja.klucz_dymku(), &[("czas_dnia", czas::minuty_tekst(self.dz.minuty_sesji()))]);
        // Zachwycona: konfetti 3 s, dumna: 1,5 s (B4).
        match reakcja {
            Reakcja::Zachwycona => self.efekt("konfetti_dlugie"),
            Reakcja::Dumna => self.efekt("konfetti"),
            _ => {}
        }
        self.zapisz_stan(teraz);
    }

    /// „Do jutra”: postać macha na pożegnanie i chowa się do zasobnika.
    pub fn do_jutra(&mut self, teraz: Chwila) {
        match self.faza {
            Faza::KoniecDnia { etap: EtapKonca::Reakcja { .. }, .. } => self.rozpocznij_pozegnanie(teraz),
            Faza::KoniecDnia { etap: EtapKonca::Spi, .. } => self.ukrycie.recznie = true,
            _ => {}
        }
    }

    fn rozpocznij_pozegnanie(&mut self, teraz: Chwila) {
        let Faza::KoniecDnia { reakcja, .. } = self.faza else { return };
        self.faza =
            Faza::KoniecDnia { etap: EtapKonca::Pozegnanie { do_: teraz + Duration::seconds(POZEGNANIE_S) }, reakcja };
        self.dymek("do_jutra", &[]);
    }

    fn schowaj_do_zasobnika(&mut self) {
        let Faza::KoniecDnia { reakcja, .. } = self.faza else { return };
        self.faza = Faza::KoniecDnia { etap: EtapKonca::Spi, reakcja };
        self.ukrycie.recznie = true;
    }

    /// „Wróć do pracy”: otwiera ten sam dzień od nowa i usuwa podsumowanie z dziennika.
    pub fn wroc_do_pracy(&mut self, teraz: Chwila) {
        if !matches!(
            self.faza,
            Faza::KoniecDnia { etap: EtapKonca::Reakcja { .. } | EtapKonca::Pozegnanie { .. } | EtapKonca::Spi, .. }
        ) {
            return;
        }
        self.dz.zamkniety = false;
        self.dz.reakcja = None;
        self.dz.nie_zamykaj_automatycznie = true;
        self.zapisz_dziennik();
        self.ukrycie.recznie = false;
        if self.plan_zadania().is_empty() {
            self.otworz_wybor();
        } else {
            self.faza = Faza::Lista;
            self.dymek("wznowienie", &[]);
        }
        self.zapisz_stan(teraz);
    }

    /// Po powitaniu: podsumowanie wczorajszego dnia, który zamknąłem za Ciebie.
    fn pokaz_wczoraj(&mut self) {
        let Some(w) = &self.wczoraj else { return };
        self.faza = Faza::KoniecDnia { etap: EtapKonca::Wczoraj, reakcja: w.reakcja };
        self.dymek("zamkniety_wczoraj", &[]);
    }

    /// „Dalej” po podsumowaniu wczoraj: wybór zadań na dziś.
    pub fn dalej_po_wczoraju(&mut self) {
        if matches!(self.faza, Faza::KoniecDnia { etap: EtapKonca::Wczoraj, .. }) {
            self.wczoraj = None;
            self.faza = Faza::Lista;
            self.otworz_wybor();
        }
    }

    /// Po 10 s gratulacje wracają do listy z dymkiem o tym, co jeszcze zostało.
    /// Po ostatnim zadaniu planu zostają z przyciskami, bo to Ty decydujesz,
    /// czy kończysz dzień, czy coś dokładasz.
    fn po_gratulacjach(&mut self) {
        let Faza::Gratulacje { info, klucz, .. } = self.faza.clone() else { return };
        self.brudny = true;
        if info.plan_wykonany {
            self.faza = Faza::Gratulacje { do_: None, info, klucz };
            self.dymek("plan_wykonany", &[]);
        } else {
            self.faza = Faza::Lista;
            self.dymek_zostalo();
        }
    }

    /// „Zostało 2 zadania, ok. 1 h 30 min.” Bez dymku, gdy nic nie zostało albo
    /// pozostałe zadania nie mają szacunków.
    fn dymek_zostalo(&mut self) {
        let otwarte: Vec<&Zadanie> = self.plan_zadania().into_iter().filter(|z| !z.zrobione).collect();
        let pozostalo: u32 = otwarte.iter().map(|z| self.pozostaly_czas(z)).sum();
        if otwarte.is_empty() || pozostalo == 0 {
            return;
        }
        let wstawki =
            [("zadan", czas::zadan_tekst(otwarte.len() as u32)), ("pozostalo", czas::minuty_tekst(pozostalo))];
        self.dymek("zostalo", &wstawki);
    }

    fn przypomnienie_o_przerwie(&mut self, teraz: Chwila) {
        let co = self.ust.przypomnienie_przerwy_min;
        if co == 0 || self.stan() != Stan::Praca {
            return;
        }
        let Some(s) = self.dz.sesja.as_mut() else { return };
        let minuty = s.sekundy_odcinka(teraz) / 60;
        if minuty >= i64::from((s.przypomnienia + 1) * co) {
            s.przypomnienia += 1;
            let ile = czas::minuty_tekst(minuty as u32);
            self.dymek("przypomnienie_przerwy", &[("minuty", ile)]);
            self.efekt("przeciaganie");
            self.brudny = true;
        }
    }

    // ---- powitanie i wybór zadań ------------------------------------------------------------

    fn powitaj(&mut self, teraz: Chwila) {
        self.faza = Faza::Powitanie { do_: teraz + Duration::seconds(POWITANIE_S) };
        self.dz.powitano = true;
        self.brudny = true;
        self.dymek("powitanie", &[]);
    }

    /// Powitanie liczy się od chwili, gdy frontend jest gotowy.
    pub fn przesun_powitanie(&mut self, teraz: Chwila) {
        if let Faza::Powitanie { do_ } = &mut self.faza {
            *do_ = teraz + Duration::seconds(POWITANIE_S);
        }
    }

    fn po_powitaniu(&mut self) {
        if self.wczoraj.is_some() {
            self.pokaz_wczoraj();
        } else if self.plan_zadania().is_empty() {
            self.otworz_wybor();
        } else {
            self.faza = Faza::Lista;
            self.dymek("lista", &[]);
        }
    }

    /// Zadania zaznaczone z góry: w Wyborze po „Zmień plan” otwarte zadania
    /// planu, a przy pierwszym wyborze dnia zadania w toku i niedokończone z
    /// poprzedniego planu.
    fn domyslny_wybor(&self) -> BTreeSet<Klucz> {
        if !self.dz.plan.is_empty() {
            return self.dz.plan.iter().filter(|k| self.zadanie(k).is_some_and(|z| !z.zrobione)).cloned().collect();
        }
        self.zadania
            .otwarte()
            .filter(|z| z.w_toku() || self.dz.przeniesione.contains(&z.klucz))
            .map(|z| z.klucz.clone())
            .collect()
    }

    fn suma_wyboru(&self) -> u32 {
        self.wybor.iter().filter_map(|k| self.zadanie(k)).map(|z| self.pozostaly_czas(z)).sum()
    }

    fn przekroczony(&self) -> bool {
        f64::from(self.suma_wyboru()) > f64::from(self.ust.cel_dzienny_min) * PROG_ZA_DUZO
    }

    /// Stan „Wybór zadań”: pierwszy start dnia albo „Zmień plan”.
    pub fn otworz_wybor(&mut self) {
        if self.dz.sesja.is_some() || matches!(self.faza, Faza::Konfiguracja | Faza::KoniecDnia { .. }) {
            return;
        }
        self.faza = Faza::Wybor;
        self.wybor = self.domyslny_wybor();
        self.wybor_przekroczony = self.przekroczony();
        let klucz = if self.zadania.otwarte().next().is_none() { "wybor_pusty" } else { "wybor" };
        self.dymek(klucz, &[]);
        self.brudny = true;
    }

    pub fn przelacz_wybor(&mut self, klucz: &Klucz) {
        if !matches!(self.faza, Faza::Wybor) || self.zadanie(klucz).is_none_or(|z| z.zrobione) {
            return;
        }
        if !self.wybor.remove(klucz) {
            self.wybor.insert(klucz.clone());
        }
        self.sprawdz_przekroczenie();
        self.brudny = true;
    }

    /// Dymek „To sporo jak na jeden dzień. Na pewno?” przy przejściu przez próg.
    fn sprawdz_przekroczenie(&mut self) {
        let teraz_za_duzo = self.przekroczony();
        if teraz_za_duzo && !self.wybor_przekroczony {
            self.dymek("wybor_za_duzo", &[]);
        }
        self.wybor_przekroczony = teraz_za_duzo;
    }

    /// „Zaczynamy”: plan to wybrane zadania i te z planu, które już mają ptaszka.
    pub fn zatwierdz_wybor(&mut self, teraz: Chwila) {
        if !matches!(self.faza, Faza::Wybor) {
            return;
        }
        let mut klucze: BTreeSet<Klucz> = self.wybor.clone();
        for k in &self.dz.plan {
            if self.zadanie(k).is_some_and(|z| z.zrobione) {
                klucze.insert(k.clone());
            }
        }
        let mut plan: Vec<(usize, Klucz)> =
            klucze.into_iter().filter_map(|k| Some((self.zadanie(&k)?.linia, k))).collect();
        plan.sort_by_key(|(linia, _)| *linia);
        if plan.is_empty() {
            return;
        }
        self.dz.plan = plan.into_iter().map(|(_, k)| k).collect();
        self.dz.przeniesione.clear();
        self.pozna_praca(teraz);
        self.faza = Faza::Lista;
        self.dymek("lista", &[]);
        self.brudny = true;
    }

    /// Wrócenie z „Zmień plan” do listy bez zmian.
    pub fn anuluj_wybor(&mut self) {
        if matches!(self.faza, Faza::Wybor) && !self.plan_zadania().is_empty() {
            self.faza = Faza::Lista;
            self.brudny = true;
        }
    }

    /// „Dodaj zadanie”: dopisuje linię pod nagłówkiem projektu i od razu dodaje
    /// zadanie do dzisiejszego planu (w Wyborze: do zaznaczonych).
    pub fn dodaj_zadanie(&mut self, nazwa: &str, szac_min: Option<u32>, projekt: &str) -> Result<(), String> {
        if self.dz.zamkniety {
            return Err("Dzień jest zamknięty. Wróć do pracy, żeby dopisać zadanie.".into());
        }
        let sciezka = self.ust.plik_zadan().ok_or("Nie wskazano pliku zadań.")?;
        let projekt = if projekt.trim().is_empty() { plik_zadan::INBOX } else { projekt.trim() };
        let klucz =
            plik_zadan::zmien(&sciezka, |t| plik_zadan::dopisz_zadanie(t, projekt, nazwa, szac_min)).map_err(|e| {
                match e {
                    BladZmiany::Edycja(plik_zadan::BladEdycji::PustaNazwa) => "Wpisz nazwę zadania.".to_string(),
                    e => format!("Nie mogę dopisać zadania do pliku: {e}"),
                }
            })?;
        self.wczytaj_zadania();
        if matches!(self.faza, Faza::Wybor) {
            self.wybor.insert(klucz);
            self.sprawdz_przekroczenie();
        } else if !self.dz.plan.contains(&klucz) {
            self.dz.plan.push(klucz);
        }
        self.brudny = true;
        Ok(())
    }

    // ---- działania użytkownika ----------------------------------------------------------------

    pub fn rozpocznij(&mut self, klucz: &Klucz, teraz: Chwila) -> Result<(), String> {
        if self.dz.zamkniety {
            return Err("dzień jest zamknięty; wróć do pracy".into());
        }
        if self.dz.sesja.is_some() {
            return Err("trwa już sesja; najpierw Przerwij albo Zrobione".into());
        }
        let z = self.zadanie(klucz).ok_or_else(|| format!("nie znam zadania „{}”", klucz.zadanie))?;
        if z.zrobione {
            return Err("to zadanie jest już zrobione".into());
        }
        let zadanie = ZadanieSesji { klucz: klucz.clone(), szac_min: z.szac_min, czas_pliku_min: self.czas_zadania(z) };
        if !self.dz.plan.contains(klucz) {
            self.dz.plan.push(klucz.clone());
        }
        self.pozna_praca(teraz);
        self.dymek("start", &[("zadanie", klucz.zadanie.clone())]);
        self.dz.sesja = Some(Sesja::nowa(zadanie, teraz));
        self.faza = Faza::Lista;
        self.brudny = true;
        Ok(())
    }

    /// Plan albo sesja po godzinie zamknięcia to Twój wybór: dzień nie zamyka się już sam.
    fn pozna_praca(&mut self, teraz: Chwila) {
        if teraz.time() >= self.ust.koniec_dnia() {
            self.dz.nie_zamykaj_automatycznie = true;
        }
    }

    pub fn pauza(&mut self, teraz: Chwila) {
        if let Some(s) = self.dz.sesja.as_mut().filter(|s| s.pauza.is_none()) {
            s.pauzuj(teraz, PowodPauzy::Reczna);
            self.dymek("pauza", &[]);
            self.brudny = true;
        }
    }

    pub fn wznow(&mut self, teraz: Chwila) {
        if let Some(s) = self.dz.sesja.as_mut().filter(|s| s.pauza.is_some()) {
            s.wznow(teraz);
            self.dymek("wznowienie", &[]);
            self.brudny = true;
        }
    }

    pub fn przelacz_pauze(&mut self, teraz: Chwila) {
        match self.stan() {
            Stan::Pauza => self.wznow(teraz),
            Stan::Praca | Stan::Jestes => self.pauza(teraz),
            _ => {}
        }
    }

    pub fn zrobione(&mut self, teraz: Chwila) {
        let Some((wpis, czas_zadania)) = self.zakoncz_sesje(true, teraz) else { return };
        let czas_dnia = self.dz.minuty_sesji();
        let plan_wykonany = self.plan_wykonany();
        let mut wstawki = vec![
            ("zadanie", wpis.zadanie.clone()),
            ("czas", czas::minuty_tekst(czas_zadania)),
            ("czas_dnia", czas::minuty_tekst(czas_dnia)),
        ];
        if let Some(s) = wpis.szac_min {
            wstawki.push(("szac", czas::minuty_tekst(s)));
        }
        self.dymek("zrobione", &wstawki);
        self.efekt("konfetti");
        self.faza = Faza::Gratulacje {
            do_: Some(teraz + Duration::seconds(GRATULACJE_S)),
            klucz: Klucz { projekt: wpis.projekt, zadanie: wpis.zadanie.clone() },
            info: GratulacjeInfo {
                zadanie: wpis.zadanie,
                czas_min: czas_zadania,
                szac_min: wpis.szac_min,
                czas_dnia_min: czas_dnia,
                plan_wykonany,
                mozna_cofnac: false,
            },
        };
    }

    pub fn przerwij(&mut self, teraz: Chwila) {
        if self.dz.sesja.is_none() {
            return;
        }
        self.zakoncz_sesje(false, teraz);
        self.faza = Faza::Lista;
    }

    /// Kliknięcie w dymek gratulacji: dalej bez czekania na koniec 10 s.
    pub fn dalej_po_gratulacjach(&mut self) {
        if matches!(self.faza, Faza::Gratulacje { do_: Some(_), .. }) {
            self.po_gratulacjach();
        }
    }

    /// Cofnij w dymku gratulacji (B5), do 10 s po Zrobione: zdejmuje ptaszka i datę
    /// w pliku zadań, w linii sesji w dzienniku zmienia `[x]` na `[ ]` i zaczyna
    /// nową sesję tego samego zadania z dymkiem `cofniete`. Zapisany czas zostaje.
    pub fn cofnij(&mut self, teraz: Chwila) {
        let Faza::Gratulacje { do_: Some(_), klucz, .. } = self.faza.clone() else { return };
        let Some(wpis) =
            self.dz.sesje.last_mut().filter(|w| w.zrobione && w.projekt == klucz.projekt && w.zadanie == klucz.zadanie)
        else {
            return;
        };
        wpis.zrobione = false;
        // Ptaszek, który jeszcze czeka w kolejce (plik był zajęty), po prostu nie trafi do pliku.
        match self.dz.oczekujace.iter().rposition(|e| e.klucz == klucz && e.zrobione.is_some()) {
            Some(i) => self.dz.oczekujace[i].zrobione = None,
            None => self.dz.oczekujace.push(Edycja { klucz: klucz.clone(), minuty: 0, zrobione: None, odznacz: true }),
        }
        self.zastosuj_edycje(teraz);
        self.zapisz_dziennik();
        // Wracamy do pracy nad tym samym zadaniem; czas z pliku obejmuje już zapisaną sesję.
        let (szac_min, czas_pliku_min) = self.zadanie(&klucz).map_or((None, 0), |z| (z.szac_min, self.czas_zadania(z)));
        self.pozna_praca(teraz);
        self.dymek("cofniete", &[("zadanie", klucz.zadanie.clone())]);
        self.dz.sesja = Some(Sesja::nowa(ZadanieSesji { klucz, szac_min, czas_pliku_min }, teraz));
        self.faza = Faza::Lista;
        self.brudny = true;
        self.zapisz_stan(teraz);
    }

    /// „Przerwa” odejmuje czas od ostatniej aktywności, „Pracowałem” go zostawia.
    pub fn odpowiedz_jestes(&mut self, przerwa: bool, teraz: Chwila) {
        let Some(s) = self.dz.sesja.as_mut().filter(|s| s.bezczynna_od.is_some()) else { return };
        if przerwa {
            s.odejmij_bezczynnosc(teraz);
            self.dymek("wznowienie", &[]);
        } else {
            s.bezczynna_od = None;
        }
        self.brudny = true;
    }

    /// Z gratulacji z powrotem do listy, np. po kliknięciu w dymek.
    pub fn wroc_do_listy(&mut self) {
        if matches!(self.faza, Faza::Gratulacje { .. }) {
            self.faza = Faza::Lista;
            self.brudny = true;
        }
    }

    // ---- zdarzenia systemowe --------------------------------------------------------------------

    /// Brak myszy i klawiatury od `sekundy` (GetLastInputInfo, co 30 s).
    pub fn bezczynnosc(&mut self, sekundy: u64, teraz: Chwila) {
        let prog = u64::from(self.ust.prog_bezczynnosci_min) * 60;
        if prog == 0 || sekundy < prog || self.stan() != Stan::Praca || self.schowana_przez_pelny_ekran() {
            return;
        }
        let Some(s) = self.dz.sesja.as_mut() else { return };
        let ostatnia = teraz - Duration::seconds(sekundy as i64);
        s.bezczynna_od = Some(s.otwarty_od().map_or(ostatnia, |od| ostatnia.max(od)));
        self.dymek("jestes", &[]);
        self.brudny = true;
    }

    /// Blokada ekranu, uśpienie, zamknięcie aplikacji lub wylogowanie.
    pub fn pauza_automatyczna(&mut self, powod: PowodPauzy, teraz: Chwila) {
        if let Some(s) = self.dz.sesja.as_mut().filter(|s| s.pauza.is_none()) {
            // Wiszące „Jesteś?” i blokada: nikogo nie było, więc pauza od ostatniej aktywności.
            let od = s.bezczynna_od.unwrap_or(teraz);
            s.pauzuj(od, powod);
            log::info!("pauza automatyczna: {powod:?}");
            self.brudny = true;
        }
        self.zapisz_stan(teraz);
    }

    /// Odblokowanie ekranu albo wybudzenie komputera.
    pub fn powrot(&mut self, teraz: Chwila) {
        self.tick(teraz);
        if self.dz.sesja.as_ref().is_some_and(|s| s.pauza.is_some()) {
            let dawno = self.ostatni_dymek_powrotu.is_none_or(|t| teraz - t > Duration::seconds(10));
            if dawno {
                self.dymek("po_blokadzie", &[]);
                self.ostatni_dymek_powrotu = Some(teraz);
            }
        }
    }

    pub fn ustaw_pelny_ekran(&mut self, pelny: bool) {
        if self.ukrycie.pelny_ekran != pelny {
            self.ukrycie.pelny_ekran = pelny;
            if !pelny {
                self.ukrycie.wymuszone = false;
            }
        }
    }

    /// Pokaż / Ukryj z zasobnika i skrót globalny.
    pub fn przelacz_widocznosc(&mut self) {
        if self.widoczna() {
            self.ukrycie.recznie = true;
        } else {
            self.pokaz();
        }
    }

    pub fn pokaz(&mut self) {
        self.ukrycie.recznie = false;
        if self.ukrycie.pelny_ekran {
            self.ukrycie.wymuszone = true;
        }
    }

    /// Użytkownik przeniósł widget: nowy monitor, róg i odstępy trafiają do `okno.json`.
    pub fn ustaw_kotwice(&mut self, kotwica: KotwicaOkna, _teraz: Chwila) {
        self.naroznik = kotwica.kotwica;
        if self.okno != kotwica {
            self.okno = kotwica;
            if let Err(e) = self.magazyn.zapisz_okno(&self.okno) {
                log::warn!("nie mogę zapisać położenia okna: {e}");
            }
        }
    }

    /// Róg, który naprawdę obowiązuje po rozwiązaniu monitora (zapis zostaje,
    /// gdy monitor chwilowo zniknął).
    pub fn ustaw_naroznik(&mut self, naroznik: Naroznik) {
        self.naroznik = naroznik;
    }

    /// „Przywróć pozycję”: domyślne miejsce na monitorze głównym (B6).
    pub fn przywroc_pozycje(&mut self, teraz: Chwila) {
        self.ustaw_kotwice(KotwicaOkna::default(), teraz);
    }

    pub fn zmien_ustawienia(&mut self, nowe: Ustawienia, teraz: Chwila) {
        self.ust = nowe;
        self.wczytaj_zadania();
        if matches!(self.faza, Faza::Wybor) {
            self.wybor_przekroczony = self.przekroczony();
        }
        self.brudny = true;
        self.zapisz_stan(teraz);
    }

    /// „Zamknij asystentkę”: trwająca sesja zapisuje się jak Przerwij, a stan
    /// dnia na dysk. Dzień zostaje otwarty, więc kolejne uruchomienie tego
    /// samego dnia wraca do Listy (B6).
    pub fn zamknij_asystentke(&mut self, teraz: Chwila) {
        if self.dz.sesja.is_some() {
            self.zakoncz_sesje(false, teraz);
        }
        self.brudny = true;
        self.zapisz_stan(teraz);
    }

    /// Ustawienia nie dały się wczytać: zostają poprzednie.
    pub fn blad_ustawien(&mut self) {
        self.dymek("blad_ustawien", &[]);
    }

    // ---- sesje i pliki -----------------------------------------------------------------------------

    /// Zapisuje czas (i ptaszka) w pliku zadań, od razu, gdy sesja się kończy.
    /// Co się nie uda z powodu dostępu do pliku (Obsidian, antywirus, synchronizacja
    /// chwilowo go trzymają), czeka w kolejce i jest ponawiane po sekundzie, potem
    /// coraz rzadziej; zadanie, którego już nie ma w pliku, zostaje tylko w dzienniku.
    fn zastosuj_edycje(&mut self, teraz: Chwila) {
        let Some(sciezka) = self.ust.plik_zadan() else { return };
        let poczatek = std::time::Instant::now();
        let mut zmiana = false;
        let mut blad_zapisu = false;
        while let Some(e) = self.dz.oczekujace.first().cloned() {
            let wynik = plik_zadan::zmien(&sciezka, |t| {
                let nowy = if e.odznacz {
                    plik_zadan::odznacz_zrobione(t, &e.klucz)?
                } else {
                    plik_zadan::dopisz_czas(t, &e.klucz, e.minuty, e.zrobione)?
                };
                Ok((nowy, ()))
            });
            match wynik {
                Ok(()) => {
                    log::info!(
                        "plik zadań: {} · {}{} ({} ms)",
                        e.klucz.zadanie,
                        if e.odznacz { "ptaszek zdjęty".to_string() } else { format!("+{} min", e.minuty) },
                        if e.zrobione.is_some() { " · ✅" } else { "" },
                        poczatek.elapsed().as_millis()
                    );
                    self.dz.oczekujace.remove(0);
                    zmiana = true;
                }
                Err(BladZmiany::Edycja(_)) => {
                    self.dz.oczekujace.remove(0);
                    self.dymek("nie_znaleziono", &[("zadanie", e.klucz.zadanie.clone())]);
                    zmiana = true;
                }
                Err(BladZmiany::Io(err)) => {
                    let proba = self.proby_zapisu;
                    if proba == 0 {
                        log::warn!(
                            "zapis w pliku zadań czeka w kolejce, ponowię za chwilę ({}): {err}",
                            sciezka.display()
                        );
                    } else {
                        log::debug!("zapis w pliku zadań wciąż czeka (próba {proba}): {err}");
                    }
                    blad_zapisu = true;
                    break;
                }
            }
        }
        if blad_zapisu {
            let za = PONOWIENIA_ZAPISU_S[self.proby_zapisu.min(PONOWIENIA_ZAPISU_S.len() - 1)];
            self.zapis_ponow = Some(teraz + Duration::seconds(za));
            self.proby_zapisu += 1;
        } else {
            if self.proby_zapisu > 0 && self.dz.oczekujace.is_empty() {
                log::info!("zapis w pliku zadań udał się po {} ponowieniach", self.proby_zapisu);
            }
            self.zapis_ponow = None;
            self.proby_zapisu = 0;
        }
        if zmiana {
            self.wczytaj_zadania();
            self.brudny = true;
        }
    }

    fn dzien_pracy(&self) -> DzienPracy {
        let (zrobione, zaplanowane) = self.licz_plan();
        DzienPracy {
            data: self.dz.data,
            sesje: self.dz.sesje.clone(),
            cel_min: self.ust.cel_dzienny_min,
            zaplanowane,
            zrobione,
            podsumowanie: self
                .dz
                .reakcja
                .filter(|_| self.dz.zamkniety)
                .map(|r| Podsumowanie { reakcja: r.nazwa().to_string(), na_jutro: self.na_jutro() }),
        }
    }

    fn zapisz_dziennik(&mut self) {
        let Some(folder) = self.ust.folder_dziennika() else { return };
        match dziennik::zapisz(&folder, &self.dzien_pracy()) {
            Ok(()) => self.dz.dziennik_do_zapisu = false,
            Err(e) => {
                log::warn!("dziennik czeka w kolejce: {e}");
                self.dz.dziennik_do_zapisu = true;
            }
        }
        self.brudny = true;
    }

    fn ponow_zalegle_dzienniki(&mut self) {
        let Some(folder) = self.ust.folder_dziennika() else { return };
        let przed = self.dz.zalegle.len();
        self.dz.zalegle.retain(|d| dziennik::zapisz(&folder, d).is_err());
        if self.dz.zalegle.len() != przed {
            self.brudny = true;
        }
    }

    /// Zapis lokalnego stanu dnia: co 30 s w czasie pracy i przy każdej zmianie.
    pub fn zapisz_stan(&mut self, teraz: Chwila) {
        if !self.brudny {
            return;
        }
        self.dz.zapisano = Some(teraz);
        match self.magazyn.zapisz_dzien(&self.dz) {
            Ok(()) => self.brudny = false,
            Err(e) => log::error!("nie mogę zapisać stanu dnia: {e}"),
        }
    }

    /// Kończy sesję: wpis do rejestru dnia, zapis do pliku zadań i dziennika.
    /// Zwraca wpis i łączny czas zadania (plik plus ta sesja).
    fn zakoncz_sesje(&mut self, zrobione: bool, teraz: Chwila) -> Option<(WpisSesji, u32)> {
        let mut s = self.dz.sesja.take()?;
        self.brudny = true;
        s.zakoncz(teraz);
        let sekundy = s.sekundy(teraz);
        if !zrobione && sekundy < MIN_SESJA_S {
            log::info!("sesja krótsza niż minuta, nie zapisuję: {}", s.zadanie.nazwa());
            return None;
        }
        let minuty = czas::sekundy_na_minuty(sekundy);
        let wpis = WpisSesji {
            projekt: s.zadanie.klucz.projekt.clone(),
            zadanie: s.zadanie.klucz.zadanie.clone(),
            szac_min: s.zadanie.szac_min,
            start: s.start,
            koniec: s.koniec(),
            sekundy,
            minuty,
            zrobione,
        };
        self.dz.sesje.push(wpis.clone());
        self.dz.oczekujace.push(Edycja {
            klucz: s.zadanie.klucz.clone(),
            minuty,
            zrobione: zrobione.then_some(s.dzien),
            odznacz: false,
        });
        log::info!(
            "sesja zapisana: {} · {minuty} min · {}",
            wpis.zadanie,
            if zrobione { "Zrobione" } else { "W toku" }
        );
        self.zastosuj_edycje(teraz);
        self.zapisz_dziennik();
        self.zapisz_stan(teraz);
        Some((wpis, s.zadanie.czas_pliku_min + minuty))
    }

    // ---- dymki --------------------------------------------------------------------------------------

    /// Dymek z wstawkami; `imie` (forma zwracania się) i `postac` (imię postaci) są dodawane zawsze.
    fn dymek(&mut self, klucz: &str, wstawki: &[(&str, String)]) {
        let mut mapa: std::collections::BTreeMap<String, String> =
            wstawki.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        mapa.insert("imie".into(), self.ust.imie.clone());
        mapa.insert("postac".into(), self.ust.postac().to_string());
        self.zdarzenia.push(Zdarzenie::Dymek { klucz: klucz.to_string(), wstawki: mapa });
    }

    fn efekt(&mut self, nazwa: &str) {
        self.zdarzenia.push(Zdarzenie::Efekt { nazwa: nazwa.to_string() });
    }

    // ---- migawka --------------------------------------------------------------------------------------

    fn widok_zadania(&self, z: &Zadanie, zaznaczone: bool) -> ZadanieWidok {
        ZadanieWidok {
            projekt: z.klucz.projekt.clone(),
            zadanie: z.klucz.zadanie.clone(),
            szac_min: z.szac_min,
            czas_min: self.czas_zadania(z),
            zrobione: z.zrobione,
            w_toku: !z.zrobione && self.czas_zadania(z) > 0,
            zaznaczone,
            termin: z.termin.map(czas::data_iso),
        }
    }

    fn widok_wyboru(&self) -> WyborWidok {
        let priorytet = self.domyslny_wybor();
        let mut projekty: Vec<(String, Vec<&Zadanie>)> = Vec::new();
        for z in self.zadania.otwarte() {
            match projekty.iter_mut().find(|(n, _)| *n == z.klucz.projekt) {
                Some((_, lista)) => lista.push(z),
                None => projekty.push((z.klucz.projekt.clone(), vec![z])),
            }
        }
        let projekty = projekty
            .into_iter()
            .map(|(nazwa, mut lista)| {
                // Zaznaczone z góry, potem według terminu, a potem według kolejności w pliku.
                lista.sort_by_key(|z| (!priorytet.contains(&z.klucz), z.termin.is_none(), z.termin, z.linia));
                ProjektWidok {
                    nazwa,
                    zadania: lista.into_iter().map(|z| self.widok_zadania(z, self.wybor.contains(&z.klucz))).collect(),
                }
            })
            .collect();
        WyborWidok {
            projekty,
            suma_min: self.suma_wyboru(),
            cel_min: self.ust.cel_dzienny_min,
            przekroczony: self.przekroczony(),
            zaznaczonych: self.wybor.len(),
            mozna_wrocic: !self.plan_zadania().is_empty(),
        }
    }

    fn widok_listy(&self) -> ListaWidok {
        let zadania: Vec<&Zadanie> = self.plan_zadania();
        let (zrobione, razem) = self.licz_plan();
        ListaWidok {
            pozostalo_min: zadania.iter().filter(|z| !z.zrobione).map(|z| self.pozostaly_czas(z)).sum(),
            zadania: zadania.into_iter().map(|z| self.widok_zadania(z, false)).collect(),
            zrobione,
            razem,
        }
    }

    pub fn migawka(&self) -> Migawka {
        let stan = self.stan();
        let sesja = self.dz.sesja.as_ref().map(|s| SesjaWidok {
            projekt: s.zadanie.klucz.projekt.clone(),
            zadanie: s.zadanie.klucz.zadanie.clone(),
            szac_min: s.zadanie.szac_min,
            sekundy_zamkniete: i64::from(s.zadanie.czas_pliku_min) * 60 + s.sekundy_zamkniete(),
            odcinek_od_ms: s.otwarty_od().map(czas::ms),
            pauza_od_ms: s.pauza.as_ref().map(|p| czas::ms(p.od)),
            powod_pauzy: s.pauza.as_ref().map(|p| p.powod),
            bezczynna_od_ms: s.bezczynna_od.map(czas::ms),
        });
        let gratulacje = match &self.faza {
            Faza::Gratulacje { info, do_, .. } if self.dz.sesja.is_none() => {
                Some(GratulacjeInfo { mozna_cofnac: do_.is_some(), ..info.clone() })
            }
            _ => None,
        };
        let koniec = match &self.faza {
            Faza::KoniecDnia { etap, reakcja } if self.dz.sesja.is_none() => Some(KoniecWidok {
                etap: etap.nazwa(),
                reakcja: *reakcja,
                podsumowanie: match (etap, &self.wczoraj) {
                    (EtapKonca::Wczoraj, Some(w)) => w.widok.clone(),
                    _ => self.podsumowanie_widok(),
                },
            }),
            _ => None,
        };
        let w_toku_minuty = self.dz.sesja.as_ref().map_or(0, |s| s.minuty(czas::teraz()));
        Migawka {
            stan: stan.nazwa(),
            widoczna: self.widoczna(),
            strona: self.naroznik.strona(),
            pion: self.naroznik.pion(),
            imie: self.ust.imie.clone(),
            wybor: (stan == Stan::WyborZadan).then(|| self.widok_wyboru()),
            lista: matches!(stan, Stan::Lista | Stan::Gratulacje).then(|| self.widok_listy()),
            sesja,
            gratulacje,
            koniec,
            dzien: DzienWidok { minuty: self.dz.minuty_sesji() + w_toku_minuty, cel_min: self.ust.cel_dzienny_min },
            blad_pliku: self.blad_pliku.clone(),
            projekty_do_dopisania: self.zadania.projekty_do_dopisania(),
        }
    }
}

/// Lokalna chwila dla daty i godziny bez strefy, w strefie `wzor`.
fn lokalna(naiwna: chrono::NaiveDateTime, wzor: Chwila) -> Option<Chwila> {
    naiwna.and_local_timezone(*wzor.offset()).single()
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::czas::chwila;
    use crate::magazyn::Magazyn;
    use std::fs;
    use std::path::PathBuf;

    const ZADANIA: &str = include_str!("../../przyklady/Zadania.md");

    struct Srodowisko {
        _katalog: tempfile::TempDir,
        vault: PathBuf,
        lokalny: PathBuf,
    }

    impl Srodowisko {
        fn nowe() -> Srodowisko {
            let katalog = tempfile::tempdir().unwrap();
            let vault = katalog.path().join("vault");
            fs::create_dir_all(&vault).unwrap();
            fs::write(vault.join("Zadania.md"), ZADANIA).unwrap();
            let lokalny = katalog.path().join("AppData").join("Asystentka");
            Srodowisko { vault, lokalny, _katalog: katalog }
        }

        fn plik(&self) -> PathBuf {
            self.vault.join("Zadania.md")
        }

        fn ustawienia(&self) -> Ustawienia {
            Ustawienia {
                imie: "Dżiordżio".into(),
                plik_zadan: self.plik().to_string_lossy().into(),
                folder_dziennika: self.vault.join("Dziennik pracy").to_string_lossy().into(),
                ..Ustawienia::default()
            }
        }

        fn nowy_rdzen(&self, teraz: Chwila) -> Rdzen {
            Rdzen::nowy(Magazyn::new(self.lokalny.clone()), self.ustawienia(), teraz)
        }

        fn rdzen(&self, teraz: Chwila) -> Rdzen {
            let mut r = self.nowy_rdzen(teraz);
            r.start(teraz);
            r
        }

        fn zadania(&self) -> String {
            fs::read_to_string(self.plik()).unwrap()
        }

        fn dziennik(&self, data: &str) -> String {
            fs::read_to_string(self.vault.join("Dziennik pracy").join(format!("{data}.md"))).unwrap()
        }
    }

    fn t(godzina: &str) -> Chwila {
        chwila(&format!("2026-10-30T{godzina}+01:00"))
    }

    fn k(projekt: &str, zadanie: &str) -> Klucz {
        Klucz::new(projekt, zadanie)
    }

    fn montaz() -> Klucz {
        k("Film #1 · FIFA", "Montaż: plansze 1–5")
    }

    fn miniatura() -> Klucz {
        k("Film #1 · FIFA", "Miniatura")
    }

    fn mail() -> Klucz {
        k("Inbox", "Odpisać na maila od montażysty")
    }

    fn dymki(r: &mut Rdzen) -> Vec<String> {
        r.wez_zdarzenia()
            .into_iter()
            .filter_map(|z| match z {
                Zdarzenie::Dymek { klucz, .. } => Some(klucz),
                Zdarzenie::Efekt { .. } => None,
            })
            .collect()
    }

    fn wstawki(zdarzenia: &[Zdarzenie], klucz_dymku: &str) -> std::collections::BTreeMap<String, String> {
        zdarzenia
            .iter()
            .find_map(|z| match z {
                Zdarzenie::Dymek { klucz, wstawki } if klucz == klucz_dymku => Some(wstawki.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("brak dymku {klucz_dymku}"))
    }

    /// Powitanie i zatwierdzenie wyboru z domyślnymi zaznaczeniami.
    fn zacznij_dzien(r: &mut Rdzen, godzina: &str) {
        r.tick(t(godzina));
        r.tick(t(godzina).max(t(godzina) + Duration::seconds(10)));
        assert_eq!(r.stan(), Stan::WyborZadan);
        r.zatwierdz_wybor(t(godzina));
        assert_eq!(r.stan(), Stan::Lista);
    }

    // ---- krok 3: wybór zadań ------------------------------------------------------------

    #[test]
    fn pierwszy_start_dnia_wita_i_pokazuje_wybor_z_zadaniami_w_toku() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        assert_eq!(r.stan(), Stan::Powitanie);
        let zdarzenia = r.wez_zdarzenia();
        assert!(zdarzenia.contains(&Zdarzenie::Efekt { nazwa: "glitch_wejscie".into() }));
        assert_eq!(wstawki(&zdarzenia, "powitanie")["imie"], "Dżiordżio");
        r.tick(t("09:00:05"));
        assert_eq!(r.stan(), Stan::Powitanie);
        r.tick(t("09:00:06"));
        assert_eq!(r.stan(), Stan::WyborZadan);
        assert_eq!(dymki(&mut r), vec!["wybor"]);

        let w = r.migawka().wybor.unwrap();
        let nazwy: Vec<(&str, Vec<(&str, bool)>)> = w
            .projekty
            .iter()
            .map(|p| (p.nazwa.as_str(), p.zadania.iter().map(|z| (z.zadanie.as_str(), z.zaznaczone)).collect()))
            .collect();
        assert_eq!(
            nazwy,
            vec![
                ("Film #1 · FIFA", vec![("Montaż: plansze 1–5", true), ("Miniatura", false)]),
                ("Inbox", vec![("Odpisać na maila od montażysty", false)]),
            ],
            "zrobione zadania nie są do wyboru, a zadanie w toku jest zaznaczone z góry"
        );
        // Suma liczy pozostały czas: 120 − 45 min.
        assert_eq!((w.suma_min, w.cel_min, w.przekroczony, w.zaznaczonych), (75, 240, false, 1));
        assert!(!w.mozna_wrocic);
        assert_eq!(r.migawka().projekty_do_dopisania, ["Film #1 · FIFA", "Inbox"]);
    }

    #[test]
    fn restart_tego_samego_dnia_wraca_do_listy_z_tym_samym_planem() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&mail());
        r.zatwierdz_wybor(t("09:01:00"));
        assert_eq!(r.stan(), Stan::Lista);
        r.zapisz_stan(t("09:01:00"));
        drop(r);

        let mut r2 = sr.rdzen(t("11:00:00"));
        assert_eq!(r2.stan(), Stan::Lista);
        assert_eq!(dymki(&mut r2), vec!["powrot"]);
        let lista = r2.migawka().lista.unwrap();
        let nazwy: Vec<&str> = lista.zadania.iter().map(|z| z.zadanie.as_str()).collect();
        assert_eq!(nazwy, ["Montaż: plansze 1–5", "Odpisać na maila od montażysty"]);
        assert_eq!((lista.zrobione, lista.razem), (0, 2));
    }

    #[test]
    fn dzien_bez_planu_po_restarcie_wraca_do_wyboru() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.zapisz_stan(t("09:00:01"));
        drop(r);
        let mut r2 = sr.rdzen(t("09:30:00"));
        assert_eq!(r2.stan(), Stan::WyborZadan, "bez planu nie ma do czego wracać, a powitanie już było");
        assert_eq!(dymki(&mut r2), vec!["wybor"]);
    }

    #[test]
    fn suma_robi_sie_koralowa_po_przekroczeniu_celu_o_20_procent() {
        let sr = Srodowisko::nowe();
        let mut r = sr.nowy_rdzen(t("09:00:00"));
        r.ust.cel_dzienny_min = 95; // próg: 114 min
        r.start(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.wez_zdarzenia();
        r.przelacz_wybor(&miniatura()); // 75 + 30 = 105
        assert!(!r.migawka().wybor.unwrap().przekroczony);
        assert!(dymki(&mut r).is_empty());
        r.przelacz_wybor(&mail()); // 120 > 114
        let w = r.migawka().wybor.unwrap();
        assert_eq!((w.suma_min, w.przekroczony), (120, true));
        assert_eq!(dymki(&mut r), vec!["wybor_za_duzo"]);
        r.przelacz_wybor(&k("Film #1 · FIFA", "Montaż: plansze 1–5")); // zostaje 45
        assert!(!r.migawka().wybor.unwrap().przekroczony);
        r.przelacz_wybor(&montaz());
        assert_eq!(dymki(&mut r), vec!["wybor_za_duzo"], "znów po przekroczeniu progu");
        // Zadania zrobionego nie da się zaznaczyć.
        r.przelacz_wybor(&k("Film #1 · FIFA", "Scenariusz: intro"));
        assert_eq!(r.migawka().wybor.unwrap().zaznaczonych, 3);
    }

    #[test]
    fn suma_liczy_min_10_min_na_zadanie_i_zero_bez_szacunku() {
        let sr = Srodowisko::nowe();
        fs::write(
            sr.plik(),
            "## P\n- [ ] Prawie gotowe [szac:: 30] [czas:: 28]\n- [ ] Przekroczone [szac:: 30] [czas:: 50]\n- [ ] Bez szacunku\n- [ ] Zwykłe [szac:: 40]\n",
        )
        .unwrap();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        // Dwa zadania w toku są zaznaczone z góry; dokładam dwa pozostałe.
        for zadanie in ["Bez szacunku", "Zwykłe"] {
            r.przelacz_wybor(&k("P", zadanie));
        }
        let w = r.migawka().wybor.unwrap();
        // 28/30 → 10, 50/30 → 10, bez szacunku → 0, zwykłe → 40.
        assert_eq!(w.zaznaczonych, 4);
        assert_eq!(w.suma_min, 60);
    }

    #[test]
    fn kolejnosc_w_projekcie_zaznaczone_potem_termin_potem_plik() {
        let sr = Srodowisko::nowe();
        fs::write(
            sr.plik(),
            "## P\n- [ ] Pierwsze w pliku\n- [ ] Z późnym terminem [termin:: 2026-12-01]\n- [ ] Z wczesnym terminem [termin:: 2026-11-01]\n- [ ] W toku [czas:: 5] [szac:: 20]\n- [ ] Ostatnie w pliku\n",
        )
        .unwrap();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        let w = r.migawka().wybor.unwrap();
        let nazwy: Vec<&str> = w.projekty[0].zadania.iter().map(|z| z.zadanie.as_str()).collect();
        assert_eq!(
            nazwy,
            ["W toku", "Z wczesnym terminem", "Z późnym terminem", "Pierwsze w pliku", "Ostatnie w pliku"]
        );
        assert_eq!(w.projekty[0].zadania[2].termin.as_deref(), Some("2026-12-01"));
    }

    #[test]
    fn niedokonczone_z_poprzedniego_planu_sa_zaznaczone_z_gory() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&montaz()); // odznacz zadanie w toku
        r.przelacz_wybor(&miniatura());
        r.zatwierdz_wybor(t("09:01:00"));
        r.zapisz_stan(t("09:01:00"));
        drop(r);
        // Następnego dnia Miniatura (bez czasu, więc nie „w toku”) wraca zaznaczona.
        let jutro = chwila("2026-10-31T08:00:00+01:00");
        let mut r2 = sr.rdzen(jutro);
        assert_eq!(r2.stan(), Stan::Powitanie, "nowy dzień ma pełne powitanie");
        r2.tick(jutro + Duration::seconds(10));
        assert_eq!(r2.stan(), Stan::KoniecDnia, "najpierw podsumowanie wczoraj");
        r2.dalej_po_wczoraju();
        assert_eq!(r2.stan(), Stan::WyborZadan);
        let w = r2.migawka().wybor.unwrap();
        let zaznaczone: Vec<&str> =
            w.projekty.iter().flat_map(|p| &p.zadania).filter(|z| z.zaznaczone).map(|z| z.zadanie.as_str()).collect();
        assert_eq!(zaznaczone, ["Montaż: plansze 1–5", "Miniatura"]);
    }

    #[test]
    fn zmien_plan_zachowuje_zrobione_i_pozwala_zmienic_reszte() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&miniatura());
        r.zatwierdz_wybor(t("09:01:00"));
        r.rozpocznij(&miniatura(), t("09:02:00")).unwrap();
        r.zrobione(t("09:50:00"));
        r.tick(t("09:50:10"));
        assert_eq!(r.stan(), Stan::Lista);
        r.otworz_wybor();
        assert_eq!(r.stan(), Stan::WyborZadan);
        let w = r.migawka().wybor.unwrap();
        assert!(w.mozna_wrocic);
        assert_eq!(w.zaznaczonych, 1, "zaznaczony zostaje tylko otwarty Montaż z planu");
        r.przelacz_wybor(&montaz());
        r.przelacz_wybor(&mail());
        r.zatwierdz_wybor(t("09:51:00"));
        let lista = r.migawka().lista.unwrap();
        let nazwy: Vec<(&str, bool)> = lista.zadania.iter().map(|z| (z.zadanie.as_str(), z.zrobione)).collect();
        assert_eq!(nazwy, [("Miniatura", true), ("Odpisać na maila od montażysty", false)]);
        // Anulowanie wraca do listy bez zmian.
        r.otworz_wybor();
        r.przelacz_wybor(&mail());
        r.anuluj_wybor();
        assert_eq!(r.stan(), Stan::Lista);
        assert_eq!(r.migawka().lista.unwrap().zadania.len(), 2);
    }

    #[test]
    fn dodaj_zadanie_dopisuje_linie_i_dodaje_do_planu() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        zacznij_dzien(&mut r, "09:00:00");
        r.dodaj_zadanie("Napisać opis", Some(20), "Film #1 · FIFA").unwrap();
        assert_eq!(sr.zadania(), ZADANIA.replace("✅ 2026-10-29\n", "✅ 2026-10-29\n- [ ] Napisać opis [szac:: 20]\n"));
        let nazwy: Vec<String> = r.migawka().lista.unwrap().zadania.into_iter().map(|z| z.zadanie).collect();
        assert_eq!(nazwy, ["Montaż: plansze 1–5", "Napisać opis"]);
        // Do pustego Inboxu, bez szacunku.
        r.dodaj_zadanie("Zadzwonić", None, "Inbox").unwrap();
        assert!(sr.zadania().contains("- [ ] Odpisać na maila od montażysty [szac:: 15]\n- [ ] Zadzwonić\n"));
        assert!(r.dodaj_zadanie("  ", None, "Inbox").unwrap_err().contains("nazwę"));
    }

    #[test]
    fn dodaj_zadanie_w_wyborze_zaznacza_je_i_liczy_do_sumy() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.dodaj_zadanie("Nowe", Some(30), "Inbox").unwrap();
        let w = r.migawka().wybor.unwrap();
        assert_eq!((w.zaznaczonych, w.suma_min), (2, 105));
        r.zatwierdz_wybor(t("09:01:00"));
        assert_eq!(r.migawka().lista.unwrap().razem, 2);
    }

    #[test]
    fn pusty_plik_zadan_daje_wybor_pusty_i_odswieza_sie_po_dopisaniu() {
        let sr = Srodowisko::nowe();
        fs::write(sr.plik(), "# Zadania\n## Inbox\n").unwrap();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        assert_eq!(r.stan(), Stan::WyborZadan);
        assert_eq!(dymki(&mut r), vec!["powitanie", "wybor_pusty"]);
        // Dopisujesz zadanie w Obsidianie: lista pojawia się po odświeżeniu pliku.
        fs::write(sr.plik(), "# Zadania\n## Inbox\n- [ ] Coś [szac:: 20]\n").unwrap();
        assert!(r.odswiez_zadania());
        assert_eq!(r.migawka().wybor.unwrap().projekty[0].zadania.len(), 1);
        assert!(!r.odswiez_zadania(), "bez zmiany nic się nie dzieje");
    }

    #[test]
    fn brak_pliku_zadan_jest_zgloszony_w_migawce() {
        let sr = Srodowisko::nowe();
        fs::remove_file(sr.plik()).unwrap();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        assert!(r.migawka().blad_pliku.unwrap().contains("Nie mogę wczytać"));
        fs::write(sr.plik(), ZADANIA).unwrap();
        r.odswiez_zadania();
        assert!(r.migawka().blad_pliku.is_none());
    }

    #[test]
    fn kreator_zastepuje_powitanie_i_prowadzi_do_wyboru() {
        let sr = Srodowisko::nowe();
        let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
        r.wymus_kreator();
        r.start(t("09:00:00"));
        assert_eq!(r.stan(), Stan::Konfiguracja);
        assert_eq!(dymki(&mut r), vec!["konfiguracja_imie"]);
        r.tick(t("09:00:30"));
        assert_eq!(r.stan(), Stan::Konfiguracja, "kreator czeka na użytkownika");
        r.zakoncz_kreator(sr.ustawienia(), t("09:02:00"));
        assert_eq!(r.stan(), Stan::WyborZadan);
        assert_eq!(dymki(&mut r), vec!["konfiguracja_gotowe"]);
        // Kolejny start tego samego dnia nie wita ponownie.
        r.zapisz_stan(t("09:02:01"));
        let mut r2 = sr.rdzen(t("11:00:00"));
        assert_eq!(r2.stan(), Stan::WyborZadan);
        assert_eq!(dymki(&mut r2), vec!["wybor"]);
    }

    // ---- sesje, plik zadań i dziennik ---------------------------------------------------------

    #[test]
    fn dzien_pracy_zapisuje_plik_zadan_i_dziennik() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&miniatura());
        r.zatwierdz_wybor(t("09:00:30"));

        r.rozpocznij(&miniatura(), t("11:30:00")).unwrap();
        assert!(r.rozpocznij(&montaz(), t("11:31:00")).is_err(), "w Pracy jest zawsze jedno zadanie");
        r.zrobione(t("12:10:00"));
        assert_eq!(r.stan(), Stan::Gratulacje);
        r.tick(t("12:10:10"));
        assert_eq!(r.stan(), Stan::Lista);

        r.rozpocznij(&montaz(), t("10:20:00")).unwrap();
        r.pauza(t("10:40:00"));
        r.wznow(t("10:50:00"));
        r.przerwij(t("11:05:00"));

        assert_eq!(
            sr.zadania(),
            ZADANIA
                .replace("- [ ] Miniatura [szac:: 30]", "- [x] Miniatura [szac:: 30] [czas:: 40] ✅ 2026-10-30")
                .replace("[czas:: 45]", "[czas:: 80]"),
            "Zrobione dopisuje czas, ptaszka i datę; Przerwij tylko czas"
        );
        let dziennik = sr.dziennik("2026-10-30");
        assert!(dziennik.starts_with(
            "---\ndata: 2026-10-30\nminuty: 75\ncel_min: 240\nzaplanowane: 2\nzrobione: 1\n---\n<!-- widget:start -->\n"
        ));
        assert!(dziennik.contains("- [x] 11:30–12:10 · Miniatura · 40 min (szac. 30) · Film #1 · FIFA\n"));
        assert!(dziennik.contains("- [ ] 10:20–11:05 · Montaż: plansze 1–5 · 35 min · w toku · Film #1 · FIFA\n"));
        assert!(dziennik.contains("**Razem:** 1 h 15 min z celu 4 h · **Plan:** 1 z 2 zadań\n"));
        let lista = r.migawka().lista.unwrap();
        assert_eq!((lista.zrobione, lista.razem), (1, 2));
        let montaz = lista.zadania.iter().find(|z| z.zadanie.starts_with("Montaż")).unwrap();
        assert_eq!((montaz.czas_min, montaz.w_toku), (80, true));
    }

    #[test]
    fn licznik_zaczyna_od_czasu_zapisanego_w_pliku() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:10"));
        r.rozpocznij(&montaz(), t("09:10:00")).unwrap();
        let s = r.migawka().sesja.unwrap();
        assert_eq!(s.sekundy_zamkniete, 45 * 60, "licznik pokazuje łączny czas: plik plus bieżąca sesja");
        assert_eq!(s.szac_min, Some(120));
        r.pauza(t("09:30:00"));
        assert_eq!(r.migawka().sesja.unwrap().sekundy_zamkniete, 45 * 60 + 20 * 60);
    }

    #[test]
    fn krotka_przerwana_sesja_nie_trafia_do_pliku_ani_dziennika() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:10"));
        r.rozpocznij(&montaz(), t("09:10:00")).unwrap();
        r.przerwij(t("09:10:40"));
        assert_eq!(sr.zadania(), ZADANIA);
        assert!(!sr.vault.join("Dziennik pracy").exists());
    }

    #[test]
    fn zadanie_ze_zmienionym_tekstem_zostaje_tylko_w_dzienniku() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&miniatura());
        r.zatwierdz_wybor(t("09:00:30"));
        r.rozpocznij(&miniatura(), t("09:10:00")).unwrap();
        // Zmieniasz tekst zadania w Obsidianie w trakcie sesji.
        fs::write(sr.plik(), ZADANIA.replace("Miniatura [szac:: 30]", "Miniatura v2 [szac:: 30]")).unwrap();
        r.wez_zdarzenia();
        r.zrobione(t("09:50:00"));
        assert_eq!(sr.zadania(), ZADANIA.replace("Miniatura [szac:: 30]", "Miniatura v2 [szac:: 30]"));
        let zd = r.wez_zdarzenia();
        assert_eq!(wstawki(&zd, "nie_znaleziono")["zadanie"], "Miniatura");
        assert!(sr.dziennik("2026-10-30").contains("- [x] 09:10–09:50 · Miniatura · 40 min (szac. 30)"));
    }

    #[test]
    fn niedostepny_plik_zadan_zapisy_czekaja_w_kolejce() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:10"));
        r.rozpocznij(&montaz(), t("09:10:00")).unwrap();
        let schowany = sr.plik().with_extension("offline");
        fs::rename(sr.plik(), &schowany).unwrap();
        r.przerwij(t("10:10:00"));
        assert_eq!(r.dz.oczekujace.len(), 1);
        assert!(
            sr.dziennik("2026-10-30").contains("60 min · w toku"),
            "dziennik jest w vaulcie i zapisuje się od razu"
        );
        fs::rename(&schowany, sr.plik()).unwrap();
        r.cykl(t("10:10:30"));
        assert!(r.dz.oczekujace.is_empty());
        assert!(sr.zadania().contains("[czas:: 105]"));
    }

    #[test]
    fn po_awarii_sesja_wraca_zapauzowana_bez_przestoju() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:10"));
        r.rozpocznij(&montaz(), t("09:00:00")).unwrap();
        r.zapisz_stan(t("09:00:00"));
        r.cykl(t("09:20:00"));
        r.cykl(t("09:20:30"));
        drop(r); // proces zabity bez sprzątania
        let mut r = sr.rdzen(t("10:30:00"));
        assert_eq!(r.stan(), Stan::Pauza);
        assert_eq!(dymki(&mut r), vec!["po_blokadzie"]);
        let s = r.dz.sesja.as_ref().unwrap();
        assert_eq!(s.sekundy(t("10:30:00")), 20 * 60 + 30);
        assert_eq!(s.pauza.as_ref().unwrap().powod, PowodPauzy::Awaria);
        assert_eq!(r.migawka().sesja.unwrap().sekundy_zamkniete, (45 * 60) + 20 * 60 + 30);
    }

    #[test]
    fn nowy_dzien_zamyka_zapauzowana_sesje_z_wczoraj_jako_w_toku() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("18:00:00"));
        r.tick(t("18:00:06"));
        r.zatwierdz_wybor(t("18:00:10"));
        r.rozpocznij(&montaz(), t("18:00:00")).unwrap();
        r.pauza(t("18:45:00"));
        r.zapisz_stan(t("18:45:00"));
        drop(r);
        let mut r = sr.rdzen(chwila("2026-10-31T09:00:00+01:00"));
        assert!(r.dz.sesja.is_none());
        assert_eq!(r.stan(), Stan::Powitanie);
        assert!(sr.dziennik("2026-10-30").contains("- [ ] 18:00–18:45 · Montaż: plansze 1–5 · 45 min · w toku"));
        assert!(sr.zadania().contains("[czas:: 90]"));
        assert!(dymki(&mut r).contains(&"powitanie".to_string()));
    }

    #[test]
    fn sesja_przez_polnoc_nalezy_do_dnia_startu() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("23:00:00"));
        r.tick(t("23:00:06"));
        r.zatwierdz_wybor(t("23:00:10"));
        r.rozpocznij(&montaz(), t("23:30:00")).unwrap();
        r.cykl(chwila("2026-10-31T00:20:00+01:00"));
        assert_eq!(r.stan(), Stan::Praca, "trwająca sesja liczy się dalej");
        r.zrobione(chwila("2026-10-31T00:30:00+01:00"));
        assert!(sr.dziennik("2026-10-30").contains("- [x] 23:30–00:30 · Montaż: plansze 1–5 · 60 min (szac. 120)"));
        assert!(sr.zadania().contains("✅ 2026-10-30"), "ptaszek ma datę dnia, w którym zaczęła się sesja");
        // Po sesji dzień się zmienia i otwiera się wybór nowego dnia.
        r.tick(chwila("2026-10-31T00:30:10+01:00"));
        assert_eq!(r.stan(), Stan::KoniecDnia, "dzień, którego nie zamknąłeś, zamyka się za Ciebie z podsumowaniem");
        assert!(sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
        r.dalej_po_wczoraju();
        assert_eq!(r.stan(), Stan::WyborZadan);
    }

    // ---- krok 4: podsumowanie po zadaniu --------------------------------------------------------

    #[test]
    fn po_zadaniu_dymek_pokazuje_czas_zadania_szacunek_i_czas_dnia() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&miniatura());
        r.zatwierdz_wybor(t("09:00:30"));
        // Miniatura: 40 min; Montaż ma już 45 min w pliku i dochodzi 30 min.
        r.rozpocznij(&miniatura(), t("10:00:00")).unwrap();
        r.wez_zdarzenia();
        r.zrobione(t("10:40:00"));
        let zd = r.wez_zdarzenia();
        let w = wstawki(&zd, "zrobione");
        assert_eq!(w["zadanie"], "Miniatura");
        assert_eq!(w["czas"], "40 min");
        assert_eq!(w["szac"], "30 min");
        assert_eq!(w["czas_dnia"], "40 min");
        assert!(zd.contains(&Zdarzenie::Efekt { nazwa: "konfetti".into() }));
        let g = r.migawka().gratulacje.unwrap();
        assert_eq!((g.czas_min, g.szac_min, g.czas_dnia_min, g.plan_wykonany), (40, Some(30), 40, false));

        // Drugie zadanie tego samego dnia: łączny czas zadania z pliku i czas całego dnia.
        r.tick(t("10:40:10"));
        assert_eq!(dymki(&mut r), vec!["zostalo"]);
        r.rozpocznij(&montaz(), t("11:00:00")).unwrap();
        r.zrobione(t("11:30:00"));
        let w = wstawki(&r.wez_zdarzenia(), "zrobione");
        assert_eq!(w["czas"], "1 h 15 min", "45 min z pliku plus 30 min tej sesji");
        assert_eq!(w["szac"], "2 h");
        assert_eq!(w["czas_dnia"], "1 h 10 min");
    }

    #[test]
    fn zadanie_bez_szacunku_pomija_szacunek_w_dymku() {
        let sr = Srodowisko::nowe();
        fs::write(
            sr.plik(),
            "## P
- [ ] Bez szacunku
- [ ] Drugie [szac:: 20]
",
        )
        .unwrap();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&k("P", "Bez szacunku"));
        r.przelacz_wybor(&k("P", "Drugie"));
        r.zatwierdz_wybor(t("09:00:30"));
        r.rozpocznij(&k("P", "Bez szacunku"), t("10:00:00")).unwrap();
        r.wez_zdarzenia();
        r.zrobione(t("10:10:00"));
        let zd = r.wez_zdarzenia();
        assert!(!wstawki(&zd, "zrobione").contains_key("szac"));
        assert_eq!(r.migawka().gratulacje.unwrap().szac_min, None);
    }

    #[test]
    fn zostalo_liczy_zadania_i_pozostaly_czas() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&miniatura());
        r.przelacz_wybor(&mail());
        r.zatwierdz_wybor(t("09:00:30"));
        r.rozpocznij(&mail(), t("10:00:00")).unwrap();
        r.zrobione(t("10:10:00"));
        r.tick(t("10:10:10"));
        let zd = r.wez_zdarzenia();
        let w = wstawki(&zd, "zostalo");
        // Zostają Montaż (120 − 45) i Miniatura (30): 2 zadania, 1 h 45 min.
        assert_eq!((w["zadan"].as_str(), w["pozostalo"].as_str()), ("2 zadania", "1 h 45 min"));
        assert_eq!(r.migawka().lista.unwrap().pozostalo_min, 105);
    }

    #[test]
    fn po_ostatnim_zadaniu_planu_postac_pyta_czy_konczymy() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&montaz()); // tylko Miniatura
        r.przelacz_wybor(&miniatura());
        r.zatwierdz_wybor(t("09:00:30"));
        r.rozpocznij(&miniatura(), t("10:00:00")).unwrap();
        r.zrobione(t("10:40:00"));
        assert!(r.migawka().gratulacje.unwrap().plan_wykonany);
        r.wez_zdarzenia();
        r.tick(t("10:40:10"));
        assert_eq!(r.stan(), Stan::Gratulacje, "po ostatnim zadaniu to Ty wybierasz, co dalej");
        assert_eq!(dymki(&mut r), vec!["plan_wykonany"]);
        r.tick(t("10:50:00"));
        assert_eq!(r.stan(), Stan::Gratulacje, "bez automatycznego powrotu do listy");
        assert!(r.najblizszy_termin(t("10:50:00")) > t("10:50:00"));
        // „Dodaj z listy”: ten sam wybór z planem jako punktem wyjścia.
        r.otworz_wybor();
        assert_eq!(r.stan(), Stan::WyborZadan);
        r.przelacz_wybor(&mail());
        r.zatwierdz_wybor(t("10:51:00"));
        let lista = r.migawka().lista.unwrap();
        assert_eq!((lista.zrobione, lista.razem), (1, 2));
    }

    #[test]
    fn zadanie_dopisane_w_ciagu_dnia_wydluza_plan() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:30"));
        r.rozpocznij(&montaz(), t("10:00:00")).unwrap();
        r.dodaj_zadanie("Pilne", Some(10), "Inbox").unwrap();
        r.zrobione(t("10:30:00"));
        assert!(!r.migawka().gratulacje.unwrap().plan_wykonany, "dopisane w trakcie pracy zadanie liczy się do planu");
    }

    // ---- krok 5: koniec dnia -------------------------------------------------------------------

    /// Sesja `od`–`do_` na zadaniu: Zrobione albo Przerwij.
    fn sesja(r: &mut Rdzen, klucz: &Klucz, od: &str, do_: &str, zrobione: bool) {
        r.rozpocznij(klucz, t(od)).unwrap();
        if zrobione {
            r.zrobione(t(do_));
        } else {
            r.przerwij(t(do_));
        }
    }

    /// Plan z jednym zadaniem i dzień z sesją o podanej długości.
    fn dzien_z(sr: &Srodowisko, minuty: u32, zrobione: bool) -> Rdzen {
        let mut r = sr.rdzen(t("08:00:00"));
        r.tick(t("08:00:06"));
        r.przelacz_wybor(&montaz()); // tylko Miniatura
        r.przelacz_wybor(&miniatura());
        r.zatwierdz_wybor(t("08:00:30"));
        if minuty > 0 {
            let koniec = chwila("2026-10-30T08:10:00+01:00") + Duration::minutes(i64::from(minuty));
            let koniec = koniec.format("%H:%M:%S").to_string();
            sesja(&mut r, &miniatura(), "08:10:00", &koniec, zrobione);
        }
        r
    }

    #[test]
    fn szesc_reakcji_na_koniec_dnia_z_tabeli_b4() {
        // (minuty, zrobione, reakcja, dymek, nazwa w dzienniku)
        let przypadki = [
            (250, true, Reakcja::Zachwycona, "koniec_zachwycona", "zachwycona"),
            (250, false, Reakcja::Zadowolona, "koniec_zadowolona", "zadowolona"),
            (120, true, Reakcja::Dumna, "koniec_dumna", "dumna"),
            (120, false, Reakcja::LekkoPochwalajaca, "koniec_pochwala", "lekko pochwalająca"),
            (30, true, Reakcja::Czula, "koniec_czula", "czuła"),
            (0, false, Reakcja::Zmartwiona, "koniec_zmartwiona", "zmartwiona"),
        ];
        for (minuty, zrobione, reakcja, dymek, nazwa) in przypadki {
            let sr = Srodowisko::nowe();
            let mut r = dzien_z(&sr, minuty, zrobione);
            r.wez_zdarzenia();
            r.koniec_pracy(chwila("2026-10-30T14:00:00+01:00"));
            let zd = r.wez_zdarzenia();
            assert_eq!(wstawki(&zd, dymek)["imie"], "Dżiordżio", "{nazwa}");
            let k = r.migawka().koniec.unwrap();
            assert_eq!((k.etap, k.reakcja), ("reakcja", reakcja), "{nazwa}");
            let dz = sr.dziennik("2026-10-30");
            assert!(dz.contains(&format!("### Podsumowanie dnia\nReakcja: {nazwa}\n")), "{nazwa}: {dz}");
            // Zachwycona ma konfetti 3 s, dumna 1,5 s.
            let konfetti_dlugie = zd.contains(&Zdarzenie::Efekt { nazwa: "konfetti_dlugie".into() });
            let konfetti = zd.contains(&Zdarzenie::Efekt { nazwa: "konfetti".into() });
            assert_eq!(
                (konfetti_dlugie, konfetti),
                (reakcja == Reakcja::Zachwycona, reakcja == Reakcja::Dumna),
                "{nazwa}"
            );
        }
    }

    #[test]
    fn podsumowanie_dnia_ma_zrobione_czas_zadan_i_liste_na_jutro() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("08:00:00"));
        r.tick(t("08:00:06"));
        r.przelacz_wybor(&miniatura());
        r.przelacz_wybor(&mail());
        r.zatwierdz_wybor(t("08:00:30"));
        sesja(&mut r, &miniatura(), "08:10:00", "08:50:00", true);
        sesja(&mut r, &montaz(), "09:00:00", "09:30:00", false);
        sesja(&mut r, &montaz(), "10:00:00", "10:15:00", false);
        r.koniec_pracy(t("12:00:00"));
        let p = r.migawka().koniec.unwrap().podsumowanie;
        assert_eq!((p.zrobione, p.zaplanowane, p.czas_dnia_min, p.cel_min), (1, 3, 85, 240));
        let zadania: Vec<(&str, u32, Option<u32>, bool)> =
            p.zadania.iter().map(|z| (z.zadanie.as_str(), z.czas_min, z.szac_min, z.zrobione)).collect();
        assert_eq!(
            zadania,
            [("Miniatura", 40, Some(30), true), ("Montaż: plansze 1–5", 45, Some(120), false)],
            "kolejne sesje tego samego zadania sumują się"
        );
        assert_eq!(p.na_jutro, ["Montaż: plansze 1–5", "Odpisać na maila od montażysty"]);
        let dz = sr.dziennik("2026-10-30");
        assert!(dz.contains("**Razem:** 1 h 25 min z celu 4 h · **Plan:** 1 z 3 zadań\n"));
        assert!(dz.contains("Na jutro: Montaż: plansze 1–5, Odpisać na maila od montażysty\n"));
    }

    #[test]
    fn koniec_pracy_przy_dzialajacym_liczniku_zapisuje_sesje_jak_przerwij() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("08:00:00"));
        r.tick(t("08:00:06"));
        r.zatwierdz_wybor(t("08:00:10"));
        r.rozpocznij(&montaz(), t("08:10:00")).unwrap();
        r.koniec_pracy(t("09:10:00"));
        assert!(r.dz.sesja.is_none());
        assert_eq!(r.stan(), Stan::KoniecDnia);
        assert!(sr.zadania().contains("[czas:: 105]"), "60 min dopisane do 45");
        assert!(!sr.zadania().contains("[x] Montaż"), "bez ptaszka");
        assert!(sr.dziennik("2026-10-30").contains("- [ ] 08:10–09:10 · Montaż: plansze 1–5 · 60 min · w toku"));
    }

    #[test]
    fn po_reakcji_postac_macha_i_chowa_sie_do_zasobnika() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 30, true);
        r.koniec_pracy(t("14:00:00"));
        r.wez_zdarzenia();
        r.tick(t("14:00:07"));
        assert_eq!(r.migawka().koniec.unwrap().etap, "reakcja");
        assert!(r.widoczna());
        r.tick(t("14:00:08"));
        assert_eq!(r.migawka().koniec.unwrap().etap, "pozegnanie");
        assert_eq!(dymki(&mut r), vec!["do_jutra"]);
        r.tick(t("14:00:11"));
        assert_eq!(r.migawka().koniec.unwrap().etap, "spi");
        assert!(!r.widoczna(), "okno chowa się do zasobnika");
        // Pokazane z zasobnika: „śpi” z podsumowaniem i przyciskiem „Wróć do pracy”.
        r.pokaz();
        assert!(r.widoczna());
        assert_eq!(r.migawka().koniec.unwrap().etap, "spi");
        r.do_jutra(t("14:30:00"));
        assert!(!r.widoczna());
    }

    #[test]
    fn do_jutra_skraca_reakcje() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 30, true);
        r.koniec_pracy(t("14:00:00"));
        r.wez_zdarzenia();
        r.do_jutra(t("14:00:02"));
        assert_eq!(r.migawka().koniec.unwrap().etap, "pozegnanie");
        assert_eq!(dymki(&mut r), vec!["do_jutra"]);
    }

    #[test]
    fn wroc_do_pracy_otwiera_dzien_i_usuwa_podsumowanie_z_dziennika() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 30, true);
        r.koniec_pracy(t("14:00:00"));
        assert!(sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
        r.wez_zdarzenia();
        r.wroc_do_pracy(t("14:01:00"));
        assert_eq!(r.stan(), Stan::Lista);
        assert!(!sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
        assert!(sr.dziennik("2026-10-30").contains("- [x] 08:10–08:40 · Miniatura"), "sesje zostają");
        assert!(!r.dz.zamkniety);
        // Dzień można zamknąć ponownie, a podsumowanie powstaje od nowa.
        r.koniec_pracy(t("15:00:00"));
        assert!(sr.dziennik("2026-10-30").contains("Reakcja: czuła"));
        assert!(r.rozpocznij(&montaz(), t("15:01:00")).is_err(), "w zamkniętym dniu nie ma pracy");
        assert!(r.dodaj_zadanie("X", None, "Inbox").is_err());
    }

    #[test]
    fn o_22_dzien_zamyka_sie_sam_nawet_przy_dzialajacym_liczniku() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("20:00:00"));
        r.tick(t("20:00:06"));
        r.zatwierdz_wybor(t("20:00:10"));
        r.rozpocznij(&montaz(), t("21:30:00")).unwrap();
        assert_eq!(
            r.najblizszy_termin(t("21:31:00")),
            t("22:00:00"),
            "zamknięcie dnia przed przypomnieniem o przerwie"
        );
        r.tick(t("21:59:59"));
        assert_eq!(r.stan(), Stan::Praca);
        r.tick(t("22:00:00"));
        assert_eq!(r.stan(), Stan::KoniecDnia);
        assert!(sr.dziennik("2026-10-30").contains("- [ ] 21:30–22:00 · Montaż: plansze 1–5 · 30 min · w toku"));
        assert!(sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
        // Po zamknięciu nie ma już terminu na 22:00.
        r.tick(t("22:00:20"));
        assert!(r.najblizszy_termin(t("22:00:20")) > t("22:00:20"));
    }

    #[test]
    fn dzien_bez_aktywnosci_nie_zamyka_sie_sam() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("20:00:00"));
        r.tick(t("20:00:06"));
        assert_eq!(r.stan(), Stan::WyborZadan);
        r.tick(t("22:05:00"));
        assert_eq!(r.stan(), Stan::WyborZadan);
        // Plan wybrany po godzinie zamknięcia to Twoja decyzja: dzień nie zamyka się sam.
        r.zatwierdz_wybor(t("22:10:00"));
        r.tick(t("22:11:00"));
        assert_eq!(r.stan(), Stan::Lista);
        r.rozpocznij(&montaz(), t("22:12:00")).unwrap();
        r.tick(t("23:00:00"));
        assert_eq!(r.stan(), Stan::Praca);
    }

    #[test]
    fn po_wroc_do_pracy_dzien_nie_zamyka_sie_ponownie_o_22() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 30, true);
        r.koniec_pracy(t("21:00:00"));
        r.wroc_do_pracy(t("21:30:00"));
        r.tick(t("22:00:00"));
        assert_eq!(r.stan(), Stan::Lista);
    }

    #[test]
    fn komputer_wlaczony_po_22_zamyka_dzien_przy_starcie() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 120, false);
        r.zapisz_stan(t("11:00:00"));
        drop(r);
        let mut r2 = sr.rdzen(t("23:10:00"));
        assert_eq!(r2.stan(), Stan::Lista, "start pokazuje to, co było");
        r2.tick(t("23:10:01"));
        assert_eq!(r2.stan(), Stan::KoniecDnia);
        assert_eq!(r2.migawka().koniec.unwrap().reakcja, Reakcja::LekkoPochwalajaca);
    }

    #[test]
    fn restart_w_zamknietym_dniu_pokazuje_podsumowanie_i_wroc_do_pracy() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 30, true);
        r.koniec_pracy(t("14:00:00"));
        drop(r);
        let r2 = sr.rdzen(t("15:00:00"));
        assert_eq!(r2.stan(), Stan::KoniecDnia);
        let k = r2.migawka().koniec.unwrap();
        assert_eq!((k.etap, k.reakcja, k.podsumowanie.czas_dnia_min), ("spi", Reakcja::Czula, 30));
    }

    #[test]
    fn niezamkniety_wczorajszy_dzien_zamyka_sie_za_ciebie_i_pokazuje_podsumowanie() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 120, false);
        r.zapisz_stan(t("11:00:00"));
        drop(r);
        let jutro = chwila("2026-10-31T08:00:00+01:00");
        let mut r2 = sr.rdzen(jutro);
        assert_eq!(r2.stan(), Stan::Powitanie);
        assert!(sr.dziennik("2026-10-30").contains("Reakcja: lekko pochwalająca"), "wczorajszy dziennik zamknięty");
        r2.wez_zdarzenia();
        r2.tick(jutro + Duration::seconds(10));
        let k = r2.migawka().koniec.unwrap();
        assert_eq!((k.etap, k.reakcja, k.podsumowanie.czas_dnia_min), ("wczoraj", Reakcja::LekkoPochwalajaca, 120));
        assert_eq!(dymki(&mut r2), vec!["zamkniety_wczoraj"]);
        // „Dalej”: wybór zadań na dziś, z niedokończonymi zadaniami z wczoraj.
        r2.dalej_po_wczoraju();
        assert_eq!(r2.stan(), Stan::WyborZadan);
        assert!(r2.migawka().wybor.unwrap().zaznaczonych >= 1);
    }

    #[test]
    fn dzien_zamkniety_przez_ciebie_nie_pokazuje_wczoraj_ponownie() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 30, true);
        r.koniec_pracy(t("14:00:00"));
        drop(r);
        let jutro = chwila("2026-10-31T08:00:00+01:00");
        let mut r2 = sr.rdzen(jutro);
        r2.tick(jutro + Duration::seconds(10));
        assert_eq!(r2.stan(), Stan::WyborZadan);
    }

    #[test]
    fn polnoc_w_zamknietym_dniu_otwiera_wybor_nowego_dnia() {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, 30, true);
        r.koniec_pracy(t("23:00:00"));
        r.tick(t("23:00:20"));
        r.tick(chwila("2026-10-31T00:00:05+01:00"));
        assert_eq!(r.stan(), Stan::WyborZadan);
    }

    #[test]
    fn nie_zamyka_dnia_w_kreatorze() {
        let sr = Srodowisko::nowe();
        let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("23:00:00"));
        r.wymus_kreator();
        r.start(t("23:00:00"));
        r.koniec_pracy(t("23:00:10"));
        r.tick(t("23:30:00"));
        assert_eq!(r.stan(), Stan::Konfiguracja);
    }

    // ---- krok 6: menu i zamykanie ---------------------------------------------------------------

    #[test]
    fn menu_zalezy_od_stanu() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        let wybor = r.mozliwosci();
        assert_eq!((wybor.koniec_pracy, wybor.zmien_plan, wybor.dodaj_zadanie, wybor.pauza), (true, false, true, None));
        r.zatwierdz_wybor(t("09:00:10"));
        let lista = r.mozliwosci();
        assert_eq!((lista.koniec_pracy, lista.zmien_plan, lista.dodaj_zadanie), (true, true, true));
        r.rozpocznij(&montaz(), t("09:01:00")).unwrap();
        let praca = r.mozliwosci();
        assert_eq!(
            (praca.koniec_pracy, praca.zmien_plan, praca.dodaj_zadanie, praca.pauza),
            (true, false, true, Some(false))
        );
        r.pauza(t("09:02:00"));
        assert_eq!(r.mozliwosci().pauza, Some(true));
        r.koniec_pracy(t("09:03:00"));
        let koniec = r.mozliwosci();
        assert_eq!((koniec.koniec_pracy, koniec.zmien_plan, koniec.dodaj_zadanie), (false, false, false));
        assert!(koniec.widoczna);
    }

    #[test]
    fn kreator_nie_ma_menu_dnia() {
        let sr = Srodowisko::nowe();
        let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
        r.wymus_kreator();
        r.start(t("09:00:00"));
        let m = r.mozliwosci();
        assert_eq!((m.koniec_pracy, m.zmien_plan, m.dodaj_zadanie), (false, false, false));
    }

    #[test]
    fn zamknij_asystentke_zapisuje_sesje_i_zostawia_dzien_otwarty() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:10"));
        r.rozpocznij(&montaz(), t("09:10:00")).unwrap();
        r.zamknij_asystentke(t("09:40:00"));
        assert!(r.dz.sesja.is_none());
        assert!(sr.zadania().contains("[czas:: 75]"), "30 min dopisane do 45");
        assert!(!sr.zadania().contains("[x] Montaż"), "bez ptaszka");
        assert!(sr.dziennik("2026-10-30").contains("- [ ] 09:10–09:40 · Montaż: plansze 1–5 · 30 min · w toku"));
        assert!(!sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
        drop(r);
        // Kolejne uruchomienie tego samego dnia wraca do Listy z tym samym planem.
        let mut r2 = sr.rdzen(t("10:00:00"));
        assert_eq!(r2.stan(), Stan::Lista);
        assert_eq!(dymki(&mut r2), vec!["powrot"]);
        assert!(r2.dz.sesja.is_none());
        assert_eq!(r2.migawka().lista.unwrap().razem, 1);
    }

    #[test]
    fn zamknij_asystentke_bez_sesji_tylko_zapisuje_stan() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:10"));
        r.zamknij_asystentke(t("09:05:00"));
        assert!(sr.lokalny.join("dzien.json").exists());
        assert!(!sr.vault.join("Dziennik pracy").exists());
    }

    // ---- bezczynność, pauza automatyczna, pełny ekran (z wersji 1) -------------------------------

    fn w_pracy(sr: &Srodowisko) -> Rdzen {
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:10"));
        r.rozpocznij(&montaz(), t("09:00:00")).unwrap();
        r.wez_zdarzenia();
        r
    }

    #[test]
    fn bezczynnosc_pyta_i_odejmuje_czas() {
        let sr = Srodowisko::nowe();
        let mut r = w_pracy(&sr);
        r.bezczynnosc(14 * 60, t("09:30:00"));
        assert_eq!(r.stan(), Stan::Praca);
        r.bezczynnosc(16 * 60, t("09:46:00"));
        assert_eq!(r.stan(), Stan::Jestes);
        assert_eq!(dymki(&mut r), vec!["jestes"]);
        r.odpowiedz_jestes(true, t("09:50:00"));
        assert_eq!(r.stan(), Stan::Praca);
        // 09:00–09:30 praca, 09:30–09:50 przerwa, potem 10 min pracy.
        assert_eq!(r.dz.sesja.as_ref().unwrap().sekundy(t("10:00:00")), 40 * 60);
    }

    #[test]
    fn pracowalem_zostawia_czas_i_brak_pytania_przy_pelnym_ekranie() {
        let sr = Srodowisko::nowe();
        let mut r = w_pracy(&sr);
        r.ustaw_pelny_ekran(true);
        assert!(!r.widoczna());
        r.bezczynnosc(20 * 60, t("09:30:00"));
        assert_eq!(r.stan(), Stan::Praca, "schowany przez pełny ekran nie pyta");
        r.ustaw_pelny_ekran(false);
        r.bezczynnosc(20 * 60, t("09:31:00"));
        r.odpowiedz_jestes(false, t("09:32:00"));
        assert_eq!(r.dz.sesja.as_ref().unwrap().sekundy(t("09:40:00")), 40 * 60);
    }

    #[test]
    fn blokada_i_uspienie_pauzuja() {
        let sr = Srodowisko::nowe();
        let mut r = w_pracy(&sr);
        r.pauza_automatyczna(PowodPauzy::Blokada, t("09:20:00"));
        assert_eq!(r.stan(), Stan::Pauza);
        r.powrot(t("09:45:00"));
        assert_eq!(r.stan(), Stan::Pauza, "po powrocie pauza zostaje");
        assert_eq!(dymki(&mut r), vec!["po_blokadzie"]);
        r.powrot(t("09:45:02"));
        assert!(dymki(&mut r).is_empty());
        r.wznow(t("09:46:00"));
        r.pauza_automatyczna(PowodPauzy::Uspienie, t("10:00:00"));
        assert_eq!(r.dz.sesja.as_ref().unwrap().sekundy(t("12:00:00")), 34 * 60);
    }

    #[test]
    fn przypomnienie_o_przerwie_po_50_min_ciaglej_pracy() {
        let sr = Srodowisko::nowe();
        let mut r = w_pracy(&sr);
        assert_eq!(r.najblizszy_termin(t("09:10:00")), t("09:50:00"));
        r.tick(t("09:49:59"));
        assert!(dymki(&mut r).is_empty());
        r.tick(t("09:50:00"));
        let zd = r.wez_zdarzenia();
        assert_eq!(wstawki(&zd, "przypomnienie_przerwy")["minuty"], "50 min");
        assert!(zd.contains(&Zdarzenie::Efekt { nazwa: "przeciaganie".into() }));
        r.tick(t("09:55:00"));
        assert!(r.wez_zdarzenia().is_empty());
    }

    #[test]
    fn ukrycie_reczne_i_przez_pelny_ekran() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        assert!(r.widoczna());
        r.ustaw_pelny_ekran(true);
        assert!(!r.widoczna());
        r.przelacz_widocznosc();
        assert!(r.widoczna(), "skrót pokazuje widget mimo pełnego ekranu");
        r.ustaw_pelny_ekran(false);
        r.przelacz_widocznosc();
        assert!(!r.widoczna());
        r.ustaw_pelny_ekran(true);
        r.ustaw_pelny_ekran(false);
        assert!(!r.widoczna(), "ręcznie schowany zostaje schowany po pełnym ekranie");
        r.pokaz();
        assert!(r.widoczna());
    }

    #[test]
    fn dzien_z_wersji_1_zachowuje_linie_w_dzienniku() {
        let sr = Srodowisko::nowe();
        fs::create_dir_all(sr.lokalny.join("dni")).unwrap();
        fs::write(
            sr.lokalny.join("dni").join("2026-10-30.json"),
            r##"{"data": "2026-10-30", "sesje": [{"id": "12", "nazwa": "Scenariusz: intro", "szac_min": 60,
              "start": "2026-10-30T09:12:00+01:00", "koniec": "2026-10-30T10:05:00+01:00",
              "sekundy": 2820, "minuty": 47, "status": "Zrobione"}]}"##,
        )
        .unwrap();
        let mut r = sr.rdzen(t("11:00:00"));
        assert_eq!(r.stan(), Stan::WyborZadan, "dzień z wersji 1 miał już powitanie");
        r.tick(t("11:00:06"));
        r.zatwierdz_wybor(t("11:00:10"));
        r.rozpocznij(&montaz(), t("11:10:00")).unwrap();
        r.przerwij(t("11:40:00"));
        let dziennik = sr.dziennik("2026-10-30");
        assert!(dziennik.contains("- [x] 09:12–10:05 · Scenariusz: intro · 47 min (szac. 60)\n"));
        assert!(dziennik.contains("- [ ] 11:10–11:40 · Montaż: plansze 1–5 · 30 min · w toku · Film #1 · FIFA\n"));
        assert!(dziennik.contains("minuty: 77\n"));
    }

    #[test]
    fn polozenie_widgetu_trafia_do_okno_json_i_do_migawki() {
        use crate::okno::{MonitorOkna, Naroznik, Pion, Strona};
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("08:00:00"));
        let m = r.migawka();
        assert_eq!((m.strona, m.pion), (Strona::Prawa, Pion::Dol), "domyślnie prawy dolny róg");

        let drugi = MonitorOkna { nazwa: "\\\\.\\DISPLAY2".into(), x: 1920, y: 0, szer: 2560, wys: 1440 };
        let kotwica =
            KotwicaOkna { monitor: Some(drugi), kotwica: Naroznik::LewyGorny, odstep_x: 40.0, odstep_y: 20.0 };
        r.ustaw_kotwice(kotwica.clone(), t("08:01:00"));
        let m = r.migawka();
        assert_eq!(
            (m.strona, m.pion),
            (Strona::Lewa, Pion::Gora),
            "lewa połowa monitora: postać odbita, panel rośnie w dół"
        );
        assert_eq!(
            sr.nowy_rdzen(t("08:02:00")).kotwica(),
            kotwica,
            "po restarcie widget stoi tam, gdzie go zostawiono"
        );
    }

    #[test]
    fn zastepczy_rog_nie_nadpisuje_zapisanej_pozycji() {
        use crate::okno::{MonitorOkna, Naroznik};
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("08:00:00"));
        let drugi = MonitorOkna { nazwa: "DP-2".into(), x: 1600, y: 0, szer: 1600, wys: 1000 };
        let kotwica =
            KotwicaOkna { monitor: Some(drugi), kotwica: Naroznik::PrawyGorny, odstep_x: 10.0, odstep_y: 10.0 };
        r.ustaw_kotwice(kotwica.clone(), t("08:01:00"));
        // monitor zniknął: okno stoi w domyślnym rogu, ale zapis zostaje
        r.ustaw_naroznik(Naroznik::PrawyDolny);
        assert_eq!(r.migawka().pion, Pion::Dol);
        assert_eq!(sr.nowy_rdzen(t("08:02:00")).kotwica(), kotwica);
    }

    #[test]
    fn przywroc_pozycje_wraca_na_monitor_glowny() {
        use crate::okno::{MonitorOkna, Naroznik};
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("08:00:00"));
        let drugi = MonitorOkna { nazwa: "DP-2".into(), x: 1600, y: 0, szer: 1600, wys: 1000 };
        r.ustaw_kotwice(
            KotwicaOkna { monitor: Some(drugi), kotwica: Naroznik::LewyGorny, odstep_x: 300.0, odstep_y: 200.0 },
            t("08:01:00"),
        );
        r.przywroc_pozycje(t("08:02:00"));
        assert_eq!(r.kotwica(), KotwicaOkna::default());
        assert_eq!(r.migawka().strona, Strona::Prawa);
        assert_eq!(sr.nowy_rdzen(t("08:03:00")).kotwica(), KotwicaOkna::default());
    }

    // ---- Zrobione od razu w pliku i Cofnij (2.1) -----------------------------------------------

    fn zaczynamy_miniature(sr: &Srodowisko) -> Rdzen {
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.przelacz_wybor(&miniatura());
        r.zatwierdz_wybor(t("09:00:30"));
        r.rozpocznij(&miniatura(), t("11:30:00")).unwrap();
        r.wez_zdarzenia();
        r
    }

    const MINIATURA_ZROBIONA: &str = "- [x] Miniatura [szac:: 30] [czas:: 40] ✅ 2026-10-30";

    #[test]
    fn zrobione_zapisuje_ptaszka_w_pliku_od_razu_bez_czekania_na_dymek_i_koniec_dnia() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        r.zrobione(t("12:10:00"));
        // Bez ticka, bez cyklu, bez zamknięcia dymku: plik jest już gotowy.
        assert!(sr.zadania().contains(MINIATURA_ZROBIONA), "{}", sr.zadania());
        assert!(r.dz.oczekujace.is_empty());
        assert!(sr.dziennik("2026-10-30").contains("- [x] 11:30–12:10 · Miniatura"));
    }

    #[test]
    fn zajety_plik_jest_ponawiany_po_sekundzie_a_nie_po_30_sekundach() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        let schowany = sr.plik().with_extension("zajety");
        fs::rename(sr.plik(), &schowany).unwrap();
        r.zrobione(t("12:10:00"));
        assert_eq!(r.dz.oczekujace.len(), 1, "zapis czeka, bo plik jest niedostępny");
        assert_eq!(r.najblizszy_termin(t("12:10:00")), t("12:10:01"), "ponowienie za sekundę");
        fs::rename(&schowany, sr.plik()).unwrap();
        r.tick(t("12:10:01"));
        assert!(r.dz.oczekujace.is_empty(), "po sekundzie, bez cyklu co 30 s");
        assert!(sr.zadania().contains(MINIATURA_ZROBIONA), "{}", sr.zadania());
    }

    #[test]
    fn ponowienia_zapisu_zwalniaja_ale_nie_przekraczaja_10_sekund() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        fs::rename(sr.plik(), sr.plik().with_extension("zajety")).unwrap();
        r.zrobione(t("12:10:00"));
        let mut teraz = t("12:10:00");
        let mut odstepy = Vec::new();
        for _ in 0..8 {
            let nastepny = r.zapis_ponow.expect("zapis czeka na ponowienie");
            odstepy.push((nastepny - teraz).num_seconds());
            teraz = nastepny;
            r.tick(teraz);
        }
        assert_eq!(odstepy, [1, 1, 2, 3, 5, 10, 10, 10]);
    }

    #[test]
    fn cofnij_zdejmuje_ptaszka_i_wraca_do_pracy_nad_tym_samym_zadaniem() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        r.zrobione(t("12:10:00"));
        assert_eq!(r.stan(), Stan::Gratulacje);
        assert!(r.migawka().gratulacje.unwrap().mozna_cofnac);
        r.wez_zdarzenia();

        r.cofnij(t("12:10:07"));
        assert_eq!(
            sr.zadania(),
            ZADANIA.replace("- [ ] Miniatura [szac:: 30]", "- [ ] Miniatura [szac:: 30] [czas:: 40]"),
            "ptaszek i data zdjęte, zapisany czas zostaje"
        );
        let dziennik = sr.dziennik("2026-10-30");
        assert!(dziennik.contains("- [ ] 11:30–12:10 · Miniatura · 40 min · w toku · Film #1 · FIFA\n"), "{dziennik}");
        assert!(dziennik.contains("zrobione: 0\n"), "{dziennik}");
        assert_eq!(r.stan(), Stan::Praca);
        let s = r.migawka().sesja.unwrap();
        assert_eq!(s.zadanie, "Miniatura");
        assert_eq!(s.sekundy_zamkniete, 40 * 60, "bez utraty czasu: licznik zaczyna od zapisanych 40 min");
        assert_eq!(wstawki(&r.wez_zdarzenia(), "cofniete")["zadanie"], "Miniatura");
        assert!(r.migawka().gratulacje.is_none());
    }

    #[test]
    fn po_cofnieciu_kolejne_zrobione_znowu_zapisuje_ptaszka_i_czas() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        r.zrobione(t("12:10:00"));
        r.cofnij(t("12:10:05"));
        r.zrobione(t("12:25:05"));
        assert!(sr.zadania().contains("- [x] Miniatura [szac:: 30] [czas:: 55] ✅ 2026-10-30"), "{}", sr.zadania());
        let dziennik = sr.dziennik("2026-10-30");
        assert!(dziennik.contains("- [ ] 11:30–12:10 · Miniatura · 40 min · w toku"), "{dziennik}");
        assert!(dziennik.contains("- [x] 12:10–12:25 · Miniatura · 15 min (szac. 30)"), "{dziennik}");
    }

    #[test]
    fn cofnij_przed_zapisem_ptaszka_w_zajetym_pliku_nie_dopisuje_go_wcale() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        let schowany = sr.plik().with_extension("zajety");
        fs::rename(sr.plik(), &schowany).unwrap();
        r.zrobione(t("12:10:00"));
        r.cofnij(t("12:10:03"));
        assert_eq!(r.stan(), Stan::Praca, "wracamy do pracy mimo zajętego pliku");
        fs::rename(&schowany, sr.plik()).unwrap();
        r.tick(t("12:10:10"));
        assert_eq!(
            sr.zadania(),
            ZADANIA.replace("- [ ] Miniatura [szac:: 30]", "- [ ] Miniatura [szac:: 30] [czas:: 40]"),
            "czas zapisany, ptaszka nie ma"
        );
        assert!(r.dz.oczekujace.is_empty());
    }

    #[test]
    fn cofnij_po_10_sekundach_albo_poza_gratulacjami_nic_nie_robi() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        r.cofnij(t("11:31:00"));
        assert_eq!(r.stan(), Stan::Praca, "w Pracy nie ma czego cofać");
        r.zrobione(t("12:10:00"));
        r.tick(t("12:10:10"));
        assert_eq!(r.stan(), Stan::Lista);
        r.cofnij(t("12:10:11"));
        assert_eq!(r.stan(), Stan::Lista);
        assert!(sr.zadania().contains(MINIATURA_ZROBIONA));
    }

    #[test]
    fn gratulacje_trwaja_10_sekund_a_klikniecie_w_dymek_idzie_do_listy_od_razu() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        r.zrobione(t("12:10:00"));
        r.tick(t("12:10:09"));
        assert_eq!(r.stan(), Stan::Gratulacje, "przez 9 s dymek z przyciskiem Cofnij jeszcze trwa");
        r.dalej_po_gratulacjach();
        assert_eq!(r.stan(), Stan::Lista);
        assert!(dymki(&mut r).contains(&"zostalo".to_string()));
    }

    #[test]
    fn po_ostatnim_zadaniu_cofnij_dziala_przez_10_sekund_potem_zostaja_przyciski() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.zatwierdz_wybor(t("09:00:10"));
        for (k, od, do_) in [(montaz(), "09:10:00", "09:40:00"), (miniatura(), "09:41:00", "10:11:00")] {
            r.rozpocznij(&k, t(od)).unwrap();
            r.zrobione(t(do_));
            r.tick(t(&format!("{}0", &do_[..7])));
        }
        let g = r.migawka().gratulacje.unwrap();
        assert!(g.plan_wykonany);
        r.tick(t("10:11:10"));
        assert!(!r.migawka().gratulacje.unwrap().mozna_cofnac, "po 10 s Cofnij znika, zostają przyciski końca");
        r.cofnij(t("10:11:11"));
        assert_eq!(r.stan(), Stan::Gratulacje);
    }

    // ---- zajęty skrót (2.1) -------------------------------------------------------------------

    #[test]
    fn zajety_skrot_daje_jeden_dymek_po_powitaniu_i_nastepnym_dymku() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        assert_eq!(r.stan(), Stan::Powitanie);
        r.skrot_zajety("Ctrl+Alt+Shift+Space", t("09:00:13"));
        r.wez_zdarzenia();
        r.tick(t("09:00:06"));
        r.tick(t("09:00:12"));
        assert!(
            !dymki(&mut r).contains(&"skrot_zajety".to_string()),
            "nie zagłusza powitania ani „Co bierzemy na dziś?”"
        );
        assert_eq!(r.najblizszy_termin(t("09:00:12")), t("09:00:13"));
        r.tick(t("09:00:13"));
        let zd = r.wez_zdarzenia();
        assert_eq!(wstawki(&zd, "skrot_zajety")["skrot"], "Ctrl+Alt+Shift+Spacja");
        r.tick(t("09:00:20"));
        assert!(dymki(&mut r).is_empty(), "dymek tylko raz");
        assert!(r.najblizszy_termin(t("09:00:20")) > t("09:00:30"), "nic już nie budzi wątku");
    }

    #[test]
    fn zajety_skrot_czeka_na_koniec_powitania_i_kreatora() {
        let sr = Srodowisko::nowe();
        let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
        r.wymus_kreator();
        r.start(t("09:00:00"));
        r.skrot_zajety("Ctrl+Alt+Shift+Space", t("09:00:13"));
        r.wez_zdarzenia();
        r.tick(t("09:05:00"));
        assert!(dymki(&mut r).is_empty(), "w kreatorze dymek o skrócie nie przeszkadza");
        r.zakoncz_kreator(sr.ustawienia(), t("09:06:00"));
        assert_eq!(dymki(&mut r), vec!["konfiguracja_gotowe"]);
        r.tick(t("09:06:05"));
        assert!(dymki(&mut r).is_empty(), "najpierw „Gotowe!”");
        r.tick(t("09:06:08"));
        assert_eq!(dymki(&mut r), vec!["skrot_zajety"]);
    }

    #[test]
    fn zmiana_skrotu_w_ustawieniach_mowi_o_konflikcie_od_razu() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:06"));
        r.tick(t("09:00:12"));
        r.wez_zdarzenia();
        r.skrot_zajety("Ctrl+Shift+F9", t("11:00:00"));
        r.tick(t("11:00:00"));
        assert_eq!(wstawki(&r.wez_zdarzenia(), "skrot_zajety")["skrot"], "Ctrl+Shift+F9");
    }

    // ---- sekwencja przerwy (2.1) ------------------------------------------------------------

    #[test]
    fn piec_sekund_po_pauzie_postac_mowi_ze_gra_ale_tylko_raz() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        r.pauza(t("11:40:00"));
        assert_eq!(dymki(&mut r), vec!["pauza"]);
        assert_eq!(r.najblizszy_termin(t("11:40:00")), t("11:40:05"));
        r.tick(t("11:40:04"));
        assert!(dymki(&mut r).is_empty(), "przez pierwsze 5 s postać się przeciąga");
        r.tick(t("11:40:05"));
        assert_eq!(dymki(&mut r), vec!["pauza_gra"]);
        r.tick(t("11:40:30"));
        assert!(dymki(&mut r).is_empty(), "dymek tylko raz");
        assert!(r.najblizszy_termin(t("11:40:30")) > t("11:41:30"), "nic już nie budzi wątku");
    }

    #[test]
    fn kolejna_przerwa_znowu_dostaje_dymek_o_graniu() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        r.pauza(t("11:40:00"));
        r.tick(t("11:40:05"));
        r.wez_zdarzenia();
        r.wznow(t("11:45:00"));
        r.pauza(t("11:50:00"));
        r.wez_zdarzenia();
        r.tick(t("11:50:05"));
        assert_eq!(dymki(&mut r), vec!["pauza_gra"]);
    }

    #[test]
    fn przerwa_z_blokady_ekranu_i_spozniony_tick_nie_daja_dymku_o_graniu() {
        let sr = Srodowisko::nowe();
        let mut r = zaczynamy_miniature(&sr);
        r.pauza_automatyczna(PowodPauzy::Blokada, t("11:40:00"));
        r.wez_zdarzenia();
        r.tick(t("11:40:05"));
        r.tick(t("11:55:00"));
        assert!(dymki(&mut r).is_empty(), "blokada ekranu: nikt nie patrzy");
        // ręczna przerwa, a wątek obudził się dopiero po minucie (np. po uśpieniu)
        r.wznow(t("12:00:00"));
        r.pauza(t("12:10:00"));
        r.wez_zdarzenia();
        r.tick(t("12:11:00"));
        assert!(dymki(&mut r).is_empty(), "spóźniony dymek byłby bez sensu");
        assert!(r.najblizszy_termin(t("12:11:00")) > t("12:12:00"));
    }

    // ---- imię postaci (2.1) -----------------------------------------------------------------

    #[test]
    fn kazdy_dymek_niesie_imie_postaci_do_wstawki_postac() {
        let sr = Srodowisko::nowe();
        let ust = Ustawienia { nazwa_postaci: "Bajtka".into(), ..sr.ustawienia() };
        let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), ust, t("09:00:00"));
        r.start(t("09:00:00"));
        let zd = r.wez_zdarzenia();
        assert_eq!(wstawki(&zd, "powitanie")["postac"], "Bajtka");
        assert_eq!(wstawki(&zd, "powitanie")["imie"], "Dżiordżio", "forma zwracania się to osobne pole");
    }

    #[test]
    fn kreator_przedstawia_sie_imieniem_postaci_a_zmiana_w_ustawieniach_dziala_od_razu() {
        let sr = Srodowisko::nowe();
        let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
        r.wymus_kreator();
        r.start(t("09:00:00"));
        assert_eq!(wstawki(&r.wez_zdarzenia(), "konfiguracja_imie")["postac"], "Sai");
        r.zakoncz_kreator(sr.ustawienia(), t("09:01:00"));
        r.wez_zdarzenia();
        r.zmien_ustawienia(Ustawienia { nazwa_postaci: "才ちゃん".into(), ..sr.ustawienia() }, t("09:02:00"));
        r.skrot_zajety("Ctrl+Shift+F9", t("09:02:00"));
        r.tick(t("09:02:10"));
        assert_eq!(wstawki(&r.wez_zdarzenia(), "skrot_zajety")["postac"], "才ちゃん");
    }
}
