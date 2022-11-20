[![Crates.io](https://img.shields.io/crates/v/easy-cli.svg)](https://crates.io/crates/easy-cli)
# easy-cli
A tool for building personal and team CLI tools.

easy-cli takes a CLI-Tool name option and a path to a directory of scripts as arguments. It assumes each script ist to be a CLI command, and builds an appropriate CLI parser.

It could be used as an alias, but (in my experience, in zsh) this does not work with completion. You can define a function for your cli, for instance:

```
mycli() {
    <Path>/easy-cli --name mycli <Path-to-easy-cli-root>/example -- $@
}
```
and then calling
```
mycli
```
produces output: 
```
error: 'mycli' requires a subcommand but one was not provided
  [subcommands: hello, list, help]

Usage: mycli <COMMAND>

For more information try '--help'
```
Then calling
```
mycli hello
```
will print
```
Hello, world!
```
Arguments trailing the command will be passed to the relevant script.

## Completion

easy-cli offers completion for your cli in a number of shells - those supported by [clap_complete](https://crates.io/crates/clap_complete). To generate completions for your cli, run:

```
easy-cli --name <cli-name> <Path-to-cli-dir> --completions <shell> > <completions_file>
```
and proceed as required by your shell.
## Next Planned Features

1. CLI subcommands (for instance, ```cli <commandA> <subcommandA1>```), to group related commands.
1. Parsing script options from the script and adding them to the CLI, for better help.
1. Environments, so that the CLI can easily be used to apply different environments (e.g. for staging).