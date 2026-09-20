# Talaria — livre blanc

**Version :** 0.1 (démo publique)  
**Langue :** français  
**Objet :** comment Talaria présente une vie humaine — ce qui est *situé*, ce qui est *discutable*, et pourquoi l’opinion n’est jamais confondue avec le fait.

---

## 1. Thèse

Talaria n’est pas une encyclopédie qui tranche.  
C’est un **espace de lecture** : d’un côté la **géographie d’une vie** (lieux, dates, occurrences sourcées) ; de l’autre l’**agora des lectures** (théories, controverses, avis savants).  

Le seul mot d’ordre est la **liberté d’expression et d’opinion** : chacun peut croire, contester, nuancer — sans que le système prétende détenir la vérité historique.  
Ce que le système refuse, en revanche, c’est de **mélanger les genres** : un débat n’est pas un pin sur la carte ; un pin sur la carte n’est pas un verdict moral.

---

## 2. Deux surfaces, deux régimes de vérité

| Surface | Nom produit | Contenu | Régime |
|--------|-------------|---------|--------|
| **Carte + timeline** | Explorer | Occurrences biographiques datées / situées | **Fait culturel situé** — « quelque chose a été dit, daté, localisé, sourcé » |
| **Agora** | Agora | Théories, controverses, travaux, bibliographie | **Opinion / lecture** — « quelqu’un affirme, interprète, dispute » |
| **Intuition** | Stance (Croire / Contester) | Publication d’une prise de position sur une *théorie* | **Expression publique** — dépôt / signal, pas vérité d’État |

### Ce que signifie « fait » ici

Sur la carte, un point n’est pas « la vérité absolue ».  
C’est une **occurrence** : un type d’événement (naissance, bataille, exil…), un temps typé (année / mois / jour), éventuellement un lieu, et des **preuves** (citations, documents).  

Plusieurs sources peuvent renforcer *la même* occurrence (évidence idempotente) — elles ne multiplient pas les pins pour « gagner » le débat.

### Ce que signifie « opinion » ici

Dans l’Agora, une théorie ou une controverse est une **lecture** : historiographique, polémique, académique.  
Elle peut être mal fondée, brillante, partisane, incomplète.  
Talaria l’**expose** avec son contexte et ses sources ; elle ne la **canonise** pas.

---

## 3. Ce qui est critiquable — et ce qui ne l’est pas de la même façon

### Critiquable (et c’est voulu)

- Toute **théorie** ou **controverse** dans l’Agora.  
- Toute **prise de position** (Croire / Contester) sur Intuition.  
- Toute **interprétation** d’un fait (« était-ce un coup d’État ? », « empoisonnement ou cancer ? »).  
- Le **choix des sources** et leurs angles (Wikipedia n’est pas neutre ; une thèse non plus).

La critique est le **moteur** de l’Agora, pas un bug.

### Discutable techniquement (qualité du pipeline)

- Une date approximative présentée comme précise.  
- Un lieu mal géocodé (ex. Ajaccio placé ailleurs).  
- Un événement mal typé (exil classé ailleurs).  
- Une citation tronquée ou mal attribuée.

Ici la réponse n’est pas « mon opinion contre la tienne » : c’est **corriger la preuve**, la précision, le gate — sans transformer la carte en tribunal.

### Ce que Talaria ne prétend pas trancher

- Qui a « raison » dans un conflit moral ou politique.  
- La « vraie nature » d’un personnage (héros / monstre).  
- L’histoire officielle d’un État, d’un parti ou d’une école.

Dès qu’une affirmation devient **jugement** ou **théorie**, elle quitte le pin et entre dans l’Agora — où elle peut être contestée.

---

## 4. Liberté d’expression : le contrat produit

1. **Pas de vérité unique** — Talaria assemble des traces ; les humains argumentent.  
2. **Séparation stricte** — les faits situés ne portent pas de badge « croyez-moi » Intuition ; les opinions ne créent pas de points de carte.  
3. **Droit de croire et de contester** — sur les théories publiées (ou modélisées) vers Intuition, le geste public est un signal, pas une censure.  
4. **Transparence des sources** — chaque surface renvoie autant que possible à des locators, catalogues, citations.  
5. **Append-only / supersession** — on ne réécrit pas l’histoire en silence ; on corrige en traçant.

En une phrase :  
**la liberté d’opinion n’autorise pas à faire passer une thèse pour un lieu sur la carte — et la carte n’autorise pas à étouffer une thèse.**

---

## 5. Lecture guidée pour une démo

1. **Choisir une figure** (grille d’accueil).  
2. **Explorer** — carte + timeline + légende filtrable : « où et quand », pas « qui a raison ».  
3. **Faits sans lieu** — onglet dédié : la biographie n’est pas seulement géographique.  
4. **Agora** — théories / débats / bibliographie, filtrables par catalogue (HAL, Persée, theses.fr…).  
5. **Intuition** — sur une théorie, Croire / Contester : expression, éventuellement testnet.

Napoléon illustre le duo : densités de faits *et* densités de lectures.  
Les figures modernes (Macron, Trump, De Gaulle, Obama…) montrent que le même contrat s’applique hors « grand récit scolaire ».

---

## 6. Limites honnêtes (pour rester crédibles)

- Le pipeline extrait et filtre ; il **erre**. La démo assume la correction continue des lieux et des types.  
- Les catalogues savants enrichissent l’Agora ; ils ne « votent » pas la carte.  
- Intuition testnet est un **laboratoire d’expression**, pas une cour d’appel historique.  
- Absent de preuve = pas de pin magique. Mieux un fait *sans lieu* qu’un faux GPS.

---

## 7. Formule de clôture

Talaria propose une **discipline du regard** :

> Situer ce qui peut l’être.  
> Discuter ce qui doit l’être.  
> Ne jamais confondre les deux.  
> Et laisser à chacun — sans exception — la liberté de dire ce qu’il croit.

La carte montre des vies dans l’espace.  
L’Agora montre des esprits en débat.  
Intuition laisse une trace de ce que l’on ose soutenir.

---

*Document de doctrine produit — aligné sur le contrat technique : faits dans `canonical_events` (pipeline `person`) ; opinions dans `soft_claims` / Agora ; export Intuition hors carte.*
