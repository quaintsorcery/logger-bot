use futures::future::join_all;
use teloxide::{
    prelude::*,
    types::{KeyboardButton, KeyboardMarkup, ReplyMarkup},
    utils::command::BotCommands,
};
use tracing::{debug, error};

use crate::database::Database;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    #[command(description = "Start the bot")]
    Start,
    #[command(description = "Log when you're done")]
    Done,
    #[command(description = "Show your stats")]
    Stats,
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
            return Ok(());
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
                return Ok(());
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
                    return Ok(());
                }
            };
            bot.send_message(chat_id, format!("Your score: {count}"))
                .reply_markup(main_keyboard())
                .await?;
        }
        Command::Leaderboard => {
            let leaderboard = match db.get_leaderboard().await {
                Ok(lb) => lb,
                Err(err) => {
                    error!("Failed to get the leaderboard: {err}");
                    bot.send_message(chat_id, "Database error :(")
                        .reply_markup(main_keyboard())
                        .await?;
                    return Ok(());
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
