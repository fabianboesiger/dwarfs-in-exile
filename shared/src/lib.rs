use rand::{rngs::SmallRng, seq::SliceRandom, Rng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};

#[cfg(not(debug_assertions))]
const TICKS_PER_MINUTE: u32 = 60;
#[cfg(debug_assertions)]
const TICKS_PER_MINUTE: u32 = 600;


pub type UserId = i64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EventData {
    pub event: Event,
    pub user_id: Option<UserId>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Req {
    Event(Event),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Res {
    Sync(SyncData),
    Event(EventData),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SyncData {
    pub user_id: UserId,
    pub state: State,
}

// MODIFY EVENTS AND STATE BELOW

use std::{collections::{HashMap}, hash::Hash};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct State {
    pub players: HashMap<UserId, Player>,
}

impl State {
    pub fn update(&mut self, EventData { event, user_id }: EventData) {
        match event {
            Event::Tick(seed) => {

            },
            Event::AddPlayer(user_id, username) => {
                let mut player = Player {
                    username,
                    money: 0,
                    notifications: Vec::new()
                };

                player.add_notification(Notification { message: String::from("You made an inheritance!"), money: 1000000 });

                self.players.insert(
                    user_id,
                    player
                );
            }
            Event::EditPlayer(user_id, username) => {
                self.players.get_mut(&user_id).unwrap().username = username;
            }
            Event::RemovePlayer(user_id) => {
                self.players.remove(&user_id);
            }
            Event::RandRes(seed, event) => {
                let mut rng = SmallRng::seed_from_u64(seed);

                match event {
                    
                }
            },
            Event::RandReq(_) => unreachable!(),
            Event::ReadNotification(idx) => {
                let user = self.players.get_mut(&user_id.unwrap()).unwrap();
                user.notifications.remove(idx);
            }
        }
    }

    pub fn view(&self, _receiver: UserId) -> Self {
        State { ..self.clone() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Player {
    pub username: String,
    pub money: Money,
    pub notifications: Vec<Notification>,
}

impl Player {
    pub fn add_notification(&mut self, notification: Notification) {
        self.money += notification.money;
        self.notifications.push(notification);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Notification {
    pub message: String,
    pub money: Money,
}


pub type Money = i64;
pub type Seed = u64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event {
    RandRes(Seed, RandEvent),
    RandReq(RandEvent),
    Tick(Seed),
    AddPlayer(UserId, String),
    EditPlayer(UserId, String),
    RemovePlayer(UserId),
    ReadNotification(usize)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RandEvent {
}

impl EventData {
    pub fn filter(&self, _receiver: UserId) -> bool {
        /*
        let EventData { event, user_id } = self;
        let user_id = *user_id;

        match event {
            _ => true,
        }
        */
        true
    }
}
