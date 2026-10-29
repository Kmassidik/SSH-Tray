# SSH Tray

A native SSH menu-bar and management app written in Rust. Connect to saved hosts in one click, organize machines by network, and manage SSH hosts and keys without editing config files by hand.

<img src="assets/panel-demo.png" alt="Monochrome SSH Tray sidebar and host details with fictional demo data" width="960">

*The screenshot contains only fictional hosts, `example.com` domains, and TEST-NET addresses. No real SSH data is included.*

## Features

- **One-click SSH** from the macOS menu bar or Linux system tray.
- **Host CRUD:** add, edit, and delete hosts through a native panel.
- **Network groups:** organize hosts as Office, LAN, Cloudflare, or any custom group.
- **Clean host list:** only the first alias from each `Host` block is displayed; other aliases still work in Terminal.
- **Safe editing:** preserves `ProxyJump`, `ProxyCommand`, `RemoteCommand`, `Control*`, comments, includes, and unrelated config blocks.
- **Key management:** generate, inspect, copy, import, update, and remove key pairs.
- **Secure installation:** copy a public key to a server with `ssh-copy-id`.
- **Fast navigation:** searchable sidebar with **Cmd/Ctrl+F** to focus and **Escape** to clear.
- **Manual refresh only:** no background host or network scanning.

## Supported platforms

| Platform | Architectures | Terminal support |
|---|---|---|
| macOS | Apple Silicon | Apple Terminal |
| Linux | x86_64, aarch64 | `x-terminal-emulator`, GNOME Terminal, KGX, Konsole, XFCE Terminal, Alacritty, Kitty, WezTerm, Foot, or XTerm |

Linux requires a graphical desktop and an AppIndicator-compatible system tray. `$TERMINAL` can select a supported terminal explicitly.

## Requirements

- Nix with flakes enabled for the reproducible development shell
- Rust 1.95 or newer when building without Nix
- OpenSSH tools: `ssh`, `ssh-keygen`, and `ssh-copy-id`
- An SSH config at `~/.ssh/config`

## Quick start

```sh
git clone git@github.com:Kmassidik/SSH-Tray.git
cd SSH-Tray
nix develop
cargo run --bin ssh-tray
```

Click the tray icon and select a host. Choose **Open Panel** to manage hosts and keys, **Refresh** to reload the config manually, or **Quit** to exit.

Open the management panel directly during development:

```sh
cargo run --bin ssh-tray-panel --features panel
```

## Build

### macOS app bundle

```sh
nix develop --command ./scripts/package-macos.sh
open "target/SSH Tray.app"
```

The bundle uses `LSUIElement`, so SSH Tray runs in the menu bar without a Dock icon. Local builds are unsigned; macOS may require approval under **System Settings → Privacy & Security**.

### Linux release binary

```sh
nix develop
cargo build --release --features panel --bins
./target/release/ssh-tray
```

Open only the management panel:

```sh
./target/release/ssh-tray-panel
```

## SSH host configuration

SSH Tray reads standard OpenSSH aliases and options. For example:

```sshconfig
# ssh-tray:group Development
Host dev-api
    HostName dev.example.com
    User developer
    Port 22
    IdentityFile ~/.ssh/id_ed25519

Host staging
    HostName staging.example.com
    User developer
    Port 22

# ssh-tray:group Production
Host prod-web
    HostName 192.0.2.20
    User deploy
    Port 22
```

A `# ssh-tray:group <name>` comment applies to following hosts until the next group directive. An empty `# ssh-tray:group` returns subsequent hosts to the top level. SSH ignores these comments.

Wildcard and negated `Host` patterns are omitted from the interface. For a block such as `Host macstudio studio`, only `macstudio` appears in the app; both aliases remain usable with `ssh`, and both remain editable in the host form.

## Panel workflows

### Hosts

