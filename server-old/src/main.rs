#[macro_use]
extern crate diesel;

mod schema;

use ws::{listen, CloseCode, Handler, Request, Response, Result, Sender};

use diesel::prelude::*;
use diesel::pg::PgConnection;
//use diesel::{Queryable, Insertable};
use dotenv::dotenv;
use std::env;
use schema::messages;

pub fn establish_connection() -> PgConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

#[derive(Queryable)]
pub struct Message {
    pub id: i32,
    pub body: String,
}

#[derive(Insertable)]
#[table_name="messages"]
pub struct NewMessage<'a> {
    pub body: &'a str,
}

struct Server {
    out: Sender,
    connection: PgConnection,
}

impl Handler for Server {
    fn on_request(&mut self, req: &Request) -> Result<Response> {
        let res = match req.resource() {
            "/ws" => Response::from_request(req),
            _ => Ok(Response::new(
                200,
                "OK",
                b"Websocket server is running".to_vec(),
            )),
        };

        res
    }

    // Handle messages recieved in the websocket (in this case, only on `/ws`).
    fn on_message(&mut self, msg: ws::Message) -> Result<()> {
        let client_id: usize = self.out.token().into();

        let server_msg = if msg.is_text() {
            Some(handle_text_message(client_id, msg))
        } else if msg.is_binary() {
            Some(handle_binary_message(client_id, msg))
        } else {
            None
        };

        // Broadcast to all connections.
        server_msg.map_or(Ok(()), |msg| {
            

            let new_message = NewMessage {
                body: msg.as_text().unwrap()
            };
        
            diesel::insert_into(messages::table)
                .values(&new_message)
                .execute(&self.connection)
                .expect("Error saving new message");

            self.out.broadcast(msg)
        })
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        let client_id: usize = self.out.token().into();
        let code_number: u16 = code.into();
        println!(
            "WebSocket closing - client: {}, code: {} {:?}, reason: {}",
            client_id, code_number, code, reason
        );
    }
}

/*
fn handle_text_message(client_id: usize, msg: ws::Message) -> ws::Message {
    let client_msg: shared::ClientMessage =
        serde_json::from_str(&msg.into_text().unwrap()).unwrap();

    println!(
        "Server received text message\ntext: '{}'\nfrom: '{}'\n",
        client_msg.text, client_id
    );

    let server_msg: ws::Message = serde_json::to_string(&shared::ServerMessage {
        id: client_id,
        text: client_msg.text,
    })
    .unwrap()
    .into();

    server_msg
}
*/

fn handle_binary_message(client_id: usize, msg: ws::Message) -> ws::Message {
    let binary_msg: shared::ClientMessage = rmp_serde::from_slice(&msg.into_data()).unwrap();

    println!(
        "Server received binary message\ntext: '{}'\nfrom: '{}'\n",
        binary_msg.text, client_id
    );

    let server_msg: ws::Message = rmp_serde::to_vec(&shared::ServerMessage {
        id: client_id,
        text: binary_msg.text,
    })
    .unwrap()
    .into();

    server_msg
}

fn main() {
    // Listen on an address and call the closure for each connection
    listen("127.0.0.1:9000", |out| Server { out, connection: establish_connection() }).unwrap();
}
