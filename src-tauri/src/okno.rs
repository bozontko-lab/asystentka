//! Położenie okna (SPEC.md, B6): widget stoi na jednym monitorze, przy jednym
//! z czterech rogów jego obszaru roboczego (kotwica). Przy zmianie rozmiaru
//! okna ten róg zostaje na miejscu, a okno rośnie w stronę środka monitora.
//! Wszystko liczy się względem monitora widgetu, nigdy monitora głównego;
//! monitor główny jest tylko miejscem domyślnym.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strona {
    #[default]
    Prawa,
    Lewa,
}

/// Przy której krawędzi monitora stoi widget: od niej panel rośnie w stronę środka.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pion {
    #[default]
    Dol,
    Gora,
}

/// Róg obszaru roboczego monitora, przy którym stoi widget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Naroznik {
    #[default]
    #[serde(rename = "prawy-dolny")]
    PrawyDolny,
    #[serde(rename = "lewy-dolny")]
    LewyDolny,
    #[serde(rename = "prawy-gorny")]
    PrawyGorny,
    #[serde(rename = "lewy-gorny")]
    LewyGorny,
}

impl Naroznik {
    pub fn z(strona: Strona, pion: Pion) -> Naroznik {
        match (strona, pion) {
            (Strona::Prawa, Pion::Dol) => Naroznik::PrawyDolny,
            (Strona::Lewa, Pion::Dol) => Naroznik::LewyDolny,
            (Strona::Prawa, Pion::Gora) => Naroznik::PrawyGorny,
            (Strona::Lewa, Pion::Gora) => Naroznik::LewyGorny,
        }
    }

    pub fn strona(self) -> Strona {
        match self {
            Naroznik::PrawyDolny | Naroznik::PrawyGorny => Strona::Prawa,
            Naroznik::LewyDolny | Naroznik::LewyGorny => Strona::Lewa,
        }
    }

    pub fn pion(self) -> Pion {
        match self {
            Naroznik::PrawyDolny | Naroznik::LewyDolny => Pion::Dol,
            Naroznik::PrawyGorny | Naroznik::LewyGorny => Pion::Gora,
        }
    }
}

/// Monitor widgetu w `okno.json`: nazwa oraz położenie i rozmiar w pikselach
/// fizycznych. Po odłączeniu i podłączeniu Windows potrafi zmienić nazwę
/// (`\\.\DISPLAY2` na `\\.\DISPLAY3`), dlatego monitor szukany jest też po geometrii.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorOkna {
    pub nazwa: String,
    pub x: i32,
    pub y: i32,
    pub szer: u32,
    pub wys: u32,
}

/// Pozycja widgetu: monitor, kotwica i odstępy od rogu w pikselach logicznych.
/// Bez monitora (stary plik, ustawienie domyślne) widget stoi na monitorze głównym.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "ZapisOkna")]
pub struct KotwicaOkna {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor: Option<MonitorOkna>,
    pub kotwica: Naroznik,
    pub odstep_x: f64,
    pub odstep_y: f64,
}

impl Default for KotwicaOkna {
    fn default() -> Self {
        KotwicaOkna {
            monitor: None,
            kotwica: Naroznik::PrawyDolny,
            odstep_x: ODSTEP_DOMYSLNY,
            odstep_y: ODSTEP_DOMYSLNY,
        }
    }
}

/// Domyślny odstęp od krawędzi obszaru roboczego (B6), w pikselach logicznych.
pub const ODSTEP_DOMYSLNY: f64 = 16.0;

/// Wczytywany `okno.json`: nowy format albo ten z wersji 1 i 2 (`strona`,
/// `odstep_dol`), w którym widget zawsze stał przy dolnej krawędzi monitora głównego.
#[derive(Deserialize, Default)]
#[serde(default)]
struct ZapisOkna {
    monitor: Option<MonitorOkna>,
    kotwica: Option<Naroznik>,
    strona: Option<Strona>,
    odstep_x: Option<f64>,
    odstep_y: Option<f64>,
    odstep_dol: Option<f64>,
}

