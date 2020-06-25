#[macro_use]
extern crate diesel;
#[macro_use]
extern crate serenity;
#[macro_use]
extern crate lazy_static;
extern crate chrono_tz;

use chrono::prelude::*;
use serenity::prelude::*;

use serenity::framework::standard::macros::*;

mod schema;

use regex::Regex;
use serenity::framework::standard::{CheckResult, CommandError, CommandOptions, CommandResult};
use serenity::model::channel::{Message, Reaction};

lazy_static! {
    static ref DATE_REGEX: Regex = Regex::new(r"(\d{2}|\d)+[.:]+(\d{2})+(pm|am)?").unwrap();
}

#[group]
#[commands(set_timezone, set_bot_timezone)]
struct General;

#[command]
#[checks(Admin)]
fn set_bot_timezone(ctx: &mut Context, msg: &Message) -> CommandResult {
    use diesel::prelude::*;
    if msg.mentions.is_empty() {
        return Err(CommandError(String::from(
            "You need to @mention at least one bot.",
        )));
    }
    let timezone = msg.content.split("`").nth(1).unwrap();
    let _: chrono_tz::Tz = timezone.parse().unwrap();
    let mut data = ctx.data.write();
    let pool = data.get_mut::<PooledConnection>().unwrap();
    for mention in &msg.mentions {
        if mention.bot {
            if !user_is_in_database(&mention.id.0, &pool) {
                diesel::insert_into(schema::user::dsl::user).values(NewUser {
                    discord_id: mention.id.0 as i32,
                    timezone: &msg.content,
                });
            } else {
                diesel::update(schema::user::dsl::user)
                    .set(schema::user::dsl::timezone.eq(msg.clone().content));
            };
        }
    }
    Ok(())
}

use diesel::expression::exists;
use schema::user;
use serenity::framework::standard::Args;

#[derive(Insertable)]
#[table_name = "user"]
struct NewUser<'a> {
    discord_id: i32,
    timezone: &'a str,
}

#[command]
fn set_timezone(ctx: &mut Context, msg: &Message) -> CommandResult {
    let mut data = ctx.data.write();
    let pool = data.get_mut::<PooledConnection>().unwrap();
    let stripped_timezone = msg.content.replace("~set_timezone ", "");
    let _: chrono_tz::Tz = stripped_timezone.as_str().parse().unwrap();
    use diesel::expression::exists;
    use diesel::prelude::*;
    use schema::user::dsl as user_dsl;
    if !user_is_in_database(&msg.author.id.0, &pool) {
        diesel::insert_into(schema::user::dsl::user)
            .values(NewUser {
                discord_id: msg.author.id.0 as i32,
                timezone: &stripped_timezone,
            })
            .execute(&pool.get().unwrap())
            .unwrap();
    } else {
        diesel::update(schema::user::dsl::user)
            .set(schema::user::dsl::timezone.eq(msg.clone().content))
            .filter(schema::user::dsl::discord_id.eq(msg.author.id.0 as i32))
            .execute(&pool.get().unwrap());
    };
    msg.reply(&ctx, format!("Your timezone was successfully set to '{}'.", stripped_timezone));
    Ok(())
}

fn user_is_in_database(user_id: &u64, pool: &Pool) -> bool {
    use diesel::expression::exists;
    use diesel::prelude::*;
    use schema::user::dsl as user_dsl;
    diesel::select(exists::exists(
        user_dsl::user.filter(user_dsl::discord_id.eq(*user_id as i32)),
    ))
    .get_result::<bool>(&pool.get().unwrap())
    .unwrap()
}

type Pool = diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::PgConnection>>;

struct PooledConnection(Pool);

impl TypeMapKey for PooledConnection {
    type Value = Pool;
}

struct Handler;

#[derive(Queryable)]
struct User {
    id: i32,
    discord_id: i32,
    timezone: String,
}

