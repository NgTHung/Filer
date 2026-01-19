use flume::{Receiver, Sender};

use crate::{Command, Event, errors::CoreError};

pub struct FilerCore {
    command_tx: Sender<Command>,
    event_rx: Receiver<Event>,
}

impl FilerCore {
    pub async fn new() -> Result<Self, CoreError> {
        todo!()
    }
    pub fn send(&self, command: Command) -> Result<(), CoreError> {
        todo!()
    }
    pub fn try_recv(&self) -> Option<Event> {
        todo!()
    }
    pub fn event_receiver(&self) -> Receiver<Event> {
        self.event_rx.clone()
    }
    pub fn command_sender(&self) -> Sender<Command> {
        self.command_tx.clone()
    }
    pub fn shutdown(&self) -> Result<(), CoreError>{
        todo!()
    }
}