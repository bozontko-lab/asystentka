// Konfetti w kolorach palety (B7): miedź, fiolet, zieleń i krem, przez 1,5 s,
// a po ostatnim zadaniu dnia 3 s. Kolory to zmienne CSS z kolory.css.
// Każdy kawałek to element z animacją CSS (transform i opacity).

const KOLORY = ["var(--miedz)", "var(--fiolet)", "var(--zielen)", "var(--tekst)"];

export const KONFETTI_MS = 1500;
export const KONFETTI_DLUGIE_MS = 3000;

export function konfetti(pojemnik: HTMLElement, czasMs: number, ile = 36): void {
  const kawalki: HTMLElement[] = [];
  for (let i = 0; i < ile; i++) {
    const kawalek = document.createElement("i");
    kawalek.className = "konfetto";
    const czas = czasMs * (0.65 + Math.random() * 0.35);
    const ustaw = (nazwa: string, wartosc: string) => kawalek.style.setProperty(nazwa, wartosc);
    ustaw("--x", `${30 + Math.random() * 40}%`);
    ustaw("--dx", `${(Math.random() - 0.5) * 180}px`);
    ustaw("--dy", `${70 + Math.random() * 120}px`);
    ustaw("--obrot", `${(Math.random() - 0.5) * 720}deg`);
    ustaw("--kolor", KOLORY[i % KOLORY.length] ?? "var(--tekst)");
    ustaw("--czas", `${Math.round(czas)}ms`);
    ustaw("--opoznienie", `${Math.round(Math.random() * Math.min(400, czasMs / 4))}ms`);
    kawalki.push(kawalek);
  }
  pojemnik.append(...kawalki);
  window.setTimeout(() => kawalki.forEach((k) => k.remove()), czasMs + 500);
}
