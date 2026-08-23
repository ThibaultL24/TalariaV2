// web/src/lib/i18n.ts
import { useLocaleStore, type AppLocale } from "@/stores/locale-store";

export type { AppLocale };

export interface AppMessages {
  productName: string;
  productSubtitle: string;
  search: string;
  searchPlaceholder: string;
  searchHint: string;
  searchInLibrary: string;
  searchNew: string;
  loading: string;
  loadingMap: string;
  noResults: string;
  emptySearch: string;
  close: string;
  closeDetail: string;
  summary: string;
  sources: string;
  noSources: string;
  openSource: string;
  openParagraph: string;
  untilYear: string;
  eventsVisible: (visible: number, total: number) => string;
  legendTitle: string;
  personSearch: string;
  imageUnavailable: string;
}

const EN: AppMessages = {
  productName: "Talaria",
  productSubtitle: "Life geography",
  search: "Search",
  searchPlaceholder: "Search a historical figure…",
  searchHint: "Choose a person to place their life on the map.",
  searchInLibrary: "In library",
  searchNew: "New",
  loading: "Loading…",
  loadingMap: "Placing events on the map…",
  noResults: "No matches.",
  emptySearch: "Search a historical figure to see their life on the map.",
  close: "Close",
  closeDetail: "Close event",
  summary: "Summary",
  sources: "Sources",
  noSources: "No sources for this event.",
  openSource: "Open source",
  openParagraph: "Open the paragraph",
  untilYear: "Up to",
  eventsVisible: (visible, total) => `${visible} / ${total} events`,
  legendTitle: "Legend",
  personSearch: "Person search",
  imageUnavailable: "",
};

const FR: AppMessages = {
  productName: "Talaria",
  productSubtitle: "Géographie d’une vie",
  search: "Rechercher",
  searchPlaceholder: "Rechercher une personnalité…",
  searchHint: "Choisissez une personne pour placer sa vie sur la carte.",
  searchInLibrary: "En bibliothèque",
  searchNew: "Nouveau",
  loading: "Chargement…",
  loadingMap: "Placement des événements sur la carte…",
  noResults: "Aucun résultat.",
  emptySearch: "Recherchez une personnalité pour voir sa vie sur la carte.",
  close: "Fermer",
  closeDetail: "Fermer l’événement",
  summary: "Résumé",
  sources: "Sources",
  noSources: "Aucune source pour cet événement.",
  openSource: "Ouvrir la source",
  openParagraph: "Ouvrir le paragraphe",
  untilYear: "Jusqu’en",
  eventsVisible: (visible, total) => `${visible} / ${total} événements`,
  legendTitle: "Légende",
  personSearch: "Recherche de personnalité",
  imageUnavailable: "",
};

export const messages: Record<AppLocale, AppMessages> = { en: EN, fr: FR };

export function useI18n(): {
  locale: AppLocale;
  setLocale: (locale: AppLocale) => void;
  t: AppMessages;
} {
  const locale = useLocaleStore((state) => state.locale);
  const setLocale = useLocaleStore((state) => state.setLocale);
  return { locale, setLocale, t: messages[locale] };
}
