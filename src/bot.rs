use futures::future::join_all;
use teloxide::{
    prelude::*,
    types::{InputFile, KeyboardButton, KeyboardMarkup, ReplyMarkup},
    utils::command::BotCommands,
};
use tracing::{debug, error};

use crate::{
    chart::{generate_personal_annual_chart, generate_personal_hourly_chart},
    database::Database,
};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    #[command(description = "Start the bot")]
    Start,
    #[command(description = "Log when you're done")]
    Done,
    #[command(description = "Show your stats")]
    Stats,
    #[command(description = "Show your annual stats")]
    AnnualStats,
    #[command(description = "Show your hourly stats")]
    HourlyStats,
    #[command(description = "Show the leaderboard")]
    Leaderboard,
    #[command(description = "Delete all your data")]
    Delete,
}

fn main_keyboard() -> ReplyMarkup {
    let keyboard = KeyboardMarkup::new(vec![
        vec![KeyboardButton::new("/done")],
        vec![
            KeyboardButton::new("/stats"),
            KeyboardButton::new("/leaderboard"),
        ],
        vec![
            KeyboardButton::new("/annualstats"),
            KeyboardButton::new("/hourlystats"),
        ],
    ])
    .resize_keyboard();
    ReplyMarkup::Keyboard(keyboard)
}

pub async fn run_bot(database: Database) -> anyhow::Result<()> {
    let bot = Bot::from_env();

    let handler = Update::filter_message()
        .filter_command::<Command>()
        .endpoint(handle_command);
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![database])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    Ok(())
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    command: Command,
    db: Database,
) -> ResponseResult<()> {
    let user = match msg.from {
        Some(u) => u,
        None => return respond(()),
    };
    let chat_id = msg.chat.id;
    let user_id = match db.get_user_id(user.id.0 as i64).await {
        Ok(id) => id,
        Err(err) => {
            error!("Failed to get user ID from the DB: {err}");
            bot.send_message(chat_id, "Database error :(")
                .reply_markup(main_keyboard())
                .await?;
            return respond(());
        }
    };

    match command {
        Command::Start => {
            bot.send_message(chat_id, &Command::descriptions().to_string())
                .reply_markup(main_keyboard())
                .await?;
        }
        Command::Done => {
            let ts = msg.date.timestamp();
            if let Err(err) = db.insert_log(user_id, ts).await {
                error!("Failed to insert a log for the user {user_id}: {err}");
                bot.send_message(chat_id, "Database error :(")
                    .reply_markup(main_keyboard())
                    .await?;
                return respond(());
            }
            bot.send_message(chat_id, "👍")
                .reply_markup(main_keyboard())
                .await?;
        }
        Command::Stats => {
            let count = match db.get_user_stats(user_id).await {
                Ok(c) => c,
                Err(err) => {
                    error!("Failed to get stats for the user {user_id}: {err}");
                    bot.send_message(chat_id, "Database error :(")
                        .reply_markup(main_keyboard())
                        .await?;
                    return respond(());
                }
            };
            bot.send_message(chat_id, format!("Your score: {count}"))
                .reply_markup(main_keyboard())
                .await?;
        }
        Command::AnnualStats => {
            let timestamps = match db.get_all_user_timestamps(user_id).await {
                Ok(ts) => ts,
                Err(err) => {
                    error!("Failed to get timestamps for the user {user_id}: {err}");
                    bot.send_message(chat_id, "Database error :(")
                        .reply_markup(main_keyboard())
                        .await?;
                    return respond(());
                }
            };
            let username = match bot.get_chat(user.id).await {
                Ok(chat) => chat.username().map(|u| u.to_string()),
                Err(err) => {
                    debug!("Failed to get the username for {user_id}: {err}");
                    None
                }
            };
            let name = username.unwrap_or_else(|| user.id.to_string());
            match generate_personal_annual_chart(&name, timestamps, None) {
                Ok(png_bytes) => {
                    bot.send_photo(chat_id, InputFile::memory(png_bytes))
                        .await?;
                }
                Err(err) => {
                    error!("Failed to generate the chart for {user_id}: {err}");
                    bot.send_message(chat_id, "Error generating the chart :(")
                        .reply_markup(main_keyboard())
                        .await?;
                    return respond(());
                }
            }
        }
        Command::HourlyStats => {
            let timestamps = match db.get_all_user_timestamps(user_id).await {
                Ok(ts) => ts,
                Err(err) => {
                    error!("Failed to get timestamps for the user {user_id}: {err}");
                    bot.send_message(chat_id, "Database error :(")
                        .reply_markup(main_keyboard())
                        .await?;
                    return respond(());
                }
            };
            let username = match bot.get_chat(user.id).await {
                Ok(chat) => chat.username().map(|u| u.to_string()),
                Err(err) => {
                    debug!("Failed to get the username for {user_id}: {err}");
                    None
                }
            };
            let name = username.unwrap_or_else(|| user.id.to_string());
            match generate_personal_hourly_chart(&name, timestamps) {
                Ok(png_bytes) => {
                    bot.send_photo(chat_id, InputFile::memory(png_bytes))
                        .await?;
                }
                Err(err) => {
                    error!("Failed to generate the chart for {user_id}: {err}");
                    bot.send_message(chat_id, "Error generating the chart :(")
                        .reply_markup(main_keyboard())
                        .await?;
                    return respond(());
                }
            }
        }
        Command::Leaderboard => {
            let leaderboard = match db.get_leaderboard().await {
                Ok(lb) => lb,
                Err(err) => {
                    error!("Failed to get the leaderboard: {err}");
                    bot.send_message(chat_id, "Database error :(")
                        .reply_markup(main_keyboard())
                        .await?;
                    return respond(());
                }
            };
            let futures = leaderboard.iter().enumerate().map(|(i, r)| {
                let bot = bot.clone();
                async move {
                    let username = match bot.get_chat(ChatId(r.0)).await {
                        Ok(chat) => chat.username().map(|u| u.to_string()),
                        Err(err) => {
                            debug!("Failed to get the username for {}: {err}", r.0);
                            None
                        }
                    };

                    let name = username.unwrap_or_else(|| r.0.to_string());
                    format!("{}. @{name} - {}\n", i + 1, r.1)
                }
            });
            let mut text: String = join_all(futures).await.concat();
            if text.is_empty() {
                text = "The leaderboard is empty".into();
            }
            bot.send_message(chat_id, text)
                .reply_markup(main_keyboard())
                .await?;
        }
        Command::Delete => {
            if let Err(err) = db.delete_user_data(user_id).await {
                error!("Failed to delete data for the user {user_id}: {err}");
                bot.send_message(chat_id, "Database error :(")
                    .reply_markup(main_keyboard())
                    .await?;
                return Ok(());
            }
            bot.send_message(chat_id, "All your data has been deleted")
                .reply_markup(main_keyboard())
                .await?;
        }
    }
    respond(())
}
