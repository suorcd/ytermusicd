{
  description = "ytermusicd development environment and package";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;

      ytermusicdFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          inherit (pkgs) lib;
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "ytermusicd";
          version = "0.1.0";
          src = self;

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "rusty_ytdl-0.7.4" = "sha256-mCX+vylB4uV8GFhv8vLC69MEszEkfUgA7xXtD1fP6go=";
            };
          };

          cargoBuildFlags = [ "--package" "ytermusicd" ];
          doCheck = false;

          nativeBuildInputs = [ pkgs.makeWrapper ];

          postInstall = ''
            wrapProgram $out/bin/ytermusicd \
              --prefix PATH : ${lib.makeBinPath [ pkgs.yt-dlp pkgs.ffmpeg ]}
            install -Dm644 packaging/systemd/ytermusicd.service \
              $out/lib/systemd/user/ytermusicd.service
            install -Dm644 packaging/systemd/ytermusicd.env.example \
              $out/share/doc/ytermusicd/ytermusicd.env.example
            install -Dm644 config.example.toml \
              $out/share/doc/ytermusicd/config.example.toml
          '';

          meta = {
            description = "Headless YouTube Music daemon with a web UI, MPRIS and MCP";
            license = lib.licenses.asl20;
            mainProgram = "ytermusicd";
          };
        };

      nixosModule =
        { config, lib, pkgs, ... }:
        let
          cfg = config.services.ytermusicd;
        in
        {
          options.services.ytermusicd = {
            enable = lib.mkEnableOption "ytermusicd headless daemon";
            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.ytermusicd;
              description = "ytermusicd package to use.";
            };
            user = lib.mkOption {
              type = lib.types.str;
              default = "ytermusicd";
              description = "System user for the service.";
            };
            group = lib.mkOption {
              type = lib.types.str;
              default = "ytermusicd";
              description = "System group for the service.";
            };
            bind = lib.mkOption {
              type = lib.types.str;
              default = "127.0.0.1";
              description = "HTTP bind address.";
            };
            port = lib.mkOption {
              type = lib.types.port;
              default = 8080;
              description = "HTTP port.";
            };
            environmentFile = lib.mkOption {
              type = lib.types.nullOr lib.types.path;
              default = null;
              description = "Optional systemd EnvironmentFile (headers path, secrets, etc.).";
            };
          };

          config = lib.mkIf cfg.enable {
            users.groups.${cfg.group} = { };
            users.users.${cfg.user} = {
              isSystemUser = true;
              group = cfg.group;
            };

            systemd.services.ytermusicd = {
              description = "ytermusicd headless YouTube Music daemon";
              after = [ "network-online.target" ];
              wants = [ "network-online.target" ];
              wantedBy = [ "multi-user.target" ];

              serviceConfig = {
                ExecStart = "${cfg.package}/bin/ytermusicd serve";
                User = cfg.user;
                Group = cfg.group;
                Restart = "on-failure";
                RestartSec = 5;
                StateDirectory = "ytermusicd";
                StateDirectoryMode = "0700";
                CacheDirectory = "ytermusicd";
                NoNewPrivileges = true;
                PrivateTmp = true;
                Environment = [
                  "HOME=/var/lib/ytermusicd"
                  "YTERMUSICD_BIND=${cfg.bind}"
                  "YTERMUSICD_PORT=${toString cfg.port}"
                ];
              } // lib.optionalAttrs (cfg.environmentFile != null) {
                EnvironmentFile = cfg.environmentFile;
              };
            };
          };
        };

      homeManagerModule =
        { config, lib, pkgs, ... }:
        let
          cfg = config.services.ytermusicd;
        in
        {
          options.services.ytermusicd = {
            enable = lib.mkEnableOption "ytermusicd user service";
            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.ytermusicd;
              description = "ytermusicd package to use.";
            };
            bind = lib.mkOption {
              type = lib.types.str;
              default = "127.0.0.1";
              description = "HTTP bind address.";
            };
            port = lib.mkOption {
              type = lib.types.port;
              default = 8080;
              description = "HTTP port.";
            };
            environmentFile = lib.mkOption {
              type = lib.types.nullOr lib.types.path;
              default = null;
              description = "Optional systemd user EnvironmentFile.";
            };
          };

          config = lib.mkIf cfg.enable {
            home.packages = [ cfg.package ];
            systemd.user.services.ytermusicd = {
              Unit = {
                Description = "ytermusicd headless YouTube Music daemon";
                After = [ "network-online.target" ];
                Wants = [ "network-online.target" ];
              };
              Service = {
                ExecStart = "${cfg.package}/bin/ytermusicd serve";
                Restart = "on-failure";
                RestartSec = 5;
                Environment = [
                  "YTERMUSICD_BIND=${cfg.bind}"
                  "YTERMUSICD_PORT=${toString cfg.port}"
                ];
              } // lib.optionalAttrs (cfg.environmentFile != null) {
                EnvironmentFile = cfg.environmentFile;
              };
              Install.WantedBy = [ "default.target" ];
            };
          };
        };
    in
    {
      packages = forAllSystems (system: {
        default = ytermusicdFor system;
        ytermusicd = ytermusicdFor system;
      });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.ytermusicd}/bin/ytermusicd";
        };
      });

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

      nixosModules.default = nixosModule;
      nixosModules.ytermusicd = nixosModule;
      homeManagerModules.default = homeManagerModule;
      homeManagerModules.ytermusicd = homeManagerModule;
    };
}
