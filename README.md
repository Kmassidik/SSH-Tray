# SSH Tray

A small macOS menu-bar app that turns the host aliases in `~/.ssh/config` into one-click SSH connections. Selecting a host opens Terminal and runs `ssh <host>`; authentication, keys, agents, and SSH options stay in your existing SSH setup.

## Requirements

- macOS
- Nix with flakes enabled
- Apple Terminal

## Develop

```sh
nix develop
cargo run
```

Click the tray icon, then choose a host. Use **Open Panel** to manage hosts and keys, **Refresh** to reload hosts manually, and **Quit** to exit. There is no background scanning.

## Management panel

Choose **Open Panel** from the tray menu, or run `ssh-tray --panel`. The native egui window uses a custom dark theme and Phosphor icons. Search hosts by alias or group; **Cmd/Ctrl+F** focuses search and **Escape** clears it.

The panel provides:

- Host **Add / Edit / Delete** forms for aliases, HostName, User, Port, IdentityFile, and network group. Edits preserve advanced options such as `ProxyJump`, `ProxyCommand`, and `RemoteCommand`.
- Grouped hosts with **Connect**, **Copy SSH Command**, and **Install on Server** (`ssh-copy-id`).
- Public-key fingerprints and comments with **Copy Public Key** and **Edit Public Comment**.
- Ed25519 key generation, existing key-pair import, interactive passphrase changes, and confirmed key-pair removal.

Key generation, passphrase changes, and server installation open Terminal for secure secret entry or authentication. The panel never displays private keys or handles their passphrases. Import expects a private key with a sibling `.pub` file and never overwrites existing keys. Generated or imported keys appear after **Refresh**.

Example SSH config:

```sshconfig
Host dev
    HostName 192.168.1.20
    User alice
    IdentityFile ~/.ssh/id_ed25519

Host prod
    HostName example.com
    User deploy
```

Wildcard and negated `Host` patterns are intentionally omitted from the menu. To avoid duplicate machines, only the first alias from each `Host` block appears in the tray and panel; other aliases remain in the config, work in Terminal, and stay editable in the host form. If no config exists, create one with `touch ~/.ssh/config` and add an alias like the example above.

## Network groups

Add a comment directive before one or more `Host` blocks to place their aliases in a submenu:

```sshconfig
# ssh-tray:group Office
Host macstudio studio
    HostName 202.72.223.27

# ssh-tray:group Cloudflare
Host kurnia-mac
    HostName ssh.kurniatech.my.id
```

The group applies to following hosts until the next `# ssh-tray:group <name>` directive. An empty `# ssh-tray:group` directive returns subsequent hosts to the top-level menu. These comments are ignored by SSH.

## Checks

```sh
nix develop --command cargo test
nix develop --command cargo-fmt -- --check
nix develop --command cargo-clippy clippy -- -D warnings
```

## Build an app bundle

```sh
nix develop --command ./scripts/package-macos.sh
open "target/SSH Tray.app"
```

The bundle uses `LSUIElement`, so it appears in the menu bar without a Dock icon. It is an unsigned local build; macOS may require approving it in System Settings > Privacy & Security.

## Development environment

`flake.nix` supplies the Rust toolchain and native build tools. The application itself is Rust with `tao` and `tray-icon`; it does not embed an SSH client or store credentials.
# SSH-Tray
