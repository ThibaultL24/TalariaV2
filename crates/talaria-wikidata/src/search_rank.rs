// crates/talaria-wikidata/src/search_rank.rs
//! POC EntitySearchRanker: prefer humans, penalize artworks/statues.

use crate::WikidataSearchHit;

pub fn person_search_score(label: &str, description: Option<&str>) -> i32 {
    let desc = description.unwrap_or("").to_lowercase();
    let label = label.to_lowercase();
    let mut score = 0;
    if desc.split(|c: char| !c.is_alphabetic()).any(|w| {
        matches!(
            w,
            "human"
                | "person"
                | "politician"
                | "military"
                | "emperor"
                | "empress"
                | "king"
                | "queen"
                | "monarch"
                | "writer"
                | "composer"
                | "scientist"
                | "philosopher"
                | "explorer"
                | "physician"
                | "physicist"
                | "chemist"
                | "mathematician"
                | "president"
                | "journalist"
                | "poet"
                | "novelist"
                | "historian"
                | "general"
                | "admiral"
                | "officer"
                | "artist"
                | "painter"
                | "architect"
                | "actor"
                | "actress"
                | "singer"
                | "duke"
                | "prince"
                | "princess"
                | "cardinal"
                | "pope"
                | "saint"
                | "pharaoh"
                | "humain"
                | "personne"
                | "politicien"
                | "politicienne"
                | "militaire"
                | "empereur"
                | "imperatrice"
                | "impératrice"
                | "roi"
                | "reine"
                | "monarque"
                | "ecrivain"
                | "écrivain"
                | "compositeur"
                | "scientifique"
                | "philosophe"
                | "explorateur"
                | "medecin"
                | "médecin"
                | "physicien"
                | "chimiste"
                | "mathematicien"
                | "mathématicien"
                | "president"
                | "président"
                | "journaliste"
                | "poete"
                | "poète"
                | "romancier"
                | "historien"
                | "general"
                | "général"
                | "amiral"
                | "officier"
                | "artiste"
                | "peintre"
                | "architecte"
                | "acteur"
                | "actrice"
                | "chanteur"
                | "duc"
                | "princesse"
                | "pape"
                | "pharaon"
        )
    }) || desc.contains("prime minister")
        || desc.contains("computer scientist")
        || desc.contains("homme politique")
        || desc.contains("femme politique")
        || desc.contains("personnalité")
    {
        score += 50;
    }
    if desc.contains("sculpture")
        || desc.contains("statue")
        || desc.contains("painting")
        || desc.contains("artwork")
        || desc.contains("bust")
        || desc.contains("wikimedia category")
        || desc.contains("fictional character")
        || desc.contains("asteroid")
        || desc.contains("taxon")
        || desc.contains("species of")
        || desc.contains("chemical compound")
        || desc.contains("video game")
        || desc.contains("album")
        || desc.contains("film")
        || desc.contains("ship named")
        || desc.contains("catégorie wikimedia")
        || desc.contains("personnage de fiction")
        || desc.contains("espèce de")
        || desc.contains("composé chimique")
        || desc.contains("jeu vidéo")
        || desc.contains("navire nommé")
    {
        score -= 70;
    }
    if desc.contains("located in")
        || desc.contains("depicts")
        || desc.contains("work by")
        || desc.contains("statue of")
        || desc.contains("œuvre de")
        || desc.contains("situé à")
        || desc.contains("située à")
    {
        score -= 35;
    }
    let _ = label;
    score
}

pub fn sort_person_search_hits(mut hits: Vec<WikidataSearchHit>, known_qids: &[String]) -> Vec<WikidataSearchHit> {
    hits.sort_by(|a, b| {
        let a_local = known_qids.iter().any(|q| q == &a.qid);
        let b_local = known_qids.iter().any(|q| q == &b.qid);
        b_local
            .cmp(&a_local)
            .then_with(|| {
                person_search_score(&b.label, b.description.as_deref())
                    .cmp(&person_search_score(&a.label, a.description.as_deref()))
            })
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_outranks_statue() {
        let human = person_search_score("Napoleon", Some("French military officer"));
        let statue = person_search_score("Napoleon", Some("statue located in Paris"));
        assert!(human > statue, "human={human} statue={statue}");
    }

    #[test]
    fn scientist_outranks_taxon_and_artwork() {
        let human = person_search_score("Marie Curie", Some("Polish-French physicist and chemist"));
        let taxon = person_search_score("Curie", Some("species of beetle"));
        let painting = person_search_score("Marie Curie", Some("painting by a student"));
        assert!(human > taxon && human > painting, "human={human} taxon={taxon} painting={painting}");
    }

    #[test]
    fn french_human_outranks_statue() {
        let human = person_search_score("Napoléon", Some("empereur des Français et militaire"));
        let statue = person_search_score("Napoléon", Some("statue située à Paris"));
        assert!(human > statue, "human={human} statue={statue}");
    }
}
