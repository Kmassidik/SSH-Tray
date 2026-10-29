fn main() {
    if let Err(error) = ssh_tray::app::run() {
        eprintln!("ssh-tray: {error}");
        std::process::exit(1);
    }
}
