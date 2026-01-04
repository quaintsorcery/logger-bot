use std::env;

use sqlx::{SqlitePool, migrate};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new() -> anyhow::Result<Self> {
        let url = env::var("DATABASE_URL")?;
        let pool = SqlitePool::connect(&url).await?;
        migrate!("./migrations/").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn get_user_id(&self, tg_id: i64) -> anyhow::Result<i64> {
        Ok(sqlx::query_scalar!(
            r#"
            INSERT INTO users (telegram_id) VALUES (?)
            ON CONFLICT(telegram_id) DO UPDATE SET telegram_id = telegram_id
            RETURNING id;
            "#,
            tg_id,
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn insert_log(&self, user_id: i64, ts: i64) -> anyhow::Result<()> {
        sqlx::query!(
            "INSERT INTO logs (user_id, timestamp) VALUES (?, ?)",
            user_id,
            ts,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_stats(&self, user_id: i64) -> anyhow::Result<i64> {
        Ok(
            sqlx::query_scalar!("SELECT COUNT(*) FROM logs WHERE user_id = ?;", user_id)
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn get_leaderboard(&self) -> anyhow::Result<Vec<(i64, i64)>> {
        Ok(sqlx::query!(
            r#"
            SELECT u.telegram_id, COUNT(l.id) as logs
            FROM users u
            JOIN logs l on l.user_id = u.id
            GROUP BY u.id
            ORDER BY logs DESC
            LIMIT 10;
            "#,
        )
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| (r.telegram_id, r.logs))
        .collect())
    }

    pub async fn delete_user_data(&self, user_id: i64) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM logs WHERE user_id = ?;
            "#,
            user_id,
        )
        .execute(&self.pool)
        .await?;
        sqlx::query!(
            r#"
            DELETE FROM users WHERE id = ?;
            "#,
            user_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
