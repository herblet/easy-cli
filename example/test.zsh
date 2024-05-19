#!/usr/bin/env zsh

typeset -A arvar
arvar=('parm' 1 'parm2' 'two' )

echo $arvar
echo ${arvar[parm]}
echo ${arvar[parm2]}

fun noArgs() {
  echo "noArgs: ${arvar[parm]}"
}

fun withArgs() {
  declare -A ar1var=(${(@kv)${(P)1}})
  echo "withArgs: ${ar1var[parm2]}"
  echo "withArgs: ${(@kv)ar1var}"
}

noArgs
withArgs arvar
