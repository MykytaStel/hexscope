import { expect, it } from "vitest";
import { matchLine, summaryLine, TRUNCATED } from "./archive-explorer";
import { translateText } from "./i18n";
import "./i18n-uk";

const uk = (text: string) => translateText(text, "uk");
const n = (value: number) => value.toLocaleString("uk");

it("reads in Ukrainian, numbers in agreement", () => {
  expect(uk(summaryLine(3, 1, 1024, 4096))).toBe("3 записи · 1 тека · стиснуто 1,0 КБ, розпаковано 4,0 КБ");
  expect(uk(summaryLine(1, 0, 10, 20))).toBe("1 запис · стиснуто 10 Б, розпаковано 20 Б");
  expect(uk(summaryLine(100_000, 12, 1, 1))).toBe(`${n(100_000)} записів · 12 тек · стиснуто 1 Б, розпаковано 1 Б`);
  expect(uk(matchLine(0))).toBe("Нічого не знайдено");
  expect(uk(matchLine(1))).toBe("Знайдено 1 запис");
  expect(uk(matchLine(12_345))).toBe(`Знайдено ${n(12_345)} записів`);
  expect(uk("1 entry")).toBe("1 запис");
  expect(uk("3 entries")).toBe("3 записи");
  expect(uk("Up to “docs”")).toBe("Угору до «docs»");
  expect(uk(TRUNCATED)).toBe("Архів містить більше записів: hexscope прочитав перші 100 000.");
});

it("sums an archive up in one line", () => {
  expect(summaryLine(3, 1, 1024, 4096)).toBe("3 entries in 1 folder · 1.0 KB packed, 4.0 KB unpacked");
  expect(summaryLine(1, 0, 10, 20)).toBe("1 entry · 10 B packed, 20 B unpacked");
  expect(summaryLine(100_000, 12, 1, 1)).toBe("100,000 entries in 12 folders · 1 B packed, 1 B unpacked");
});

it("says how many entries a filter found", () => {
  expect(matchLine(0)).toBe("Nothing matches");
  expect(matchLine(1)).toBe("1 entry matches");
  expect(matchLine(12_345)).toBe("12,345 entries match");
});