impl From<ZapisOkna> for KotwicaOkna {
    fn from(z: ZapisOkna) -> Self {
        let kotwica = z.kotwica.unwrap_or_else(|| Naroznik::z(z.strona.unwrap_or_default(), Pion::Dol));
        KotwicaOkna {
            monitor: z.monitor,
            kotwica,
            odstep_x: z.odstep_x.unwrap_or(ODSTEP_DOMYSLNY),
            odstep_y: z.odstep_y.or(z.odstep_dol).unwrap_or(ODSTEP_DOMYSLNY),
        }
    }
}

/// Prostokąt w pikselach fizycznych.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prostokat {
    pub x: i32,
    pub y: i32,
    pub szer: u32,
    pub wys: u32,
}

impl Prostokat {
    pub fn prawa(&self) -> i32 {
        self.x + self.szer as i32
    }

    pub fn dol(&self) -> i32 {
        self.y + self.wys as i32
    }

    fn srodek(&self) -> (i32, i32) {
        (self.x + self.szer as i32 / 2, self.y + self.wys as i32 / 2)
    }

    fn zawiera(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.prawa() && y >= self.y && y < self.dol()
    }

    /// Środek tego prostokąta leży w `inny` (np. ten sam monitor widziany przez dwa API).
    pub fn srodek_w(&self, inny: &Prostokat) -> bool {
        let (x, y) = self.srodek();
        inny.zawiera(x, y)
    }
}

/// Obszar roboczy monitora (bez paska zadań) w pikselach fizycznych
/// i skalowanie tego monitora.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obszar {
    pub x: i32,
    pub y: i32,
    pub szer: u32,
    pub wys: u32,
    pub skala: f64,
}

impl Obszar {
    fn prostokat(&self) -> Prostokat {
        Prostokat { x: self.x, y: self.y, szer: self.szer, wys: self.wys }
    }

    fn prawa(&self) -> i32 {
        self.x + self.szer as i32
    }

    fn dol(&self) -> i32 {
        self.y + self.wys as i32
    }
}

/// Monitor widziany przez system.
#[derive(Debug, Clone, PartialEq)]
pub struct Ekran {
    pub nazwa: String,
    /// Cały monitor, razem z paskiem zadań.
    pub pelny: Prostokat,
    pub obszar: Obszar,
    pub glowny: bool,
}

impl Ekran {
    pub fn monitor_okna(&self) -> MonitorOkna {
        MonitorOkna {
            nazwa: self.nazwa.clone(),
            x: self.pelny.x,
            y: self.pelny.y,
            szer: self.pelny.szer,
            wys: self.pelny.wys,
        }
    }
}

/// Monitory z opisu tekstowego, jedna linia na monitor:
/// `nazwa x y szerokość wysokość skalowanie [pasek_zadań_px] [glowny]`.
/// Służy do testów w wersji deweloperskiej (`ASYSTENTKA_MONITORY`), bo wirtualny
/// ekran Xvfb ma tylko jeden monitor; pierwszy monitor jest głównym, gdy żaden
/// nie ma znacznika `glowny`.
#[cfg(any(debug_assertions, test))]
pub fn ekrany_z_opisu(opis: &str) -> Vec<Ekran> {
    let mut ekrany: Vec<Ekran> = opis
        .lines()
        .filter_map(|linia| {
            let pola: Vec<&str> = linia.split_whitespace().collect();
            let [nazwa, x, y, szer, wys, skala, reszta @ ..] = pola.as_slice() else { return None };
            let (x, y, szer, wys): (i32, i32, u32, u32) =
                (x.parse().ok()?, y.parse().ok()?, szer.parse().ok()?, wys.parse().ok()?);
            let pasek: u32 = reszta.iter().find_map(|p| p.parse().ok()).unwrap_or(0);
            Some(Ekran {
                nazwa: (*nazwa).to_string(),
                pelny: Prostokat { x, y, szer, wys },
                obszar: Obszar { x, y, szer, wys: wys.saturating_sub(pasek), skala: skala.parse().ok()? },
                glowny: reszta.contains(&"glowny"),
            })
        })
        .collect();
    if !ekrany.iter().any(|e| e.glowny) {
        if let Some(pierwszy) = ekrany.first_mut() {
            pierwszy.glowny = true;
        }
    }
    ekrany
}

