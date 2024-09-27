#!/usr/bin/env bash
set -eu -o pipefail

git init -q

echo a > a
echo b > b
echo union > union
echo e > e-no-attr
echo unset > unset
echo unspecified > unspecified

cat <<EOF >.gitattributes
a merge=a
b merge=b
union merge=union
missing merge=missing
unset -merge
unspecified !merge
EOF

git add . && git commit -m "init"
