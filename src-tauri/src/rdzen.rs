//! Rdzeń widgetu: stany postaci i przejścia z SPEC.md (B4, B5).
//!
//! Rdzeń nie zna Tauri ani Windows. Dostaje zdarzenia z czasem `teraz`,
//! zmienia stan, zapisuje pliki i zbiera dymki oraz efekty dla frontendu.
//! Dzięki temu cały przepływ dnia da się sprawdzić testami.

use std::collections::BTreeSet;

use chrono::{Duration, NaiveTime};
use serde::Serialize;

use crate::czas::{self, Chwila};
use crate::dzien::{DzienStan, Edycja};
use crate::dziennik::{self, DzienPracy, WpisSesji};
use crate::magazyn::{KotwicaOkna, Magazyn, Strona};
use crate::plik_zadan::{self, BladZmiany, Klucz, PlikZadan, Zadanie};
use crate::sesja::{PowodPauzy, Sesja, ZadanieSesji};
use crate::ustawienia::Ustawienia;

/// Jak długo trwa powitanie, zanim postać przejdzie dalej.
const POWITANIE_S: i64 = 6;
/// Gratulacje wracają do listy „po 4 s” (diagram stanów w SPEC.md).
const GRATULACJE_S: i64 = 4;
/// Krótszej sesji przerwanej przez „Przerwij” nie zapisujemy.
const MIN_SESJA_S: i64 = 60;
/// Wybór zadań liczy co najmniej tyle minut na zadanie (B4).
const MIN_NA_ZADANIE_MIN: u32 = 10;
/// Wybór przekracza cel, gdy suma jest większa o ponad 20% (B4).
const PROG_ZA_DUZO: f64 = 1.2;

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
}

/// Faza poza sesją. W czasie sesji stan wynika z samej sesji (Praca, Pauza, Jesteś?).
#[derive(Debug, Clone, PartialEq)]
enum Faza {
    Konfiguracja,
    Powitanie { do_: Chwila },
    Wybor,
    Lista,
    Gratulacje { do_: Option<Chwila>, info: GratulacjeInfo },
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
    pub strona: Strona,
    pub imie: String,
    pub wybor: Option<WyborWidok>,
    pub lista: Option<ListaWidok>,
    pub sesja: Option<SesjaWidok>,
    pub gratulacje: Option<GratulacjeInfo>,
    pub dzien: DzienWidok,
    /// Problem z plikiem zadań (brak ścieżki, nieczytelny plik).
    pub blad_pliku: Option<String>,
    /// Projekty do wyboru w „Dodaj zadanie”: nagłówki z pliku i Inbox.
    pub projekty_do_dopisania: Vec<String>,
}

pub struct Rdzen {
    pub ust: Ustawienia,
    magazyn: Magazyn,
    dz: DzienStan,
    zadania: PlikZadan,
    blad_pliku: Option<String>,
    okno: KotwicaOkna,
    faza: Faza,
    wybor: BTreeSet<Klucz>,
    wybor_przekroczony: bool,
    pub ukrycie: Ukrycie,
    zdarzenia: Vec<Zdarzenie>,
    brudny: bool,
    kreator: bool,
    obserwacja_pliku: bool,
    ostatni_dymek_powrotu: Option<Chwila>,
}

