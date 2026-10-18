// Currencies offered in selects: a curated ISO 4217 list; an unknown current value is kept
// as an extra option so stored data always round-trips.
export const CURRENCIES: [code: string, name: string][] = [
  ["EUR", "Euro"], ["USD", "US dollar"], ["GBP", "Pound sterling"], ["CHF", "Swiss franc"],
  ["PLN", "Polish złoty"], ["CZK", "Czech koruna"], ["SEK", "Swedish krona"], ["NOK", "Norwegian krone"], ["DKK", "Danish krone"],
  ["RUB", "Russian ruble"], ["KZT", "Kazakhstani tenge"], ["UAH", "Ukrainian hryvnia"], ["TRY", "Turkish lira"], ["GEL", "Georgian lari"], ["AMD", "Armenian dram"], ["AZN", "Azerbaijani manat"], ["UZS", "Uzbek som"], ["KGS", "Kyrgyz som"], ["BYN", "Belarusian ruble"],
  ["AED", "UAE dirham"], ["SAR", "Saudi riyal"], ["ILS", "Israeli new shekel"], ["EGP", "Egyptian pound"], ["KWD", "Kuwaiti dinar"], ["BHD", "Bahraini dinar"], ["JOD", "Jordanian dinar"], ["OMR", "Omani rial"],
  ["INR", "Indian rupee"], ["CNY", "Chinese yuan"], ["JPY", "Japanese yen"], ["KRW", "South Korean won"], ["THB", "Thai baht"], ["VND", "Vietnamese dong"], ["IDR", "Indonesian rupiah"], ["MYR", "Malaysian ringgit"], ["SGD", "Singapore dollar"], ["HKD", "Hong Kong dollar"],
  ["AUD", "Australian dollar"], ["NZD", "New Zealand dollar"], ["CAD", "Canadian dollar"], ["MXN", "Mexican peso"], ["BRL", "Brazilian real"], ["ARS", "Argentine peso"], ["CLP", "Chilean peso"], ["ZAR", "South African rand"],
];

export function currencyOptions(current: string): [code: string, name: string][] {
  const c = current.trim().toUpperCase();
  return c && !CURRENCIES.some(([code]) => code === c) ? [[c, "as stored"], ...CURRENCIES] : CURRENCIES;
}
