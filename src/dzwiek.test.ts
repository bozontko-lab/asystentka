import { describe, expect, it } from "vitest";

import { DOMYSLNA_GLOSNOSC, glosnoscZPola } from "./dzwiek";
import { opisDzwieku } from "./widok";

describe("glosnoscZPola", () => {
  it("liczba z suwaka zostaje w zakresie od 0 do 100", () => {
    expect(glosnoscZPola("0")).toBe(0);
    expect(glosnoscZPola("25")).toBe(25);
    expect(glosnoscZPola("100")).toBe(100);
    expect(glosnoscZPola("180")).toBe(100);
    expect(glosnoscZPola("-5")).toBe(0);
  });

  it("coś, co nie jest liczbą, daje domyślne 25", () => {
    expect(glosnoscZPola("")).toBe(DOMYSLNA_GLOSNOSC);
    expect(glosnoscZPola("cicho")).toBe(25);
  });
});

describe("opisDzwieku", () => {
  it("mówi, co robi dźwięk i co zrobi kliknięcie, w każdym z czterech stanów", () => {
    expect(opisDzwieku({ wlaczony: true, gra: true, wyciszony_przez_obs: false })).toBe("Dźwięk 40 Hz gra. Kliknij, żeby wyłączyć.");
    expect(opisDzwieku({ wlaczony: false, gra: false, wyciszony_przez_obs: false })).toBe(
      "Dźwięk 40 Hz jest wyłączony. Kliknij, żeby włączyć.",
    );
    expect(opisDzwieku({ wlaczony: true, gra: false, wyciszony_przez_obs: true })).toContain("OBS");
    expect(opisDzwieku({ wlaczony: true, gra: false, wyciszony_przez_obs: false })).toContain("brak urządzenia");
  });
});
