/** SQLite data types grouped by category, for the column type combobox. */
export const SQLITE_TYPE_GROUPS: { label: string; types: string[] }[] = [
  {
    label: 'Numeric',
    types: ['integer', 'real', 'numeric'],
  },
  {
    label: 'Text',
    types: ['text', 'varchar', 'char'],
  },
  {
    label: 'Binary',
    types: ['blob'],
  },
];

/** Flat list of all common SQLite types. */
export const SQLITE_TYPES: string[] = SQLITE_TYPE_GROUPS.flatMap(g => g.types);

/** Types that accept a length parameter in SQLite. */
export const SQLITE_PRECISION_TYPES: Record<string, string> = {
  'varchar': '255',
  'char': '1',
};
