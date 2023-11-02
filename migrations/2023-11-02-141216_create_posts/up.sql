-- Your SQL goes here

-- User contains ip, username and gender
create table user
(
    ip       varchar primary key,
    username text not null,
    gender   text not null

)