#!/usr/bin/env bash
# scripts/seed-demo-sections.sh — offline FR Naissance-style section for Napoleon dossier
set -o errexit -o pipefail
export TZ='Asia/Jakarta'
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
source .env

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 <<'SQL'
INSERT INTO wiki_pages (page_id, title, wiki_lang, revision_id, content_hash, raw_path)
VALUES (51701, 'Napoléon Ier', 'fr', 1366416084, 'seed-napoleon-fr-sections', '')
ON CONFLICT (wiki_lang, title) DO UPDATE SET revision_id = EXCLUDED.revision_id;

DELETE FROM wiki_sections
WHERE wiki_page_id = (
  SELECT id FROM wiki_pages WHERE wiki_lang = 'fr' AND title = 'Napoléon Ier'
);

INSERT INTO wiki_sections (wiki_page_id, ordinal, title, text)
SELECT p.id, v.ordinal, v.title, v.body
FROM wiki_pages p
CROSS JOIN (
  VALUES
  (0, 'Lead', 'Napoléon Bonaparte, né le 15 août 1769 à Ajaccio et mort le 5 mai 1821 sur l''île de Sainte-Hélène, est un militaire et homme d''État français.'),
  (1, 'Naissance et origines',
   $txt$Napoléon Bonaparte naît à Ajaccio le 15 août 1769, le jour de la Sainte-Marie (patronne de la Corse), dans la maison familiale, aujourd'hui transformée en musée. Napoléon naît un an après le traité de Versailles, par lequel la république de Gênes cède l'administration de la Corse à la France (l'île sera ensuite intégrée à la France en 1789). Ondoyé à domicile, il a pour nom de baptême Napoleone Buonaparte (prénom donné en mémoire d'un oncle décédé à Corte en 1767), et n'est baptisé à la cathédrale Notre-Dame-de-l'Assomption d'Ajaccio que le 21 juillet 1771. La famille Bonaparte est d'origine italienne et passée en Corse à la fin du XVe siècle.$txt$),
  (2, 'Enfance et formation',
   $txt$Il grandit à Ajaccio avant de quitter l'île pour suivre une formation militaire sur le continent. En 1779, il entre à l'école militaire de Brienne, puis à l'École militaire de Paris. Ces années forgent sa discipline et son attachement à l'artillerie.$txt$)
) AS v(ordinal, title, body)
WHERE p.wiki_lang = 'fr' AND p.title = 'Napoléon Ier';
SQL

echo "seeded offline wiki sections"
psql "$DATABASE_URL" -c "SELECT wp.title, wp.wiki_lang, ws.ordinal, ws.title AS section FROM wiki_sections ws JOIN wiki_pages wp ON wp.id = ws.wiki_page_id ORDER BY wp.title, ws.ordinal;"
