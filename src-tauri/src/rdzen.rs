//! Rdzeń widgetu: stany postaci i przejścia z SPEC.md (B4, B5).
//!
//! Rdzeń nie zna Tauri ani Windows. Dostaje zdarzenia z czasem `teraz`,
//! zmienia stan, zapisuje pliki i zbiera dymki oraz efekty dla frontendu.
//! Dzięki temu cały przepływ da się sprawdzić testami.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{Duration, NaiveDate, NaiveTime};
use serde::Serialize;

use crate::czas::{self, Chwila};
use crate::dziennik::{self, DzienPracy, WpisSesji};
use crate::magazyn::{Magazyn, StanLokalny, Strona};
use crate::plan::{self, Plan};
use crate::pliki;
use crate::sesja::{InfoZadania, PowodPauzy, Sesja};
use crate::ustawienia::Ustawienia;
use crate::zadania::{StanZadan, W_TOKU, ZROBIONE};

/// Jak długo trwa powitanie, zanim postać przejdzie do listy.
const POWITANIE_S: i64 = 6;
/// Gratulacje wracają do listy „po 4 s” (diagram stanów w SPEC.md).
const GRATULACJE_S: i64 = 4;
/// Krótszej sesji przerwanej przez „Przerwij” nie zapisujemy.
const MIN_SESJA_S: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stan {
    Konfiguracja,
    Powitanie,
    Lista,
    CzekaNaPlan,
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
            Stan::Lista => "lista",
            Stan::CzekaNaPlan => "czeka_na_plan",
            Stan::Praca => "praca",
            Stan::Pauza => "pauza",
            Stan::Jestes => "jestes",
            Stan::Gratulacje => "gratulacje",
            Stan::KoniecDnia => "koniec_dnia",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Gratulacje {
    pub nazwa: String,
    pub minuty: u32,
    pub szac_min: Option<u32>,
    /// Ostatnie zadanie z planu na dziś: dłuższe konfetti i koniec dnia.
    pub ostatnie: bool,
}

/// Faza poza sesją. W czasie sesji stan wynika z samej sesji (Praca, Pauza, Jesteś?).
#[derive(Debug, Clone, PartialEq)]
enum Faza {
    Konfiguracja,
    Powitanie { do_: Chwila },
    Lista,
    Gratulacje { do_: Chwila, info: Gratulacje, potem_koniec: bool },
    KoniecDnia,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "rodzaj", rename_all = "snake_case")]
