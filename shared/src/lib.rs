use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Game {
    pub cnt: u32,
}

impl Game {
    pub fn update(&mut self, event: Event) {
        match event {
            Increment => {
                self.cnt += 1;
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event {
    Increment
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Req {
    Event(Event)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Res {
    Sync(Game),
    Event(Event)
}