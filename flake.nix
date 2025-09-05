# SPDX-FileCopyrightText: 2025 Olivier 'reivilibre'
#
# SPDX-License-Identifier: GPL-3.0-or-later

{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/23.05";
    # Output a development shell for x86_64/aarch64 Linux/Darwin (MacOS).
    systems.url = "github:nix-systems/default";
    # A development environment manager built on Nix. See https://devenv.sh.
    devenv.url = "github:cachix/devenv/v0.6.3";
    # Updated Rust
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, devenv, systems, fenix, ... } @ inputs:
    let
      forEachSystem = nixpkgs.lib.genAttrs (import systems);
    in {
      devShells = forEachSystem (system:
        let
          pkgs = import nixpkgs {
            inherit system;
          };

          fenixRustToolchain =
            fenix.packages."${system}".stable.withComponents [
              "cargo"
              "clippy"
              "rust-src"
              "rustc"
              "rustfmt"
              "rust-analyzer"
            ];
        in {
          # Everything is configured via devenv - a Nix module for creating declarative
          # developer environments. See https://devenv.sh/reference/options/ for a list
          # of all possible options.
          default = devenv.lib.mkShell {
            inherit inputs pkgs;
            modules = [
              {
                # Configure packages to install.
                # Search for package names at https://search.nixos.org/packages?channel=unstable
                packages = with pkgs; [
                  fenixRustToolchain

                  gcc
                ];

                # Postgres is needed to run Synapse with postgres support and
                # to run certain unit tests that require postgres.
                services.postgres.enable = true;

                # On the first invocation of `devenv up`, create a Postgres database.
                services.postgres.initdbArgs = ["--locale=C" "--encoding=UTF8"];
                services.postgres.initialDatabases = [
                  { name = "testdb"; }
                ];
                services.postgres.initialScript = ''
                CREATE USER testuser;
                ALTER DATABASE testdb OWNER TO testuser;
                '';
                # Set PGxxx env vars for psql to pick up.
                # PGHOST set automatically
                env.PGUSER = "testuser";
                env.PGDATABASE = "testdb";


                # Clear the LD_LIBRARY_PATH environment variable on shell init.
                #
                # By default, devenv will set LD_LIBRARY_PATH to point to .devenv/profile/lib. This causes
                # issues when we include `gcc` as a dependency to build C libraries, as the version of glibc
                # that the development environment's cc compiler uses may differ from that of the system.
                #
                # When LD_LIBRARY_PATH is set, system tools will attempt to use the development environment's
                # libraries. Which, when built against a different glibc version lead, to "version 'GLIBC_X.YY'
                # not found" errors.
                enterShell = ''
                  unset LD_LIBRARY_PATH
                '';
              }
            ];
          };
        });
    };
}
