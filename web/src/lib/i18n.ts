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
  seeMore: string;
  seeLess: string;
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
  support: string;
  dispute: string;
  uncertain: string;
  signalEconomicHint: string;
  signalSignInRequired: string;
  signalWalletMismatch: string;
  signalNotPublished: string;
  signalPreparing: string;
  signalAwaitingSignature: string;
  signalSubmitted: string;
  signalConfirming: string;
  signalConfirmed: string;
  signalFailed: string;
  signalRejected: string;
  signalSyncRetry: string;
  signalConfirmTitle: string;
  signalConfirm: string;
  signalPreview: string;
  signalCustomAmount: string;
  comments: (n: number) => string;
  commentsTitle: string;
  openDiscussion: string;
  closeDiscussion: string;
  commentPlaceholder: string;
  replyPlaceholder: string;
  postComment: string;
  postReply: string;
  commentDeleted: string;
  commentEdited: string;
  anonymousHistorian: string;
  reactionRelevant: string;
  reactionWellSourced: string;
  reactionInteresting: string;
  reactionNeedsNuance: string;
  reactionDisagree: string;
  commentSignInRequired: string;
  argumentSignInRequired: string;
  openArguments: string;
  closeArguments: string;
  argumentsCount: (n: number) => string;
  sourcesCount: (n: number) => string;
  claimSourcesTitle: string;
  addSource: string;
  addArgument: string;
  argumentRelation: string;
  argumentSupports: string;
  argumentContradicts: string;
  argumentQualifies: string;
  argumentStatement: string;
  argumentEvidence: string;
  submitArgument: string;
  argumentsFor: string;
  argumentsAgainst: string;
  argumentsQualify: string;
  argumentsTitle: string;
  sourceNotIndexed: string;
  pipelineOrigin: string;
  openClaimDetails: string;
  closeClaimDetails: string;
  corpusEvidenceTitle: string;
  noSourcesYet: string;
  noArgumentsYet: string;
  noCommentsYet: string;
  communityContribution: string;
  proposedStatus: string;
  promoteToArgument: string;
  selectSource: string;
  retrySection: string;
  sectionError: string;
  intuitionPublished: string;
  intuitionSignalAvailable: string;
  intuitionNotPublished: string;
  immersiveBack: string;
  immersiveTools: string;
  immersiveLegend: string;
  immersiveCategories: string;
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
  walletWrongNetwork: string;
  walletSwitchNetwork: string;
  walletPanelKicker: string;
  walletPanelTitle: string;
  walletPanelLead: string;
  walletConnectModalTitle: string;
  walletEduTitle: string;
  walletEduAssetsTitle: string;
  walletEduAssetsBody: string;
  walletEduLoginTitle: string;
  walletEduLoginBody: string;
  walletGetWallet: string;
  walletLearnMore: string;
  walletConnectHintNetwork: (network: string) => string;
  walletNetworkHub: string;
  walletConnected: string;
  walletWrongNetworkDetail: string;
  talariaSignIn: string;
  talariaSignInHint: string;
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
  seeMore: "See more",
  seeLess: "See less",
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
    "Support, dispute or mark uncertain in the Agora. Support and dispute deposit TRUST on Intuition; uncertain stays in Talaria. Expression is the rule; Talaria does not crown a single historical truth.",
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
  support: "Support",
  dispute: "Dispute",
  uncertain: "Uncertain",
  signalEconomicHint:
    "Support and Dispute deposit TRUST on Intuition. Uncertain is a Talaria position only.",
  signalSignInRequired: "Sign in to Talaria to record a position.",
  signalWalletMismatch: "Wallet changed — sign in to Talaria again",
  signalNotPublished: "This claim is not published on Intuition yet.",
  signalPreparing: "Preparing signal…",
  signalAwaitingSignature: "Wallet confirmation required",
  signalSubmitted: "Transaction submitted",
  signalConfirming: "Waiting for confirmation",
  signalConfirmed: "Signal confirmed",
  signalFailed: "Transaction failed",
  signalRejected: "Signature cancelled — no Talaria position was recorded.",
  signalSyncRetry: "Transaction confirmed — retry saving to Talaria",
  signalConfirmTitle: "Confirm Intuition deposit",
  signalConfirm: "Sign deposit",
  signalPreview: "Preview deposit",
  signalCustomAmount: "Custom",
  comments: (n) => (n === 1 ? "1 comment" : `${n} comments`),
  commentsTitle: "Discussion",
  openDiscussion: "Open comments",
  closeDiscussion: "Hide comments",
  commentPlaceholder: "Why do you support or dispute this interpretation?",
  replyPlaceholder: "Reply",
  postComment: "Post comment",
  postReply: "Post reply",
  commentDeleted: "Comment deleted",
  commentEdited: "edited",
  anonymousHistorian: "Anonymous historian",
  reactionRelevant: "Relevant",
  reactionWellSourced: "Well sourced",
  reactionInteresting: "Interesting",
  reactionNeedsNuance: "Needs nuance",
  reactionDisagree: "Disagree",
  commentSignInRequired: "Sign in to Talaria to comment.",
  argumentSignInRequired: "Sign in to Talaria to add a source or argument.",
  openArguments: "Open sources and arguments",
  closeArguments: "Hide sources and arguments",
  argumentsCount: (n) => (n === 1 ? "1 argument" : `${n} arguments`),
  sourcesCount: (n) => (n === 1 ? "1 source" : `${n} sources`),
  claimSourcesTitle: "Sources",
  addSource: "Add source",
  addArgument: "Add argument",
  argumentRelation: "Relation",
  argumentSupports: "Supports",
  argumentContradicts: "Contradicts",
  argumentQualifies: "Qualifies",
  argumentStatement: "Statement",
  argumentEvidence: "Evidence excerpt",
  submitArgument: "Submit argument",
  argumentsFor: "Arguments for",
  argumentsAgainst: "Arguments against",
  argumentsQualify: "Nuances",
  argumentsTitle: "Arguments",
  sourceNotIndexed: "Source not yet indexed",
  pipelineOrigin: "Talaria corpus",
  openClaimDetails: "Open evidence, sources and discussion",
  closeClaimDetails: "Hide details",
  corpusEvidenceTitle: "Evidence",
  noSourcesYet: "No sources added yet",
  noArgumentsYet: "No structured arguments yet",
  noCommentsYet: "No comments yet",
  communityContribution: "Community contribution",
  proposedStatus: "Proposed",
  promoteToArgument: "Promote to argument",
  selectSource: "Source",
  retrySection: "Retry",
  sectionError: "Could not load this section.",
  intuitionPublished: "Published on Intuition",
  intuitionSignalAvailable: "Signal available",
  intuitionNotPublished: "This claim is not published on Intuition",
  immersiveBack: "Back",
  immersiveTools: "Search and filters",
  immersiveLegend: "Legend",
  immersiveCategories: "Categories",
  intuitionTheoriesTitle: "Intuition — community trust",
  intuitionTheoriesHint:
    "Theories, controversies and scholarship — support or dispute published claims on Intuition.",
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
  visionAgoraDesc: "Theories, controversies and scholarship — support or dispute on Intuition testnet.",
  visionAgoraCta: "Open Agora",
  walletConnect: "Connect wallet",
  walletDisconnect: "Disconnect",
  walletConnecting: "Connecting…",
  walletNoProvider: "Install a Web3 wallet (e.g. MetaMask) for testnet.",
  walletConnectFailed: "Could not connect wallet.",
  walletConnectHint: "Testnet — on-chain triples come later; connection prepares your identity.",
  walletConnectForStance: "Connect your wallet to record a stance (simulation on testnet).",
  walletWrongNetwork: "Wrong network",
  walletSwitchNetwork: "Switch network",
  walletPanelKicker: "Intuition",
  walletPanelTitle: "Your wallet",
  walletPanelLead: "Connect your wallet to take a stance on Intuition testnet.",
  walletConnectModalTitle: "Connect a wallet",
  walletEduTitle: "What is a wallet?",
  walletEduAssetsTitle: "A home for your digital assets",
  walletEduAssetsBody: "Wallets are used to send, receive, store, and display digital assets.",
  walletEduLoginTitle: "A new way to log in",
  walletEduLoginBody:
    "Instead of creating new accounts and passwords on every website, just connect your wallet.",
  walletGetWallet: "Get a wallet",
  walletLearnMore: "Learn more",
  walletConnectHintNetwork: (network) =>
    `Connect on ${network}. Add the network in your wallet if prompted.`,
  walletNetworkHub: "Network hub",
  walletConnected: "Connected",
  walletWrongNetworkDetail: "You're on the wrong network. Switch to continue.",
  talariaSignIn: "Sign in to Talaria",
  talariaSignInHint: "Sign a message to create a Talaria session. No gas, no Intuition transaction.",
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
  seeMore: "Voir plus",
  seeLess: "Voir moins",
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
    "Soutenir, contester ou marquer incertain dans l’Agora. Soutenir et contester déposent du TRUST sur Intuition ; incertain reste dans Talaria. L’expression est la règle ; Talaria ne couronne aucune vérité historique unique.",
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
  support: "Soutenir",
  dispute: "Contester",
  uncertain: "Incertain",
  signalEconomicHint:
    "Soutenir et Contester déposent du TRUST sur Intuition. Incertain est une position Talaria seulement.",
  signalSignInRequired: "Connectez-vous à Talaria pour enregistrer une position.",
  signalWalletMismatch: "Portefeuille changé — reconnectez-vous à Talaria",
  signalNotPublished: "Cette thèse n’est pas encore publiée sur Intuition.",
  signalPreparing: "Préparation du signal…",
  signalAwaitingSignature: "Confirmation du portefeuille requise",
  signalSubmitted: "Transaction envoyée",
  signalConfirming: "En attente de confirmation",
  signalConfirmed: "Signal confirmé",
  signalFailed: "La transaction a échoué",
  signalRejected: "Signature annulée — aucune position Talaria n’a été enregistrée.",
  signalSyncRetry: "Transaction confirmée — réessayer l’enregistrement Talaria",
  signalConfirmTitle: "Confirmer le dépôt Intuition",
  signalConfirm: "Signer le dépôt",
  signalPreview: "Prévisualiser le dépôt",
  signalCustomAmount: "Personnalisé",
  comments: (n) => (n === 1 ? "1 commentaire" : `${n} commentaires`),
  commentsTitle: "Discussion",
  openDiscussion: "Ouvrir les commentaires",
  closeDiscussion: "Masquer les commentaires",
  commentPlaceholder: "Pourquoi soutenez-vous ou contestez-vous cette interprétation ?",
  replyPlaceholder: "Répondre",
  postComment: "Publier",
  postReply: "Répondre",
  commentDeleted: "Commentaire supprimé",
  commentEdited: "modifié",
  anonymousHistorian: "Historien anonyme",
  reactionRelevant: "Pertinent",
  reactionWellSourced: "Bien sourcé",
  reactionInteresting: "Intéressant",
  reactionNeedsNuance: "À nuancer",
  reactionDisagree: "Pas d’accord",
  commentSignInRequired: "Connectez-vous à Talaria pour commenter.",
  argumentSignInRequired: "Connectez-vous à Talaria pour ajouter une source ou un argument.",
  openArguments: "Ouvrir sources et arguments",
  closeArguments: "Masquer sources et arguments",
  argumentsCount: (n) => (n === 1 ? "1 argument" : `${n} arguments`),
  sourcesCount: (n) => (n === 1 ? "1 source" : `${n} sources`),
  claimSourcesTitle: "Sources",
  addSource: "Ajouter une source",
  addArgument: "Ajouter un argument",
  argumentRelation: "Relation",
  argumentSupports: "Soutient",
  argumentContradicts: "Contredit",
  argumentQualifies: "Qualifie",
  argumentStatement: "Énoncé",
  argumentEvidence: "Extrait d’évidence",
  submitArgument: "Envoyer l’argument",
  argumentsFor: "Arguments pour",
  argumentsAgainst: "Arguments contre",
  argumentsQualify: "Nuances",
  argumentsTitle: "Arguments",
  sourceNotIndexed: "Source pas encore indexée",
  pipelineOrigin: "Corpus Talaria",
  openClaimDetails: "Ouvrir preuves, sources et discussion",
  closeClaimDetails: "Masquer le détail",
  corpusEvidenceTitle: "Preuves",
  noSourcesYet: "Aucune source ajoutée pour l’instant",
  noArgumentsYet: "Aucun argument structuré pour l’instant",
  noCommentsYet: "Aucun commentaire pour l’instant",
  communityContribution: "Contribution communautaire",
  proposedStatus: "Proposé",
  promoteToArgument: "Proposer comme argument",
  selectSource: "Source",
  retrySection: "Réessayer",
  sectionError: "Impossible de charger cette section.",
  intuitionPublished: "Publié sur Intuition",
  intuitionSignalAvailable: "Signal disponible",
  intuitionNotPublished: "Cette affirmation n’est pas publiée sur Intuition",
  immersiveBack: "Retour",
  immersiveTools: "Recherche et filtres",
  immersiveLegend: "Légende",
  immersiveCategories: "Catégories",
  intuitionTheoriesTitle: "Intuition — confiance communautaire",
  intuitionTheoriesHint:
    "Soutenir ou contester une thèse publiée sur Intuition. Les signaux ne remplacent jamais les preuves.",
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
    "Théories, controverses et érudition — soutenir ou contester sur Intuition testnet.",
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
  walletWrongNetwork: "Mauvais réseau",
  walletSwitchNetwork: "Changer de réseau",
  walletPanelKicker: "Intuition",
  walletPanelTitle: "Votre wallet",
  walletPanelLead: "Connectez votre wallet pour prendre position sur Intuition testnet.",
  walletConnectModalTitle: "Connecter un wallet",
  walletEduTitle: "Qu’est-ce qu’un wallet ?",
  walletEduAssetsTitle: "Un lieu pour vos actifs numériques",
  walletEduAssetsBody:
    "Un wallet sert à envoyer, recevoir, stocker et afficher des actifs numériques.",
  walletEduLoginTitle: "Une autre façon de se connecter",
  walletEduLoginBody:
    "Au lieu de créer un compte et un mot de passe sur chaque site, connectez simplement votre wallet.",
  walletGetWallet: "Obtenir un wallet",
  walletLearnMore: "En savoir plus",
  walletConnectHintNetwork: (network) =>
    `Connexion sur ${network}. Ajoutez le réseau dans votre wallet si demandé.`,
  walletNetworkHub: "Hub réseau",
  walletConnected: "Connecté",
  walletWrongNetworkDetail: "Vous n’êtes pas sur le bon réseau. Changez de réseau pour continuer.",
  talariaSignIn: "Connexion Talaria",
  talariaSignInHint:
    "Signez un message pour créer une session Talaria. Pas de gas, pas de transaction Intuition.",
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
