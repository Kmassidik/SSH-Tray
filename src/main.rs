mod app;
mod config;
mod keys;
mod panel;
mod terminal;

fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        Some("--panel") => panel::run(),
        None => app::run(),
        Some(argument) => Err(format!("unknown argument: {argument}")),
    };
    if let Err(error) = result {
        eprintln!("ssh-tray: {error}");
        std::process::exit(1);
    }
}
