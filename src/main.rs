use color_eyre::Result;
use std::path::Path;
use tokio::runtime as tr;

mod app;
mod runtime;
mod storage;
mod tg_client_builder;
mod ui;

fn main() -> Result<()> {
    color_eyre::install()?;
    simple_logging::log_to_file("/tmp/geekgram.log", log::LevelFilter::Debug).unwrap();
    let storage = storage::Storage::new(Path::new("/tmp/geekgram.db"))?;
    let tokio_rt = tr::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let tg_client = tokio_rt.block_on(
        tg_client_builder::TgClientBuilder::make_signed_in_client(&storage),
    )?;
    let app_runtime = runtime::Runtime::new(storage, tg_client, &tokio_rt);
    let mut app = app::App::new(&app_runtime);
    let terminal = ratatui::init();
    let result = tokio_rt.block_on(app.run(terminal));
    let stop_result = tokio_rt.block_on(app_runtime.stop());
    if let Err(e) = stop_result {
        log::error!("Error during stopping runtime. {:?}", e);
    }
    ratatui::restore();
    result
}
