# rustyparty

A real-time party game platform built in Rust. Players join a room from their phone with a short code, vote on a game, and play together live.

This project is in early development. Right now only the backend server exists.

## How it works

- **No app, no account.** Everyone plays in their phone's browser. One player creates a room and gets a 4-letter code like `KXTP`.
- **Easy to join.** Friends type the code, scan a QR code, or open a shared link. They pick a nickname and an avatar.
- **Vote on the game.** When everyone is ready, the room votes on which game to play. Votes show up live.
- **Play again.** After a game the scores are shown and the room goes back to the lobby, so the same group can play again or pick another game.
- **Drop-in friendly.** If your phone locks or the connection drops, you rejoin with your seat, cards and score. Players who arrive late join at the next round.

## Bad Takes

The first game is **Bad Takes**, an adult fill-in-the-blank card game for 3 to 10 players.

Each round one player is the judge and reveals a prompt with a blank. Everyone else secretly plays the funniest answer card from their hand. The judge reads the answers without knowing who played what and picks a favorite. That player gets a point, and the judge role moves to the next player. The first to reach the target score wins.

## Built to grow

Every game is a separate module on a shared engine. Rooms, voting, reconnecting and timers work the same for every game, so new games can be added without changing the rest. Ideas for later include a drawing game, trivia, and a shared screen mode where a TV shows the game and phones become controllers.

## Tech

- **Backend:** Rust, Axum, Tokio
- **Frontend (coming later):** React, TypeScript, Vite
