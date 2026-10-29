fn main() {
    if let Err(error) = ssh_tray::panel::run() {
        eprintln!("ssh-tray-panel: {error}");
        std::process::exit(1);
    }
}
