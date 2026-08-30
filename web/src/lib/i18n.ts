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
  searchInProgress: string;
  noResults: string;
  emptySearch: string;
  close: string;
  closeDetail: string;
  summary: string;
  dossierTitle: string;
  dossierHint: string;
  sources: string;
  noSources: string;
  openSource: string;
  openParagraph: string;
  untilYear: string;
  eventsVisible: (visible: number, total: number) => string;
  legendTitle: string;
  personSearch: string;
  imageUnavailable: string;
  home: string;
  agora: string;
  agoraHint: string;
  collectAgora: string;
  explorer: string;
  heroEyebrow: string;
  heroSubtitle: string;
  startExploration: string;
  openAgora: string;
  livingMap: string;
  livingMapDesc: string;
  homeAboutTitle: string;
  homeAboutSources: string;
  agoraEmpty: string;
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
  searchInProgress: "Search in progress",
  noResults: "No matches.",
  emptySearch: "Search a historical figure to see their life on the map.",
  close: "Close",
  closeDetail: "Close event",
  summary: "Summary",
  dossierTitle: "Context",
  dossierHint: "A short sourced recap — tap [n] to open the citation.",
  sources: "Sources",
  noSources: "No sources for this event.",
  openSource: "Open source",
  openParagraph: "Open the paragraph",
  untilYear: "Up to",
  eventsVisible: (visible, total) => `${visible} / ${total} events`,
  legendTitle: "Legend",
  personSearch: "Person search",
  imageUnavailable: "",
  home: "Home",
  agora: "Agora",
  agoraHint: "Works, opinions, theories and controversies about this person.",
  collectAgora: "Collect scholarship",
  explorer: "Map",
  heroEyebrow: "Historical geography",
  heroSubtitle: "Search a person. See their life on the map. Read the debates in the Agora.",
  startExploration: "Open the map",
  openAgora: "Open the Agora",
  livingMap: "Living map",
  livingMapDesc: "Dated facts, anecdotes and places from Wikipedia and catalogs.",
  homeAboutTitle: "A life in space and argument",
  homeAboutSources: "Each point keeps its summary and the sources that mention it.",
  agoraEmpty: "Search a historical figure to load works, theories and controversies.",
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
  searchInProgress: "Recherche en cours",
  noResults: "Aucun résultat.",
  emptySearch: "Recherchez une personnalité pour voir sa vie sur la carte.",
  close: "Fermer",
  closeDetail: "Fermer l’événement",
  summary: "Résumé",
  dossierTitle: "Contexte",
  dossierHint: "Un minimum de contexte sourcé — tapez [n] pour ouvrir la citation.",
  sources: "Sources",
  noSources: "Aucune source pour cet événement.",
  openSource: "Ouvrir la source",
  openParagraph: "Ouvrir le paragraphe",
  untilYear: "Jusqu’en",
  eventsVisible: (visible, total) => `${visible} / ${total} événements`,
  legendTitle: "Légende",
  personSearch: "Recherche de personnalité",
  imageUnavailable: "",
  home: "Accueil",
  agora: "Agora",
  agoraHint: "Travaux, avis, théories et controverses autour de cette personne.",
  collectAgora: "Collecter l’agora",
  explorer: "Carte",
  heroEyebrow: "Géographie historique",
  heroSubtitle: "Cherchez une personne. Voyez sa vie sur la carte. Lisez les débats dans l’Agora.",
  startExploration: "Ouvrir la carte",
  openAgora: "Ouvrir l’Agora",
  livingMap: "Carte vivante",
  livingMapDesc: "Faits datés, anecdotes et lieux issus de Wikipédia et des catalogues.",
  homeAboutTitle: "Une vie dans l’espace et le débat",
  homeAboutSources: "Chaque point garde son résumé et les sources qui en parlent.",
  agoraEmpty: "Recherchez une personnalité pour charger travaux, théories et controverses.",
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