/// Rozmiar treści z frontendu (piksele logiczne) w pikselach fizycznych
/// monitora, na którym stoi okno.
pub fn rozmiar_fizyczny(szer: f64, wys: f64, skala: f64) -> (u32, u32) {
    let przelicz = |v: f64| (v.max(1.0) * skala).ceil() as u32;
    (przelicz(szer), przelicz(wys))
}

/// Gdzie stoi widget: monitor i kotwica, która naprawdę obowiązuje.
#[derive(Debug, Clone, PartialEq)]
pub struct Polozenie {
    pub ekran: Ekran,
    pub kotwica: KotwicaOkna,
    /// Zapisany monitor zniknął, więc widget stoi w domyślnym miejscu na monitorze głównym.
    pub zastepcze: bool,
}

fn glowny(ekrany: &[Ekran]) -> Option<&Ekran> {
    ekrany.iter().find(|e| e.glowny).or_else(|| ekrany.first())
}

/// Zapisany monitor szuka się po nazwie, potem po położeniu i rozmiarze (B6).
fn znajdz<'a>(monitor: &MonitorOkna, ekrany: &'a [Ekran]) -> Option<&'a Ekran> {
    ekrany.iter().find(|e| !monitor.nazwa.is_empty() && e.nazwa == monitor.nazwa).or_else(|| {
        ekrany.iter().find(|e| {
            (e.pelny.x, e.pelny.y, e.pelny.szer, e.pelny.wys) == (monitor.x, monitor.y, monitor.szer, monitor.wys)
        })
    })
}

/// Rozwiązuje zapisaną pozycję na istniejący monitor. Bez zapisanego monitora
/// widget stoi na głównym, ze swoją kotwicą. Gdy zapisanego monitora już nie
/// ma, staje w domyślnym miejscu na głównym, a zapis zostaje nietknięty:
/// po podłączeniu monitora widget wraca tam, gdzie go zostawiłeś.
pub fn polozenie(zapisana: &KotwicaOkna, ekrany: &[Ekran]) -> Option<Polozenie> {
    let wybrany = match &zapisana.monitor {
        Some(monitor) => znajdz(monitor, ekrany),
        None => glowny(ekrany),
    };
    match wybrany {
        Some(ekran) => Some(Polozenie { ekran: ekran.clone(), kotwica: zapisana.clone(), zastepcze: false }),
        None => glowny(ekrany).map(|ekran| Polozenie {
            ekran: ekran.clone(),
            kotwica: KotwicaOkna::default(),
            zastepcze: true,
        }),
    }
}

/// Prostokąt okna przy kotwicy, w obszarze roboczym jego monitora. Okno nie
/// wychodzi poza ten obszar.
pub fn umiesc(kotwica: &KotwicaOkna, szer: u32, wys: u32, obszar: &Obszar) -> Prostokat {
    let dx = (kotwica.odstep_x * obszar.skala).round() as i32;
    let dy = (kotwica.odstep_y * obszar.skala).round() as i32;
    let x = match kotwica.kotwica.strona() {
        Strona::Prawa => obszar.prawa() - dx - szer as i32,
        Strona::Lewa => obszar.x + dx,
    };
    let y = match kotwica.kotwica.pion() {
        Pion::Dol => obszar.dol() - dy - wys as i32,
        Pion::Gora => obszar.y + dy,
    };
    Prostokat {
        x: x.clamp(obszar.x, (obszar.prawa() - szer as i32).max(obszar.x)),
        y: y.clamp(obszar.y, (obszar.dol() - wys as i32).max(obszar.y)),
        szer,
        wys,
    }
}

/// Monitor okna to ten, na którym leży jego środek (B6). Gdy środek wypada
/// poza wszystkimi monitorami, bierze najbliższy.
pub fn ekran_okna<'a>(prostokat: &Prostokat, ekrany: &'a [Ekran]) -> Option<&'a Ekran> {
    let (x, y) = prostokat.srodek();
    ekrany.iter().find(|e| e.pelny.zawiera(x, y)).or_else(|| {
        ekrany.iter().min_by(|a, b| {
            odleglosc(&a.pelny, x, y).partial_cmp(&odleglosc(&b.pelny, x, y)).unwrap_or(std::cmp::Ordering::Equal)
        })
    })
}

