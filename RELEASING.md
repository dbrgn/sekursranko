# Releasing

Set variables:

    $ export VERSION=X.Y.Z
    $ git config gpg.format ssh
    $ git config user.signingkey 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIM7kBQZCsAc/yXDWQOv9U41nMT5MmmlE+eEEY4urrNdM danilo@c3po'

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
