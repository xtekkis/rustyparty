-- A named set of cards for one game, like the Bad Takes starter deck
CREATE TABLE decks (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    game_type_id TEXT NOT NULL REFERENCES game_types (id),
    name         TEXT NOT NULL CHECK (name <> ''),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (game_type_id, name)
);

-- Prompts and answers are separate tables because only prompts have pick.
-- One table with a nullable pick would make a column depend on the card type
CREATE TABLE prompts (
    id        BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    deck_id   BIGINT NOT NULL REFERENCES decks (id),
    text      TEXT NOT NULL CHECK (text <> ''),
    pick      SMALLINT NOT NULL CHECK (pick IN (1, 2)),
    -- False once the card is taken out of the deck. Old rounds may still point to it
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (deck_id, text)
);

CREATE TABLE answers (
    id        BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    deck_id   BIGINT NOT NULL REFERENCES decks (id),
    text      TEXT NOT NULL CHECK (text <> ''),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (deck_id, text)
);