enum PmAm {
    Pm,
    Am,
    None,
}

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        if msg.content.starts_with("~") {
            return;
        }
        use diesel::prelude::*;
        use schema::user::dsl::*;
        let mut data = ctx.data.write();
        let pool = data.get_mut::<PooledConnection>().unwrap();
        let conn = pool.get().unwrap();
        let resulting_user: Result<User, diesel::result::Error> = user
            .filter(discord_id.eq(msg.author.id.0 as i32))
            .first::<User>(&conn);
        if let Ok(found_user) = resulting_user {
            let mentioned_dates = DATE_REGEX.captures_iter(&msg.content);
            if mentioned_dates.count() > 0 {
                msg.react(&ctx, "⏰");
            }
        } else {
            if msg.author.bot {
                return;
            }
            msg.reply(&ctx, format!("Hi {} – you haven't set your timezone yet. DM this bot with a (canonical) timezone from this list https://en.wikipedia.org/wiki/List_of_tz_database_time_zones, e.g. `~set_timezone Europe/London`", msg.author.name)).unwrap();
        }
    }
    fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        use diesel::prelude::*;
        use schema::user::dsl as user_dsl;
        if ctx.http.get_user(add_reaction.user_id.0).unwrap().bot {
            return;
        }
        if add_reaction.emoji.as_data() != "⏰" {
            return;
        }
        let mut data = ctx.data.write();
        let pool = data.get_mut::<PooledConnection>().unwrap();
        let conn = pool.get().unwrap();
        let message = ctx
            .http
            .get_message(add_reaction.channel_id.0, add_reaction.message_id.0)
            .unwrap();
        let sending_user = match user_dsl::user
            .filter(user_dsl::discord_id.eq(message.author.id.0 as i32))
            .first::<User>(&conn)
        {
            Ok(u) => u,
            Err(_) => return,
        };
        let reacting_user = match user_dsl::user
            .filter(user_dsl::discord_id.eq(add_reaction.user_id.0 as i32))
            .first::<User>(&conn)
        {
            Ok(u) => u,
            Err(e) => {
                if let diesel::result::Error::NotFound = e {
                    add_reaction
                        .user(&ctx)
                        .unwrap()
                        .create_dm_channel(&ctx)
                        .unwrap()
                        .send_message(&ctx, |c| {
                            c.content("Hi, you reacted to a message but haven't set your timezone.")
                        });
                }
                return;
            }
        };
        let reacting_user_timezone: chrono_tz::Tz = reacting_user.timezone.parse().unwrap();
        let sending_user_timezone: chrono_tz::Tz = sending_user.timezone.parse().unwrap();
        let sending_user_current_time =
            sending_user_timezone.from_utc_datetime(&chrono::Utc::now().naive_utc());
        let mut output = Vec::new();
        for time in DATE_REGEX.captures_iter(&message.content) {
            let hours: u32 = match &time[1].parse() {
                Ok(t) => *t,
                Err(_) => return,
            };
            let minutes: u32 = match &time[2].parse() {
                Ok(t) => *t,
                Err(_) => return,
            };
            let am_pm: PmAm = match &time[3] {
                "am" => PmAm::Am,
                "pm" => PmAm::Pm,
                _ => PmAm::None,
            };
            if let PmAm::Am | PmAm::Pm = am_pm {
                if hours > 12 {
                    return;
                }
            }
            let sending_user_message_time = sending_user_timezone
                .ymd(
                    sending_user_current_time.year(),
                    sending_user_current_time.month(),
                    sending_user_current_time.day(),
                )
                .and_hms(
                    match am_pm {
                        PmAm::Am => hours,
                        PmAm::Pm => hours + 12,
                        PmAm::None => hours,
                    },
                    minutes,
                    0,
                );
            let receiving_message_user_time =
                sending_user_message_time.with_timezone(&reacting_user_timezone);
            output.push(format!(
                "The time {} was mentioned – in your timezone this was {}",
                sending_user_message_time, receiving_message_user_time
            ));
        }
        add_reaction
            .user_id
            .create_dm_channel(&ctx)
            .unwrap()
            .send_message(&ctx, |c| c.content(output.join("\n")));
    }
}

#[check]
#[name = "Admin"]
#[check_in_help(true)]
#[display_in_help(true)]
fn admin_check(ctx: &mut Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> CheckResult {
    if let Some(member) = msg.member(&ctx.cache) {
        if let Ok(permissions) = member.permissions(&ctx.cache) {
            return permissions.administrator().into();
        }
    }
    false.into()
}

fn main() {
    let pool: Pool = diesel::r2d2::Pool::new(diesel::r2d2::ConnectionManager::new(
        &std::env::var("DATABASE_URL").expect("No `DATABASE_URL` environment variable set."),
    ))
    .expect("Failed to build pool.");
    let mut client = Client::new(
        &std::env::var("DISCORD_TOKEN").expect("Missing token"),
        Handler,
    )
    .expect("Error creating client");
    client.with_framework(
        serenity::framework::standard::StandardFramework::new()
            .configure(|c| c.prefix("~").allow_dm(true))
            .group(&GENERAL_GROUP),
    );
    {
        let mut data = client.data.write();
        data.insert::<PooledConnection>(pool);
    }
    if let Err(why) = client.start() {
        println!("Error starting the Discord client: {:?}", why);
    }
}