use color_eyre::Result;
use grammers_client::types::{Chat, Dialog};
use grammers_client::{Client, Update};
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;

use super::storage;

#[derive(Debug)]
enum Command {
    RefreshMessages(Chat),
}

struct SharedState {
    storage: storage::Storage,
}

pub struct Runtime {
    shared_state: Arc<Mutex<SharedState>>,
    update_loop_handle: tokio::task::JoinHandle<()>,
    command_sender: Sender<Command>,
}

const COMMAND_BUFFER_SIZE: usize = 10;
const MESSAGES_PORTION_SIZE: usize = 20;

impl Runtime {
    pub fn new(
        storage: storage::Storage,
        tg_client: Client,
        tokio_rt: &tokio::runtime::Runtime,
    ) -> Self {
        let (sender, receiver) = channel::<Command>(COMMAND_BUFFER_SIZE);
        let shared_state = SharedState { storage };
        let wrapped_shared_state = Arc::new(Mutex::new(shared_state));
        let update_loop_handle = tokio_rt.spawn(Self::update_loop(
            wrapped_shared_state.clone(),
            tg_client,
            receiver,
        ));
        Self {
            shared_state: wrapped_shared_state,
            update_loop_handle,
            command_sender: sender,
        }
    }

    async fn update_loop(
        shared_state: Arc<Mutex<SharedState>>,
        tg_client: Client,
        mut command_receiver: Receiver<Command>,
    ) {
        Self::do_initial_update(&shared_state, &tg_client)
            .await
            .unwrap();
        loop {
            tokio::select! {
                maybe_command = command_receiver.recv() => {
                    if let Some(command) = maybe_command {
                        if let Err(e) = Self::handle_command(&command, &shared_state, &tg_client).await {
                            log::error!("Error during command {:?} handling {:?}", command, e);
                        }
                    } else {
                        return;
                    }
                },
                maybe_update = tg_client.next_update() => {
                    if let Ok(update) = maybe_update {
                        if let Err(e) = Self::handle_update(&shared_state, update).await {
                            log::error!("Error during update handling {:?}", e);
                        }
                    } else {
                        log::error!("Failed get update {:?}", maybe_update);
                    }
                }
            }
        }
    }

    async fn handle_command(
        command: &Command,
        shared_state: &Arc<Mutex<SharedState>>,
        tg_client: &Client,
    ) -> Result<()> {
        match command {
            Command::RefreshMessages(chat) => {
                Self::refresh_messages(chat, shared_state, tg_client).await?;
            }
        }
        Ok(())
    }

    async fn refresh_messages(
        chat: &Chat,
        shared_state: &Arc<Mutex<SharedState>>,
        tg_client: &Client,
    ) -> Result<()> {
        let mut messages = tg_client.iter_messages(chat).limit(MESSAGES_PORTION_SIZE);
        let mut all_messages = Vec::new();
        while let Some(msg) = messages.next().await? {
            all_messages.push(msg);
        }
        let locked_state = shared_state.lock().await;
        for message in all_messages {
            locked_state.storage.save_message(&message)?;
        }
        Ok(())
    }

    async fn handle_update(shared_state: &Arc<Mutex<SharedState>>, update: Update) -> Result<()> {
        match update {
            Update::NewMessage(message) => {
                let locked_state = shared_state.lock().await;
                locked_state.storage.save_message(&message)?;
            }
            Update::MessageEdited(message) => {
                let locked_state = shared_state.lock().await;
                locked_state.storage.save_message(&message)?;
            }
            Update::MessageDeleted(message_deletion) => {
                let locked_state = shared_state.lock().await;
                locked_state.storage.delete_message(&message_deletion)?;
            }
            _ => {
                log::info!("Not handled yet update {:?}", update);
            }
        }
        Ok(())
    }

    async fn do_initial_update(
        shared_state: &Arc<Mutex<SharedState>>,
        tg_client: &Client,
    ) -> Result<()> {
        let mut retrieved_dialogs = Vec::new();
        let mut it = tg_client.iter_dialogs();
        while let Some(dialog) = it.next().await? {
            retrieved_dialogs.push(dialog);
        }

        let i = shared_state.lock().await;
        for dialog in retrieved_dialogs {
            i.storage.save_dialog(&dialog)?
        }
        Ok(())
    }

    pub async fn get_dialogs(&self) -> Result<Vec<Dialog>> {
        let i = self.shared_state.lock().await;
        i.storage.select_all_dialogs()
    }

    pub async fn start_message_refreshing(&self, chat: Chat) -> Result<()> {
        self.command_sender
            .send(Command::RefreshMessages(chat))
            .await?;
        Ok(())
    }

    pub async fn stop(self) -> Result<()> {
        drop(self.command_sender);
        self.update_loop_handle.await?;
        Ok(())
    }
}
