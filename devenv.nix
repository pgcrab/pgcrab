# SPDX-FileCopyrightText: 2025 Olivier 'reivilibre'
#
# SPDX-License-Identifier: GPL-3.0-or-later

{ pkgs, lib, config, inputs, ... }:

{
  cachix.enable = false;

  languages.rust = {
    enable = true;
  };

  services.postgres.enable = true;

  packages = with pkgs; [
    cargo-insta
    cargo-release

    mdbook

    reuse
  ];

  # See full reference at https://devenv.sh/reference/options/
}
