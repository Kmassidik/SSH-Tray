#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("ssh-tray supports only Linux and macOS");

fn open_panel(process: &mut Option<std::process::Child>) -> Result<(), String> {
    let running = match process.as_mut() {
        Some(child) => match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(error) => return Err(format!("failed to inspect panel process: {error}")),
        },
        None => false,
    };
    if running {
        return Ok(());
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to locate ssh-tray executable: {error}"))?
        .with_file_name("ssh-tray-panel");
    *process = Some(
        std::process::Command::new(executable)
            .spawn()
            .map_err(|error| format!("failed to open panel: {error}"))?,
    );
    Ok(())
}

fn close_panel(process: &mut Option<std::process::Child>) {
    if let Some(mut child) = process.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub fn run() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        linux::run()
    }
    #[cfg(target_os = "macos")]
    {
        macos::run()
    }
}
