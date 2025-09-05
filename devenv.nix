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
  ];

  # See full reference at https://devenv.sh/reference/options/
}
