// The configuration in effect as groups of key/value rows (spec web-ui-shell AC-12). The
// server masks secrets before they reach the browser; this only arranges the values.

export type ConfigRow = { readonly key: string; readonly value: string };
export type ConfigGroup = { readonly title: string; readonly rows: readonly ConfigRow[] };

/** The title of the group for top-level values that are no table. */
export const GENERAL = 'Allgemein';

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** One value as German text: "nicht gesetzt", "ja"/"nein", "keine", lists joined. */
export function valueText(value: unknown): string {
  if (value === null || value === undefined) return 'nicht gesetzt';
  if (typeof value === 'boolean') return value ? 'ja' : 'nein';
  if (typeof value === 'string') return value === '' ? '(leer)' : value;
  if (typeof value === 'number' || typeof value === 'bigint') return String(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return 'keine';
    return value
      .map((item) => (isRecord(item) ? JSON.stringify(item) : valueText(item)))
      .join(', ');
  }
  return JSON.stringify(value);
}

/** Rows of a table; nested tables become dotted keys (`a.b`). */
function rowsOf(table: Readonly<Record<string, unknown>>, prefix: string): ConfigRow[] {
  return Object.entries(table).flatMap(([key, value]) => {
    const name = prefix === '' ? key : `${prefix}.${key}`;
    return isRecord(value) ? rowsOf(value, name) : [{ key: name, value: valueText(value) }];
  });
}

function isTableList(value: unknown): value is readonly Readonly<Record<string, unknown>>[] {
  return Array.isArray(value) && value.length > 0 && value.every(isRecord);
}

/**
 * Groups like the sections of `sofarr.toml`: one per table, one per entry of a list of
 * tables (`[[indexer]]` → "indexer 1", "indexer 2"), and the remaining top-level values
 * under [`GENERAL`]. Keys keep the order of the answer.
 */
export function configGroups(config: Readonly<Record<string, unknown>>): ConfigGroup[] {
  const general: ConfigRow[] = [];
  const groups: ConfigGroup[] = [];
  for (const [key, value] of Object.entries(config)) {
    if (isRecord(value)) {
      groups.push({ title: key, rows: rowsOf(value, '') });
    } else if (isTableList(value)) {
      value.forEach((entry, i) =>
        groups.push({ title: `${key} ${i + 1}`, rows: rowsOf(entry, '') }),
      );
    } else {
      general.push({ key, value: valueText(value) });
    }
  }
  return general.length > 0 ? [{ title: GENERAL, rows: general }, ...groups] : groups;
}
