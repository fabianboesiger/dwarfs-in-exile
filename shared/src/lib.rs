use serde::{Deserialize, Serialize};

pub type UserId = i64;

/*
pub trait CloneState
where
    Self: Sized,
{
    fn clone_state(&self, user_id: UserId) -> Self;
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Private<T> {
    owner: UserId,
    content: Option<T>,
}

impl<T> Private<T>
where
    T: CloneState,
{
    pub fn new(owner: UserId, content: T) -> Self {
        Private {
            owner,
            content: Some(content),
        }
    }

    fn new_empty(owner: UserId) -> Self {
        Private {
            owner,
            content: None,
        }
    }
}

impl<T> Deref for Private<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.content.as_ref().expect("cannot access private data")
    }
}

impl<T> DerefMut for Private<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.content.as_mut().expect("cannot access private data")
    }
}

impl<T> CloneState for Private<T>
where
    T: CloneState,
{
    fn clone_state(&self, user_id: UserId) -> Self {
        let mut new = Private::new_empty(self.owner);
        if let Some(content) = &self.content {
            if user_id == new.owner {
                new.content = Some(content.clone_state(user_id));
            }
        }
        new
    }
}

impl<T> CloneState for T
where
    T: Clone + Debug,
{
    fn clone_state(&self, _user_id: UserId) -> Self {
        self.clone()
    }
}
*/

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

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct State {
    pub cnt: u32,
    pub cnt_private: HashMap<UserId, u32>,
}

impl State {
    pub fn update(&mut self, EventData { event, user_id }: EventData) {
        match event {
            Event::Increment => {
                self.cnt += 1;
            }
            Event::IncrementPrivate => {
                *self.cnt_private.entry(user_id.unwrap()).or_default() += 1;
            },
            Event::Tick => {
                self.cnt += 1;
            }
        }
    }

    pub fn view(&self, receiver: UserId) -> Self {
        State {
            cnt_private: HashMap::from_iter(
                self.cnt_private
                    .get_key_value(&receiver)
                    .map(|(&k, &v)| (k, v)),
            ),
            ..self.clone()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event {
    Increment,
    IncrementPrivate,
    Tick,
}

impl EventData {
    pub fn filter(&self, receiver: UserId) -> bool {
        let EventData { event, user_id } = self;
        let user_id = *user_id;

        match event {
            Event::IncrementPrivate if user_id.unwrap() != receiver => false,
            _ => true,
        }
    }
}
