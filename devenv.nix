{ pkgs, ... }:
{
  languages.rust.enable = true;

  packages = with pkgs; [
    just
    nixfmt
  ];

  enterShell = ''
    echo "qownnotes-tui development environment"
  '';
}
