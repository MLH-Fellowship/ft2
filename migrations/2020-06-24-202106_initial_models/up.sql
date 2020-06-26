-- Your SQL goes here
create table if not exists "server" (
    id serial primary key,
    name text not null
);
create table if not exists "user" (
    id serial primary key,
    discord_id bigint not null,
    timezone text not null
);
create table if not exists "user_server" (
    id serial primary key,
    user_id integer not null references "user" (id) on delete cascade,
    server_id integer not null references "server" (id) on delete cascade
)
