import { describe, expect, it } from "vitest";

import { opisBledu } from "./pola";

describe("opisBledu", () => {
  it("komunikat z backendu, z wyjątku albo domyślny", () => {
    expect(opisBledu("Wskaż plik zadań.")).toBe("Wskaż plik zadań.");
    expect(opisBledu(new Error("brak dostępu"))).toBe("brak dostępu");
    expect(opisBledu(42)).toBe("Nieznany błąd.");
    expect(opisBledu(null, "Nie udało się dopisać zadania.")).toBe("Nie udało się dopisać zadania.");
  });
});