/// Kotwica po przeciągnięciu okna: monitor widgetu, róg jego obszaru
/// roboczego najbliższy środkowi okna i odstępy od tego rogu w pikselach
/// logicznych tego monitora.
pub fn kotwica_po_przeciagnieciu(prostokat: &Prostokat, ekrany: &[Ekran]) -> Option<KotwicaOkna> {
    let ekran = ekran_okna(prostokat, ekrany)?;
    let obszar = &ekran.obszar;
    let (sx, sy) = prostokat.srodek();
    let (ox, oy) = obszar.prostokat().srodek();
    let strona = if sx < ox { Strona::Lewa } else { Strona::Prawa };
    let pion = if sy < oy { Pion::Gora } else { Pion::Dol };
    let odstep_x = match strona {
        Strona::Prawa => obszar.prawa() - prostokat.prawa(),
        Strona::Lewa => prostokat.x - obszar.x,
    };
    let odstep_y = match pion {
        Pion::Dol => obszar.dol() - prostokat.dol(),
        Pion::Gora => prostokat.y - obszar.y,
    };
    let logiczne = |v: i32| ((v.max(0) as f64 / obszar.skala) * 10.0).round() / 10.0;
    Some(KotwicaOkna {
        monitor: Some(ekran.monitor_okna()),
        kotwica: Naroznik::z(strona, pion),
        odstep_x: logiczne(odstep_x),
        odstep_y: logiczne(odstep_y),
    })
}

/// Odległość punktu od prostokąta (0 w środku), w pikselach fizycznych.
pub fn odleglosc(prostokat: &Prostokat, x: i32, y: i32) -> f64 {
    let dx = (prostokat.x - x).max(x - prostokat.prawa()).max(0);
    let dy = (prostokat.y - y).max(y - prostokat.dol()).max(0);
    f64::from(dx).hypot(f64::from(dy))
}

/// Czy okno (po przesunięciu przez system, nie przez użytkownika) leży daleko
/// od miejsca, które mu wyznaczyliśmy. Mały rozjazd to zaokrąglenia i
/// obramowanie okna, a nie przeniesienie.
pub fn przeniesione_przez_system(wyznaczone: &Prostokat, rzeczywiste_x: i32, rzeczywiste_y: i32) -> bool {
    (wyznaczone.x - rzeczywiste_x).abs() > 24 || (wyznaczone.y - rzeczywiste_y).abs() > 24
}

#[cfg(test)]
mod testy {
    use super::*;

    fn ekran(nazwa: &str, (x, y, szer, wys): (i32, i32, u32, u32), pasek: u32, skala: f64, glowny: bool) -> Ekran {
        Ekran {
            nazwa: nazwa.to_string(),
            pelny: Prostokat { x, y, szer, wys },
            obszar: Obszar { x, y, szer, wys: wys - pasek, skala },
            glowny,
        }
    }

    /// Główny 1920×1080 (pasek zadań 40 px) i drugi 2560×1440 po jego prawej, ze skalowaniem 150%.
    fn dwa_monitory() -> Vec<Ekran> {
        vec![
            ekran("\\\\.\\DISPLAY1", (0, 0, 1920, 1080), 40, 1.0, true),
            ekran("\\\\.\\DISPLAY2", (1920, 0, 2560, 1440), 60, 1.5, false),
        ]
    }

    fn kotwica(naroznik: Naroznik, x: f64, y: f64) -> KotwicaOkna {
        KotwicaOkna { monitor: None, kotwica: naroznik, odstep_x: x, odstep_y: y }
    }

    #[test]
    fn domyslnie_prawy_dolny_rog_16_px_od_krawedzi() {
        let ekrany = dwa_monitory();
        let p = umiesc(&KotwicaOkna::default(), 480, 300, &ekrany[0].obszar);
        assert_eq!(p, Prostokat { x: 1920 - 16 - 480, y: 1040 - 16 - 300, szer: 480, wys: 300 });
    }