impl Rdzen {
    pub fn nowy(magazyn: Magazyn, ust: Ustawienia, teraz: Chwila) -> Rdzen {
        let okno = magazyn.wczytaj_okno();
        Rdzen {
            ust,
            dz: DzienStan::nowy(czas::dzien(teraz)),
            magazyn,
            zadania: PlikZadan::default(),
            blad_pliku: None,
            okno,
            faza: Faza::Lista,
            wybor: BTreeSet::new(),
            wybor_przekroczony: false,
            ukrycie: Ukrycie::default(),
            zdarzenia: Vec::new(),
            brudny: false,
            kreator: false,
            obserwacja_pliku: false,
            ostatni_dymek_powrotu: None,
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

    pub fn kotwica(&self) -> KotwicaOkna {
        self.okno.clone()
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
            _ => {}
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
                let przeniesione = self.zamknij_stary_dzien(teraz);
                self.dz = DzienStan { przeniesione, ..DzienStan::nowy(dzis) };
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
        } else if !self.dz.powitano {
            self.powitaj(teraz);
        } else if self.plan_zadania().is_empty() {
            self.otworz_wybor();
        } else {
            self.faza = Faza::Lista;
            self.dymek("powrot", &[]);
        }
        self.zastosuj_edycje();
        self.brudny = true;
        self.zapisz_stan(teraz);
    }

    /// Wywoływany co sekundę albo w `najblizszy_termin`.
    pub fn tick(&mut self, teraz: Chwila) {
        self.zmiana_dnia(teraz);
        match &self.faza {
            Faza::Powitanie { do_ } if teraz >= *do_ => self.po_powitaniu(),
            Faza::Gratulacje { do_: Some(do_), .. } if teraz >= *do_ => {
                self.faza = Faza::Lista;
                self.brudny = true;
            }
            _ => {}
        }
        self.przypomnienie_o_przerwie(teraz);
    }

    /// Co 30 s: ponowienie zapisów z kolejki i zapis stanu dnia.
    pub fn cykl(&mut self, teraz: Chwila) {
        self.tick(teraz);
        // Przy działającej obserwacji pliku zadań nie odpytujemy go co 30 s.
        if !(self.obserwacja_pliku && self.ust.plik_zadan().is_some()) {
            self.wczytaj_zadania();
        }
        if !self.dz.oczekujace.is_empty() {
            self.zastosuj_edycje();
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
        let przeniesione = self.zamknij_stary_dzien(teraz);
        self.dz = DzienStan { przeniesione, powitano: true, ..DzienStan::nowy(dzis) };
        self.wczytaj_zadania();
        if !self.kreator {
            self.otworz_wybor();
        }
        self.brudny = true;
    }

    /// Kończy dzień, który minął: trwająca sesja zapisuje się jak „Przerwij”,
    /// a dziennik dostaje ostatni stan. Zwraca niedokończone zadania z planu.
    fn zamknij_stary_dzien(&mut self, teraz: Chwila) -> Vec<Klucz> {
        if let Some(mut s) = self.dz.sesja.take() {
            let koniec = s.pauza.as_ref().map_or(s.koniec(), |p| p.od);
            s.zakoncz(koniec);
            self.dz.sesja = Some(s);
            self.zakoncz_sesje(false, koniec);
        }
        self.zastosuj_edycje();
        if !self.dz.sesje.is_empty() {
            self.zapisz_dziennik();
        }
        if self.dz.dziennik_do_zapisu {
            let dzien = self.dzien_pracy();
            self.dz.zalegle.push(dzien);
        }
        let _ = teraz;
        self.dz.plan.iter().filter(|k| self.zadanie(k).is_some_and(|z| !z.zrobione)).cloned().collect()
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
        if self.plan_zadania().is_empty() {
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
        if self.dz.sesja.is_some() || matches!(self.faza, Faza::Konfiguracja) {
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
    pub fn zatwierdz_wybor(&mut self, _teraz: Chwila) {
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
        self.dymek("start", &[("zadanie", klucz.zadanie.clone())]);
        self.dz.sesja = Some(Sesja::nowa(zadanie, teraz));
        self.faza = Faza::Lista;
        self.brudny = true;
        Ok(())
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
            info: GratulacjeInfo {
                zadanie: wpis.zadanie,
                czas_min: czas_zadania,
                szac_min: wpis.szac_min,
                czas_dnia_min: czas_dnia,
                plan_wykonany,
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

    pub fn ustaw_kotwice(&mut self, kotwica: KotwicaOkna, _teraz: Chwila) {
        if self.okno != kotwica {
            self.okno = kotwica;
            if let Err(e) = self.magazyn.zapisz_okno(&self.okno) {
                log::warn!("nie mogę zapisać położenia okna: {e}");
            }
        }
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

    /// Ustawienia nie dały się wczytać: zostają poprzednie.
    pub fn blad_ustawien(&mut self) {
        self.dymek("blad_ustawien", &[]);
    }

    // ---- sesje i pliki -----------------------------------------------------------------------------

    /// Zapisuje czas (i ptaszka) w pliku zadań. Co się nie uda z powodu
    /// dostępu do pliku, czeka w kolejce na kolejny cykl; zadanie, którego
    /// już nie ma w pliku, zostaje tylko w dzienniku.
    fn zastosuj_edycje(&mut self) {
        let Some(sciezka) = self.ust.plik_zadan() else { return };
        let mut zmiana = false;
        while let Some(e) = self.dz.oczekujace.first().cloned() {
            let wynik =
                plik_zadan::zmien(&sciezka, |t| Ok((plik_zadan::dopisz_czas(t, &e.klucz, e.minuty, e.zrobione)?, ())));
            match wynik {
                Ok(()) => {
                    self.dz.oczekujace.remove(0);
                    zmiana = true;
                }
                Err(BladZmiany::Edycja(_)) => {
                    self.dz.oczekujace.remove(0);
                    self.dymek("nie_znaleziono", &[("zadanie", e.klucz.zadanie.clone())]);
                    zmiana = true;
                }
                Err(BladZmiany::Io(err)) => {
                    log::warn!("zapis w pliku zadań czeka w kolejce ({}): {err}", sciezka.display());
                    break;
                }
            }
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
            podsumowanie: None,
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
        });
        log::info!(
            "sesja zapisana: {} · {minuty} min · {}",
            wpis.zadanie,
            if zrobione { "Zrobione" } else { "W toku" }
        );
        self.zastosuj_edycje();
        self.zapisz_dziennik();
        self.zapisz_stan(teraz);
        Some((wpis, s.zadanie.czas_pliku_min + minuty))
    }

    // ---- dymki --------------------------------------------------------------------------------------

    /// Dymek z wstawkami; `imie` jest dodawane zawsze.
    fn dymek(&mut self, klucz: &str, wstawki: &[(&str, String)]) {
        let mut mapa: std::collections::BTreeMap<String, String> =
            wstawki.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        mapa.insert("imie".into(), self.ust.imie.clone());
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
            Faza::Gratulacje { info, .. } if self.dz.sesja.is_none() => Some(info.clone()),
            _ => None,
        };
        let w_toku_minuty = self.dz.sesja.as_ref().map_or(0, |s| s.minuty(czas::teraz()));
        Migawka {
            stan: stan.nazwa(),
            widoczna: self.widoczna(),
            strona: self.okno.strona,
            imie: self.ust.imie.clone(),
            wybor: (stan == Stan::WyborZadan).then(|| self.widok_wyboru()),
            lista: matches!(stan, Stan::Lista | Stan::Gratulacje).then(|| self.widok_listy()),
            sesja,
            gratulacje,
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
        r.tick(t("09:50:05"));
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
        r.tick(t("12:10:04"));
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
        r.tick(chwila("2026-10-31T00:30:20+01:00"));
        assert_eq!(r.stan(), Stan::WyborZadan);
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
}
