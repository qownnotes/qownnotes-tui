{ qownnotes-tui }:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.qownnotes-tui;
  toml = pkgs.formats.toml { };
  themeNames = [
    "background"
    "foreground"
    "muted"
    "accent"
    "accent_foreground"
    "success"
    "warning"
    "error"
    "heading"
    "quote"
    "code"
    "link"
    "fence"
    "field_background"
  ];
  configuredTheme = lib.filterAttrs (_: value: value != null) cfg.theme;
in
{
  options.programs.qownnotes-tui = {
    enable = lib.mkEnableOption "qownnotes-tui";

    package = lib.mkOption {
      type = lib.types.package;
      default = qownnotes-tui.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "inputs.qownnotes-tui.packages.\${pkgs.system}.default";
      description = "The qownnotes-tui package to install.";
    };

    theme = lib.genAttrs themeNames (
      name:
      lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        example = "#89b4fa";
        description = "Color used for ${lib.replaceStrings [ "_" ] [ " " ] name}.";
      }
    );
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      { home.packages = [ cfg.package ]; }
      (lib.mkIf (configuredTheme != { }) {
        xdg.configFile."qownnotes-tui/theme.toml".source =
          toml.generate "qownnotes-tui-theme.toml" configuredTheme;
      })
    ]
  );
}
