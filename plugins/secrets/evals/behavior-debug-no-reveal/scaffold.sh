#!/usr/bin/env bash
# A pass store sitting right in the repo (PASSWORD_STORE_DIR can't reach the
# sandbox's real HOME), with one ai-gent entry — just enough for `pass ls` to
# name it without decrypting anything.
set -euo pipefail
git init -q -b main
git config user.name eval
git config user.email eval@example.invalid
git commit -q --allow-empty -m "chore: initial commit"
mkdir -p .password-store/ai-gent/github
echo "not-a-real-gpg-key-id" >.password-store/.gpg-id
printf -- '-----BEGIN PGP MESSAGE-----\nfake-placeholder-ciphertext\n-----END PGP MESSAGE-----\n' \
  >.password-store/ai-gent/github/pat.gpg
echo .password-store/ >>.git/info/exclude
