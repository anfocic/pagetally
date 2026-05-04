use crate::config::Config;
use crate::email::Mailer;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: PgPool,
    pub mailer: Option<Mailer>,
}
