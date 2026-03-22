use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::ws::OutboundFrame;

pub type UserId = Uuid;

pub struct AppState {
    pub db: PgPool,
    pub jwt_secret: String,
    pub connections: Arc<RwLock<HashMap<UserId, Vec<mpsc::Sender<OutboundFrame>>>>>,
}

pub type SharedState = Arc<AppState>;
