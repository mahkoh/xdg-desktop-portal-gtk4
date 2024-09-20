use {
    crate::{gui::Ui, portal::Portal},
    clap::Parser,
    error_reporter::Report,
};

/// The xdg-desktop-portal-gtk4 portal.
#[derive(Parser, Debug)]
struct Cli {
    /// Replace the portal if it is already running.
    #[clap(long)]
    pub replace: bool,
}

pub fn main() {
    let args = Cli::parse();
    let ui = Ui::new();
    let _portal = match Portal::create(ui.proxy(), args.replace) {
        Ok(p) => p,
        Err(e) => {
            log::error!("Could not create the portal: {}", Report::new(e));
            std::process::exit(1);
        }
    };
    ui.run();
}
