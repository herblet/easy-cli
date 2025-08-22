#!/usr/bin/env zsh

# Demonstrates the default (non-embedded) mode, whereby easy-cli calls this script and passes the
# sub-command as the first argument, and then the args and options in the order they were defined.

# @about A command with multiple sub-commands

# Each @sub tag starts a new sub-command, tags after it are applied to
# that sub-command.

# @sub one
# @about Prints a message
# @opt option 'o' An option for this sub-command
# @arg Message Will be printed
# This sub-command will accept one arguments
one() {
  #Print "option set " if the second argument is 'true', "option not set" otherwise
  [[ $1 == "true" ]] && echo "option set" || echo "option not set"

  echo "Sub-command 'one' called with message: $2"
}

# @sub two
# This sub-command will accept no arguments
two() {
  echo "two: $@"
}

# The selected sub-command will be passed as the first argument; the
# remaining args are args for the command to use.
"$1" "${@:2}"
