// web/src/lib/i18n.ts
import { useLocaleStore, type AppLocale } from "@/stores/locale-store";

export type { AppLocale };

export interface AppMessages {
  productName: string;
  productSubtitle: string;
  search: string;
  searchPlaceholder: string;
  searchUnavailable: string;
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
  readPassage: string;
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
  chronPreviewTitle: string;
  chronPreviewHint: string;
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
  homeVisionsTitle: string;
  homeVisionsHint: string;
  visionScholarTitle: string;
  visionScholarDesc: string;
  visionScholarCta: string;
  visionVisitTitle: string;
  visionVisitDesc: string;
  visionVisitBadge: string;
  visionAgoraTitle: string;
  visionAgoraDesc: string;
  visionAgoraCta: string;
  walletConnect: string;
  walletDisconnect: string;
  walletConnecting: string;
  walletNoProvider: string;
  walletConnectFailed: string;
  walletConnectHint: string;
  walletConnectForStance: string;
  intuitionTestnetPanel: string;
  lensToggleLabel: string;
  lensScholar: string;
  lensVisit: string;
  visitHeritageTab: string;
  visitHeritageEmpty: string;
  visitNowTab: string;
  visitNowEmpty: string;
  visitNowEnds: string;
  visitNowOpenSource: string;
  visionVisitCta: string;
}

const EN: AppMessages = {
  productName: "Talaria",
  productSubtitle: "Life geography",
  search: "Search",
  searchPlaceholder: "Search a historical figure…",
  searchUnavailable: "Search is closed for now — open a demo figure below.",
  searchHint: "Choose a person to read their life.",
  searchInLibrary: "In library",
  searchNew: "New",
  loading: "Loading…",
  loadingMap: "Placing events on the map…",
  searchInProgress: "Search in progress",
  noResults: "No matches.",
  emptySearch: "Open a demo figure from the home page to explore a life.",
  close: "Close",
  closeDetail: "Close event",
  summary: "Summary",
  dossierTitle: "Context",
  dossierHint: "A short sourced recap — tap [n] to open the citation.",
  sources: "Sources",
  noSources: "No sources for this event.",
  openSource: "Open source",
  openParagraph: "Open the paragraph",
  readPassage: "Read the passage",
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
  heroSubtitle: "Open a demo life. See it as points in time. Read the debates in the Agora.",
  startExploration: "Open a life",
  openAgora: "Open the Agora",
  livingMap: "A life in points",
  livingMapDesc: "Dated facts and anecdotes from the person's life, each one tied to its source.",
  homeAboutTitle: "A life in space and argument",
  homeAboutSources: "Each point keeps its summary and the sources that mention it.",
  homeAboutDoctrine: "About",
  homeAboutDoctrineBody:
    "Explore a life. Examine the evidence. Form your own judgment.",
  homeAboutSeparates: "Three spaces",
  homeAboutSeparatesBody:
    "Scholar for the sourced life, Visit for memory sites and exhibitions, Agora for debate.",
  homeAboutFreedom: "Freedom of opinion",
  homeAboutFreedomBody:
    "Believe or dispute on Intuition — person, event, theory, interpretation or source. Expression is the rule; Talaria does not crown a single historical truth.",
  homeAboutWhitepaper: "Read the Talaria about page →",
  whitepaperNav: "About",
  whitepaperToc: "Contents",
  agoraEmpty: "Open a demo figure to load works, theories and controversies.",
  emptyTimeline: "No dated facts yet. Collect the life trace to fill the map and timeline.",
  chronPreviewTitle: "Landmark moments",
  chronPreviewHint:
    "Birth, death, and the major turning points — not every dated mention in the sources.",
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
  demoRosterHint: "Ten curated lives — open the map or the Agora. Free search is paused for this demo.",
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
  homeVisionsTitle: "Three ways to explore",
  homeVisionsHint:
    "Scholar for the sourced life, Visit for museums and exhibitions, Agora for theories and debate on Intuition.",
  visionScholarTitle: "Scholar",
  visionScholarDesc:
    "Timeline, map and sources — the documented life, with evidence and precision on dates.",
  visionScholarCta: "Open Explorer",
  visionVisitTitle: "Visit",
  visionVisitDesc:
    "Museums, memorials and exhibitions linked to a person — what to see and what’s on now.",
  visionVisitBadge: "Preview soon",
  visionAgoraTitle: "Agora",
  visionAgoraDesc: "Theories, controversies and scholarship — believe or dispute on Intuition testnet.",
  visionAgoraCta: "Open Agora",
  walletConnect: "Connect wallet",
  walletDisconnect: "Disconnect",
  walletConnecting: "Connecting…",
  walletNoProvider: "Install a Web3 wallet (e.g. MetaMask) for testnet.",
  walletConnectFailed: "Could not connect wallet.",
  walletConnectHint: "Testnet — on-chain triples come later; connection prepares your identity.",
  walletConnectForStance: "Connect your wallet to record a stance (simulation on testnet).",
  intuitionTestnetPanel: "Intuition · testnet",
  lensToggleLabel: "Explorer mode",
  lensScholar: "Scholar",
  lensVisit: "Visit",
  visitHeritageTab: "Places",
  visitHeritageEmpty: "No visit places indexed for this person yet — museums and memorials appear here after ingest.",
  visitNowTab: "Now",
  visitNowEmpty:
    "No exhibitions or events near this person’s places in the current window — run visit-enrich or widen dates.",
  visitNowEnds: "Ends",
  visitNowOpenSource: "Source",
  visionVisitCta: "Browse lives",
};

