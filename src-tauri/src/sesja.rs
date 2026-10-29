//! Sesja pracy (SPEC.md, B5): jedno zadanie i lista odcinków aktywnej pracy.
//! Licznik to suma odcinków, więc pauzy, blokada i uśpienie po prostu
//! zamykają bieżący odcinek.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::czas::{self, Chwila};
use crate::plan::ZadaniePlanu;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfoZadania {
    pub id: String,
    pub nazwa: String,
    #[serde(default)]
    pub etap: Option<String>,
    #[serde(default)]
    pub czesc: Option<String>,
    #[serde(default)]
    pub szac_min: Option<u32>,
}

impl InfoZadania {
    /// Nazwa z dzisiejszym fragmentem: `Montaż: plansze 1–5`.
    pub fn pelna_nazwa(&self) -> String {
        match self.czesc.as_deref().map(str::trim) {
            Some(czesc) if !czesc.is_empty() => format!("{}: {czesc}", self.nazwa),
            _ => self.nazwa.clone(),
        }
    }
}

impl From<&ZadaniePlanu> for InfoZadania {
    fn from(z: &ZadaniePlanu) -> Self {
        InfoZadania {
            id: z.id.clone(),
            nazwa: z.nazwa.clone(),
            etap: z.etap.clone(),
            czesc: z.czesc.clone(),
            szac_min: z.szac_min,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Odcinek {
    pub od: Chwila,
    #[serde(rename = "do")]
    pub do_: Option<Chwila>,
}

impl Odcinek {
    fn sekundy(&self, teraz: Chwila) -> i64 {
        let koniec = self.do_.unwrap_or(teraz);
        (koniec - self.od).num_seconds().max(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowodPauzy {
    Reczna,
    Blokada,
    Uspienie,
    Zamkniecie,
    Awaria,
}

impl PowodPauzy {
    pub fn automatyczna(self) -> bool {
        !matches!(self, PowodPauzy::Reczna)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pauza {
    pub od: Chwila,
    pub powod: PowodPauzy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sesja {
    pub zadanie: InfoZadania,
    /// Dzień, do którego należy sesja: ten, w którym się zaczęła (także po północy).
    pub dzien: NaiveDate,
    pub start: Chwila,
    pub odcinki: Vec<Odcinek>,
    #[serde(default)]
    pub pauza: Option<Pauza>,
    /// Czas ostatniej aktywności, gdy wisi pytanie „Jesteś?”.
    #[serde(default)]
    pub bezczynna_od: Option<Chwila>,
    /// Ile przypomnień o przerwie padło w bieżącym odcinku.
    #[serde(default)]
    pub przypomnienia: u32,
}

impl Sesja {
    pub fn nowa(zadanie: InfoZadania, teraz: Chwila) -> Sesja {
        Sesja {
            zadanie,
            dzien: czas::dzien(teraz),
            start: teraz,
            odcinki: vec![Odcinek { od: teraz, do_: None }],
            pauza: None,
            bezczynna_od: None,
            przypomnienia: 0,
        }
    }

    pub fn trwa(&self) -> bool {
        self.odcinki.last().is_some_and(|o| o.do_.is_none())
    }

    pub fn otwarty_od(&self) -> Option<Chwila> {
        self.odcinki.last().filter(|o| o.do_.is_none()).map(|o| o.od)
    }

    pub fn sekundy_zamkniete(&self) -> i64 {
        self.odcinki.iter().filter(|o| o.do_.is_some()).map(|o| o.sekundy(o.od)).sum()
    }

    /// Czas aktywnej pracy, bez pauz.
    pub fn sekundy(&self, teraz: Chwila) -> i64 {
        self.odcinki.iter().map(|o| o.sekundy(teraz)).sum()
    }

    /// Czas ciągłej pracy w bieżącym odcinku, dla przypomnienia o przerwie.
    pub fn sekundy_odcinka(&self, teraz: Chwila) -> i64 {
        self.odcinki.last().filter(|o| o.do_.is_none()).map_or(0, |o| o.sekundy(teraz))
    }

    pub fn minuty(&self, teraz: Chwila) -> u32 {
        czas::sekundy_na_minuty(self.sekundy(teraz))
    }

    /// Koniec sesji do dziennika: koniec ostatniego odcinka.
    pub fn koniec(&self) -> Chwila {
        self.odcinki.iter().rev().find_map(|o| o.do_).unwrap_or(self.start)
    }

    /// Zamyka otwarty odcinek w chwili `kiedy` (nie wcześniej niż jego początek).
    fn zamknij_odcinek(&mut self, kiedy: Chwila) -> Option<Chwila> {
        let odcinek = self.odcinki.last_mut().filter(|o| o.do_.is_none())?;
        let koniec = kiedy.max(odcinek.od);
        odcinek.do_ = Some(koniec);
        if koniec == odcinek.od && self.odcinki.len() > 1 {
            self.odcinki.pop();
        }
        Some(koniec)
    }

    pub fn pauzuj(&mut self, kiedy: Chwila, powod: PowodPauzy) {
        if self.pauza.is_some() {
            return;
        }
        let od = self.zamknij_odcinek(kiedy).unwrap_or(kiedy);
        self.pauza = Some(Pauza { od, powod });
        self.bezczynna_od = None;
    }

    pub fn wznow(&mut self, teraz: Chwila) {
        if self.pauza.take().is_some() || !self.trwa() {
            self.odcinki.push(Odcinek { od: teraz, do_: None });
        }
        self.bezczynna_od = None;
        self.przypomnienia = 0;
    }

    /// Odpowiedź „Przerwa” na „Jesteś?”: czas od ostatniej aktywności nie
    /// jest liczony, a praca trwa dalej od teraz.
    pub fn odejmij_bezczynnosc(&mut self, teraz: Chwila) {
        let Some(od) = self.bezczynna_od.take() else { return };
        if self.zamknij_odcinek(od).is_some() {
            self.odcinki.push(Odcinek { od: teraz, do_: None });
            self.przypomnienia = 0;
        }
    }

    /// Zamyka sesję przed zapisem do dziennika.
    pub fn zakoncz(&mut self, teraz: Chwila) {
        self.zamknij_odcinek(teraz);
        self.pauza = None;
        self.bezczynna_od = None;
    }

    /// Po awarii albo wyłączeniu komputera: czas liczy się do ostatniego
    /// zapisu, a sesja wraca zapauzowana (B5, „Trwałość”).
    pub fn odtworz(&mut self, ostatni_zapis: Chwila) {
        if self.trwa() && self.pauza.is_none() {
            self.pauzuj(ostatni_zapis, PowodPauzy::Awaria);
        }
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::czas::chwila;

    fn zadanie() -> InfoZadania {
        InfoZadania {
            id: "15".into(),
            nazwa: "Montaż".into(),
            etap: Some("Montaż".into()),
            czesc: Some("plansze 1–5".into()),
            szac_min: Some(120),
        }
    }

    #[test]
    fn liczy_tylko_aktywna_prace() {
        let mut s = Sesja::nowa(zadanie(), chwila("2026-10-30T10:00:00+01:00"));
        assert_eq!(s.sekundy(chwila("2026-10-30T10:10:00+01:00")), 600);
        s.pauzuj(chwila("2026-10-30T10:20:00+01:00"), PowodPauzy::Reczna);
        assert_eq!(s.sekundy(chwila("2026-10-30T11:00:00+01:00")), 1200);
        s.wznow(chwila("2026-10-30T10:50:00+01:00"));
        assert_eq!(s.sekundy(chwila("2026-10-30T11:05:00+01:00")), 1200 + 900);
        assert_eq!(s.sekundy_odcinka(chwila("2026-10-30T11:05:00+01:00")), 900);
        s.zakoncz(chwila("2026-10-30T11:05:00+01:00"));
        assert_eq!(s.minuty(chwila("2026-10-30T23:00:00+01:00")), 35);
        assert_eq!(s.koniec(), chwila("2026-10-30T11:05:00+01:00"));
        assert_eq!(s.zadanie.pelna_nazwa(), "Montaż: plansze 1–5");
    }

    #[test]
    fn druga_pauza_nie_przesuwa_pierwszej() {
        let mut s = Sesja::nowa(zadanie(), chwila("2026-10-30T10:00:00+01:00"));
        s.pauzuj(chwila("2026-10-30T10:30:00+01:00"), PowodPauzy::Reczna);
        s.pauzuj(chwila("2026-10-30T10:40:00+01:00"), PowodPauzy::Blokada);
        let pauza = s.pauza.clone().unwrap();
        assert_eq!(pauza.od, chwila("2026-10-30T10:30:00+01:00"));
        assert_eq!(pauza.powod, PowodPauzy::Reczna);
    }

    #[test]
    fn przerwa_odejmuje_czas_od_ostatniej_aktywnosci() {
        let mut s = Sesja::nowa(zadanie(), chwila("2026-10-30T10:00:00+01:00"));
        s.bezczynna_od = Some(chwila("2026-10-30T10:40:00+01:00"));
        s.odejmij_bezczynnosc(chwila("2026-10-30T11:00:00+01:00"));
        assert!(s.trwa());
        assert_eq!(s.sekundy(chwila("2026-10-30T11:10:00+01:00")), 40 * 60 + 10 * 60);
        assert_eq!(s.bezczynna_od, None);
    }

    #[test]
    fn pracowalem_zostawia_czas() {
        let mut s = Sesja::nowa(zadanie(), chwila("2026-10-30T10:00:00+01:00"));
        s.bezczynna_od = Some(chwila("2026-10-30T10:40:00+01:00"));
        s.bezczynna_od = None;
        assert_eq!(s.sekundy(chwila("2026-10-30T11:00:00+01:00")), 3600);
    }

    #[test]
    fn odtworzenie_po_awarii_liczy_do_ostatniego_zapisu() {
        let mut s = Sesja::nowa(zadanie(), chwila("2026-10-30T10:00:00+01:00"));
        let json = serde_json::to_string(&s).unwrap();
        let mut odtworzona: Sesja = serde_json::from_str(&json).unwrap();
        assert_eq!(odtworzona, s);
        odtworzona.odtworz(chwila("2026-10-30T10:29:30+01:00"));
        assert_eq!(odtworzona.pauza.as_ref().unwrap().powod, PowodPauzy::Awaria);
        assert_eq!(odtworzona.sekundy(chwila("2026-10-30T14:00:00+01:00")), 29 * 60 + 30);
        // Sesja zapauzowana przed zamknięciem zostaje bez zmian.
        s.pauzuj(chwila("2026-10-30T10:10:00+01:00"), PowodPauzy::Zamkniecie);
        let przed = s.clone();
        s.odtworz(chwila("2026-10-30T10:29:30+01:00"));
        assert_eq!(s, przed);
    }

    #[test]
    fn pauza_przed_poczatkiem_odcinka_nie_daje_ujemnego_czasu() {
        let mut s = Sesja::nowa(zadanie(), chwila("2026-10-30T10:00:00+01:00"));
        s.pauzuj(chwila("2026-10-30T09:59:00+01:00"), PowodPauzy::Uspienie);
        assert_eq!(s.sekundy(chwila("2026-10-30T12:00:00+01:00")), 0);
        s.wznow(chwila("2026-10-30T10:05:00+01:00"));
        assert_eq!(s.sekundy(chwila("2026-10-30T10:06:00+01:00")), 60);
    }
}
