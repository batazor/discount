import { describe, expect, it } from "vitest";
import { formatAmount, formatAmountInput, formatBps, formatPercentInput, parseAmount, parsePercent } from "./units";

describe("spec admin-ui: human units", () => {
  it("Fixed amount in minor units: 5.00 EUR → 500", () => {
    expect(parseAmount("5.00", "EUR")).toBe(500);
    expect(parseAmount("5", "EUR")).toBe(500);
    expect(parseAmount("0.5", "EUR")).toBe(50);
    expect(parseAmount("5,25", "EUR")).toBe(525);
  });
  it("Currency without decimals: 500 JPY → 500, 500.5 JPY refused", () => {
    expect(parseAmount("500", "JPY")).toBe(500);
    expect(parseAmount("500.5", "JPY")).toBeUndefined();
    expect(parseAmount("5.123", "EUR")).toBeUndefined();
    expect(parseAmount("abc", "EUR")).toBeUndefined();
  });
  it("Three-decimal currency: 1500 KWD minor units reads 1.500 KWD", () => {
    expect(formatAmount(1500, "KWD")).toBe("1.500 KWD");
    expect(formatAmount(500, "EUR")).toBe("5.00 EUR");
    expect(formatAmount(7, "EUR")).toBe("0.07 EUR");
    expect(formatAmount(500, "JPY")).toBe("500 JPY");
    expect(formatAmountInput(0, "EUR")).toBe("0.00");
    expect(formatAmount(1234, "XYZ")).toBe("12.34 XYZ");
  });
  it("Percent to basis points: 12.5 → 1250, 12.345 refused, over 100 refused", () => {
    expect(parsePercent("12.5")).toBe(1250);
    expect(parsePercent("10")).toBe(1000);
    expect(parsePercent("100")).toBe(10000);
    expect(parsePercent("12.345")).toBeUndefined();
    expect(parsePercent("101")).toBeUndefined();
  });
  it("formats basis points back", () => {
    expect(formatBps(1250)).toBe("12.5 %");
    expect(formatBps(1000)).toBe("10 %");
    expect(formatBps(1)).toBe("0.01 %");
    expect(formatPercentInput(1000)).toBe("10");
  });
});