const FR: AppMessages = {
  productName: "Talaria",
  productSubtitle: "Géographie d’une vie",
  search: "Rechercher",
  searchPlaceholder: "Rechercher une personnalité…",
  searchUnavailable: "Recherche fermée pour l’instant — ouvrez une figure de la démo.",
  searchHint: "Choisissez une personne pour lire sa vie.",
  searchInLibrary: "En bibliothèque",
  searchNew: "Nouveau",
  loading: "Chargement…",
  loadingMap: "Placement des événements sur la carte…",
  searchInProgress: "Recherche en cours",
  noResults: "Aucun résultat.",
  emptySearch: "Ouvrez une figure de la démo depuis l’accueil pour explorer une vie.",
  close: "Fermer",
  closeDetail: "Fermer l’événement",
  summary: "Résumé",
  dossierTitle: "Contexte",
  dossierHint: "Un minimum de contexte sourcé — tapez [n] pour ouvrir la citation.",
  sources: "Sources",
  noSources: "Aucune source pour cet événement.",
  openSource: "Ouvrir la source",
  openParagraph: "Ouvrir le paragraphe",
  readPassage: "Lire le passage",
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
  heroSubtitle: "Ouvrez une vie de la démo. Voyez-la en points dans le temps. Lisez les débats dans l’Agora.",
  startExploration: "Ouvrir une vie",
  openAgora: "Ouvrir l’Agora",
  livingMap: "Une vie en points",
  livingMapDesc: "Faits datés et anecdotes de la vie de la personne, chacun rattaché à sa source.",
  homeAboutTitle: "Une vie dans l’espace et le débat",
  homeAboutSources: "Chaque point garde son résumé et les sources qui en parlent.",
  homeAboutDoctrine: "À propos",
  homeAboutDoctrineBody:
    "Explorer une vie. Examiner les preuves. Former son propre jugement.",
  homeAboutSeparates: "Trois espaces",
  homeAboutSeparatesBody:
    "Scholar pour la vie sourcée, Visit pour lieux de mémoire et expositions, Agora pour le débat.",
  homeAboutFreedom: "Liberté d’opinion",
  homeAboutFreedomBody:
    "Croire ou contester sur Intuition — personne, événement, théorie, interprétation ou source. L’expression est la règle ; Talaria ne couronne aucune vérité historique unique.",
  homeAboutWhitepaper: "Lire la page À propos Talaria →",
  whitepaperNav: "À propos",
  whitepaperToc: "Sommaire",
  agoraEmpty: "Ouvrez une figure de la démo pour charger travaux, théories et controverses.",
  emptyTimeline: "Pas encore de faits datés. Lancez l’ingest pour remplir la carte et la frise.",
  chronPreviewTitle: "Dates majeures",
  chronPreviewHint:
    "Naissance, mort et grands tournants — pas chaque mention datée dans les sources.",
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
  demoRosterHint: "Dix vies choisies — ouvrir la carte ou l’Agora. La recherche libre est en pause pour cette démo.",
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
  homeVisionsTitle: "Trois façons d’explorer",
  homeVisionsHint:
    "Scholar pour la vie sourcée, Visit pour musées et expositions, Agora pour théories et débat sur Intuition.",
  visionScholarTitle: "Scholar",
  visionScholarDesc:
    "Frise, carte et sources — la vie documentée, avec preuves et précision des dates.",
  visionScholarCta: "Ouvrir l’Explorer",
  visionVisitTitle: "Visit",
  visionVisitDesc:
    "Musées, mémoriaux et expositions liés à une personne — quoi voir et quoi faire maintenant.",
  visionVisitBadge: "Bientôt",
  visionAgoraTitle: "Agora",
  visionAgoraDesc:
    "Théories, controverses et érudition — croire ou contester sur Intuition testnet.",
  visionAgoraCta: "Ouvrir l’Agora",
  walletConnect: "Connecter le wallet",
  walletDisconnect: "Déconnecter",
  walletConnecting: "Connexion…",
  walletNoProvider: "Installez un wallet Web3 (ex. MetaMask) pour le testnet.",
  walletConnectFailed: "Connexion au wallet impossible.",
  walletConnectHint:
    "Testnet — l’écriture des triples viendra ensuite ; la connexion prépare votre identité.",
  walletConnectForStance:
    "Connectez votre wallet pour enregistrer une position (simulation testnet).",
  intuitionTestnetPanel: "Intuition · testnet",
  lensToggleLabel: "Mode d’exploration",
  lensScholar: "Scholar",
  lensVisit: "Visit",
  visitHeritageTab: "Lieux",
  visitHeritageEmpty:
    "Aucun lieu visitable indexé pour cette personne — musées et mémoriaux apparaissent ici après ingest.",
  visitNowTab: "En ce moment",
  visitNowEmpty:
    "Aucune expo ou événement près des lieux de cette personne sur la période — lancez visit-enrich ou élargissez les dates.",
  visitNowEnds: "Fin",
  visitNowOpenSource: "Source",
  visionVisitCta: "Parcourir les vies",
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
