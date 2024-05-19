#!/usr/bin/env zsh

# @sub one
# @arg Message Will be printed
one() {
  echo "one: $@"
}

# @sub two
two() {
  echo "two: $@"
}

# Call the function named in the first argument, passing the remaining args
"$1" "${@:2}"
