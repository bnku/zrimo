import type { ViewerLocale, ViewerTranslations } from "./contracts.js";

export const englishTranslations: ViewerTranslations = Object.freeze({
  previous: "Previous",
  next: "Next",
  page: "Page",
  of: "of",
  zoomIn: "Zoom in",
  zoomOut: "Zoom out",
  fitWidth: "Fit width",
  fitPage: "Fit page",
  search: "Search",
  searchPlaceholder: "Search document",
  matches: "matches",
  thumbnails: "Thumbnails",
  fullscreen: "Fullscreen",
  exitFullscreen: "Exit fullscreen",
  download: "Download original",
  sheets: "Sheets",
  selectedRange: "Selected range",
  loading: "Loading…",
  close: "Close",
  noMatches: "No matches",
});

export const russianTranslations: ViewerTranslations = Object.freeze({
  previous: "Назад",
  next: "Вперёд",
  page: "Страница",
  of: "из",
  zoomIn: "Увеличить",
  zoomOut: "Уменьшить",
  fitWidth: "По ширине",
  fitPage: "Вся страница",
  search: "Поиск",
  searchPlaceholder: "Найти в документе",
  matches: "совпадений",
  thumbnails: "Миниатюры",
  fullscreen: "На весь экран",
  exitFullscreen: "Выйти из полноэкранного режима",
  download: "Скачать оригинал",
  sheets: "Листы",
  selectedRange: "Выбранный диапазон",
  loading: "Загрузка…",
  close: "Закрыть",
  noMatches: "Совпадений нет",
});

const dictionaries: Readonly<Record<ViewerLocale, ViewerTranslations>> = {
  en: englishTranslations,
  ru: russianTranslations,
};

export function resolveTranslations(
  locale: ViewerLocale | undefined,
  overrides: Partial<ViewerTranslations> | undefined,
): ViewerTranslations {
  return Object.freeze({
    ...englishTranslations,
    ...(locale ? dictionaries[locale] : undefined),
    ...overrides,
  });
}
