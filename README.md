# easy-cli
A tool for building personal and team CLI tools.

easy-cli takes a CLI-Tool name option and a path to a directory of scripts as arguments. It assumes each script ist to be a CLI command, and builds an appropriate CLI parser.

Thus it can be used simply as an alias. For example, defining the alias:

```
alias mycli="easy-cli -n mycli ./example"
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

## Next Planned Features

1. Parsing script options from the script and adding them to the CLI, for better help.
1. Completion. 
1. Environments, so that the CLI can easily be used to apply different environments (e.g. for staging).