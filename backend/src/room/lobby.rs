use tokio::sync::mpsc::WeakSender;
use tokio::time::Instant;

use super::actor::{Connection, RoomCommand, remind, send};
use super::player::is_known_avatar;
use super::{
    DISCONNECT_GRACE, MAX_ROOM_PLAYERS, Room, RoomState, clean_nickname, is_valid_points_to_win,
};
use crate::protocol::ServerMessage;

impl Room {
    // Add a new player, or give a returning player their seat back
    pub(super) fn join(
        &mut self,
        nickname: String,
        avatar: Option<String>,
        token: Option<String>,
        connection: Connection,
        me: &WeakSender<RoomCommand>,
    ) -> Option<u32> {
        // A saved token means this phone was here before
        if let Some(token) = token {
            if self.is_banned(&token) {
                send(
                    &connection,
                    ServerMessage::error("banned_from_room", "You were banned from this room"),
                );
                return None;
            }
            if let Some(id) = self.reconnect(&token, connection.clone()) {
                tracing::info!("Player {id} reconnected to room {}", self.code);
                send(
                    &connection,
                    ServerMessage::Welcome {
                        player_id: id,
                        token,
                    },
                );
                self.broadcast_players();
                // Back mid-game: show them where things are right away
                self.send_game_view_to(id);
                return Some(id);
            }
        }

        let error = |code, message| {
            send(&connection, ServerMessage::error(code, message));
            None
        };
        // Players in their grace period still hold a seat
        if self.players.len() >= MAX_ROOM_PLAYERS {
            return error("room_full", "This room is full");
        }
        let Some(nickname) = clean_nickname(&nickname) else {
            return error("invalid_nickname", "Nickname must be 1 to 16 characters");
        };
        if !is_known_avatar(avatar.as_deref()) {
            return error("unknown_avatar", "That avatar doesn't exist");
        }
        if self.has_nickname(&nickname) {
            return error("nickname_taken", "That nickname is taken");
        }

        let player = self.add_player(nickname, avatar, connection.clone());
        let (id, token) = (player.id, player.token.clone());
        tracing::info!("Player {id} joined room {}", self.code);
        // Welcome first, so the phone knows its id before the lobby arrives
        send(
            &connection,
            ServerMessage::Welcome {
                player_id: id,
                token,
            },
        );
        self.broadcast_players();
        self.tell_game_about_late_joiner(id, me);
        Some(id)
    }

    pub(super) fn set_avatar(&mut self, player_id: u32, avatar: Option<String>) {
        if !is_known_avatar(avatar.as_deref()) {
            self.send_error(player_id, "unknown_avatar", "That avatar doesn't exist");
            return;
        }
        if let Some(player) = self.player_mut(player_id) {
            player.avatar = avatar;
            self.broadcast(&self.lobby_message());
        }
    }

    pub(super) fn set_ready(&mut self, player_id: u32, ready: bool) {
        if !matches!(self.state, RoomState::Lobby) {
            self.send_error(
                player_id,
                "not_in_lobby",
                "That can only be changed in the lobby",
            );
            return;
        }
        if let Some(player) = self.player_mut(player_id) {
            player.ready = ready;
            tracing::debug!("Player {player_id} ready: {ready}");
            self.broadcast(&self.lobby_message());
        }
    }

    pub(super) fn set_points_to_win(&mut self, player_id: u32, points_to_win: u8) {
        if !is_valid_points_to_win(points_to_win) {
            self.send_error(
                player_id,
                "invalid_points_to_win",
                "Points to win must be 3 to 15",
            );
            return;
        }
        if self.host != Some(player_id) {
            self.send_error(player_id, "not_host", "Only the host can do that");
            return;
        }
        if !matches!(self.state, RoomState::Lobby) {
            self.send_error(
                player_id,
                "not_in_lobby",
                "That can only be changed in the lobby",
            );
            return;
        }
        self.settings.points_to_win = points_to_win;
        self.broadcast(&self.lobby_message());
    }

    // Host kicks or bans another player
    pub(super) fn remove_by_host(
        &mut self,
        player_id: u32,
        target_id: u32,
        ban: bool,
        me: &WeakSender<RoomCommand>,
    ) {
        if self.host != Some(player_id) {
            self.send_error(player_id, "not_host", "Only the host can do that");
            return;
        }
        if target_id == player_id {
            self.send_error(
                player_id,
                "cannot_remove_yourself",
                "You can't remove yourself",
            );
            return;
        }

        let removed = if ban {
            self.ban_player(target_id)
        } else {
            self.kick_player(target_id)
        };
        if !removed {
            self.send_error(
                player_id,
                "player_not_found",
                "That player isn't in the room",
            );
            return;
        }

        let action = if ban { "banned" } else { "kicked" };
        tracing::info!("Player {target_id} was {action} from room {}", self.code);
        self.broadcast_players();
        self.tell_game_about_leaver(target_id, me);
    }

    pub(super) fn leave(&mut self, player_id: u32, me: &WeakSender<RoomCommand>) {
        if self.remove_player(player_id) {
            tracing::info!("Player {player_id} left room {}", self.code);
            self.broadcast_players();
            self.tell_game_about_leaver(player_id, me);
        }
    }

    // Keep the seat for a while, the player can come back with their token
    pub(super) fn disconnect(
        &mut self,
        player_id: u32,
        connection: &Connection,
        me: &WeakSender<RoomCommand>,
    ) {
        let since = Instant::now();
        let Some(player) = self.player_mut(player_id) else {
            return;
        };
        // Skip if a newer connection has taken over this seat
        if !player.sender.same_channel(connection) {
            return;
        }
        player.disconnected_since = Some(since);
        tracing::info!("Player {player_id} disconnected from room {}", self.code);
        self.broadcast(&self.lobby_message());

        remind(
            me,
            DISCONNECT_GRACE,
            RoomCommand::GraceOver { player_id, since },
        );
    }

    // Nobody is ready again until they choose to be, or the vote would start straight away
    pub(super) fn return_to_lobby(&mut self) {
        self.state = RoomState::Lobby;
        for player in &mut self.players {
            player.ready = false;
        }
        tracing::info!("Room {} is back in the lobby", self.code);
        self.broadcast_players();
    }
}
