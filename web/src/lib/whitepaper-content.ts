// web/src/lib/whitepaper-content.ts
import type { AppLocale } from "@/stores/locale-store";

export interface WhitepaperSubsection {
  title: string;
  paragraphs?: string[];
  bullets?: string[];
  bulletStyle?: "ol" | "ul";
}

export interface WhitepaperSection {
  id: string;
  eyebrow?: string;
  title: string;
  paragraphs?: string[];
  bullets?: string[];
  bulletStyle?: "ol" | "ul";
  quote?: string;
  table?: { headers: string[]; rows: string[][] };
  cards?: { title: string; body: string }[];
  subsections?: WhitepaperSubsection[];
}

export interface WhitepaperDoc {
  meta: string;
  title: string;
  lede: string;
  intro?: string[];
  closingTitle?: string;
  closingLines?: string[];
  closing: string;
  sections: WhitepaperSection[];
}

const EN: WhitepaperDoc = {
  meta: "Product doctrine · public prototype · v0.1",
  title: "Talaria",
  lede: "Explore a life. Examine the evidence. Form your own judgment.",
  intro: [
    "Talaria is a historical exploration tool designed to trace the lives of major figures across places and time.",
    "Its first purpose is simple: to show where a person lived, travelled, worked, fought, created, governed or influenced the course of history. Each event is presented with the greatest level of precision the available evidence allows, together with real and verifiable sources.",
    "When several sources describe the same event, Talaria brings them together. When they disagree, the disagreement should remain visible. The purpose is not to impose a definitive version of history, but to give users enough context and evidence to pursue their own understanding of it.",
    "This search for truth belongs to the user.",
  ],
  closingTitle: "Follow the traces",
  closingLines: [
    "Explore where a person went.",
    "See what happened there.",
    "Read the sources.",
    "Compare the interpretations.",
    "Accept, question or contest them.",
  ],
  closing: "Talaria provides the map and the evidence. The search for truth remains yours.",
  sections: [
    {
      id: "purpose",
      eyebrow: "01",
      title: "Purpose",
      paragraphs: [
        "Biographies usually present a life as a sequence of paragraphs and dates. Talaria approaches it as a trajectory.",
        "A life unfolds through places: cities, homes, schools, workplaces, courts, battlefields, prisons, ports and places of exile. By reconnecting events with their geography, Talaria helps users understand not only what happened, but where a person was and how their life moved through the world.",
        "Talaria therefore aims to:",
      ],
      bullets: [
        "reconstruct the geographical and chronological trajectory of a person;",
        "gather as many relevant sources as possible for each event;",
        "preserve the real precision and uncertainty of dates and places;",
        "make every source accessible and verifiable;",
        "expose contradictions instead of hiding them;",
        "distinguish documented occurrences from their interpretations.",
      ],
      bulletStyle: "ul",
      quote:
        "Talaria does not claim to possess the truth about a person or an event. It provides the traces, sources and viewpoints from which each user can form a judgment.",
    },
    {
      id: "surfaces",
      eyebrow: "02",
      title: "Explorer and Agora",
      paragraphs: ["Talaria is divided into two complementary spaces."],
      table: {
        headers: ["Space", "Content", "Purpose"],
        rows: [
          [
            "Explorer",
            "Dated, located and sourced events from a person’s life",
            "Reconstruct their trajectory through space and time",
          ],
          [
            "Agora",
            "Historical theses, controversies, rumours, received ideas and scholarly interpretations",
            "Present the different ways their life and actions have been understood",
          ],
          [
            "Intuition",
            "Community signals concerning people, events and interpretations",
            "Make collective trust, support and disagreement visible",
          ],
        ],
      },
      subsections: [
        {
          title: "Explorer",
          paragraphs: [
            "Explorer displays the documented course of a life.",
            "Each event may contain an event type; a date or period with explicit precision; a location when it can be reliably identified; the people or organisations involved; one or more verifiable sources; and an indication of uncertainty or contradiction.",
            "A point on the map is not declared to be an absolute truth. It is the best-supported representation Talaria can currently produce from the available evidence.",
            "Several sources referring to the same occurrence reinforce or challenge that occurrence. They must not create artificial duplicates on the map.",
          ],
        },
        {
          title: "Agora",
          paragraphs: [
            "Agora presents what people have said, believed or argued about a historical figure.",
            "Its content can range from peer-reviewed historical research, university theses and scholarly publications, competing interpretations between historians, political or philosophical readings, unresolved controversies, to popular rumours and received ideas.",
            "These materials do not all have the same evidentiary value. Talaria should identify their nature, provenance and level of support so that a rumour is never visually presented as equivalent to documented academic research.",
            "The purpose is not to hide weak or controversial interpretations, nor to validate them automatically. It is to place them in context and allow users to examine why they exist.",
          ],
        },
      ],
    },
    {
      id: "evidence",
      eyebrow: "03",
      title: "Sources, evidence and interpretation",
      paragraphs: [
        "Talaria seeks to present each event through the widest reasonable range of relevant sources. These may include:",
      ],
      bullets: [
        "Wikidata and Wikimedia projects;",
        "libraries and national archives;",
        "museum and heritage collections;",
        "academic articles;",
        "university theses;",
        "books and bibliographic catalogues;",
        "digitised newspapers;",
        "primary historical documents.",
      ],
      bulletStyle: "ul",
      quote:
        "Every source contributes a trace. No source becomes unquestionable simply because it comes from a recognised institution, and no claim becomes true merely because it is repeated.",
      cards: [
        {
          title: "What Talaria preserves",
          body: "The origin of the information; the relevant document or fragment; the date on which it was retrieved; the precision of the extracted information; agreements and contradictions between sources; the distinction between primary evidence, secondary analysis and popular belief.",
        },
        {
          title: "Who interprets",
          body: "The system assembles and organises evidence. The final interpretation remains with the user.",
        },
      ],
    },
    {
      id: "intuition",
      eyebrow: "04",
      title: "Community trust with Intuition",
      paragraphs: [
        "Intuition Protocol adds a public trust layer to Talaria.",
        "Users may express support or disagreement concerning:",
      ],
      bullets: [
        "a historical person;",
        "a documented event;",
        "a theory or controversy;",
        "an interpretation of an event;",
        "a source or claim.",
      ],
      bulletStyle: "ul",
      cards: [
        {
          title: "Signals are not proof",
          body: "These signals make the community’s level of confidence visible over time. They do not rewrite the event, remove it from the map or transform popularity into proof.",
        },
        {
          title: "Three separate dimensions",
          body: "Evidence describes what the sources support. Interpretation explains what people conclude from them. Community signals show what users currently trust or dispute.",
        },
      ],
      quote:
        "Intuition does not serve as a court deciding historical truth. It provides an open and transparent way to represent collective confidence and disagreement.",
    },
    {
      id: "limits",
      eyebrow: "05",
      title: "Honest limits",
      paragraphs: [
        "Talaria depends on incomplete and sometimes contradictory historical records.",
        "Its extraction pipeline can miss relevant events, misclassify an occurrence, associate an event with the wrong place, infer too much precision from an uncertain date, or fail to recognise that two sources describe the same event.",
      ],
      bullets: [
        "an unsupported event should not become a map point;",
        "an unresolved place should remain outside the map;",
        "an approximate date must remain approximate;",
        "a contradiction should remain visible until it can be resolved;",
        "an automated extraction is always a candidate before it becomes a displayed event;",
        "a community signal must never replace documentary evidence.",
      ],
      bulletStyle: "ul",
      quote:
        "Talaria is not a final history of the world. It is a tool for exploring lives, comparing sources and understanding how historical knowledge is constructed.",
    },
  ],
};

