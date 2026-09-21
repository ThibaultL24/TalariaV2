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
  factsTitle: string;
  factsSplit: (onMap: number, offMap: number, total: number) => string;
  onMapBadge: string;
  offMapBadge: string;
  factsTabAll: string;
  factsTabOnMap: string;
  factsTabOffMap: string;
  legendTitle: string;
  legendClickHint: string;
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
  homeAboutDoctrine: string;
  homeAboutDoctrineBody: string;
  homeAboutSeparates: string;
  homeAboutSeparatesBody: string;
  homeAboutFreedom: string;
  homeAboutFreedomBody: string;
  homeAboutWhitepaper: string;
  whitepaperNav: string;
  whitepaperToc: string;
  agoraEmpty: string;
  emptyTimeline: string;
  loadingTimeline: string;
  loadingAgora: string;
  loadingSources: string;
  loadFailed: string;
  modelConfidence: (pct: number) => string;
  ingestEventsPins: (events: number, pins: number) => string;
  ingestDatedLocated: (dated: number, located: number) => string;
  ingestPagesSources: (pages: number, sources: number) => string;
  ingestQueuedPages: (n: number) => string;
  collectLifeTrace: string;
  collectLifeTraceHint: string;
  collectScholarship: string;
  collectScholarshipHint: string;
  collecting: string;
  laneAgoraTitle: string;
  laneAgoraHint: string;
  bibliographyTitle: string;
  bibliographyHint: string;
  filterCategory: string;
  filterVeracity: string;
  filterProfile: string;
  filterPeriod: string;
  filterAllVisible: string;
  filterTitle: string;
  showAll: string;
  mapFocusOn: string;
  mapFocusOff: string;
  showFactsPanel: string;
  otherDebates: string;
  sourceFallback: string;
  noEvidenceLocator: string;
  relatedEvent: string;
  openDocument: string;
  matchScore: (pct: number) => string;
  believe: string;
  dispute: string;
  stanceHint: string;
  stanceNotOnChain: string;
  stanceModeled: string;
  stanceLiveDisabled: string;
  stanceNotEligible: string;
  intuitionStanceLabel: string;
  intuitionTheoriesTitle: string;
  intuitionTheoriesHint: string;
  agoraTabTheories: string;
  agoraTabDebates: string;
  agoraTabBibliography: string;
  agoraSourceFilter: string;
  agoraFilterEmpty: string;
  demoRosterTitle: string;
  demoRosterHint: string;
  demoRosterStats: (events: number, pins: number, claims: number) => string;
  demoRosterPending: string;
  demoEraHistorical: string;
  demoEraModern: string;
  demoIntuitionStar: string;
  demoIntuitionLive: string;
  demoIntuitionModeled: string;
  stanceTestnetReady: string;
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
  eventsVisible: (visible, total) => `${visible} / ${total} facts`,
  factsTitle: "Facts",
  factsSplit: (onMap, offMap, total) =>
    `${total} facts · ${onMap} on the map · ${offMap} without a pin`,
  onMapBadge: "On map",
  offMapBadge: "No place",
  factsTabAll: "All",
  factsTabOnMap: "On map",
  factsTabOffMap: "No place",
  legendTitle: "Legend",
  legendClickHint: "Click to filter",
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
  homeAboutDoctrine: "About",
  homeAboutDoctrineBody:
    "Explore a life. Examine the evidence. Form your own judgment.",
  homeAboutSeparates: "Two spaces",
  homeAboutSeparatesBody:
    "Explorer for situated facts. Agora for readings and controversies.",
  homeAboutFreedom: "Freedom of opinion",
  homeAboutFreedomBody:
    "Believe or dispute on Intuition — person, event, theory, interpretation or source. Expression is the rule; Talaria does not crown a single historical truth.",
  homeAboutWhitepaper: "Read the Talaria about page →",
  whitepaperNav: "About",
  whitepaperToc: "Contents",
  agoraEmpty: "Search a historical figure to load works, theories and controversies.",
  emptyTimeline: "No dated facts yet. Collect the life trace to fill the map and timeline.",
  loadingTimeline: "Loading timeline…",
  loadingAgora: "Loading agora…",
  loadingSources: "Loading sources…",
  loadFailed: "Load failed",
  modelConfidence: (pct) => `Model confidence: ${pct}%`,
  ingestEventsPins: (events, pins) => `${events} events · ${pins} pins`,
  ingestDatedLocated: (dated, located) => `${dated} dated · ${located} located`,
  ingestPagesSources: (pages, sources) =>
    [pages > 0 ? `${pages} pages` : null, sources > 0 ? `${sources} sources` : null]
      .filter(Boolean)
      .join(" · "),
  ingestQueuedPages: (n) => `${n} queued`,
  collectLifeTrace: "Collect life trace",
  collectLifeTraceHint:
    "Search starts Wikipedia, Wikisource, Commons and Wikidata. Run again to add more dated places to the map.",
  collectScholarship: "Collect scholarship",
  collectScholarshipHint:
    "HAL, Persée, theses, Gallica and catalogs → theories, controversies and academic works.",
  collecting: "Collecting sources…",
  laneAgoraTitle: "Agora",
  laneAgoraHint: "Theories, controversies, opinions and academic works about this person.",
  bibliographyTitle: "Linked bibliography",
  bibliographyHint: "Academic catalogs linked to this subject — metadata only, not map events.",
  filterCategory: "Category",
  filterVeracity: "Veracity",
  filterProfile: "Profile",
  filterPeriod: "Period",
  filterAllVisible: "All visible",
  filterTitle: "Filters",
  showAll: "Show all",
  mapFocusOn: "Map view",
  mapFocusOff: "Show panels",
  showFactsPanel: "Show facts",
  otherDebates: "Other debates",
  sourceFallback: "Source",
  noEvidenceLocator: "No evidence locator.",
  relatedEvent: "Related event",
  openDocument: "Open document",
  matchScore: (pct) => `match ${pct}%`,
  believe: "I believe this",
  dispute: "I don't believe this",
  stanceHint: "Signal with a deposit on Intuition. No deposit means no position.",
  stanceNotOnChain: "This target is not on Intuition yet.",
  stanceModeled:
    "Modeled Intuition signal — community trust without rewriting evidence.",
  stanceLiveDisabled: "Deposits are paused until Intuition publish is repaired.",
  stanceNotEligible: "This item is not an Intuition stance target.",
  intuitionStanceLabel: "Intuition",
  intuitionTheoriesTitle: "Intuition — community trust",
  intuitionTheoriesHint:
    "Believe or dispute a person, event, theory, interpretation or source. Signals never replace evidence.",
  agoraTabTheories: "Theories",
  agoraTabDebates: "Debates",
  agoraTabBibliography: "Bibliography",
  agoraSourceFilter: "Filter by catalog",
  agoraFilterEmpty: "No items for this catalog.",
  demoRosterTitle: "Demo figures",
  demoRosterHint: "Ten curated lives — open the map or the Agora. Or search anyone else to collect a new life (no LLM required).",
  demoRosterStats: (events, pins, claims) =>
    `${events} facts · ${pins} on map · ${claims} debates`,
  demoRosterPending: "Not loaded yet — open to collect.",
  demoEraHistorical: "Historical",
  demoEraModern: "Modern",
  demoIntuitionStar: "Intuition",
  demoIntuitionLive: "Intuition testnet publish enabled for this deployment.",
  demoIntuitionModeled:
    "Theories are modeled (MetaSudo). Operators enable testnet with INTUITION_ALLOW_LIVE=1.",
  stanceTestnetReady:
    "Modeled — ready for Intuition testnet publish (operator: intuition-publish --live).",
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
  eventsVisible: (visible, total) => `${visible} / ${total} faits`,
  factsTitle: "Faits",
  factsSplit: (onMap, offMap, total) =>
    `${total} faits · ${onMap} sur la carte · ${offMap} sans pin`,
  onMapBadge: "Sur la carte",
  offMapBadge: "Sans lieu",
  factsTabAll: "Tous",
  factsTabOnMap: "Sur carte",
  factsTabOffMap: "Sans lieu",
  legendTitle: "Légende",
  legendClickHint: "Cliquer pour filtrer",
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
  homeAboutDoctrine: "À propos",
  homeAboutDoctrineBody:
    "Explorer une vie. Examiner les preuves. Former son propre jugement.",
  homeAboutSeparates: "Deux espaces",
  homeAboutSeparatesBody:
    "Explorer pour les faits situés. Agora pour les lectures et controverses.",
  homeAboutFreedom: "Liberté d’opinion",
  homeAboutFreedomBody:
    "Croire ou contester sur Intuition — personne, événement, théorie, interprétation ou source. L’expression est la règle ; Talaria ne couronne aucune vérité historique unique.",
  homeAboutWhitepaper: "Lire la page À propos Talaria →",
  whitepaperNav: "À propos",
  whitepaperToc: "Sommaire",
  agoraEmpty: "Recherchez une personnalité pour charger travaux, théories et controverses.",
  emptyTimeline: "Pas encore de faits datés. Lancez l’ingest pour remplir la carte et la frise.",
  loadingTimeline: "Chargement de la frise…",
  loadingAgora: "Chargement de l’agora…",
  loadingSources: "Chargement des sources…",
  loadFailed: "Échec du chargement",
  modelConfidence: (pct) => `Confiance du modèle : ${pct} %`,
  ingestEventsPins: (events, pins) => `${events} faits · ${pins} pins`,
  ingestDatedLocated: (dated, located) => `${dated} datés · ${located} localisés`,
  ingestPagesSources: (pages, sources) =>
    [pages > 0 ? `${pages} pages` : null, sources > 0 ? `${sources} sources` : null]
      .filter(Boolean)
      .join(" · "),
  ingestQueuedPages: (n) => `${n} en file`,
  collectLifeTrace: "Collecter la vie",
  collectLifeTraceHint:
    "La recherche part de Wikipédia, Wikisource, Commons et Wikidata. Relancez pour ajouter des lieux datés.",
  collectScholarship: "Collecter l’érudition",
  collectScholarshipHint:
    "HAL, Persée, thèses, Gallica et catalogues → théories, controverses et travaux savants.",
  collecting: "Collecte des sources…",
  laneAgoraTitle: "Agora",
  laneAgoraHint: "Théories, controverses, avis et travaux savants sur cette personne.",
  bibliographyTitle: "Bibliographie liée",
  bibliographyHint: "Catalogues savants liés à cette personne — métadonnées seulement, pas des points de carte.",
  filterCategory: "Catégorie",
  filterVeracity: "Véracité",
  filterProfile: "Profil",
  filterPeriod: "Période",
  filterAllVisible: "Tout visible",
  filterTitle: "Filtres",
  showAll: "Tout afficher",
  mapFocusOn: "Vue carte",
  mapFocusOff: "Afficher les panneaux",
  showFactsPanel: "Afficher les faits",
  otherDebates: "Autres débats",
  sourceFallback: "Source",
  noEvidenceLocator: "Aucun locateur de preuve.",
  relatedEvent: "Fait lié",
  openDocument: "Ouvrir le document",
  matchScore: (pct) => `correspondance ${pct} %`,
  believe: "J’y crois",
  dispute: "Je n’y crois pas",
  stanceHint: "Signaler en déposant sur Intuition. Pas de dépôt = pas de position.",
  stanceNotOnChain: "Cette cible n’est pas encore sur Intuition.",
  stanceModeled:
    "Signal Intuition modélisé — confiance communautaire sans réécrire les preuves.",
  stanceLiveDisabled:
    "Les dépôts sont en pause jusqu’à la réparation de la publication Intuition.",
  stanceNotEligible: "Cet élément n’est pas une cible de prise de position Intuition.",
  intuitionStanceLabel: "Intuition",
  intuitionTheoriesTitle: "Intuition — confiance communautaire",
  intuitionTheoriesHint:
    "Croire ou contester une personne, un événement, une théorie, une interprétation ou une source. Les signaux ne remplacent jamais les preuves.",
  agoraTabTheories: "Théories",
  agoraTabDebates: "Débats",
  agoraTabBibliography: "Bibliographie",
  agoraSourceFilter: "Filtrer par catalogue",
  agoraFilterEmpty: "Aucun élément pour ce catalogue.",
  demoRosterTitle: "Figures de la démo",
  demoRosterHint: "Dix vies choisies — ouvrir la carte ou l’Agora. Ou cherchez n’importe qui d’autre pour collecter une vie (sans LLM).",
  demoRosterStats: (events, pins, claims) =>
    `${events} faits · ${pins} sur carte · ${claims} débats`,
  demoRosterPending: "Pas encore chargé — ouvrir pour collecter.",
  demoEraHistorical: "Historique",
  demoEraModern: "Moderne",
  demoIntuitionStar: "Intuition",
  demoIntuitionLive: "Publication Intuition testnet activée sur ce déploiement.",
  demoIntuitionModeled:
    "Théories modélisées (MetaSudo). Activer le testnet avec INTUITION_ALLOW_LIVE=1.",
  stanceTestnetReady:
    "Modélisée — prête pour publication testnet (opérateur : intuition-publish --live).",
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
