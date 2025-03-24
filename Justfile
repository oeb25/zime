release *args="":
    git checkout HEAD -- CHANGELOG.md
    cargo release {{args}}

release-hook:
    git cliff -t $NEW_VERSION -o CHANGELOG.md
