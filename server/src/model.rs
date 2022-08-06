pub struct Session {
    pub session_id: String,
    pub username: String
}

pub struct User {
    pub username: String,
    pub password: String,
}

pub struct World {
    pub name: String,
    pub data: Vec<u8>,
}