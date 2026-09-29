use crate::{
    Action, ActionKind, EngineError, Game, Phase, PrivateState, PublicState, PLAYER_COUNT,
};
use axum::{
    extract::{ws::Message, ws::WebSocket, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    error::Error,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch, Mutex, OwnedSemaphorePermit, Semaphore},
    time::{interval, sleep, timeout},
};
use tower_http::services::ServeDir;

pub const MAX_MESSAGE_BYTES: usize = 4096;
pub const MAX_ROOMS: usize = 32;
pub const MAX_CONNECTIONS: usize = 128;
pub const MAX_ROOM_CONNECTIONS: usize = PLAYER_COUNT;
pub const ROOM_TTL: Duration = Duration::from_secs(5 * 60);
const OUTBOUND_QUEUE: usize = 32;
const RATE_LIMIT: usize = 10;
const RATE_WINDOW: Duration = Duration::from_secs(1);
const FIRST_MESSAGE_DEADLINE: Duration = Duration::from_secs(2);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const PING_INTERVAL: Duration = Duration::from_secs(20);
const DEAD_CONNECTION: Duration = Duration::from_secs(60);

enum WriterCommand {
    Message(Message),
    Close,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ClientMessage {
    Create {
        name: String,
    },
    Join {
        room_code: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        reconnect_token: Option<String>,
    },
    Ready {
        ready: bool,
    },
    Action {
        revision: u64,
        kind: ActionKind,
    },
    Leave,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Created {
        room_code: String,
        seat: u8,
        seat_token: String,
        state: ClientState,
    },
    Joined {
        room_code: String,
        seat: u8,
        seat_token: String,
        state: ClientState,
    },
    State {
        state: ClientState,
    },
    Error {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        state: Option<ClientState>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct ClientState {
    pub room: RoomView,
    pub viewer_seat: u8,
    pub controller: bool,
    pub public: Option<PublicState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<PrivateState>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoomView {
    pub room_code: String,
    pub started: bool,
    pub seats: Vec<SeatView>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SeatView {
    pub seat: u8,
    pub name: String,
    pub connected: bool,
    pub ready: bool,
    pub ai: bool,
    pub watching: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomError {
    pub code: String,
    pub message: String,
    pub close: bool,
}

impl RoomError {
    fn new(code: &str, message: &str, close: bool) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            close,
        }
    }
}

impl IntoResponse for RoomError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            serde_json::to_string(&ServerMessage::Error {
                code: self.code,
                message: self.message,
                state: None,
            })
            .unwrap_or_else(|_| "{\"type\":\"error\"}".to_string()),
        )
            .into_response()
    }
}

#[derive(Clone)]
struct Connection {
    generation: u64,
    controller: bool,
    outbound: mpsc::Sender<WriterCommand>,
    shutdown: watch::Sender<bool>,
}

struct Seat {
    name: String,
    token: String,
    ready: bool,
    connection: Option<Connection>,
}

pub struct Room {
    code: String,
    seats: [Option<Seat>; PLAYER_COUNT],
    game: Option<Game>,
    next_generation: u64,
    ttl: Duration,
    last_owner_disconnect: Option<Instant>,
    ai_running: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Attachment {
    seat: u8,
    generation: u64,
}

#[derive(Debug)]
struct CommandResult {
    close: bool,
}

impl Room {
    pub fn new(code: String) -> Self {
        Self::with_ttl(code, ROOM_TTL)
    }

    pub fn with_ttl(code: String, ttl: Duration) -> Self {
        Self {
            code,
            seats: std::array::from_fn(|_| None),
            game: None,
            next_generation: 0,
            ttl,
            last_owner_disconnect: Some(Instant::now()),
            ai_running: false,
        }
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        self.seats
            .iter()
            .all(|seat| seat.as_ref().is_none_or(|seat| seat.connection.is_none()))
            && self
                .last_owner_disconnect
                .is_some_and(|left| now.duration_since(left) >= self.ttl)
    }

    fn connect_new(
        &mut self,
        name: String,
        outbound: mpsc::Sender<WriterCommand>,
        shutdown: watch::Sender<bool>,
    ) -> Result<(Attachment, String), RoomError> {
        validate_name(&name)?;
        if self.game.is_some() {
            return Err(RoomError::new(
                "ROOM_STARTED",
                "牌局已開始，請使用重連憑證",
                true,
            ));
        }
        let seat = self
            .seats
            .iter()
            .position(Option::is_none)
            .ok_or_else(|| RoomError::new("ROOM_FULL", "房間已滿", true))?;
        let token = random_hex(32)?;
        let generation = self.next_generation();
        self.seats[seat] = Some(Seat {
            name,
            token: token.clone(),
            ready: false,
            connection: Some(Connection {
                generation,
                controller: true,
                outbound,
                shutdown,
            }),
        });
        self.refresh_expiry(Instant::now());
        Ok((
            Attachment {
                seat: seat as u8,
                generation,
            },
            token,
        ))
    }

