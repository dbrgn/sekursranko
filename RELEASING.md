# Releasing

Set variables:

    $ export VERSION=X.Y.Z
    $ export GPG_KEY=80F04F84787842018F165B2475B14970B357F8F6

Update version numbers:

    $ vim Cargo.toml
    $ cargo update -p sekursranko

Update changelog:

    $ vim CHANGELOG.md

Commit & tag:

    $ git commit -S${GPG_KEY} -m "Release v${VERSION}"
    $ git tag -s -u ${GPG_KEY} v${VERSION} -m "Version ${VERSION}"

Publish:

    $ git push && git push --tags
