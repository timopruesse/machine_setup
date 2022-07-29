#!/bin/sh

old_version=$(awk '/^version/ {print $NF}' Cargo.toml | sed -e "s/\"//g")

printf "Specify version (current: $old_version): "
read -r version

echo "\nCreating release for version $version..."

sed -i "s/^version.*/version = \"$version\"/" Cargo.toml


echo 'Release notes:'
read -r notes

git add Cargo.toml
git commit -m "Bump version"
git tag -a v$version -m "$notes"

git push --atomic origin main v$version