    fn connect_token(
        &mut self,
        token: &str,
        outbound: mpsc::Sender<WriterCommand>,
        shutdown: watch::Sender<bool>,
    ) -> Result<(Attachment, String), RoomError> {
        let seat = self
            .seats
            .iter()
            .position(|seat| seat.as_ref().is_some_and(|seat| seat.token == token))
            .ok_or_else(|| RoomError::new("INVALID_TOKEN", "重連憑證無效", true))?;
        if self.seats[seat]
            .as_ref()
            .is_some_and(|owner| owner.connection.is_some())
        {
            return Err(RoomError::new(
                "DUPLICATE_CONNECTION",
                "此座位已有連線",
                true,
            ));
        }
        let generation = self.next_generation();
        let owner = self.seats[seat].as_mut().expect("seat was found");
        owner.connection = Some(Connection {
            generation,
            controller: self.game.is_none(),
            outbound,
            shutdown,
        });
        let token = owner.token.clone();
        self.refresh_expiry(Instant::now());
        Ok((
            Attachment {
                seat: seat as u8,
                generation,
            },
            token,
        ))
    }

    fn next_generation(&mut self) -> u64 {
        self.next_generation = self.next_generation.saturating_add(1);
        self.next_generation
    }

    fn disconnect(&mut self, attachment: Attachment) -> Result<bool, RoomError> {
        self.disconnect_inner(attachment, true)
    }

    fn disconnect_for_error(&mut self, attachment: Attachment) -> Result<bool, RoomError> {
        self.disconnect_inner(attachment, false)
    }

    fn disconnect_inner(
        &mut self,
        attachment: Attachment,
        stop_writer: bool,
    ) -> Result<bool, RoomError> {
        let Some(owner) = self
            .seats
            .get_mut(attachment.seat as usize)
            .and_then(Option::as_mut)
        else {
            return Ok(false);
        };
        if owner
            .connection
            .as_ref()
            .is_none_or(|connection| connection.generation != attachment.generation)
        {
            return Ok(false);
        }
        if let Some(connection) = owner.connection.take() {
            if stop_writer {
                let _ = connection.shutdown.send(true);
            }
        }
        self.refresh_expiry(Instant::now());
        self.maybe_start()?;
        Ok(true)
    }

    fn handle(
        &mut self,
        attachment: Attachment,
        message: ClientMessage,
    ) -> Result<CommandResult, RoomError> {
        self.check_attachment(attachment)?;
        match message {
            ClientMessage::Ready { ready } => {
                if self
                    .game
                    .as_ref()
                    .is_some_and(|game| !matches!(game.phase(), Phase::Result))
                {
                    return Err(RoomError::new(
                        "HAND_ACTIVE",
                        "牌局進行中不能更改準備狀態",
                        false,
                    ));
                }
                self.seats[attachment.seat as usize]
                    .as_mut()
                    .expect("checked attachment")
                    .ready = ready;
                self.maybe_start()?;
                Ok(CommandResult { close: false })
            }
            ClientMessage::Action { revision, kind } => {
                let controller = self.seats[attachment.seat as usize]
                    .as_ref()
                    .and_then(|seat| seat.connection.as_ref())
                    .is_some_and(|connection| connection.controller);
                if !controller {
                    return Err(RoomError::new(
                        "WATCH_ONLY",
                        "重連觀察者要等下一局才能操作",
                        false,
                    ));
                }
                let game = self.game.as_mut().ok_or_else(|| {
                    RoomError::new("NOT_STARTED", "四位玩家準備後才能開始", false)
                })?;
                if matches!(game.phase(), Phase::Result) {
                    return Err(RoomError::new(
                        "NOT_IN_HAND",
                        "請在結算畫面準備下一局",
                        false,
                    ));
                }
                game.apply(Action {
                    revision,
                    actor: attachment.seat,
                    kind,
                })
                .map_err(engine_error)?;
                Ok(CommandResult { close: false })
            }
            ClientMessage::Leave => {
                if self.game.is_some() {
                    return Err(RoomError::new(
                        "HAND_ACTIVE",
                        "牌局開始後不能離開座位",
                        false,
                    ));
                }
                self.seats[attachment.seat as usize] = None;
                self.refresh_expiry(Instant::now());
                Ok(CommandResult { close: true })
            }
            ClientMessage::Create { .. } | ClientMessage::Join { .. } => {
                Err(RoomError::new("PROTOCOL", "連線已建立，不能重複加入", true))
            }
        }
    }

