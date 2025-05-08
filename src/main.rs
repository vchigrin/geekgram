use color_eyre::Result;
use tokio::runtime;

mod app;

fn main() -> Result<()> {
    color_eyre::install()?;
    simple_logging::log_to_file("/tmp/geekgram.log", log::LevelFilter::Debug).unwrap();
    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut app = app::App::new();
    let terminal = ratatui::init();
    let result = rt.block_on(app.run(terminal));
    ratatui::restore();
    result
}
