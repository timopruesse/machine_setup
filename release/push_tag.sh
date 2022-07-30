#!/bin/sh

old_version=$(awk '/^version/ {print $NF}' Cargo.toml | sed -e "s/\"//g")

printf "Specify version (current: $old_version): "
read -r version

echo "\nCreating release for version $version..."

sed -i "s/^version.*/version = \"$version\"/" Cargo.toml

cargo update
git add Cargo.*
git commit -m "Bump version ($version)"

${VISUAL:-${EDITOR:-vi}} "release_notes.md"

git tag --cleanup=whitespace -a -f v$version -F release_notes.md
git push --atomic origin main v$version

rm release_notes.md
