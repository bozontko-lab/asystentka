import { describe, expect, it } from "vitest";

import { folderNadrzedny, godzinyNaMinuty, minutyNaGodziny, nazwaFolderu, polacz, separator } from "./kreator";

describe("ścieżki w kreatorze", () => {
  it("łączy i dzieli ścieżki Windows i uniksowe", () => {
    expect(polacz("D:\\Second Brain", "Zadania.md")).toBe("D:\\Second Brain\\Zadania.md");
    expect(polacz("D:\\Second Brain\\", "Zadania.md")).toBe("D:\\Second Brain\\Zadania.md");
    expect(polacz("/home/ola/vault", "Zadania.md")).toBe("/home/ola/vault/Zadania.md");
    expect(polacz("", "Zadania.md")).toBe("Zadania.md");
    expect(separator("C:\\a")).toBe("\\");
    expect(folderNadrzedny("D:\\Second Brain\\Zadania.md")).toBe("D:\\Second Brain");
    expect(folderNadrzedny("/home/ola/vault/Zadania.md")).toBe("/home/ola/vault");
    expect(folderNadrzedny("Zadania.md")).toBe("");
    expect(nazwaFolderu("D:\\Second Brain\\")).toBe("Second Brain");
  });
});

describe("cel dzienny w godzinach", () => {
  it("przelicza godziny na minuty i z powrotem", () => {
    expect(godzinyNaMinuty("4")).toBe(240);
    expect(godzinyNaMinuty("3,5")).toBe(210);
    expect(godzinyNaMinuty(" 0.25 ")).toBe(15);
    expect(godzinyNaMinuty("0")).toBeNull();
    expect(godzinyNaMinuty("25")).toBeNull();
    expect(godzinyNaMinuty("abc")).toBeNull();
    expect(minutyNaGodziny(240)).toBe("4");
    expect(minutyNaGodziny(210)).toBe("3,5");
    expect(godzinyNaMinuty(minutyNaGodziny(135))).toBe(135);
  });
});
