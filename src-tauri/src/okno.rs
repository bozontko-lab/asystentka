//! Położenie okna (SPEC.md, B6): prawy dolny róg obszaru roboczego monitora
//! głównego, 16 px od krawędzi. Okno zmienia rozmiar ze stanem, a jego dolna
//! krawędź i bok, przy którym stoi, zostają na miejscu.

use crate::magazyn::{KotwicaOkna, Strona};

/// Obszar roboczy monitora (bez paska zadań) w pikselach fizycznych.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obszar {
    pub x: i32,
    pub y: i32,
    pub szer: u32,
    pub wys: u32,
    pub skala: f64,
}

/// Prostokąt okna w pikselach fizycznych.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prostokat {
    pub x: i32,
    pub y: i32,
    pub szer: u32,
    pub wys: u32,
}

impl Obszar {
    fn prawa(&self) -> i32 {
        self.x + self.szer as i32
    }

    fn dol(&self) -> i32 {
        self.y + self.wys as i32
    }
}

/// Rozmiar treści z frontendu (piksele logiczne) w pikselach fizycznych.
pub fn rozmiar_fizyczny(szer: f64, wys: f64, skala: f64) -> (u32, u32) {
    let przelicz = |v: f64| (v.max(1.0) * skala).ceil() as u32;
    (przelicz(szer), przelicz(wys))
}

pub fn umiesc(kotwica: &KotwicaOkna, szer: u32, wys: u32, obszar: &Obszar) -> Prostokat {
    let dx = (kotwica.odstep_x * obszar.skala).round() as i32;
    let dy = (kotwica.odstep_dol * obszar.skala).round() as i32;
    let x = match kotwica.strona {
        Strona::Prawa => obszar.prawa() - dx - szer as i32,
        Strona::Lewa => obszar.x + dx,
    };
    let y = obszar.dol() - dy - wys as i32;
    Prostokat {
        x: x.clamp(obszar.x, (obszar.prawa() - szer as i32).max(obszar.x)),
        y: y.clamp(obszar.y, (obszar.dol() - wys as i32).max(obszar.y)),
        szer,
        wys,
    }
}

/// Kotwica po przeciągnięciu okna: bok według środka okna, odległości od
/// krawędzi w pikselach logicznych.
pub fn kotwica(prostokat: &Prostokat, obszar: &Obszar) -> KotwicaOkna {
    let srodek = prostokat.x + prostokat.szer as i32 / 2;
    let strona = if srodek < obszar.x + obszar.szer as i32 / 2 { Strona::Lewa } else { Strona::Prawa };
    let odstep_x = match strona {
        Strona::Prawa => obszar.prawa() - (prostokat.x + prostokat.szer as i32),
        Strona::Lewa => prostokat.x - obszar.x,
    };
    let odstep_dol = obszar.dol() - (prostokat.y + prostokat.wys as i32);
    let logiczne = |v: i32| ((v.max(0) as f64 / obszar.skala) * 10.0).round() / 10.0;
    KotwicaOkna { strona, odstep_x: logiczne(odstep_x), odstep_dol: logiczne(odstep_dol) }
}

/// Odległość punktu od prostokąta (0 w środku), w pikselach fizycznych.
pub fn odleglosc(prostokat: &Prostokat, x: i32, y: i32) -> f64 {
    let dx = (prostokat.x - x).max(x - (prostokat.x + prostokat.szer as i32)).max(0);
    let dy = (prostokat.y - y).max(y - (prostokat.y + prostokat.wys as i32)).max(0);
    f64::from(dx).hypot(f64::from(dy))
}

#[cfg(test)]
mod testy {
    use super::*;

    const FULL_HD: Obszar = Obszar { x: 0, y: 0, szer: 1920, wys: 1040, skala: 1.0 };
    const KOMPUTER_150: Obszar = Obszar { x: 0, y: 0, szer: 2560, wys: 1392, skala: 1.5 };

    #[test]
    fn domyslnie_prawy_dolny_rog_16_px_od_krawedzi() {
        let p = umiesc(&KotwicaOkna::default(), 480, 300, &FULL_HD);
        assert_eq!(p, Prostokat { x: 1920 - 16 - 480, y: 1040 - 16 - 300, szer: 480, wys: 300 });
    }

    #[test]
    fn dolna_krawedz_zostaje_przy_zmianie_rozmiaru() {
        let k = KotwicaOkna::default();
        let maly = umiesc(&k, 280, 310, &KOMPUTER_150);
        let duzy = umiesc(&k, 480, 600, &KOMPUTER_150);
        assert_eq!(maly.y + maly.wys as i32, duzy.y + duzy.wys as i32);
        assert_eq!(maly.x + maly.szer as i32, duzy.x + duzy.szer as i32);
        assert_eq!(maly.x + maly.szer as i32, 2560 - 24);
    }

    #[test]
    fn kotwica_po_przeciagnieciu_i_powrot() {
        let p = Prostokat { x: 100, y: 500, szer: 300, wys: 200 };
        let k = kotwica(&p, &KOMPUTER_150);
        assert_eq!(k.strona, Strona::Lewa);
        assert_eq!(umiesc(&k, 300, 200, &KOMPUTER_150), p);
        let p = Prostokat { x: 2000, y: 900, szer: 300, wys: 200 };
        let k = kotwica(&p, &KOMPUTER_150);
        assert_eq!(k.strona, Strona::Prawa);
        assert_eq!(umiesc(&k, 300, 200, &KOMPUTER_150), p);
    }

    #[test]
    fn okno_nie_wychodzi_poza_obszar_roboczy() {
        let k = KotwicaOkna { strona: Strona::Prawa, odstep_x: 5000.0, odstep_dol: -50.0 };
        let p = umiesc(&k, 400, 300, &FULL_HD);
        assert_eq!((p.x, p.y), (0, 1040 - 300));
        let p = umiesc(&KotwicaOkna::default(), 400, 2000, &FULL_HD);
        assert_eq!(p.y, 0);
    }

    #[test]
    fn odleglosc_kursora() {
        let p = Prostokat { x: 100, y: 100, szer: 100, wys: 100 };
        assert_eq!(odleglosc(&p, 150, 150), 0.0);
        assert_eq!(odleglosc(&p, 50, 150), 50.0);
        assert_eq!(odleglosc(&p, 230, 240), 50.0);
    }
}