    #[test]
    fn dolna_kotwica_trzyma_dolna_krawedz_a_gorna_gorna() {
        let drugi = &dwa_monitory()[1];
        let dolna = kotwica(Naroznik::PrawyDolny, 16.0, 16.0);
        let maly = umiesc(&dolna, 420, 465, &drugi.obszar);
        let duzy = umiesc(&dolna, 720, 900, &drugi.obszar);
        assert_eq!(maly.dol(), duzy.dol());
        assert_eq!(maly.prawa(), duzy.prawa());
        assert_eq!(maly.prawa(), 1920 + 2560 - 24, "16 px logicznych przy skali 1,5");
        assert!(duzy.y < maly.y, "panel rośnie w górę");

        let gorna = kotwica(Naroznik::LewyGorny, 16.0, 16.0);
        let maly = umiesc(&gorna, 420, 465, &drugi.obszar);
        let duzy = umiesc(&gorna, 720, 900, &drugi.obszar);
        assert_eq!((maly.x, maly.y), (duzy.x, duzy.y));
        assert_eq!((maly.x, maly.y), (1920 + 24, 24));
        assert!(duzy.dol() > maly.dol(), "panel rośnie w dół");
    }

    #[test]
    fn okno_nie_wychodzi_poza_obszar_roboczy_swojego_monitora() {
        let drugi = &dwa_monitory()[1];
        let za_daleko = kotwica(Naroznik::PrawyDolny, 5000.0, -50.0);
        let p = umiesc(&za_daleko, 400, 300, &drugi.obszar);
        assert_eq!((p.x, p.y), (1920, 1440 - 60 - 300));
        let za_wysokie = umiesc(&KotwicaOkna::default(), 400, 5000, &drugi.obszar);
        assert_eq!(za_wysokie.y, 0);
        let w_gorze = umiesc(&kotwica(Naroznik::PrawyGorny, 0.0, 0.0), 400, 300, &drugi.obszar);
        assert_eq!(w_gorze.y, 0, "górna kotwica nie wchodzi nad monitor");
    }

    #[test]
    fn kotwica_po_przeciagnieciu_wybiera_monitor_i_najblizszy_rog() {
        let ekrany = dwa_monitory();
        // lewy górny róg drugiego monitora
        let p = Prostokat { x: 1920 + 60, y: 30, szer: 420, wys: 465 };
        let k = kotwica_po_przeciagnieciu(&p, &ekrany).unwrap();
        assert_eq!(k.kotwica, Naroznik::LewyGorny);
        assert_eq!(k.monitor.as_ref().unwrap().nazwa, "\\\\.\\DISPLAY2");
        assert_eq!((k.odstep_x, k.odstep_y), (40.0, 20.0), "60 i 30 px fizycznych to 40 i 20 logicznych");
        // prawy dolny róg głównego
        let p = Prostokat { x: 1500, y: 700, szer: 400, wys: 300 };
        let k = kotwica_po_przeciagnieciu(&p, &ekrany).unwrap();
        assert_eq!(k.kotwica, Naroznik::PrawyDolny);
        assert_eq!(k.monitor.as_ref().unwrap().nazwa, "\\\\.\\DISPLAY1");
        assert_eq!((k.odstep_x, k.odstep_y), (20.0, 40.0));
    }

    #[test]
    fn kazdy_rog_na_kazdym_monitorze_wraca_w_to_samo_miejsce() {
        let ekrany = dwa_monitory();
        for ekran in &ekrany {
            let o = &ekran.obszar;
            for (x, y) in [
                (o.x + 70, o.y + 50),
                (o.prawa() - 470, o.y + 50),
                (o.x + 70, o.dol() - 400),
                (o.prawa() - 470, o.dol() - 400),
            ] {
                let p = Prostokat { x, y, szer: 400, wys: 350 };
                let k = kotwica_po_przeciagnieciu(&p, &ekrany).unwrap();
                let polozenie = polozenie(&k, &ekrany).unwrap();
                assert!(!polozenie.zastepcze);
                assert_eq!(polozenie.ekran.nazwa, ekran.nazwa);
                let wrocilo = umiesc(&polozenie.kotwica, 400, 350, &polozenie.ekran.obszar);
                assert!((wrocilo.x - p.x).abs() <= 1 && (wrocilo.y - p.y).abs() <= 1, "{p:?} wróciło jako {wrocilo:?}");
            }
        }
    }