const FR: WhitepaperDoc = {
  meta: "Doctrine produit · prototype public · v0.1",
  title: "Talaria",
  lede: "Explorer une vie. Examiner les preuves. Former son propre jugement.",
  intro: [
    "Talaria est un outil d’exploration historique conçu pour retracer la vie de figures majeures à travers les lieux et le temps.",
    "Son premier objectif est simple : montrer où une personne a vécu, voyagé, travaillé, combattu, créé, gouverné ou influencé le cours de l’histoire. Chaque événement est présenté avec le plus grand niveau de précision que permettent les preuves disponibles, accompagné de sources réelles et vérifiables.",
    "Lorsque plusieurs sources décrivent le même événement, Talaria les rapproche. Lorsqu’elles divergent, le désaccord doit rester visible. Le but n’est pas d’imposer une version définitive de l’histoire, mais de donner assez de contexte et de preuves pour que chacun puisse poursuivre sa propre compréhension.",
    "Cette recherche de la vérité appartient à l’utilisateur.",
  ],
  closingTitle: "Suivre les traces",
  closingLines: [
    "Explorez où une personne est allée.",
    "Voyez ce qui s’y est passé.",
    "Lisez les sources.",
    "Comparez les interprétations.",
    "Acceptez-les, interrogez-les ou contestez-les.",
  ],
  closing: "Talaria fournit la carte et les preuves. La recherche de la vérité reste la vôtre.",
  sections: [
    {
      id: "purpose",
      eyebrow: "01",
      title: "Finalité",
      paragraphs: [
        "Les biographies présentent souvent une vie comme une suite de paragraphes et de dates. Talaria l’aborde comme une trajectoire.",
        "Une vie se déploie dans des lieux : villes, demeures, écoles, lieux de travail, cours, champs de bataille, prisons, ports et lieux d’exil. En reconnectant les événements à leur géographie, Talaria aide à comprendre non seulement ce qui s’est passé, mais où se trouvait la personne et comment sa vie a traversé le monde.",
        "Talaria vise donc à :",
      ],
      bullets: [
        "reconstruire la trajectoire géographique et chronologique d’une personne ;",
        "rassembler autant de sources pertinentes que possible pour chaque événement ;",
        "préserver la précision réelle et l’incertitude des dates et des lieux ;",
        "rendre chaque source accessible et vérifiable ;",
        "exposer les contradictions plutôt que de les masquer ;",
        "distinguer les occurrences documentées de leurs interprétations.",
      ],
      bulletStyle: "ul",
      quote:
        "Talaria ne prétend pas détenir la vérité sur une personne ou un événement. Elle fournit les traces, les sources et les points de vue à partir desquels chacun peut former un jugement.",
    },
    {
      id: "surfaces",
      eyebrow: "02",
      title: "Explorer et Agora",
      paragraphs: ["Talaria se divise en deux espaces complémentaires."],
      table: {
        headers: ["Espace", "Contenu", "Finalité"],
        rows: [
          [
            "Explorer",
            "Événements datés, situés et sourcés de la vie d’une personne",
            "Reconstruire sa trajectoire dans l’espace et le temps",
          ],
          [
            "Agora",
            "Thèses historiques, controverses, rumeurs, idées reçues et interprétations savantes",
            "Présenter les différentes lectures de sa vie et de ses actes",
          ],
          [
            "Intuition",
            "Signaux communautaires sur les personnes, événements et interprétations",
            "Rendre visibles la confiance collective, le soutien et le désaccord",
          ],
        ],
      },
      subsections: [
        {
          title: "Explorer",
          paragraphs: [
            "L’Explorer affiche le cours documenté d’une vie.",
            "Chaque événement peut contenir un type ; une date ou période avec une précision explicite ; un lieu lorsqu’il peut être identifié de façon fiable ; les personnes ou organisations impliquées ; une ou plusieurs sources vérifiables ; et une indication d’incertitude ou de contradiction.",
            "Un point sur la carte n’est pas déclaré comme vérité absolue. C’est la représentation la mieux étayée que Talaria peut produire à partir des preuves disponibles.",
            "Plusieurs sources portant sur la même occurrence la renforcent ou la contestent. Elles ne doivent pas créer de doublons artificiels sur la carte.",
          ],
        },
        {
          title: "Agora",
          paragraphs: [
            "L’Agora présente ce que l’on a dit, cru ou argumenté à propos d’une figure historique.",
            "Son contenu va de la recherche historique évaluée par les pairs aux thèses universitaires, publications savantes, interprétations concurrentes, lectures politiques ou philosophiques, controverses non résolues, jusqu’aux rumeurs et idées reçues.",
            "Ces matériaux n’ont pas tous la même valeur probante. Talaria doit en identifier la nature, la provenance et le niveau de soutien, afin qu’une rumeur ne soit jamais présentée visuellement comme l’équivalent d’une recherche académique documentée.",
            "Le but n’est ni de cacher les interprétations faibles ou controversées, ni de les valider automatiquement. C’est de les situer et de permettre d’examiner pourquoi elles existent.",
          ],
        },
      ],
    },
    {
      id: "evidence",
      eyebrow: "03",
      title: "Sources, preuves et interprétation",
      paragraphs: [
        "Talaria cherche à présenter chaque événement à travers le plus large éventail raisonnable de sources pertinentes. Cela peut inclure :",
      ],
      bullets: [
        "Wikidata et les projets Wikimedia ;",
        "bibliothèques et archives nationales ;",
        "collections muséales et patrimoniales ;",
        "articles académiques ;",
        "thèses universitaires ;",
        "livres et catalogues bibliographiques ;",
        "journaux numérisés ;",
        "documents historiques primaires.",
      ],
      bulletStyle: "ul",
      quote:
        "Chaque source apporte une trace. Aucune source ne devient incontestable parce qu’elle vient d’une institution reconnue, et aucune affirmation ne devient vraie parce qu’elle est répétée.",
      cards: [
        {
          title: "Ce que Talaria conserve",
          body: "L’origine de l’information ; le document ou fragment pertinent ; la date de récupération ; la précision de l’extrait ; les accords et contradictions entre sources ; la distinction entre preuve primaire, analyse secondaire et croyance populaire.",
        },
        {
          title: "Qui interprète",
          body: "Le système assemble et organise les preuves. L’interprétation finale reste celle de l’utilisateur.",
        },
      ],
    },
    {
      id: "intuition",
      eyebrow: "04",
      title: "Confiance communautaire avec Intuition",
      paragraphs: [
        "Le protocole Intuition ajoute à Talaria une couche publique de confiance.",
        "Les utilisateurs peuvent exprimer un soutien ou un désaccord concernant :",
      ],
      bullets: [
        "une personne historique ;",
        "un événement documenté ;",
        "une théorie ou controverse ;",
        "une interprétation d’un événement ;",
        "une source ou une affirmation.",
      ],
      bulletStyle: "ul",
      cards: [
        {
          title: "Les signaux ne sont pas des preuves",
          body: "Ces signaux rendent visible le niveau de confiance de la communauté dans le temps. Ils ne réécrivent pas l’événement, ne le retirent pas de la carte et ne transforment pas la popularité en preuve.",
        },
        {
          title: "Trois dimensions distinctes",
          body: "La preuve décrit ce que les sources étayent. L’interprétation explique ce que l’on en conclut. Les signaux communautaires montrent ce que les utilisateurs font confiance ou contestent.",
        },
      ],
      quote:
        "Intuition n’est pas un tribunal de la vérité historique. C’est une manière ouverte et transparente de représenter la confiance et le désaccord collectifs.",
    },
    {
      id: "limits",
      eyebrow: "05",
      title: "Limites honnêtes",
      paragraphs: [
        "Talaria dépend de sources historiques incomplètes et parfois contradictoires.",
        "Son pipeline d’extraction peut manquer des événements, mal classer une occurrence, associer un événement au mauvais lieu, inférer trop de précision depuis une date incertaine, ou ne pas reconnaître que deux sources décrivent le même événement.",
      ],
      bullets: [
        "un événement non étayé ne doit pas devenir un point sur la carte ;",
        "un lieu non résolu doit rester hors carte ;",
        "une date approximative doit rester approximative ;",
        "une contradiction doit rester visible jusqu’à résolution ;",
        "une extraction automatisée est toujours une candidate avant de devenir un événement affiché ;",
        "un signal communautaire ne doit jamais remplacer une preuve documentaire.",
      ],
      bulletStyle: "ul",
      quote:
        "Talaria n’est pas une histoire définitive du monde. C’est un outil pour explorer des vies, comparer des sources et comprendre comment se construit le savoir historique.",
    },
  ],
};

export function whitepaperForLocale(locale: AppLocale): WhitepaperDoc {
  return locale === "fr" ? FR : EN;
}
