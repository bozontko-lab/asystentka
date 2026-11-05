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

/// Dźwięk skupienia dla etykiety „♪ 40 Hz” w panelu Pracy (B7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DzwiekWidok {
    /// Przełącznik z menu i z ustawień.
    pub wlaczony: bool,
    /// Gra naprawdę: włączony, nie wyciszony przez OBS i jest urządzenie audio.
    pub gra: bool,
    /// OBS wycisza dźwięk (ustawienie „Cisza przy nagrywaniu” i działający OBS).
    pub wyciszony_przez_obs: bool,
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
    pub dzwiek: DzwiekWidok,
    /// Wysokość obszaru roboczego monitora widgetu w pikselach logicznych; `None`, zanim
    /// aplikacja ustawi okno.
    pub monitor_wys: Option<u32>,
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
    monitor_wys: Option<u32>,
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
    /// Działa OBS (proces `obs64.exe`): przy `cisza_przy_nagrywaniu` dźwięk milknie (B7).
    obs_dziala: bool,
    /// Pełny ekran na którymkolwiek monitorze: blokuje pytanie o bezczynność (B5).
    pelny_ekran_gdziekolwiek: bool,
    /// Silnik dźwięku nie otworzył urządzenia (brak słuchawek, głośników): dźwięk nie gra.
    dzwiek_niedostepny: bool,
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
            monitor_wys: None,
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
            obs_dziala: false,
            pelny_ekran_gdziekolwiek: false,
            dzwiek_niedostepny: false,
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

    pub fn monitor_wys(&self) -> Option<u32> {
        self.monitor_wys
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
        // Odroczona zmiana dnia nie jest terminem: czeka na koniec sesji albo gratulacji.
        let polnoc = lokalna(polnoc, teraz).filter(|p| !(self.zmiana_dnia_odroczona() && *p <= teraz));
        let mut termin = polnoc.unwrap_or(teraz + Duration::hours(1));
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

    // ---- dźwięk skupienia (B7) ------------------------------------------------------------

    /// OBS wycisza dźwięk tylko przy włączonym „Cisza przy nagrywaniu”.
    pub fn dzwiek_wyciszony(&self) -> bool {
        self.ust.cisza_przy_nagrywaniu && self.obs_dziala
    }

    /// Cel dla silnika dźwięku: głośność od 0 do 1, gdy ma grać, a inaczej `None`.
    /// Gra tylko wtedy, gdy licznik liczy czas zadania, czyli w stanie Praca: przy
    /// Pauzie, Przerwij, Zrobione, końcu dnia, blokadzie, uśpieniu i w „Jesteś?” cichnie.
    /// Suwak na 0 to cisza, więc sesja nie dostaje znacznika „· 40 Hz”.
    pub fn dzwiek_cel(&self) -> Option<f32> {
        let ma_grac = self.ust.dzwiek_skupienia && self.stan() == Stan::Praca && !self.dzwiek_wyciszony();
        ma_grac.then(|| self.ust.glosnosc_dzwieku()).filter(|g| *g > 0.0)
    }

    /// Czy dźwięk słychać: ma grać i silnik ma urządzenie.
    pub fn dzwiek_slychac(&self) -> bool {
        self.dzwiek_cel().is_some() && !self.dzwiek_niedostepny
    }

    /// Aplikacja mówi rdzeniowi, że OBS się włączył albo zamknął.
    pub fn ustaw_obs(&mut self, dziala: bool) {
        self.obs_dziala = dziala;
    }

    /// Silnik dźwięku zgłasza, czy ma urządzenie audio.
    pub fn ustaw_dzwiek_niedostepny(&mut self, niedostepny: bool) {
        self.dzwiek_niedostepny = niedostepny;
    }

    /// Liczy czas, w którym dźwięk gra w bieżącej sesji (znacznik „· 40 Hz” w dzienniku).
    /// Aplikacja woła to po każdej zmianie stanu; sesja zamykana w trakcie zmiany sama
    /// kończy swój odcinek dźwięku.
    pub fn synchronizuj_dzwiek(&mut self, teraz: Chwila) {
        let gra = self.dzwiek_slychac();
        if let Some(s) = self.dz.sesja.as_mut() {
            let przed = (s.dzwiek_od, s.dzwiek_s);
            s.dzwiek_ustaw(gra, teraz);
            if (s.dzwiek_od, s.dzwiek_s) != przed {
                self.brudny = true;
            }
        }
    }

    /// Przełącznik „Dźwięk 40 Hz” w menu i etykieta w panelu Pracy.
    pub fn ustaw_dzwiek_skupienia(&mut self, wlaczony: bool, teraz: Chwila) {
        self.ust.dzwiek_skupienia = wlaczony;
        self.synchronizuj_dzwiek(teraz);
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
        // Zmiana dnia odroczona przez gratulacje przychodzi zaraz po nich.
        self.zmiana_dnia(teraz);
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

    /// Trwająca sesja liczy się dalej i należy do dnia, w którym się zaczęła (B5), a po
    /// „Zrobione” gratulacje z przyciskiem Cofnij trwają swoje 10 s także po północy.
    fn zmiana_dnia_odroczona(&self) -> bool {
        self.dz.sesja.is_some() || matches!(self.faza, Faza::Gratulacje { do_: Some(_), .. })
    }

    fn zmiana_dnia(&mut self, teraz: Chwila) {
        let dzis = czas::dzien(teraz);
        if dzis == self.dz.data || self.zmiana_dnia_odroczona() {
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
        let data = czas::dzien(wpis.start);
        // Ptaszek, który jeszcze czeka w kolejce (plik był zajęty), po prostu nie trafi do pliku.
        match self.dz.oczekujace.iter().rposition(|e| e.klucz == klucz && !e.odznacz && e.zrobione.is_some()) {
            Some(i) => self.dz.oczekujace[i].zrobione = None,
            None => {
                self.dz.oczekujace.push(Edycja { klucz: klucz.clone(), minuty: 0, zrobione: Some(data), odznacz: true })
            }
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
        let pelny_ekran = self.schowana_przez_pelny_ekran() || self.pelny_ekran_gdziekolwiek;
        if prog == 0 || sekundy < prog || self.stan() != Stan::Praca || pelny_ekran {
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

    /// Pełny ekran na dowolnym monitorze, także na innym niż monitor widgetu.
    pub fn ustaw_pelny_ekran_gdziekolwiek(&mut self, pelny: bool) {
        self.pelny_ekran_gdziekolwiek = pelny;
    }

    /// Zamknięcie okna widgetu (np. Alt+F4): zawsze chowa, nigdy nie pokazuje.
    pub fn ukryj_recznie(&mut self) {
        self.ukrycie.recznie = true;
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
    /// Okno stanęło na monitorze o tej wysokości obszaru roboczego (piksele logiczne).
    pub fn ustaw_monitor_wys(&mut self, wysokosc: Option<u32>) {
        self.monitor_wys = wysokosc;
    }

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
                    plik_zadan::odznacz_zrobione(t, &e.klucz, e.zrobione)?
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
                        if !e.odznacz && e.zrobione.is_some() { " · ✅" } else { "" },
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
            dzwiek: s.dzwiek_wiekszosc(teraz),
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
            monitor_wys: self.monitor_wys,
            dzwiek: DzwiekWidok {
                wlaczony: self.ust.dzwiek_skupienia,
                gra: self.ust.dzwiek_skupienia
                    && self.ust.glosnosc_dzwieku() > 0.0
                    && !self.dzwiek_wyciszony()
                    && !self.dzwiek_niedostepny,
                wyciszony_przez_obs: self.dzwiek_wyciszony(),
            },
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
mod testy;
