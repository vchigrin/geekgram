use color_eyre::Result;
use grammers_client::types::Dialog;
use grammers_client::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::storage;

struct SharedState {
    storage: storage::Storage,
}

pub struct Runtime {
    shared_state: Arc<Mutex<SharedState>>,
    update_loop_handle: tokio::task::JoinHandle<()>,
}

impl Runtime {
    pub fn new(
        storage: storage::Storage,
        tg_client: Client,
        tokio_rt: &tokio::runtime::Runtime,
    ) -> Self {
        let shared_state = SharedState { storage };
        let wrapped_shared_state = Arc::new(Mutex::new(shared_state));
        let update_loop_handle =
            tokio_rt.spawn(Self::update_loop(wrapped_shared_state.clone(), tg_client));
        Self {
            shared_state: wrapped_shared_state,
            update_loop_handle,
        }
    }

    async fn update_loop(shared_state: Arc<Mutex<SharedState>>, tg_client: Client) {
        Self::do_initial_update(&shared_state, &tg_client)
            .await
            .unwrap();
        // TODO: poll updates.
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

    pub async fn stop(self) -> Result<()> {
        self.update_loop_handle.await?;
        Ok(())
    }
}