    fn maybe_start(&mut self) -> Result<(), RoomError> {
        if self.game.is_none() {
            let ready = self.seats.iter().all(|seat| {
                seat.as_ref()
                    .is_some_and(|seat| seat.ready && seat.connection.is_some())
            });
            if !ready {
                return Ok(());
            }
            let mut game = Game::new_secure().map_err(engine_error)?;
            for (seat, owner) in self.seats.iter().enumerate() {
                game.set_player_name(seat as u8, owner.as_ref().expect("ready seat").name.clone())
                    .map_err(engine_error)?;
            }
            game.start().map_err(engine_error)?;
            self.game = Some(game);
            self.clear_ready_and_restore_controllers();
            return Ok(());
        }

        let Some(game) = self.game.as_ref() else {
            return Ok(());
        };
        if !matches!(game.phase(), Phase::Result) {
            return Ok(());
        }
        let connected = self
            .seats
            .iter()
            .filter(|seat| seat.as_ref().is_some_and(|seat| seat.connection.is_some()))
            .count();
        let ready = connected > 0
            && self.seats.iter().all(|seat| {
                seat.as_ref()
                    .map(|seat| seat.connection.is_none() || seat.ready)
                    .unwrap_or(true)
            });
        if ready {
            self.game
                .as_mut()
                .expect("game exists")
                .next_hand()
                .map_err(engine_error)?;
            self.clear_ready_and_restore_controllers();
        }
        Ok(())
    }

    fn clear_ready_and_restore_controllers(&mut self) {
        for owner in self.seats.iter_mut().flatten() {
            owner.ready = false;
            if let Some(connection) = owner.connection.as_mut() {
                connection.controller = true;
            }
        }
    }

    fn check_attachment(&self, attachment: Attachment) -> Result<(), RoomError> {
        let valid = self
            .seats
            .get(attachment.seat as usize)
            .and_then(|seat| seat.as_ref())
            .and_then(|seat| seat.connection.as_ref())
            .is_some_and(|connection| connection.generation == attachment.generation);
        valid
            .then_some(())
            .ok_or_else(|| RoomError::new("STALE_CONNECTION", "連線已失效", true))
    }

    fn refresh_expiry(&mut self, now: Instant) {
        if self
            .seats
            .iter()
            .all(|seat| seat.as_ref().is_none_or(|seat| seat.connection.is_none()))
        {
            if self.last_owner_disconnect.is_none() {
                self.last_owner_disconnect = Some(now);
            }
        } else {
            self.last_owner_disconnect = None;
        }
    }

    fn controller(&self, seat: usize) -> bool {
        self.seats[seat]
            .as_ref()
            .and_then(|seat| seat.connection.as_ref())
            .is_some_and(|connection| connection.controller)
    }

    fn ai_seat(&self) -> Option<u8> {
        let game = self.game.as_ref()?;
        match *game.phase() {
            Phase::NeedDraw { seat } | Phase::NeedDiscard { seat } => {
                (!self.controller(seat as usize)).then_some(seat)
            }
            Phase::Claim { discarder, .. } => (1..PLAYER_COUNT)
                .map(|offset| (discarder as usize + offset) % PLAYER_COUNT)
                .map(|seat| seat as u8)
                .find(|&seat| {
                    !self.controller(seat as usize) && !game.legal_action_kinds(seat).is_empty()
                }),
            _ => None,
        }
    }

    fn run_ai_once(&mut self) -> Result<(), EngineError> {
        let Some(seat) = self.ai_seat() else {
            return Ok(());
        };
        self.game
            .as_mut()
            .expect("AI seat requires a game")
            .bot_step_for(seat)
            .map(|_| ())
    }

    fn state_for(&self, seat: u8) -> ClientState {
        let controller = self
            .seats
            .get(seat as usize)
            .and_then(|owner| owner.as_ref())
            .and_then(|owner| owner.connection.as_ref())
            .is_some_and(|connection| connection.controller);
        ClientState {
            room: self.room_view(),
            viewer_seat: seat,
            controller,
            public: self.game.as_ref().map(Game::public_state),
            private: controller
                .then(|| self.game.as_ref()?.private_state(seat).ok())
                .flatten(),
        }
    }

    fn state_for_error(&self, attachment: Attachment, error: &RoomError) -> Option<ClientState> {
        (error.code != "STALE_CONNECTION").then(|| self.state_for(attachment.seat))
    }

    fn room_view(&self) -> RoomView {
        RoomView {
            room_code: self.code.clone(),
            started: self.game.is_some(),
            seats: (0..PLAYER_COUNT)
                .map(|seat| {
                    let owner = self.seats[seat].as_ref();
                    let connected = owner.and_then(|owner| owner.connection.as_ref()).is_some();
                    let watching = connected && !self.controller(seat);
                    SeatView {
                        seat: seat as u8,
                        name: owner
                            .map(|owner| owner.name.clone())
                            .unwrap_or_else(|| "空位".to_string()),
                        connected,
                        ready: owner.is_some_and(|owner| owner.ready),
                        ai: self.game.is_some() && !connected,
                        watching,
                    }
                })
                .collect(),
        }
    }

