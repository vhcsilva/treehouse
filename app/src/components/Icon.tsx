const PATHS = {
  branch: "M6 3v12M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 9a9 9 0 0 1-9 9",
  grid: "M3 3h7v7H3zM14 3h7v7h-7zM14 14h7v7h-7zM3 14h7v7H3z",
  sliders: "M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-3.5-3.5",
  plus: "M12 5v14M5 12h14",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  chevronDown: "m6 9 6 6 6-6",
  chevronRight: "m9 6 6 6-6 6",
  split: "M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zM12 4v16",
  code: "m16 18 6-6-6-6M8 6l-6 6 6 6",
  sync: "M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5",
  close: "M18 6 6 18M6 6l12 12",
  columns: "M4 4h7v16H4zM13 4h7v16h-7z",
  gridLayout: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h16v7H4z",
  tabs: "M3 8h18v12H3zM3 8V5h6v3M11 8V5h6v3",
  maximize: "M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7",
  restore: "M4 14h6v6M20 10h-6V4M14 10l7-7M3 21l7-7",
  logo: "M6 3a2 2 0 1 0 0 4 2 2 0 0 0 0-4zM6 17a2 2 0 1 0 0 4 2 2 0 0 0 0-4zM18 7a2 2 0 1 0 0 4 2 2 0 0 0 0-4zM6 7v10M18 11c0 4-6 3-12 6",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 16, stroke = 2 }: { name: IconName; size?: number; stroke?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={stroke} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={PATHS[name]} />
    </svg>
  );
}
