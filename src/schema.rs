table! {
    server (id) {
        id -> Int4,
        name -> Text,
    }
}

table! {
    user (id) {
        id -> Int4,
        discord_id -> Int4,
        timezone -> Text,
    }
}

table! {
    user_server (id) {
        id -> Int4,
        user_id -> Int4,
        server_id -> Int4,
    }
}

joinable!(user_server -> server (server_id));
joinable!(user_server -> user (user_id));

allow_tables_to_appear_in_same_query!(server, user, user_server,);