    #[test]
    fn monitor_okna_to_ten_ze_srodkiem_okna() {
        let ekrany = dwa_monitory();
        // okno w większości na drugim monitorze, ale częściowo na głównym
        let p = Prostokat { x: 1850, y: 100, szer: 400, wys: 300 };
        assert_eq!(ekran_okna(&p, &ekrany).unwrap().nazwa, "\\\\.\\DISPLAY2");
        let p = Prostokat { x: 1700, y: 100, szer: 400, wys: 300 };
        assert_eq!(ekran_okna(&p, &ekrany).unwrap().nazwa, "\\\\.\\DISPLAY1");
        // środek w dziurze poza monitorami: najbliższy
        let p = Prostokat { x: 100, y: 1200, szer: 400, wys: 300 };
        assert_eq!(ekran_okna(&p, &ekrany).unwrap().nazwa, "\\\\.\\DISPLAY1");
    }

    #[test]
    fn monitor_po_lewej_od_glownego_ma_ujemne_wspolrzedne() {
        let ekrany =
            vec![ekran("L", (-1280, -200, 1280, 1024), 0, 1.0, false), ekran("G", (0, 0, 1920, 1080), 40, 1.0, true)];
        let p = Prostokat { x: -1280 + 16, y: -200 + 16, szer: 300, wys: 200 };
        let k = kotwica_po_przeciagnieciu(&p, &ekrany).unwrap();
        assert_eq!(k.kotwica, Naroznik::LewyGorny);
        assert_eq!(umiesc(&k, 300, 200, &ekrany[0].obszar), p);
    }

    #[test]
    fn zapisany_monitor_jest_szukany_po_nazwie_potem_po_geometrii() {
        let mut ekrany = dwa_monitory();
        let zapis = KotwicaOkna {
            monitor: Some(ekrany[1].monitor_okna()),
            kotwica: Naroznik::PrawyGorny,
            odstep_x: 10.0,
            odstep_y: 12.0,
        };
        assert_eq!(polozenie(&zapis, &ekrany).unwrap().ekran.nazwa, "\\\\.\\DISPLAY2");
        // Windows nadał monitorowi inną nazwę, ale stoi tam, gdzie stał
        ekrany[1].nazwa = "\\\\.\\DISPLAY5".to_string();
        let p = polozenie(&zapis, &ekrany).unwrap();
        assert_eq!(p.ekran.nazwa, "\\\\.\\DISPLAY5");
        assert!(!p.zastepcze);
        // inny monitor o tej samej nazwie, ale gdzie indziej, wciąż jest „tym” monitorem
        ekrany[1].nazwa = "\\\\.\\DISPLAY2".to_string();
        ekrany[1].pelny.x = 3000;
        assert_eq!(polozenie(&zapis, &ekrany).unwrap().ekran.pelny.x, 3000);
    }

    #[test]
    fn bez_zapisanego_monitora_widget_stoi_na_glownym_ze_swoja_kotwica() {
        let ekrany = vec![ekran("A", (0, 0, 800, 600), 0, 1.0, false), ekran("B", (800, 0, 1920, 1080), 0, 1.0, true)];
        let zapis = kotwica(Naroznik::LewyGorny, 30.0, 40.0);
        let p = polozenie(&zapis, &ekrany).unwrap();
        assert_eq!(p.ekran.nazwa, "B");
        assert_eq!(p.kotwica, zapis);
        assert!(!p.zastepcze);
    }

    #[test]
    fn po_odlaczeniu_monitora_widget_staje_w_domyslnym_miejscu_na_glownym() {
        let ekrany = dwa_monitory();
        let zapis = KotwicaOkna {
            monitor: Some(ekrany[1].monitor_okna()),
            kotwica: Naroznik::LewyGorny,
            odstep_x: 100.0,
            odstep_y: 100.0,
        };
        let tylko_glowny = vec![ekrany[0].clone()];
        let p = polozenie(&zapis, &tylko_glowny).unwrap();
        assert!(p.zastepcze);
        assert_eq!(p.ekran.nazwa, "\\\\.\\DISPLAY1");
        assert_eq!(p.kotwica, KotwicaOkna::default());
        let gdzie = umiesc(&p.kotwica, 420, 300, &p.ekran.obszar);
        assert_eq!((gdzie.prawa(), gdzie.dol()), (1920 - 16, 1040 - 16));
        // po podłączeniu monitora widget wraca na swoje miejsce
        let znowu = polozenie(&zapis, &ekrany).unwrap();
        assert!(!znowu.zastepcze);
        assert_eq!(znowu.ekran.nazwa, "\\\\.\\DISPLAY2");
    }

