{
  description = "Development environment for ssh-tray";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          linuxPackages = with pkgs; [
            pkg-config
            gtk3
            libappindicator
            libx11
            libxcursor
            libxrandr
            libxi
            libxfixes
            libxext
            libxrender
            libxcb
            libxkbcommon
            dbus
            libGL
            mesa
            wayland
          ];
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
            ] ++ lib.optionals stdenv.hostPlatform.isLinux linuxPackages;

            shellHook = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
              export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath linuxPackages}:$LD_LIBRARY_PATH"
            '';
          };
        });
    };
}
