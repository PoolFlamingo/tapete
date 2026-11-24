-- El formato del deck (Commander, construido, limitado) elegido a mano. NULL: el
-- que deduce `proxy_core::stats::deducir` de las cartas. Aparte de `updated_at`.
ALTER TABLE decks ADD COLUMN kind_override TEXT;
