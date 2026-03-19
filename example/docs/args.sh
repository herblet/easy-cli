# @about Demonstrates arguments

# @sub disp Prints the contents of a file
# @arg file <File> The file to print
fun disp() {
  cat "$cli_args[file]"
}

# @sub list Lists the contents of a directory
# @arg dir <Dir> The directory to list
fun list() {
  ls "$cli_args[dir]"
}

# @sub copy Copies a file from one location to another
# @opt from 'f' true <File> The file to copy
# @opt to 't' true <Dir> The destination to copy the file to
fun copy() {
  cp "$cli_opts[from]" "$cli_args[to]/"
}

# @sub hello Prints a greeting message
# @opt to 't' true Who to say hello to
# @opt yell 'y' false Whether to yell the greeting
fun hello() {
  if [ "${cli_opts[yell]}" = true ]; then
    echo "HELLO, ${(U)cli_opts[to]}!"
  else
    echo "Hello, ${cli_opts[to]}."
  fi
}