- **Connect:** open Terminal with `ssh <alias>`.
- **Copy SSH Command:** copy the exact connection command.
- **Add / Edit:** manage aliases, HostName, User, Port, IdentityFile, and group.
- **Delete:** confirm removal of the entire host block, including every alias in that block.
- **Install on Server:** select a public key and run `ssh-copy-id` for the selected host.

### SSH keys

| Action | Behavior |
|---|---|
| Copy Public Key | Copy only the `.pub` contents |
| Generate Key | Create Ed25519 keys through interactive `ssh-keygen` |
| Import Key | Import a private key and its sibling `.pub` without overwriting |
| Edit Public Comment | Change only the public-key comment; key data stays unchanged |
| Change Passphrase | Open interactive `ssh-keygen -p` in Terminal |
| Remove Key Pair | Delete private and public files after explicit confirmation |

Imported private keys use mode `0600`; public keys use `0644`.

## Security and privacy

- Private-key contents are never displayed in the panel.
- Passphrases and server authentication are entered only in Terminal.
- SSH Tray does not store credentials or embed a separate SSH client.
- Existing keys are never overwritten during generation or import.
- Config updates use a sibling temporary file and atomic rename while preserving permissions.
- Hosts and keys refresh only on launch or when you click **Refresh**.
- Documentation screenshots use disposable demo data, never your real SSH configuration.

## Development

Enter the Nix shell:

```sh
nix develop
```

Run checks:

```sh
cargo test
cargo test --features panel
cargo-fmt -- --check
cargo-clippy clippy --all-targets --features panel -- -D warnings
```

Or run them without entering the shell:

```sh
nix develop --command cargo test
nix develop --command cargo test --features panel
nix develop --command cargo-fmt -- --check
nix develop --command cargo-clippy clippy --all-targets --features panel -- -D warnings
```

## Resource usage

The always-running tray and on-demand panel are separate executables. The tray does not link `eframe`, `egui`, `rfd`, or `tao` on macOS; it uses AppKit directly. Release builds use thin LTO, one codegen unit, symbol stripping, and abort-on-panic.

Measured release builds:

| Component | Binary size | Private memory | RSS |
|---|---:|---:|---:|
| macOS native tray | 456 KB | 9.8 MB | 43.8 MB |
| Intel Linux tray | 1012 KB | 11.8 MB | 27.9 MB |
| macOS management panel | 4.7 MB | 98 MB | 114 MB |

RSS includes shared GUI frameworks; private memory is the more useful measure for comparing application-owned memory. Linux private memory is the sum of `Private_Clean` and `Private_Dirty` from `smaps_rollup`. A sub-1 MB graphical process is not realistic, but the tray binaries themselves are approximately 0.5–1 MB. Keeping the panel closed avoids its memory cost entirely.

## Project structure

```text
src/
  bin/
    ssh-tray.rs         Lightweight tray entry point
    ssh-tray-panel.rs   On-demand panel entry point
  app/
    mod.rs             Platform dispatch and panel lifecycle
    macos.rs           Direct AppKit status item and menus
    linux.rs           AppIndicator tray and menus
  config/
    mod.rs             Public host model and API
    parser.rs          Host discovery, groups, and alias deduplication
    editor.rs          Validated CRUD and atomic config writes
  keys/
    mod.rs             Public key model and API
    inventory.rs       Discovery, parsing, and fingerprints
    files.rs           Safe import, permissions, and removal
  panel/
    mod.rs             Eframe setup and action coordination
    state.rs           Panel state and selection transitions
    theme.rs           Monochrome styling and shared widgets
    hosts.rs           Host search, list, and details
    keys.rs            Key list and details
    dialogs.rs         CRUD forms and confirmations
  terminal/
    mod.rs             Public terminal actions and tests
    commands.rs        Safe command construction and validation
    macos.rs           AppleScript terminal launching
    linux.rs           Linux terminal detection and argv
assets/
  tray-icon.rgba       Linux tray icon
  tray-icon.png        AppKit tray icon
  panel-demo.png       Fictional README screenshot
packaging/
  Info.plist
scripts/
  package-macos.sh
flake.nix              macOS and Linux development shells
```