pub enum Zdarzenie {
    /// Klucz z `src/kwestie.json` i wstawki do podstawienia.
    Dymek {
        klucz: String,
        wstawki: BTreeMap<String, String>,
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

pub struct ZaladowanyPlan {
    pub plan: Plan,
    pub sciezka: PathBuf,
    pub zmieniony: SystemTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZadanieWidok {
    pub id: String,
    pub nazwa: String,
    pub etap: Option<String>,
    pub szac_min: Option<u32>,
    pub status: Option<String>,
    pub minuty_dzis: u32,
    pub minuty_razem: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanWidok {
    pub naglowek: String,
    pub zagrozone: bool,
    pub dni_do_publikacji: Option<i64>,
    pub zadania: Vec<ZadanieWidok>,
    pub zapasowe: Vec<ZadanieWidok>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SesjaWidok {
    pub id: String,
    pub nazwa: String,
    pub szac_min: Option<u32>,
    /// Suma zamkniętych odcinków; frontend dolicza otwarty odcinek sam.
    pub sekundy_zamkniete: i64,
    pub odcinek_od_ms: Option<i64>,
    pub pauza_od_ms: Option<i64>,
    pub powod_pauzy: Option<PowodPauzy>,
    pub bezczynna_od_ms: Option<i64>,
    /// Minuty z wcześniejszych sesji tego zadania w tym samym dniu.
    pub minuty_wczesniej: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DzienWidok {
    pub minuty: u32,
    pub zrobione: Vec<String>,
}

/// Wszystko, czego frontend potrzebuje do narysowania bieżącego stanu.
#[derive(Debug, Clone, Serialize)]
pub struct Migawka {
    pub stan: &'static str,
    pub widoczna: bool,
    pub strona: Strona,
    pub imie: String,
    pub plan: Option<PlanWidok>,
    pub w_toku: Vec<ZadanieWidok>,
    pub sesja: Option<SesjaWidok>,
    pub gratulacje: Option<Gratulacje>,
    pub dzien: DzienWidok,
    pub godzina_planu: String,
    /// Ścieżki z ustawień, które nadal mają znaczniki `<Dysk Google>` / `<vault>`.
    pub brakujace_sciezki: Vec<&'static str>,
}

pub struct Rdzen {
    pub ust: Ustawienia,
    pub lok: StanLokalny,
    pub plan: Option<ZaladowanyPlan>,
    pub ukrycie: Ukrycie,
    magazyn: Magazyn,
    dzis: DzienPracy,
    dzien: NaiveDate,
    faza: Faza,
    zdarzenia: Vec<Zdarzenie>,
    brudny: bool,
    ostatni_dymek_powrotu: Option<Chwila>,
    kreator: bool,
}

impl Rdzen {
    pub fn nowy(magazyn: Magazyn, ust: Ustawienia, teraz: Chwila) -> Rdzen {
        let lok = magazyn.wczytaj_stan();
        let dzien = czas::dzien(teraz);
        let dzis = magazyn.wczytaj_dzien(dzien);
        Rdzen {
            ust,
            lok,
            plan: None,
            ukrycie: Ukrycie::default(),
            magazyn,
            dzis,
            dzien,
            faza: Faza::Lista,
            zdarzenia: Vec::new(),
            brudny: false,
            ostatni_dymek_powrotu: None,
            kreator: false,
        }
    }

    // ---- odczyt -------------------------------------------------------

    pub fn stan(&self) -> Stan {
        if let Some(s) = &self.lok.sesja {
            return if s.pauza.is_some() {
                Stan::Pauza
            } else if s.bezczynna_od.is_some() {
                Stan::Jestes
            } else {
                Stan::Praca
            };
        }
        match self.faza {
            Faza::Powitanie { .. } => Stan::Powitanie,
            Faza::Lista if self.plan.is_some() => Stan::Lista,
            Faza::Lista => Stan::CzekaNaPlan,
            Faza::Gratulacje { .. } => Stan::Gratulacje,
            Faza::Konfiguracja => Stan::Konfiguracja,
            Faza::KoniecDnia => Stan::KoniecDnia,
        }
    }

    pub fn widoczna(&self) -> bool {
        !self.ukrycie.recznie && !self.schowana_przez_pelny_ekran()
    }

    fn schowana_przez_pelny_ekran(&self) -> bool {
        self.ukrycie.pelny_ekran && self.ust.chowaj_przy_pelnym_ekranie && !self.ukrycie.wymuszone
    }

    pub fn sesja_trwa(&self) -> bool {
        self.lok.sesja.as_ref().is_some_and(|s| s.pauza.is_none())
    }

    pub fn wez_zdarzenia(&mut self) -> Vec<Zdarzenie> {
        std::mem::take(&mut self.zdarzenia)
    }

    pub fn ustawienia_sciezka(&self) -> PathBuf {
        self.magazyn.ustawienia()
    }

    /// Najbliższa chwila, w której `tick` ma coś do zrobienia. Uwzględnia
    /// tylko terminy, na które `tick` zareaguje, żeby zaległy termin nie
    /// budził wątku w kółko.
    pub fn najblizszy_termin(&self, teraz: Chwila) -> Chwila {
        let polnoc = (self.dzien + Duration::days(1)).and_time(NaiveTime::MIN);
        let mut termin = lokalna(polnoc, teraz).unwrap_or(teraz + Duration::hours(1));
        let mut wez = |t: Chwila| {
            if t < termin {
                termin = t;
            }
        };
        match &self.faza {
            Faza::Powitanie { do_ } | Faza::Gratulacje { do_, .. } => wez(*do_),
            Faza::Lista if self.lok.sesja.is_none() && self.lok.koniec_dnia_pokazany != Some(self.dzien) => {
                if let Some(t) = lokalna(self.dzien.and_time(self.ust.koniec_dnia()), teraz) {
                    wez(t);
                }
            }
            _ => {}
        }
        let co = self.ust.przypomnienie_przerwy_min;
        if let (Some(s), Stan::Praca, true) = (&self.lok.sesja, self.stan(), co > 0) {
            if let Some(od) = s.otwarty_od() {
                wez(od + Duration::minutes(i64::from((s.przypomnienia + 1) * co)));
            }
        }
        termin.max(teraz)
    }

    /// Powitanie liczy się od chwili, gdy frontend jest gotowy.
    pub fn przesun_powitanie(&mut self, teraz: Chwila) {
        if let Faza::Powitanie { do_ } = &mut self.faza {
            *do_ = teraz + Duration::seconds(POWITANIE_S);
        }
    }

    // ---- start, cykl i zegar -------------------------------------------

    /// Pierwsze uruchomienie wersji 2: zamiast powitania otwiera się kreator.
    pub fn wymus_kreator(&mut self) {
        self.kreator = true;
    }

    /// Kreator skończony: ustawienia są zapisane, plik zadań istnieje.
    pub fn zakoncz_kreator(&mut self, nowe: Ustawienia, teraz: Chwila) {
        self.kreator = false;
        self.lok.ostatnie_powitanie = Some(self.dzien);
        self.faza = Faza::Lista;
        self.zmien_ustawienia(nowe, teraz);
        self.dymek("konfiguracja_gotowe", &[]);
        self.brudny = true;
        self.zapisz_stan(teraz);
    }

    pub fn start(&mut self, teraz: Chwila) {
        self.dzien = czas::dzien(teraz);
        self.dzis = self.magazyn.wczytaj_dzien(self.dzien);
        if let Some(s) = self.lok.sesja.as_mut() {
            let ostatni_zapis = self.lok.ostatni_zapis.unwrap_or(teraz);
            s.odtworz(ostatni_zapis);
            log::info!("odtworzona sesja: {} (pauza)", s.zadanie.pelna_nazwa());
        }
        self.zamknij_sesje_z_innego_dnia();
        self.wczytaj_plan();
        self.scal_stan_zadan_z_dysku();
        self.efekt("glitch_wejscie");
        if self.kreator && self.lok.sesja.is_none() {
            self.faza = Faza::Konfiguracja;
            self.dymek("konfiguracja_imie", &[]);
        } else if self.lok.sesja.is_some() {
            self.faza = Faza::Lista;
            self.dymek("po_blokadzie", &[]);
        } else if self.lok.ostatnie_powitanie != Some(self.dzien) {
            self.powitaj(teraz);
        } else {
            self.faza = Faza::Lista;
            self.dymek("powrot", &[]);
        }
        self.zapisz_pliki(teraz);
        self.brudny = true;
        self.zapisz_stan(teraz);
    }

    /// Wywoływany co sekundę albo w `najblizszy_termin`.
    pub fn tick(&mut self, teraz: Chwila) {
        self.zmiana_dnia(teraz);
        match &self.faza {
            Faza::Powitanie { do_ } if teraz >= *do_ => {
                self.faza = Faza::Lista;
                if self.lok.sesja.is_none() {
                    if self.plan.is_some() {
                        self.dymek("lista", &[]);
                    } else {
                        self.dymek_czeka_na_plan();
                    }
                }
            }
            Faza::Gratulacje { do_, potem_koniec, .. } if teraz >= *do_ => {
                self.faza = if *potem_koniec { Faza::KoniecDnia } else { Faza::Lista };
            }
            _ => {}
        }
        if self.faza == Faza::Lista
            && self.lok.sesja.is_none()
            && self.lok.koniec_dnia_pokazany != Some(self.dzien)
            && teraz.time() >= self.ust.koniec_dnia()
        {
            self.faza = Faza::KoniecDnia;
            self.lok.koniec_dnia_pokazany = Some(self.dzien);
            self.dymek("koniec_dnia", &[]);
            self.brudny = true;
        }
        self.przypomnienie_o_przerwie(teraz);
    }

    /// Co 30 s: nowy plan, ponowienie zapisów z kolejki, zapis stanu sesji.
    pub fn cykl(&mut self, teraz: Chwila) {
        self.tick(teraz);
        let czekal = self.stan() == Stan::CzekaNaPlan;
        if self.wczytaj_plan() && czekal && self.stan() == Stan::Lista {
            self.dymek("plan_dotarl", &[]);
        }
        self.zapisz_pliki(teraz);
        if self.sesja_trwa() {
            self.brudny = true;
        }
        self.zapisz_stan(teraz);
    }

    fn zmiana_dnia(&mut self, teraz: Chwila) {
        let nowy = czas::dzien(teraz);
        if nowy == self.dzien {
            return;
        }
        log::info!("nowy dzień: {nowy}");
        self.dzien = nowy;
        self.dzis = self.magazyn.wczytaj_dzien(nowy);
        self.plan = None;
        // Trwająca sesja liczy się dalej i należy do dnia, w którym się
        // zaczęła. Zapauzowana z wczoraj trafia do dziennika jako „W toku”.
        self.zamknij_sesje_z_innego_dnia();
        if self.faza != Faza::Lista {
            self.faza = Faza::Lista;
        }
        self.wczytaj_plan();
        self.brudny = true;
    }

    fn zamknij_sesje_z_innego_dnia(&mut self) {
        let Some(s) = &self.lok.sesja else { return };
        if s.dzien == self.dzien || s.pauza.is_none() {
            return;
        }
        let koniec = s.pauza.as_ref().map_or(s.start, |p| p.od);
        log::info!("sesja z {} zamknięta jako „W toku”", s.dzien);
        self.zakoncz_sesje(W_TOKU, koniec);
    }

    fn przypomnienie_o_przerwie(&mut self, teraz: Chwila) {
        let co = self.ust.przypomnienie_przerwy_min;
        if co == 0 || self.stan() != Stan::Praca {
            return;
        }
        let Some(s) = self.lok.sesja.as_mut() else { return };
        let minuty = s.sekundy_odcinka(teraz) / 60;
        if minuty >= i64::from((s.przypomnienia + 1) * co) {
            s.przypomnienia += 1;
            let ile = czas::minuty_tekst(minuty as u32);
            self.dymek("przypomnienie_przerwy", &[("minuty", ile)]);
            self.efekt("przeciaganie");
            self.brudny = true;
        }
    }

    // ---- działania użytkownika -------------------------------------------

    pub fn rozpocznij(&mut self, id: &str, teraz: Chwila) -> Result<(), String> {
        if self.lok.sesja.is_some() {
            return Err("trwa już sesja; najpierw Przerwij albo Zrobione".into());
        }
        let info = self
            .plan
            .as_ref()
            .and_then(|p| p.plan.zadanie(id))
            .map(InfoZadania::from)
            .or_else(|| self.lok.znane_zadania.get(id).cloned())
            .ok_or_else(|| format!("nie znam zadania {id}"))?;
        self.lok.znane_zadania.insert(info.id.clone(), info.clone());
        self.dymek("start", &[("zadanie", info.pelna_nazwa())]);
        self.lok.sesja = Some(Sesja::nowa(info, teraz));
        self.faza = Faza::Lista;
        self.brudny = true;
        Ok(())
    }

    pub fn pauza(&mut self, teraz: Chwila) {
        if let Some(s) = self.lok.sesja.as_mut().filter(|s| s.pauza.is_none()) {
            s.pauzuj(teraz, PowodPauzy::Reczna);
            self.dymek("pauza", &[]);
            self.brudny = true;
        }
    }

    pub fn wznow(&mut self, teraz: Chwila) {
        if let Some(s) = self.lok.sesja.as_mut().filter(|s| s.pauza.is_some()) {
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
        let Some(wpis) = self.zakoncz_sesje(ZROBIONE, teraz) else { return };
        let (wszystkie, glowne) = match &self.plan {
            Some(p) if !p.plan.zadania.is_empty() => (
                p.plan.zadania.iter().all(|z| self.lok.zadania.zrobione(&z.id)),
                p.plan.zadania.iter().any(|z| z.id == wpis.id),
            ),
            _ => (false, false),
        };
        let ostatnie = wszystkie && glowne;
        if ostatnie {
            self.dymek("wszystko_zrobione", &[]);
            self.efekt("konfetti_dlugie");
        } else {
            if wpis.szac_min.is_some_and(|s| wpis.minuty > s) {
                self.dymek("ponad_szacunek", &[("zadanie", wpis.nazwa.clone())]);
            } else {
                self.dymek("zrobione", &[("zadanie", wpis.nazwa.clone()), ("minuty", czas::minuty_tekst(wpis.minuty))]);
            }
            self.efekt("konfetti");
        }
        self.faza = Faza::Gratulacje {
            do_: teraz + Duration::seconds(GRATULACJE_S),
            info: Gratulacje { nazwa: wpis.nazwa, minuty: wpis.minuty, szac_min: wpis.szac_min, ostatnie },
            potem_koniec: wszystkie,
        };
    }

    pub fn przerwij(&mut self, teraz: Chwila) {
        if self.lok.sesja.is_none() {
            return;
        }
        if let Some(wpis) = self.zakoncz_sesje(W_TOKU, teraz) {
            self.dymek("przerwij", &[("zadanie", wpis.nazwa), ("minuty", czas::minuty_tekst(wpis.minuty))]);
        }
        self.faza = Faza::Lista;
    }

    /// „Przerwa” odejmuje czas od ostatniej aktywności, „Pracowałem” go zostawia.
    pub fn odpowiedz_jestes(&mut self, przerwa: bool, teraz: Chwila) {
        let Some(s) = self.lok.sesja.as_mut().filter(|s| s.bezczynna_od.is_some()) else { return };
        if przerwa {
            s.odejmij_bezczynnosc(teraz);
            self.dymek("wznowienie", &[]);
        } else {
            s.bezczynna_od = None;
        }
        self.brudny = true;
    }

    pub fn pokaz_liste(&mut self) {
        if matches!(self.faza, Faza::KoniecDnia | Faza::Gratulacje { .. }) {
            self.faza = Faza::Lista;
        }
    }

    // ---- zdarzenia systemowe ---------------------------------------------

    /// Brak myszy i klawiatury od `sekundy` (GetLastInputInfo, co 30 s).
    pub fn bezczynnosc(&mut self, sekundy: u64, teraz: Chwila) {
        let prog = u64::from(self.ust.prog_bezczynnosci_min) * 60;
        if prog == 0 || sekundy < prog || self.stan() != Stan::Praca || self.schowana_przez_pelny_ekran() {
            return;
        }
        let Some(s) = self.lok.sesja.as_mut() else { return };
        let ostatnia = teraz - Duration::seconds(sekundy as i64);
        s.bezczynna_od = Some(s.otwarty_od().map_or(ostatnia, |od| ostatnia.max(od)));
        self.dymek("jestes", &[]);
        self.brudny = true;
    }

    /// Blokada ekranu, uśpienie, zamknięcie aplikacji lub wylogowanie.
    pub fn pauza_automatyczna(&mut self, powod: PowodPauzy, teraz: Chwila) {
        if let Some(s) = self.lok.sesja.as_mut().filter(|s| s.pauza.is_none()) {
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
        if self.lok.sesja.as_ref().is_some_and(|s| s.pauza.is_some()) {
            let dawno = self.ostatni_dymek_powrotu.is_none_or(|t| teraz - t > Duration::seconds(10));
            if dawno {
                self.dymek("po_blokadzie", &[]);
                self.ostatni_dymek_powrotu = Some(teraz);
            }
        } else if self.lok.sesja.is_none() && self.lok.ostatnie_powitanie != Some(self.dzien) {
            self.efekt("glitch_wejscie");
            self.powitaj(teraz);
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

    pub fn ustaw_kotwice(&mut self, kotwica: crate::magazyn::KotwicaOkna, teraz: Chwila) {
        if self.lok.okno != kotwica {
            self.lok.okno = kotwica;
            self.brudny = true;
            self.zapisz_stan(teraz);
        }
    }

    pub fn zmien_ustawienia(&mut self, nowe: Ustawienia, teraz: Chwila) {
        let plany = nowe.folder_planow != self.ust.folder_planow;
        let stan = nowe.plik_stanu != self.ust.plik_stanu;
        let dziennik = nowe.folder_dziennika != self.ust.folder_dziennika;
        self.ust = nowe;
        if plany {
            self.plan = None;
            self.wczytaj_plan();
        }
        if stan {
            self.scal_stan_zadan_z_dysku();
            self.lok.do_zapisu.stan_zadan = true;
        }
        if dziennik && !self.dzis.sesje.is_empty() {
            self.lok.do_zapisu.dzienniki.insert(self.dzien);
        }
        self.brudny = true;
        self.zapisz_pliki(teraz);
        self.zapisz_stan(teraz);
    }

    /// Ustawienia nie dały się wczytać: zostają poprzednie.
    pub fn blad_ustawien(&mut self) {
        self.dymek("blad_ustawien", &[]);
    }

    // ---- pliki ----------------------------------------------------------

    /// Zwraca `true`, gdy plan się zmienił.
    fn wczytaj_plan(&mut self) -> bool {
        let Some(folder) = self.ust.folder_planow() else { return false };
        match plan::znajdz(&folder, self.dzien) {
            Ok(Some(znaleziony)) => {
                let nowy = match &self.plan {
                    Some(p) => p.sciezka != znaleziony.sciezka || p.zmieniony != znaleziony.zmieniony,
                    None => true,
                };
                if nowy {
                    log::info!("plan: {}", znaleziony.sciezka.display());
                    for z in znaleziony.plan.wszystkie_zadania() {
                        self.lok.znane_zadania.insert(z.id.clone(), InfoZadania::from(z));
                    }
                    self.plan = Some(ZaladowanyPlan {
                        plan: znaleziony.plan,
                        sciezka: znaleziony.sciezka,
                        zmieniony: znaleziony.zmieniony,
                    });
                    self.brudny = true;
                }
                nowy
            }
            Ok(None) => false,
            Err(e) => {
                log::debug!("folder planów {} niedostępny: {e}", folder.display());
                false
            }
        }
    }

    fn czytaj_stan_z_dysku(&self) -> Option<StanZadan> {
        let sciezka = self.ust.plik_stanu()?;
        let tekst = pliki::czytaj_tekst(&sciezka).ok()?;
        match serde_json::from_str(&tekst) {
            Ok(stan) => Some(stan),
            Err(e) => {
                log::warn!("{}: nie rozumiem pliku ({e}), nadpiszę go swoim stanem", sciezka.display());
                None
            }
        }
    }

    fn scal_stan_zadan_z_dysku(&mut self) {
        if let Some(z_dysku) = self.czytaj_stan_z_dysku() {
            if self.lok.zadania.uzupelnij_z(&z_dysku) {
                log::info!("przejęto zadania ze stan-zadan.json na Dysku");
                self.brudny = true;
            }
        }
    }

    /// Kolejka zapisów: stan zadań na Dysk i bloki w dziennikach. Co się nie
    /// uda (np. Dysk jeszcze nie wystartował), czeka na kolejny cykl.
    fn zapisz_pliki(&mut self, teraz: Chwila) {
        if self.lok.do_zapisu.stan_zadan {
            if let Some(sciezka) = self.ust.plik_stanu() {
                self.scal_stan_zadan_z_dysku();
                match pliki::zapisz_atomowo(&sciezka, self.lok.zadania.do_json().as_bytes()) {
                    Ok(()) => {
                        self.lok.do_zapisu.stan_zadan = false;
                        self.brudny = true;
                    }
                    Err(e) => log::warn!("stan zadań czeka w kolejce ({}): {e}", sciezka.display()),
                }
            }
        }
        if let Some(folder) = self.ust.folder_dziennika() {
            let daty: Vec<NaiveDate> = self.lok.do_zapisu.dzienniki.iter().copied().collect();
            for data in daty {
                let dzien = if data == self.dzien { self.dzis.clone() } else { self.magazyn.wczytaj_dzien(data) };
                match dziennik::zapisz(&folder, &dzien) {
                    Ok(()) => {
                        self.lok.do_zapisu.dzienniki.remove(&data);
                        self.brudny = true;
                    }
                    Err(e) => log::warn!("dziennik {data} czeka w kolejce: {e}"),
                }
            }
        }
        let _ = teraz;
    }

    /// Zapis lokalnego stanu: co 30 s w czasie pracy i przy każdej zmianie.
    pub fn zapisz_stan(&mut self, teraz: Chwila) {
        if !self.brudny {
            return;
        }
        self.lok.ostatni_zapis = Some(teraz);
        match self.magazyn.zapisz_stan(&self.lok) {
            Ok(()) => self.brudny = false,
            Err(e) => log::error!("nie mogę zapisać stanu lokalnego: {e}"),
        }
    }

    /// Kończy sesję: wpis do rejestru dnia i stanu zadań, zapis plików.
    fn zakoncz_sesje(&mut self, status: &str, teraz: Chwila) -> Option<WpisSesji> {
        let mut s = self.lok.sesja.take()?;
        self.brudny = true;
        s.zakoncz(teraz);
        let sekundy = s.sekundy(teraz);
        if status == W_TOKU && sekundy < MIN_SESJA_S {
            log::info!("sesja krótsza niż minuta, nie zapisuję: {}", s.zadanie.pelna_nazwa());
            return None;
        }
        let wpis = WpisSesji {
            id: s.zadanie.id.clone(),
            nazwa: s.zadanie.pelna_nazwa(),
            szac_min: s.zadanie.szac_min,
            start: s.start,
            koniec: s.koniec(),
            sekundy,
            minuty: czas::sekundy_na_minuty(sekundy),
            status: status.to_string(),
        };
        let mut dzien = if s.dzien == self.dzien { self.dzis.clone() } else { self.magazyn.wczytaj_dzien(s.dzien) };
        dzien.sesje.push(wpis.clone());
        if let Err(e) = self.magazyn.zapisz_dzien(&dzien) {
            log::error!("nie mogę zapisać rejestru dnia {}: {e}", s.dzien);
        }
        if s.dzien == self.dzien {
            self.dzis = dzien;
        }
        self.lok.zadania.dolicz(&wpis.id, wpis.minuty, status, wpis.koniec);
        self.lok.zadania.aktualizacja = Some(czas::rfc3339(teraz));
        self.lok.znane_zadania.insert(s.zadanie.id.clone(), s.zadanie.clone());
        self.lok.do_zapisu.stan_zadan = true;
        self.lok.do_zapisu.dzienniki.insert(s.dzien);
        log::info!("sesja zapisana: {} · {} min · {status}", wpis.nazwa, wpis.minuty);
        self.zapisz_pliki(teraz);
        self.zapisz_stan(teraz);
        Some(wpis)
    }

    // ---- dymki ------------------------------------------------------------

    fn powitaj(&mut self, teraz: Chwila) {
        self.faza = Faza::Powitanie { do_: teraz + Duration::seconds(POWITANIE_S) };
        self.lok.ostatnie_powitanie = Some(self.dzien);
        self.brudny = true;
        if self.plan.is_some() {
            self.dymek("powitanie", &[]);
        } else {
            self.dymek("powitanie_bez_planu", &[]);
        }
    }

    fn dymek_czeka_na_plan(&mut self) {
        self.dymek("czeka_na_plan", &[]);
    }

    /// Dymek z wstawkami; `imie`, `godzina_planu` i `dni` są dodawane zawsze, gdy są znane.
    fn dymek(&mut self, klucz: &str, wstawki: &[(&str, String)]) {
        let mut mapa: BTreeMap<String, String> = wstawki.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        mapa.insert("imie".into(), self.ust.imie.clone());
        mapa.insert("godzina_planu".into(), czas::godzina_do_dymku(&self.ust.godzina_planu));
        if let Some(dni) = self.plan.as_ref().and_then(|p| p.plan.dni_do_publikacji).filter(|d| *d > 0) {
            mapa.insert("dni".into(), czas::dni_tekst(dni));
        }
        self.zdarzenia.push(Zdarzenie::Dymek { klucz: klucz.to_string(), wstawki: mapa });
    }

    fn efekt(&mut self, nazwa: &str) {
        self.zdarzenia.push(Zdarzenie::Efekt { nazwa: nazwa.to_string() });
    }

    // ---- migawka ------------------------------------------------------------

    fn widok_zadania(&self, info: &InfoZadania) -> ZadanieWidok {
        let wpis = self.lok.zadania.wpis(&info.id);
        ZadanieWidok {
            id: info.id.clone(),
            nazwa: info.pelna_nazwa(),
            etap: info.etap.clone(),
            szac_min: info.szac_min,
            status: wpis.map(|w| w.status.clone()),
            minuty_dzis: self.dzis.minuty_zadania(&info.id),
            minuty_razem: wpis.map_or(0, |w| w.minuty),
        }
    }

    pub fn migawka(&self) -> Migawka {
        let plan = self.plan.as_ref().map(|p| PlanWidok {
            naglowek: p.plan.naglowek.clone(),
            zagrozone: p.plan.zagrozone(),
            dni_do_publikacji: p.plan.dni_do_publikacji,
            zadania: p.plan.zadania.iter().map(|z| self.widok_zadania(&InfoZadania::from(z))).collect(),
            zapasowe: p.plan.zapasowe.iter().map(|z| self.widok_zadania(&InfoZadania::from(z))).collect(),
        });
        let w_toku = self
            .lok
            .zadania
            .zadania
            .iter()
            .filter(|(_, w)| w.status == W_TOKU)
            .filter_map(|(id, _)| self.lok.znane_zadania.get(id))
            .map(|info| self.widok_zadania(info))
            .collect();
        let sesja = self.lok.sesja.as_ref().map(|s| {
            let dzien = if s.dzien == self.dzien { Some(&self.dzis) } else { None };
            SesjaWidok {
                id: s.zadanie.id.clone(),
                nazwa: s.zadanie.pelna_nazwa(),
                szac_min: s.zadanie.szac_min,
                sekundy_zamkniete: s.sekundy_zamkniete(),
                odcinek_od_ms: s.otwarty_od().map(czas::ms),
                pauza_od_ms: s.pauza.as_ref().map(|p| czas::ms(p.od)),
                powod_pauzy: s.pauza.as_ref().map(|p| p.powod),
                bezczynna_od_ms: s.bezczynna_od.map(czas::ms),
                minuty_wczesniej: dzien.map_or(0, |d| d.minuty_zadania(&s.zadanie.id)),
            }
        });
        let gratulacje = match &self.faza {
            Faza::Gratulacje { info, .. } => Some(info.clone()),
            _ => None,
        };
        let mut brakujace_sciezki = Vec::new();
        if self.ust.folder_planow().is_none() {
            brakujace_sciezki.push("folder_planow");
        }
        if self.ust.plik_stanu().is_none() {
            brakujace_sciezki.push("plik_stanu");
        }
        if self.ust.folder_dziennika().is_none() {
            brakujace_sciezki.push("folder_dziennika");
        }
        Migawka {
            stan: self.stan().nazwa(),
            widoczna: self.widoczna(),
            strona: self.lok.okno.strona,
            imie: self.ust.imie.clone(),
            plan,
            w_toku,
            sesja,
            gratulacje,
            dzien: DzienWidok { minuty: self.dzis.minuty(), zrobione: self.dzis.zrobione() },
            godzina_planu: self.ust.godzina_planu.clone(),
            brakujace_sciezki,
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
    use std::fs;
    use std::path::Path;

    const PLAN: &str = include_str!("../../przyklady/plan-2026-10-30.json");
    const STAN: &str = include_str!("../../przyklady/stan-zadan.json");

    struct Srodowisko {
        _katalog: tempfile::TempDir,
        dysk: PathBuf,
        vault: PathBuf,
        lokalny: PathBuf,
    }

    impl Srodowisko {
        fn nowe() -> Srodowisko {
            let katalog = tempfile::tempdir().unwrap();
            let dysk = katalog.path().join("Dysk").join("Asystentka");
            let vault = katalog.path().join("vault");
            fs::create_dir_all(dysk.join("plany")).unwrap();
            fs::create_dir_all(&vault).unwrap();
            let lokalny = katalog.path().join("AppData").join("Asystentka");
            Srodowisko { dysk, vault, lokalny, _katalog: katalog }
        }

        fn ustawienia(&self) -> Ustawienia {
            Ustawienia {
                imie: "Dżiordżio".into(),
                folder_planow: self.dysk.join("plany").to_string_lossy().into(),
                plik_stanu: self.dysk.join("stan-zadan.json").to_string_lossy().into(),
                folder_dziennika: self.vault.join("Dziennik pracy").to_string_lossy().into(),
                ..Ustawienia::default()
            }
        }

        fn rdzen(&self, teraz: Chwila) -> Rdzen {
            let mut r = Rdzen::nowy(Magazyn::new(self.lokalny.clone()), self.ustawienia(), teraz);
            r.start(teraz);
            r
        }

        fn wrzuc_plan(&self) {
            fs::write(self.dysk.join("plany").join("plan-2026-10-30.json"), PLAN).unwrap();
        }

        fn dziennik(&self) -> String {
            fs::read_to_string(self.vault.join("Dziennik pracy").join("2026-10-30.md")).unwrap()
        }

        fn stan_zadan(&self) -> StanZadan {
            serde_json::from_str(&fs::read_to_string(self.dysk.join("stan-zadan.json")).unwrap()).unwrap()
        }
    }

    fn t(godzina: &str) -> Chwila {
        chwila(&format!("2026-10-30T{godzina}+01:00"))
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

    #[test]
    fn pierwszy_start_dnia_wita_i_przechodzi_do_listy() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        assert_eq!(r.stan(), Stan::Powitanie);
        let zdarzenia = r.wez_zdarzenia();
        assert!(zdarzenia.contains(&Zdarzenie::Efekt { nazwa: "glitch_wejscie".into() }));
        let Some(Zdarzenie::Dymek { klucz, wstawki }) = zdarzenia.iter().find(|z| matches!(z, Zdarzenie::Dymek { .. }))
        else {
            panic!("brak dymku powitania")
        };
        assert_eq!(klucz, "powitanie");
        assert_eq!(wstawki["dni"], "6 dni");
        assert_eq!(wstawki["imie"], "Dżiordżio");
        r.tick(t("09:00:05"));
        assert_eq!(r.stan(), Stan::Powitanie);
        r.tick(t("09:00:06"));
        assert_eq!(r.stan(), Stan::Lista);
        assert_eq!(dymki(&mut r), vec!["lista"]);
    }

    #[test]
    fn kreator_zastepuje_powitanie_i_prowadzi_do_listy() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
        r.wymus_kreator();
        r.start(t("09:00:00"));
        assert_eq!(r.stan(), Stan::Konfiguracja);
        assert_eq!(dymki(&mut r), vec!["konfiguracja_imie"]);
        r.tick(t("09:00:30"));
        assert_eq!(r.stan(), Stan::Konfiguracja, "kreator czeka na użytkownika");
        r.zakoncz_kreator(sr.ustawienia(), t("09:02:00"));
        assert_eq!(r.stan(), Stan::Lista);
        assert_eq!(r.ust.imie, "Dżiordżio");
        assert_eq!(dymki(&mut r), vec!["konfiguracja_gotowe"]);
        // Kolejny start tego samego dnia nie wita ponownie.
        let mut r2 = sr.rdzen(t("11:00:00"));
        assert_eq!(dymki(&mut r2), vec!["powrot"]);
    }

    #[test]
    fn kolejny_start_tego_samego_dnia_idzie_prosto_do_listy() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        drop(sr.rdzen(t("09:00:00")));
        let mut r = sr.rdzen(t("11:00:00"));
        assert_eq!(r.stan(), Stan::Lista);
        assert_eq!(dymki(&mut r), vec!["powrot"]);
    }

    #[test]
    fn bez_planu_czeka_i_oglasza_plan_gdy_plik_dotrze() {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("07:00:00"));
        assert_eq!(dymki(&mut r), vec!["powitanie_bez_planu"]);
        r.tick(t("07:00:06"));
        assert_eq!(r.stan(), Stan::CzekaNaPlan);
        let zdarzenia = r.wez_zdarzenia();
        assert!(
            matches!(&zdarzenia[0], Zdarzenie::Dymek { klucz, wstawki } if klucz == "czeka_na_plan" && wstawki["godzina_planu"] == "7:40")
        );
        r.cykl(t("07:30:00"));
        assert_eq!(r.stan(), Stan::CzekaNaPlan);
        sr.wrzuc_plan();
        r.cykl(t("07:41:30"));
        assert_eq!(r.stan(), Stan::Lista);
        assert_eq!(dymki(&mut r), vec!["plan_dotarl"]);
    }

    #[test]
    fn pelny_dzien_pracy_zapisuje_dziennik_i_stan_zadan() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:10"));
        r.wez_zdarzenia();

        r.rozpocznij("12", t("09:12:00")).unwrap();
        assert_eq!(r.stan(), Stan::Praca);
        assert!(r.rozpocznij("15", t("09:13:00")).is_err(), "w Pracy jest zawsze jedno zadanie");
        r.pauza(t("09:40:00"));
        assert_eq!(r.stan(), Stan::Pauza);
        r.wznow(t("09:46:00"));
        r.zrobione(t("10:05:00"));
        assert_eq!(r.stan(), Stan::Gratulacje);
        assert_eq!(r.migawka().gratulacje.unwrap().minuty, 47);
        r.tick(t("10:05:04"));
        assert_eq!(r.stan(), Stan::Lista);

        r.rozpocznij("15", t("10:20:00")).unwrap();
        r.przerwij(t("11:05:00"));
        assert_eq!(r.stan(), Stan::Lista);

        r.rozpocznij("18", t("11:30:00")).unwrap();
        r.zrobione(t("12:10:00"));

        let dziennik = sr.dziennik();
        assert!(dziennik.starts_with("---\ndata: 2026-10-30\nminuty: 132\nzrobione: 2\n---\n"));
        assert!(dziennik.contains("- [x] 09:12–10:05 · Scenariusz: intro · 47 min (szac. 60) · #12\n"));
        assert!(dziennik.contains("- [ ] 10:20–11:05 · Montaż: plansze 1–5 · 45 min · w toku · #15\n"));
        assert!(dziennik.contains("- [x] 11:30–12:10 · Miniatura: szkic · 40 min (szac. 30) · #18\n"));
        assert!(dziennik.contains("**Razem:** 2 h 12 min"));

        let stan = sr.stan_zadan();
        assert_eq!(stan.wpis("12").unwrap().minuty, 47);
        assert_eq!(stan.wpis("12").unwrap().zakonczono.as_deref(), Some("2026-10-30T10:05:00+01:00"));
        assert_eq!(stan.wpis("15").unwrap().status, W_TOKU);
        assert_eq!(stan.wpis("18").unwrap().status, ZROBIONE);

        let migawka = r.migawka();
        assert_eq!(migawka.dzien.minuty, 132);
        let lista = migawka.plan.unwrap();
        assert_eq!(lista.zadania[0].status.as_deref(), Some(ZROBIONE));
        assert_eq!(lista.zadania[1].minuty_dzis, 45);
    }

    #[test]
    fn ostatnie_zadanie_konczy_dzien() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:10"));
        r.rozpocznij("12", t("09:10:00")).unwrap();
        r.zrobione(t("10:00:00"));
        r.tick(t("10:00:05"));
        r.rozpocznij("15", t("10:10:00")).unwrap();
        r.wez_zdarzenia();
        r.zrobione(t("12:30:00"));
        let zdarzenia = r.wez_zdarzenia();
        assert!(zdarzenia.contains(&Zdarzenie::Efekt { nazwa: "konfetti_dlugie".into() }));
        assert!(matches!(&zdarzenia[0], Zdarzenie::Dymek { klucz, .. } if klucz == "wszystko_zrobione"));
        assert!(r.migawka().gratulacje.unwrap().ostatnie);
        r.tick(t("12:30:04"));
        assert_eq!(r.stan(), Stan::KoniecDnia);
        r.pokaz_liste();
        assert_eq!(r.stan(), Stan::Lista);
    }

    #[test]
    fn ponad_szacunek_bez_wyrzutow() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.tick(t("09:00:10"));
        r.rozpocznij("18", t("11:30:00")).unwrap();
        r.wez_zdarzenia();
        r.zrobione(t("12:10:00"));
        assert_eq!(dymki(&mut r), vec!["ponad_szacunek"]);
    }

    #[test]
    fn krotka_przerwana_sesja_nie_trafia_do_dziennika() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.rozpocznij("12", t("09:10:00")).unwrap();
        r.przerwij(t("09:10:40"));
        assert!(!sr.vault.join("Dziennik pracy").join("2026-10-30.md").exists());
        assert!(r.lok.zadania.wpis("12").is_none());
    }

    #[test]
    fn bezczynnosc_pyta_i_odejmuje_czas() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.rozpocznij("12", t("09:00:00")).unwrap();
        r.bezczynnosc(14 * 60, t("09:30:00"));
        assert_eq!(r.stan(), Stan::Praca);
        r.wez_zdarzenia();
        r.bezczynnosc(16 * 60, t("09:46:00"));
        assert_eq!(r.stan(), Stan::Jestes);
        assert_eq!(dymki(&mut r), vec!["jestes"]);
        r.odpowiedz_jestes(true, t("09:50:00"));
        assert_eq!(r.stan(), Stan::Praca);
        let s = r.lok.sesja.as_ref().unwrap();
        // 09:00–09:30 praca, 09:30–09:50 przerwa, potem 10 min pracy.
        assert_eq!(s.sekundy(t("10:00:00")), 40 * 60);
    }

    #[test]
    fn pracowalem_zostawia_czas_i_brak_pytania_przy_pelnym_ekranie() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.rozpocznij("12", t("09:00:00")).unwrap();
        r.ustaw_pelny_ekran(true);
        assert!(!r.widoczna());
        r.bezczynnosc(20 * 60, t("09:30:00"));
        assert_eq!(r.stan(), Stan::Praca, "schowany przez pełny ekran nie pyta");
        r.ustaw_pelny_ekran(false);
        r.bezczynnosc(20 * 60, t("09:31:00"));
        r.odpowiedz_jestes(false, t("09:32:00"));
        assert_eq!(r.lok.sesja.as_ref().unwrap().sekundy(t("09:40:00")), 40 * 60);
    }

    #[test]
    fn blokada_i_uspienie_pauzuja() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.rozpocznij("12", t("09:00:00")).unwrap();
        r.pauza_automatyczna(PowodPauzy::Blokada, t("09:20:00"));
        assert_eq!(r.stan(), Stan::Pauza);
        r.wez_zdarzenia();
        r.powrot(t("09:45:00"));
        assert_eq!(r.stan(), Stan::Pauza, "po powrocie pauza zostaje");
        assert_eq!(dymki(&mut r), vec!["po_blokadzie"]);
        r.powrot(t("09:45:02"));
        assert!(dymki(&mut r).is_empty(), "jedno pytanie po blokadzie i wybudzeniu naraz");
        r.wznow(t("09:46:00"));
        r.pauza_automatyczna(PowodPauzy::Uspienie, t("10:00:00"));
        assert_eq!(r.lok.sesja.as_ref().unwrap().sekundy(t("12:00:00")), 34 * 60);
    }

    #[test]
    fn blokada_przy_wiszacym_pytaniu_liczy_przerwe_od_ostatniej_aktywnosci() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.rozpocznij("12", t("09:00:00")).unwrap();
        r.bezczynnosc(15 * 60, t("09:30:00"));
        r.pauza_automatyczna(PowodPauzy::Blokada, t("09:40:00"));
        assert_eq!(r.lok.sesja.as_ref().unwrap().sekundy(t("11:00:00")), 15 * 60);
    }

    #[test]
    fn po_awarii_sesja_wraca_zapauzowana_bez_przestoju() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.rozpocznij("12", t("09:00:00")).unwrap();
        r.zapisz_stan(t("09:00:00"));
        r.cykl(t("09:20:00"));
        r.cykl(t("09:20:30"));
        drop(r); // proces zabity bez sprzątania
        let mut r = sr.rdzen(t("10:30:00"));
        assert_eq!(r.stan(), Stan::Pauza);
        assert_eq!(dymki(&mut r), vec!["po_blokadzie"]);
        let s = r.lok.sesja.as_ref().unwrap();
        assert_eq!(s.sekundy(t("10:30:00")), 20 * 60 + 30);
        assert_eq!(s.pauza.as_ref().unwrap().powod, PowodPauzy::Awaria);
    }