    fn broadcast_state(&mut self) {
        let mut deliveries = Vec::new();
        for seat in 0..PLAYER_COUNT {
            let Some(owner) = self.seats[seat].as_ref() else {
                continue;
            };
            let Some(connection) = owner.connection.as_ref() else {
                continue;
            };
            deliveries.push((
                seat as u8,
                connection.generation,
                connection.outbound.clone(),
                ServerMessage::State {
                    state: self.state_for(seat as u8),
                },
            ));
        }
        for (seat, generation, outbound, message) in deliveries {
            if !queue_message(&outbound, &message)
                && self.seats[seat as usize]
                    .as_ref()
                    .and_then(|owner| owner.connection.as_ref())
                    .is_some_and(|connection| connection.generation == generation)
            {
                let owner = self.seats[seat as usize].as_mut().expect("seat exists");
                if let Some(connection) = owner.connection.take() {
                    let _ = connection.shutdown.send(true);
                }
            }
        }
        self.refresh_expiry(Instant::now());
    }
}

fn queue_message(outbound: &mpsc::Sender<WriterCommand>, message: &ServerMessage) -> bool {
    let Ok(json) = serde_json::to_string(message) else {
        return false;
    };
    outbound
        .try_send(WriterCommand::Message(Message::Text(json.into())))
        .is_ok()
}

fn validate_name(name: &str) -> Result<(), RoomError> {
    let length = name.chars().count();
    if !(1..=24).contains(&length) || name.trim().is_empty() || name.chars().any(char::is_control) {
        return Err(RoomError::new(
            "INVALID_NAME",
            "名稱需為 1 到 24 個可見字元",
            true,
        ));
    }
    Ok(())
}

fn validate_room_code(code: &str) -> Result<(), RoomError> {
    if code.len() != 16 || !code.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RoomError::new("INVALID_ROOM", "房間代碼格式無效", true));
    }
    Ok(())
}

fn random_hex(bytes: usize) -> Result<String, RoomError> {
    let mut value = vec![0; bytes];
    rand::rngs::OsRng
        .try_fill_bytes(&mut value)
        .map_err(|_| RoomError::new("RANDOM_UNAVAILABLE", "安全亂數暫時不可用", true))?;
    Ok(value.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn engine_error(error: EngineError) -> RoomError {
    let code = match error {
        EngineError::StaleRevision => "STALE_REVISION",
        EngineError::NotYourTurn => "NOT_YOUR_TURN",
        EngineError::InvalidJson(_) => "INVALID_ACTION",
        _ => "ACTION_REJECTED",
    };
    RoomError::new(code, &error.to_string(), false)
}

struct Session {
    room: Arc<Mutex<Room>>,
    attachment: Attachment,
}

struct Registry {
    rooms: HashMap<String, Arc<Mutex<Room>>>,
}

#[derive(Clone)]
struct AppState {
    registry: Arc<Mutex<Registry>>,
    connection_slots: Arc<Semaphore>,
}

impl Registry {
    fn new() -> Self {
        Self {
            rooms: HashMap::new(),
        }
    }
}

impl AppState {
    fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(Registry::new())),
            connection_slots: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
        }
    }
}

async fn create_session(
    app: &AppState,
    name: String,
    outbound: mpsc::Sender<WriterCommand>,
    shutdown: watch::Sender<bool>,
) -> Result<(Session, ServerMessage), RoomError> {
    validate_name(&name)?;
    let mut registry = app.registry.lock().await;
    if registry.rooms.len() >= MAX_ROOMS {
        return Err(RoomError::new("ROOM_LIMIT", "目前房間已滿", true));
    }
    let code = loop {
        let code = random_hex(8)?;
        if !registry.rooms.contains_key(&code) {
            break code;
        }
    };
    let mut room = Room::new(code.clone());
    let (attachment, token) = room.connect_new(name, outbound.clone(), shutdown)?;
    let state = room.state_for(attachment.seat);
    let room = Arc::new(Mutex::new(room));
    registry.rooms.insert(code.clone(), room.clone());
    let session = Session { room, attachment };
    Ok((
        session,
        ServerMessage::Created {
            room_code: code,
            seat: attachment.seat,
            seat_token: token,
            state,
        },
    ))
}

async fn join_session(
    app: &AppState,
    room_code: String,
    name: Option<String>,
    token: Option<String>,
    outbound: mpsc::Sender<WriterCommand>,
    shutdown: watch::Sender<bool>,
) -> Result<(Session, ServerMessage), RoomError> {
    validate_room_code(&room_code)?;
    if token.is_none() {
        validate_name(name.as_deref().unwrap_or(""))?;
    }
    let registry = app.registry.lock().await;
    let room = registry
        .rooms
        .get(&room_code)
        .cloned()
        .ok_or_else(|| RoomError::new("ROOM_NOT_FOUND", "找不到房間，可能已過期", true))?;
    let mut room_guard = room.lock().await;
    let (attachment, seat_token) = if let Some(token) = token {
        room_guard.connect_token(&token, outbound.clone(), shutdown)?
    } else {
        let (attachment, token) =
            room_guard.connect_new(name.expect("validated name"), outbound.clone(), shutdown)?;
        (attachment, token)
    };
    let state = room_guard.state_for(attachment.seat);
    drop(room_guard);
    let session = Session { room, attachment };
    let message = ServerMessage::Joined {
        room_code,
        seat: attachment.seat,
        seat_token,
        state,
    };
    Ok((session, message))
}

