# Releasing

Set variables:

    $ export VERSION=X.Y.Z
    $ git config user.signingkey 0xB993FF98A90C9AB1

Update version numbers:

    $ vim Cargo.toml
    $ cargo update -p sekursranko

Update changelog:

    $ vim CHANGELOG.md

Commit & tag:

    $ git commit -S -m "Release v${VERSION}"
    $ git tag -s v${VERSION} -m "Version ${VERSION}"

Publish:

    $ git push && git push --tags

Push to release branch:

    $ git push origin master:release
