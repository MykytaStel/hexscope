// The core owns finding codes; this table gives its adapters the same words.
import source from "../../../crates/hexscope-core/src/finding-kinds.tsv?raw";

interface FindingKindWords {
  code: string;
  label?: string;
  category?: string;
  reveal?: string;
  categoryOrder?: number;
}

const FINDING_KINDS: FindingKindWords[] = source
  .split(/\r?\n/)
  .filter((line) => line.length > 0 && !line.startsWith("#"))
  .map((line) => {
    const columns = line.split("\t");
    const [code, label = "", category = "", reveal = "", order = ""] = columns;
    if (!code || columns.length > 5) {
      throw new Error("Invalid finding kind row in the core catalogue");
    }
    const categoryOrder = order === "" ? undefined : Number(order);
    if (categoryOrder !== undefined && !Number.isInteger(categoryOrder)) {
      throw new Error("Invalid finding category order in the core catalogue");
    }
    return {
      code,
      ...(label ? { label } : {}),
      ...(category ? { category } : {}),
      ...(reveal ? { reveal } : {}),
      ...(categoryOrder !== undefined ? { categoryOrder } : {}),
    };
  });

export const FACT_LABELS: Record<string, string> = Object.fromEntries(
  FINDING_KINDS.filter((kind) => kind.label).map((kind) => [kind.code, kind.label!]),
);

export const CATEGORIES: [string, string][] = FINDING_KINDS
  .filter((kind) => kind.category && kind.categoryOrder !== undefined)
  .sort((left, right) => left.categoryOrder! - right.categoryOrder!)
  .map((kind): [string, string] => [kind.code, kind.category!]);

export const REVEALS: Record<string, string> = Object.fromEntries(
  FINDING_KINDS.filter((kind) => kind.reveal).map((kind) => [kind.code, kind.reveal!]),
);