async fn disconnect_session(session: &Session) -> Result<(), RoomError> {
    {
        let mut room = session.room.lock().await;
        room.disconnect(session.attachment)?;
        room.broadcast_state();
    }
    Ok(())
}

async fn disconnect_after_error(session: &Session) -> Result<(), RoomError> {
    {
        let mut room = session.room.lock().await;
        room.disconnect_for_error(session.attachment)?;
        room.broadcast_state();
    }
    Ok(())
}

async fn schedule_ai(room: Arc<Mutex<Room>>) {
    let should_start = {
        let mut room_guard = room.lock().await;
        if room_guard.ai_running || room_guard.ai_seat().is_none() {
            false
        } else {
            room_guard.ai_running = true;
            true
        }
    };
    if !should_start {
        return;
    }
    tokio::spawn(async move {
        loop {
            let continue_running = {
                let mut room_guard = room.lock().await;
                let worked = room_guard.run_ai_once().is_ok();
                room_guard.broadcast_state();
                let continue_running = worked && room_guard.ai_seat().is_some();
                if !continue_running {
                    room_guard.ai_running = false;
                }
                continue_running
            };
            if !continue_running {
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }
    });
}

async fn prune_expired(app: &AppState) {
    let mut registry = app.registry.lock().await;
    let now = Instant::now();
    let entries: Vec<(String, Arc<Mutex<Room>>)> = registry
        .rooms
        .iter()
        .map(|(code, room)| (code.clone(), room.clone()))
        .collect();
    for (code, room) in entries {
        if room.lock().await.is_expired(now) {
            registry.rooms.remove(&code);
        }
    }
}

async fn send_server_message(
    outbound: &mpsc::Sender<WriterCommand>,
    message: &ServerMessage,
) -> bool {
    let message = WriterCommand::Message(Message::Text(
        serde_json::to_string(message)
            .unwrap_or_else(|_| "{\"type\":\"error\"}".to_string())
            .into(),
    ));
    timeout(WRITE_TIMEOUT, outbound.send(message))
        .await
        .is_ok_and(|result| result.is_ok())
}

async fn send_writer_command(
    outbound: &mpsc::Sender<WriterCommand>,
    command: WriterCommand,
) -> bool {
    timeout(WRITE_TIMEOUT, outbound.send(command))
        .await
        .is_ok_and(|result| result.is_ok())
}

fn report_writer_result(result: Result<Result<(), &'static str>, tokio::task::JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => eprintln!("websocket writer stopped: {error}"),
        Err(error) if error.is_cancelled() => {}
        Err(error) => eprintln!("websocket writer task failed: {error}"),
    }
}

async fn finish_writer(
    writer: &mut tokio::task::JoinHandle<Result<(), &'static str>>,
    shutdown: &watch::Sender<bool>,
    outbound: &mpsc::Sender<WriterCommand>,
    close: bool,
) {
    if close {
        if !send_writer_command(outbound, WriterCommand::Close).await {
            let _ = shutdown.send(true);
        }
    } else {
        let _ = shutdown.send(true);
    }
    let result = match timeout(WRITE_TIMEOUT, &mut *writer).await {
        Ok(result) => result,
        Err(_) => {
            writer.abort();
            (&mut *writer).await
        }
    };
    report_writer_result(result);
}

async fn initial_message(
    app: &AppState,
    first: ClientMessage,
    outbound: mpsc::Sender<WriterCommand>,
    shutdown: watch::Sender<bool>,
) -> Result<(Session, ServerMessage), RoomError> {
    match first {
        ClientMessage::Create { name } => create_session(app, name, outbound, shutdown).await,
        ClientMessage::Join {
            room_code,
            name,
            reconnect_token,
        } => join_session(app, room_code, name, reconnect_token, outbound, shutdown).await,
        _ => Err(RoomError::new(
            "PROTOCOL",
            "第一個訊息必須是建立或加入房間",
            true,
        )),
    }
}

fn parse_message(text: &str) -> Result<ClientMessage, RoomError> {
    if text.len() > MAX_MESSAGE_BYTES {
        return Err(RoomError::new("MESSAGE_TOO_LARGE", "訊息資料過大", true));
    }
    serde_json::from_str(text).map_err(|_| RoomError::new("INVALID_MESSAGE", "訊息格式無效", true))
}

