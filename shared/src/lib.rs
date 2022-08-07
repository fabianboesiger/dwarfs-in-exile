use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type UserId = i64;

pub trait CloneState
where
    Self: Sized,
{
    fn clone_state(&self, user_id: UserId) -> Self;
}

pub struct Private<T>(HashMap<UserId, T>);

impl<T> Private<T>
where
    T: CloneState,
{
    fn new() -> Self {
        Private(HashMap::new())
    }

    fn insert(&mut self, user_id: UserId, t: T) {
        self.0.insert(user_id, t);
    }

    fn get(&self, user_id: UserId) -> Option<&T> {
        self.0.get(&user_id)
    }
}

impl<T> CloneState for Private<T>
where
    T: CloneState,
{
    fn clone_state(&self, user_id: UserId) -> Self {
        let mut new = Private::new();
        if let Some(entry) = self.get(user_id) {
            new.insert(user_id, entry.clone_state(user_id));
        }
        new
    }
}

impl<T> CloneState for T
where
    T: Clone,
{
    fn clone_state(&self, _user_id: UserId) -> Self {
        self.clone()
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct State {
    pub cnt: u32,
}

impl State {
    pub fn update(&mut self, event: Event) {
        match event {
            Event::Increment => {
                self.cnt += 1;
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event {
    Increment,
}

impl Event {
    pub fn filter(&self, user_id: UserId) -> bool {
        match self {
            _ => true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Req {
    Event(Event),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Res {
    Sync(State),
    Event(Event),
}
