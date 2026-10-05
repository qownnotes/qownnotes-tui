use std::{env, path::Path, process::Command};

use anyhow::{Context, bail};

pub fn command(path: &Path) -> anyhow::Result<Command> {
    let editor = ["VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|name| env::var(name).ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| "vi".into());
    parse_command(&editor, path)
}

fn parse_command(editor: &str, path: &Path) -> anyhow::Result<Command> {
    let words = shell_words::split(editor).context("invalid external editor command")?;
    let Some(program) = words.first().filter(|program| !program.is_empty()) else {
        bail!("external editor command is empty");
    };
    let mut command = Command::new(program);
    command.args(&words[1..]).arg(path);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_quoted_arguments_and_passes_note_as_one_literal_argument() {
        let path = Path::new("/notes/a note; $(command).md");
        let command = parse_command("'my editor' --wait --option 'two words'", path).unwrap();
        assert_eq!(command.get_program(), "my editor");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--wait", "--option", "two words", path.to_str().unwrap()]
        );
    }

    #[test]
    fn rejects_empty_or_malformed_commands() {
        for editor in ["", "''", "editor 'unterminated"] {
            assert!(parse_command(editor, Path::new("note.md")).is_err());
        }
    }
}