async fn websocket(mut socket: WebSocket, app: AppState, _permit: OwnedSemaphorePermit) {
    let first = match timeout(FIRST_MESSAGE_DEADLINE, socket.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => match parse_message(&text) {
            Ok(message) => message,
            Err(error) => {
                let _ = send_socket_message(
                    &mut socket,
                    &ServerMessage::Error {
                        code: error.code,
                        message: error.message,
                        state: None,
                    },
                )
                .await;
                let _ = timeout(WRITE_TIMEOUT, socket.send(Message::Close(None))).await;
                return;
            }
        },
        _ => return,
    };

    let (outbound, mut outbound_rx) = mpsc::channel(OUTBOUND_QUEUE);
    let (shutdown, mut shutdown_rx) = watch::channel(false);
    let writer_shutdown = shutdown_rx.clone();
    let (mut sender, mut receiver) = socket.split();
    let mut writer = tokio::spawn(async move {
        let mut writer_shutdown = writer_shutdown;
        loop {
            tokio::select! {
                _ = writer_shutdown.changed() => break,
                command = outbound_rx.recv() => {
                    let Some(command) = command else { break };
                    match command {
                        WriterCommand::Message(message) => {
                            let sent = timeout(WRITE_TIMEOUT, sender.send(message)).await;
                            if !matches!(sent, Ok(Ok(()))) {
                                return Err("message write failed");
                            }
                        }
                        WriterCommand::Close => {
                            let sent = timeout(WRITE_TIMEOUT, sender.send(Message::Close(None))).await;
                            if !matches!(sent, Ok(Ok(()))) {
                                return Err("close write failed");
                            }
                            return Ok(());
                        }
                    }
                }
            }
        }
        Ok(())
    });
    let mut writer_done = false;
    let (session, hello) =
        match initial_message(&app, first, outbound.clone(), shutdown.clone()).await {
            Ok(result) => result,
            Err(error) => {
                let _ = send_server_message(
                    &outbound,
                    &ServerMessage::Error {
                        code: error.code,
                        message: error.message,
                        state: None,
                    },
                )
                .await;
                finish_writer(&mut writer, &shutdown, &outbound, true).await;
                return;
            }
        };
    if !send_server_message(&outbound, &hello).await {
        let _ = disconnect_session(&session).await;
        schedule_ai(session.room.clone()).await;
        finish_writer(&mut writer, &shutdown, &outbound, false).await;
        return;
    }
    {
        let mut room = session.room.lock().await;
        room.broadcast_state();
    }
    schedule_ai(session.room.clone()).await;

    let mut heartbeat = interval(PING_INTERVAL);
    let mut last_received = Instant::now();
    let mut message_window = Instant::now();
    let mut message_count = 0;
    let mut intentional_close = false;
    let mut close_after_error = false;
    loop {
        tokio::select! {
            result = &mut writer => {
                writer_done = true;
                report_writer_result(result);
                break;
            }
            changed = shutdown_rx.changed() => {
                let _ = changed;
                break;
            }
            incoming = receiver.next() => {
                let Some(Ok(message)) = incoming else { break };
                match message {
                    Message::Text(text) => {
                        last_received = Instant::now();
                        let now = Instant::now();
                        if now.duration_since(message_window) >= RATE_WINDOW {
                            message_window = now;
                            message_count = 0;
                        }
                        message_count += 1;
                        if message_count > RATE_LIMIT {
                            let error = ServerMessage::Error {
                                code: "RATE_LIMIT".to_string(),
                                message: "訊息頻率過高".to_string(),
                                state: None,
                            };
                            let _ = send_server_message(&outbound, &error).await;
                            close_after_error = true;
                            break;
                        }
                        let message = match parse_message(&text) {
                            Ok(message) => message,
                            Err(error) => {
                                let _ = send_server_message(&outbound, &ServerMessage::Error {
                                    code: error.code,
                                    message: error.message,
                                    state: None,
                                }).await;
                                close_after_error = true;
                                break;
                            }
                        };
                        let result = {
                            let mut room = session.room.lock().await;
                            match room.handle(session.attachment, message) {
                                Ok(result) => {
                                    room.broadcast_state();
                                    Ok(result)
                                }
                                Err(error) => {
                                    let state = room.state_for_error(session.attachment, &error);
                                    Err((error, state))
                                }
                            }
                        };
                        match result {
                            Ok(result) => {
                                schedule_ai(session.room.clone()).await;
                                if result.close {
                                    intentional_close = true;
                                    break;
                                }
                            }
                            Err((error, state)) => {
                                let close = error.close;
                                let _ = send_server_message(&outbound, &ServerMessage::Error {
                                    code: error.code,
                                    message: error.message,
                                    state,
                                }).await;
                                if close {
                                    close_after_error = true;
                                    break;
                                }
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        let _ = outbound.try_send(WriterCommand::Message(Message::Pong(payload)));
                        last_received = Instant::now();
                    }
                    Message::Pong(_) => last_received = Instant::now(),
                    Message::Binary(_) => break,
                    Message::Close(_) => break,
                }
            }
            _ = heartbeat.tick() => {
                if Instant::now().duration_since(last_received) >= DEAD_CONNECTION {
                    break;
                }
                if outbound
                    .try_send(WriterCommand::Message(Message::Ping(Vec::new().into())))
                    .is_err()
                {
                    break;
                }
            }
        }
    }
    if !intentional_close {
        if close_after_error {
            let _ = disconnect_after_error(&session).await;
        } else {
            let _ = disconnect_session(&session).await;
        }
        schedule_ai(session.room.clone()).await;
    }
    if !writer_done {
        finish_writer(&mut writer, &shutdown, &outbound, close_after_error).await;
    }
}

async fn send_socket_message(socket: &mut WebSocket, message: &ServerMessage) -> bool {
    let json =
        serde_json::to_string(message).unwrap_or_else(|_| "{\"type\":\"error\"}".to_string());
    timeout(WRITE_TIMEOUT, socket.send(Message::Text(json.into())))
        .await
        .is_ok_and(|result| result.is_ok())
}

async fn ws_handler(upgrade: WebSocketUpgrade, State(app): State<AppState>) -> Response {
    let permit = match app.connection_slots.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "connections full\n").into_response(),
    };
    upgrade
        .max_frame_size(MAX_MESSAGE_BYTES)
        .max_message_size(MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| websocket(socket, app, permit))
        .into_response()
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok\n")
}

