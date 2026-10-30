import { describe, expect, it } from "vitest";

import { kombinacjaZeZdarzenia, nazwaKlawisza, skrotDlaCzlowieka } from "./skrot";

const bezModyfikatorow = { ctrlKey: false, altKey: false, shiftKey: false, metaKey: false };

describe("kombinacjaZeZdarzenia", () => {
  it("składa kombinację w kolejności Ctrl, Alt, Shift, Win", () => {
    expect(kombinacjaZeZdarzenia({ ...bezModyfikatorow, code: "Space", shiftKey: true, altKey: true, ctrlKey: true })).toBe(
      "Ctrl+Alt+Shift+Space",
    );
    expect(kombinacjaZeZdarzenia({ ...bezModyfikatorow, code: "F9", ctrlKey: true, shiftKey: true })).toBe("Ctrl+Shift+F9");
    expect(kombinacjaZeZdarzenia({ ...bezModyfikatorow, code: "ArrowUp", metaKey: true })).toBe("Super+ArrowUp");
  });

  it("litery i cyfry są bez przedrostka, jak w parserze skrótów", () => {
    expect(kombinacjaZeZdarzenia({ ...bezModyfikatorow, code: "KeyK", ctrlKey: true })).toBe("Ctrl+K");
    expect(kombinacjaZeZdarzenia({ ...bezModyfikatorow, code: "Digit7", ctrlKey: true, altKey: true })).toBe("Ctrl+Alt+7");
    expect(nazwaKlawisza("Numpad5")).toBe("Numpad5");
    expect(nazwaKlawisza("Backquote")).toBe("Backquote");
  });

  it("sam modyfikator to jeszcze nie skrót", () => {
    for (const code of ["ControlLeft", "AltRight", "ShiftLeft", "MetaLeft"]) {
      expect(kombinacjaZeZdarzenia({ ...bezModyfikatorow, code, ctrlKey: true })).toBeNull();
    }
  });

  it("Ctrl+Alt z literą dochodzi do backendu w tej samej postaci, którą odrzuca (AltGr)", () => {
    expect(kombinacjaZeZdarzenia({ ...bezModyfikatorow, code: "KeyA", ctrlKey: true, altKey: true })).toBe("Ctrl+Alt+A");
  });
});

describe("skrotDlaCzlowieka", () => {
  it("używa polskiej nazwy spacji i klawisza Windows", () => {
    expect(skrotDlaCzlowieka("Ctrl+Alt+Shift+Space")).toBe("Ctrl + Alt + Shift + Spacja");
    expect(skrotDlaCzlowieka("Super+ArrowUp")).toBe("Win + ↑");
    expect(skrotDlaCzlowieka("Ctrl+Shift+F9")).toBe("Ctrl + Shift + F9");
  });
});