    #[test]
    fn bez_monitorow_nie_ma_polozenia() {
        assert!(polozenie(&KotwicaOkna::default(), &[]).is_none());
        assert!(kotwica_po_przeciagnieciu(&Prostokat { x: 0, y: 0, szer: 10, wys: 10 }, &[]).is_none());
    }

    #[test]
    fn strona_i_pion_wynikaja_z_rogu() {
        assert_eq!(Naroznik::PrawyGorny.strona(), Strona::Prawa);
        assert_eq!(Naroznik::PrawyGorny.pion(), Pion::Gora);
        assert_eq!(Naroznik::LewyDolny.strona(), Strona::Lewa);
        assert_eq!(Naroznik::LewyDolny.pion(), Pion::Dol);
        for n in [Naroznik::PrawyDolny, Naroznik::LewyDolny, Naroznik::PrawyGorny, Naroznik::LewyGorny] {
            assert_eq!(Naroznik::z(n.strona(), n.pion()), n);
        }
    }

    #[test]
    fn okno_json_ma_nazwy_rogow_ze_specyfikacji() {
        let k = KotwicaOkna {
            monitor: Some(MonitorOkna { nazwa: "\\\\.\\DISPLAY2".into(), x: 1920, y: 0, szer: 2560, wys: 1440 }),
            kotwica: Naroznik::LewyGorny,
            odstep_x: 16.0,
            odstep_y: 20.5,
        };
        let json = serde_json::to_string(&k).unwrap();
        assert!(json.contains(r#""kotwica":"lewy-gorny""#), "{json}");
        assert!(
            json.contains(r#""monitor":{"nazwa":"\\\\.\\DISPLAY2","x":1920,"y":0,"szer":2560,"wys":1440}"#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<KotwicaOkna>(&json).unwrap(), k);
    }

    #[test]
    fn okno_json_z_wersji_1_i_2_jest_przejmowany() {
        let k: KotwicaOkna =
            serde_json::from_str(r#"{"strona": "lewa", "odstep_x": 30.0, "odstep_dol": 20.0}"#).unwrap();
        assert_eq!(k, KotwicaOkna { monitor: None, kotwica: Naroznik::LewyDolny, odstep_x: 30.0, odstep_y: 20.0 });
        let domyslna: KotwicaOkna = serde_json::from_str("{}").unwrap();
        assert_eq!(domyslna, KotwicaOkna::default());
    }

    #[test]
    fn monitory_z_opisu_tekstowego() {
        let ekrany = ekrany_z_opisu("L 0 0 1600 1000 1.0\nP 1600 0 1920 1080 1.5 40 glowny\nzepsuta linia\n");
        assert_eq!(ekrany.len(), 2);
        assert_eq!((ekrany[0].nazwa.as_str(), ekrany[0].glowny), ("L", false));
        assert_eq!((ekrany[1].nazwa.as_str(), ekrany[1].glowny), ("P", true));
        assert_eq!(ekrany[1].obszar, Obszar { x: 1600, y: 0, szer: 1920, wys: 1040, skala: 1.5 });
        assert_eq!(ekrany[1].pelny.wys, 1080);
        assert!(ekrany_z_opisu("L 0 0 1600 1000 1.0")[0].glowny, "pierwszy jest głównym, gdy żaden nie jest oznaczony");
        assert!(ekrany_z_opisu("").is_empty());
    }

    #[test]
    fn odleglosc_kursora() {
        let p = Prostokat { x: 100, y: 100, szer: 100, wys: 100 };
        assert_eq!(odleglosc(&p, 150, 150), 0.0);
        assert_eq!(odleglosc(&p, 50, 150), 50.0);
        assert_eq!(odleglosc(&p, 230, 240), 50.0);
    }

    #[test]
    fn przeniesienie_przez_system_to_duzy_rozjazd() {
        let p = Prostokat { x: 1000, y: 500, szer: 400, wys: 300 };
        assert!(!przeniesione_przez_system(&p, 1002, 498));
        assert!(!przeniesione_przez_system(&p, 1010, 520));
        assert!(przeniesione_przez_system(&p, 100, 500));
        assert!(przeniesione_przez_system(&p, 1000, 900));
    }
}