pub async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let address: SocketAddr = format!("0.0.0.0:{port}").parse()?;
    let app_state = AppState::new();
    let prune_state = app_state.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30));
        loop {
            ticker.tick().await;
            prune_expired(&prune_state).await;
        }
    });
    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("dist"))
        .with_state(app_state);
    let listener = TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_connection() -> (mpsc::Sender<WriterCommand>, watch::Sender<bool>) {
        let (outbound, _) = mpsc::channel(OUTBOUND_QUEUE);
        let (shutdown, _) = watch::channel(false);
        (outbound, shutdown)
    }

    #[test]
    fn names_and_codes_are_bounded() {
        assert!(validate_name("Alice").is_ok());
        assert!(validate_name("\n").is_err());
        assert!(validate_name(&"x".repeat(25)).is_err());
        assert!(validate_room_code("0123456789abcdef").is_ok());
        assert!(validate_room_code("short").is_err());
    }

    #[test]
    fn empty_room_expires_from_last_disconnect() {
        let ttl = Duration::from_secs(300);
        let room = Room::with_ttl("0123456789abcdef".to_string(), ttl);
        let now = Instant::now();
        assert!(!room.is_expired(now + Duration::from_secs(299)));
        assert!(room.is_expired(now + Duration::from_secs(300)));
    }

    #[test]
    fn four_ready_owners_start_a_room_and_reconnect_is_watch_only() {
        let mut room = Room::with_ttl("0123456789abcdef".to_string(), ROOM_TTL);
        let mut attachments = Vec::new();
        let mut tokens = Vec::new();
        for seat in 0..PLAYER_COUNT {
            let (outbound, shutdown) = test_connection();
            let (attachment, token) = room
                .connect_new(format!("P{seat}"), outbound, shutdown)
                .expect("seat should be reserved");
            attachments.push(attachment);
            tokens.push(token);
        }
        for attachment in attachments.iter().take(PLAYER_COUNT - 1) {
            room.handle(*attachment, ClientMessage::Ready { ready: true })
                .unwrap();
            assert!(room.game.is_none());
        }
        room.handle(
            *attachments.last().unwrap(),
            ClientMessage::Ready { ready: true },
        )
        .unwrap();
        assert!(room.game.is_some());

        let (outbound, _) = mpsc::channel(OUTBOUND_QUEUE);
        assert_eq!(
            room.connect_token(&tokens[0], outbound, watch::channel(false).0)
                .unwrap_err()
                .code,
            "DUPLICATE_CONNECTION"
        );
        for seat in [0usize, 1] {
            let old = attachments[seat];
            assert!(room.disconnect(old).unwrap());
            assert!(room.room_view().seats[seat].ai);
            let (outbound, shutdown) = test_connection();
            let (reconnected, returned_token) = room
                .connect_token(&tokens[seat], outbound, shutdown)
                .unwrap();
            assert_eq!(returned_token, tokens[seat]);
            assert_ne!(old.generation, reconnected.generation);
            assert!(room.state_for(seat as u8).private.is_none());
            let json = serde_json::to_string(&room.state_for(seat as u8)).unwrap();
            assert!(!json.contains(&tokens[seat]));
            assert_eq!(
                room.handle(
                    reconnected,
                    ClientMessage::Action {
                        revision: 0,
                        kind: ActionKind::Pass,
                    },
                )
                .unwrap_err()
                .code,
                "WATCH_ONLY"
            );
            assert_eq!(
                room.handle(reconnected, ClientMessage::Ready { ready: true })
                    .unwrap_err()
                    .code,
                "HAND_ACTIVE"
            );
            assert!(room.room_view().seats[seat].watching);
            assert!(!room.room_view().seats[seat].ai);
            assert!(!room.disconnect(old).unwrap());
            assert!(room.disconnect(reconnected).unwrap());
        }

        let (outbound, shutdown) = test_connection();
        assert_eq!(
            room.connect_new("Late".to_string(), outbound, shutdown)
                .unwrap_err()
                .code,
            "ROOM_STARTED"
        );
    }

    #[test]
    fn invalid_tokens_and_stale_generations_cannot_reclaim_a_seat() {
        let mut room = Room::with_ttl("0123456789abcdef".to_string(), ROOM_TTL);
        let (outbound, shutdown) = test_connection();
        let (old, token) = room
            .connect_new("Owner".to_string(), outbound, shutdown)
            .unwrap();
        let (outbound, shutdown) = test_connection();
        assert_eq!(
            room.connect_token("not-a-token", outbound, shutdown)
                .unwrap_err()
                .code,
            "INVALID_TOKEN"
        );
        assert!(room.disconnect(old).unwrap());
        let (outbound, shutdown) = test_connection();
        let fresh = room.connect_token(&token, outbound, shutdown).unwrap().0;
        assert!(!room.disconnect(old).unwrap());
        assert_eq!(
            room.handle(old, ClientMessage::Ready { ready: true })
                .unwrap_err()
                .code,
            "STALE_CONNECTION"
        );
        let error = room
            .handle(old, ClientMessage::Ready { ready: true })
            .unwrap_err();
        assert!(room.state_for_error(old, &error).is_none());
        assert!(room.disconnect(fresh).unwrap());
    }

    #[test]
    fn result_readiness_and_disconnect_start_the_next_hand_safely() {
        let mut room = Room::with_ttl("0123456789abcdef".to_string(), ROOM_TTL);
        let mut attachments = Vec::new();
        let mut tokens = Vec::new();
        for seat in 0..PLAYER_COUNT {
            let (outbound, shutdown) = test_connection();
            let (attachment, token) = room
                .connect_new(format!("P{seat}"), outbound, shutdown)
                .unwrap();
            attachments.push(attachment);
            tokens.push(token);
        }
        room.game = Some(Game::fixture(
            std::array::from_fn(|_| Vec::new()),
            std::array::from_fn(|_| Vec::new()),
            Vec::new(),
            Phase::Result,
            0,
            None,
            None,
            None,
            false,
        ));
        for owner in room.seats.iter_mut().flatten() {
            owner.ready = false;
        }

        room.disconnect(attachments[0]).unwrap();
        let (outbound, shutdown) = test_connection();
        let watcher = room
            .connect_token(&tokens[0], outbound, shutdown)
            .unwrap()
            .0;
        room.handle(watcher, ClientMessage::Ready { ready: true })
            .unwrap();
        assert!(matches!(room.game.as_ref().unwrap().phase(), Phase::Result));
        assert!(room.state_for(0).private.is_none());
        assert!(!room.state_for(0).controller);

        for attachment in attachments.iter().take(3).skip(1) {
            room.handle(*attachment, ClientMessage::Ready { ready: true })
                .unwrap();
        }
        room.disconnect(attachments[3]).unwrap();
        assert!(!matches!(
            room.game.as_ref().unwrap().phase(),
            Phase::Result
        ));
        assert!(room.room_view().seats[3].ai);
        assert!(room.state_for(0).controller);
        assert!(room.state_for(0).private.is_some());
    }

    #[tokio::test]
    async fn websocket_admission_has_a_fixed_resource_bound() {
        let app = AppState::new();
        let mut permits = Vec::new();
        for _ in 0..MAX_CONNECTIONS {
            permits.push(app.connection_slots.clone().try_acquire_owned().unwrap());
        }
        assert!(app.connection_slots.try_acquire().is_err());
        drop(permits);
        assert!(app.connection_slots.try_acquire().is_ok());
    }

    #[test]
    fn all_disconnected_result_room_does_not_start_next_hand() {
        let mut room = Room::with_ttl("0123456789abcdef".to_string(), ROOM_TTL);
        let mut attachments = Vec::new();
        for seat in 0..PLAYER_COUNT {
            let (outbound, shutdown) = test_connection();
            let (attachment, _) = room
                .connect_new(format!("P{seat}"), outbound, shutdown)
                .expect("seat should be reserved");
            attachments.push(attachment);
        }
        for attachment in attachments {
            assert!(room.disconnect(attachment).unwrap());
        }
        room.game = Some(Game::fixture(
            std::array::from_fn(|_| Vec::new()),
            std::array::from_fn(|_| Vec::new()),
            Vec::new(),
            Phase::Result,
            0,
            None,
            None,
            None,
            false,
        ));
        let before = room.game.as_ref().unwrap().public_state();
        room.maybe_start().unwrap();
        assert_eq!(room.game.as_ref().unwrap().public_state(), before);
        assert!(room.is_expired(Instant::now() + ROOM_TTL));
    }
}
