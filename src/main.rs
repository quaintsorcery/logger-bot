use crate::{bot::run_bot, database::Database};

mod bot;
mod chart;
mod database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    tracing_subscriber::fmt().init();
    let db = Database::new().await?;
    run_bot(db).await
}
