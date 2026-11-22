-- Which games exist. Display names, player limits and rules live in the code's game registry.
-- The server adds a row for each game in the registry when it starts
CREATE TABLE game_types (
    id TEXT PRIMARY KEY -- 'bad_takes'
);
