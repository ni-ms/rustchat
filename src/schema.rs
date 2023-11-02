// @generated automatically by Diesel CLI.

diesel::table! {
    user (ip) {
        ip -> Nullable<Text>,
        username -> Text,
        gender -> Text,
    }
}
