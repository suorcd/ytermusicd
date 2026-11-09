{
  description = "ytermusicd development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          inherit (pkgs) lib;

          pkgConfigPaths =
            [
              "${pkgs.openssl.dev}/lib/pkgconfig"
              "${pkgs.sqlite.dev}/lib/pkgconfig"
            ]
            ++ lib.optionals pkgs.stdenv.hostPlatform.isLinux [
              "${pkgs.alsa-lib.dev}/lib/pkgconfig"
              "${pkgs.dbus.dev}/lib/pkgconfig"
            ];
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              cargo-nextest
              clippy
              curl
              ffmpeg
              jq
              just
              nodejs_22
              rustc
              rustfmt
              yt-dlp
            ];

            buildInputs =
              with pkgs;
              [
                openssl
                sqlite
              ]
              ++ lib.optionals pkgs.stdenv.hostPlatform.isLinux [
                alsa-lib
                dbus
              ];

            nativeBuildInputs = with pkgs; [ pkg-config ];

            shellHook = ''
              export PKG_CONFIG_PATH="${lib.concatStringsSep ":" pkgConfigPaths}''${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
              export RUST_BACKTRACE=1
            '';
          };
        }
      );
    };
}