    #[test]
    fn sesja_przez_polnoc_nalezy_do_dnia_startu() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("23:00:00"));
        r.rozpocznij("12", t("23:30:00")).unwrap();
        let po_polnocy = chwila("2026-10-31T00:20:00+01:00");
        r.cykl(po_polnocy);
        assert_eq!(r.stan(), Stan::Praca, "trwająca sesja liczy się dalej");
        r.zrobione(chwila("2026-10-31T00:30:00+01:00"));
        assert!(sr.dziennik().contains("- [x] 23:30–00:30 · Scenariusz: intro · 60 min (szac. 60) · #12"));
    }

    #[test]
    fn zapauzowana_sesja_z_wczoraj_trafia_do_wczorajszego_dziennika() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("18:00:00"));
        r.rozpocznij("15", t("18:00:00")).unwrap();
        r.pauza(t("18:45:00"));
        r.zapisz_stan(t("18:45:00"));
        drop(r);
        let mut r = sr.rdzen(chwila("2026-10-31T09:00:00+01:00"));
        assert!(r.lok.sesja.is_none());
        assert_eq!(r.stan(), Stan::Powitanie);
        assert_eq!(dymki(&mut r), vec!["powitanie_bez_planu"]);
        assert!(sr.dziennik().contains("- [ ] 18:00–18:45 · Montaż: plansze 1–5 · 45 min · w toku · #15"));
    }

    #[test]
    fn koniec_dnia_o_22_tylko_raz() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("21:00:00"));
        r.tick(t("21:00:10"));
        assert!(r.najblizszy_termin(t("21:00:10")) <= t("22:00:00"));
        r.tick(t("22:00:00"));
        assert_eq!(r.stan(), Stan::KoniecDnia);
        r.pokaz_liste();
        r.tick(t("22:10:00"));
        assert_eq!(r.stan(), Stan::Lista);
    }

    #[test]
    fn zalegle_terminy_nie_budza_w_kolko() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("21:00:00"));
        r.tick(t("21:00:10"));
        r.rozpocznij("12", t("21:30:00")).unwrap();
        // 22:00 minęła w trakcie sesji, 50 min ciągłej pracy też, a wisi „Jesteś?”.
        r.bezczynnosc(20 * 60, t("22:25:00"));
        assert_eq!(r.stan(), Stan::Jestes);
        let teraz = t("22:30:00");
        assert!(r.najblizszy_termin(teraz) > teraz);
        r.przerwij(t("22:31:00"));
        assert_eq!(r.najblizszy_termin(t("22:31:00")), t("22:31:00"), "po sesji 22:00 jest zaległa");
        r.tick(t("22:31:00"));
        assert_eq!(r.stan(), Stan::KoniecDnia);
        assert!(r.najblizszy_termin(t("22:31:00")) > t("22:31:00"));
    }

    #[test]
    fn przypomnienie_o_przerwie_po_50_min_ciaglej_pracy() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        r.rozpocznij("15", t("09:00:00")).unwrap();
        r.wez_zdarzenia();
        assert_eq!(r.najblizszy_termin(t("09:10:00")), t("09:50:00"));
        r.tick(t("09:49:59"));
        assert!(dymki(&mut r).is_empty());
        r.tick(t("09:50:00"));
        let zdarzenia = r.wez_zdarzenia();
        assert!(
            matches!(&zdarzenia[0], Zdarzenie::Dymek { klucz, wstawki } if klucz == "przypomnienie_przerwy" && wstawki["minuty"] == "50 min")
        );
        assert!(zdarzenia.contains(&Zdarzenie::Efekt { nazwa: "przeciaganie".into() }));
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
    fn dysk_niedostepny_zapisy_czekaja_w_kolejce() {
        let sr = Srodowisko::nowe();
        sr.wrzuc_plan();
        let mut r = sr.rdzen(t("09:00:00"));
        let ukryty = sr.dysk.with_extension("offline");
        fs::rename(&sr.dysk, &ukryty).unwrap();
        r.rozpocznij("12", t("09:00:00")).unwrap();
        r.zrobione(t("09:30:00"));
        assert!(r.lok.do_zapisu.stan_zadan);
        assert!(sr.dziennik().contains("#12"), "vault jest lokalny, więc dziennik zapisuje się od razu");
        fs::rename(&ukryty, &sr.dysk).unwrap();
        r.cykl(t("09:30:30"));
        assert!(!r.lok.do_zapisu.stan_zadan);
        assert!(sr.stan_zadan().zrobione("12"));
    }

    #[test]
    fn przejmuje_stan_zadan_z_dysku_i_pokazuje_w_toku() {
        let sr = Srodowisko::nowe();
        fs::write(sr.dysk.join("stan-zadan.json"), STAN).unwrap();
        // Nazwa zadania 15 jest znana z wcześniejszego planu.
        let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), sr.ustawienia(), t("07:00:00"));
        r.lok.znane_zadania.insert(
            "15".into(),
            InfoZadania { id: "15".into(), nazwa: "Montaż".into(), etap: None, czesc: None, szac_min: Some(120) },
        );
        r.start(t("07:00:00"));
        r.tick(t("07:00:06"));
        assert_eq!(r.stan(), Stan::CzekaNaPlan);
        let m = r.migawka();
        assert_eq!(m.w_toku.len(), 1);
        assert_eq!(m.w_toku[0].minuty_razem, 45);
        r.rozpocznij("15", t("07:05:00")).unwrap();
        assert_eq!(r.stan(), Stan::Praca);
    }

    #[test]
    fn brakujace_sciezki_w_migawce() {
        let katalog = tempfile::tempdir().unwrap();
        let mut r = Rdzen::nowy(Magazyn::new(katalog.path().to_path_buf()), Ustawienia::default(), t("09:00:00"));
        r.start(t("09:00:00"));
        assert_eq!(r.migawka().brakujace_sciezki, vec!["folder_planow", "plik_stanu", "folder_dziennika"]);
        assert!(Path::new(&katalog.path().join("stan.json")).exists());
    }
}
