# SPDX-FileCopyrightText: 2025 Olivier 'reivilibre'
#
# SPDX-License-Identifier: GPL-3.0-or-later
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        craneLib = crane.mkLib pkgs;

        # Common arguments can be set here to avoid repeating them later
        # Note: changes here will rebuild all dependency crates
        commonArgs = {
          src = pkgs.lib.cleanSourceWith {
            src = ./.;

            filter = path: type:
              # Default crane filter
              craneLib.filterCargoSources path type
              # Keep .sql
              || (pkgs.lib.hasSuffix ".sql" path);
          };
          strictDeps = true;

          buildInputs = [
            # Add additional build inputs here
          ];
        };

        pgcrab = craneLib.buildPackage (
          commonArgs
          // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          }
        );
      in
      {
        checks = {
          inherit pgcrab;
        };

        packages.default = pgcrab;

        apps.default = flake-utils.lib.mkApp {
          drv = pgcrab;
        };
      }
    );
}
