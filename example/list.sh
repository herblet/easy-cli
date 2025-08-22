#!/usr/bin/env zsh

# Any tags in a file before the first @sub, if any, are applied to the command
# created from this file. @about is used as the description of the command
# in help. @arg and @opt add arguments and options.

# @about List files in the current directory
# @arg directory <dir> The directory to list files in

ls -ltr $1